//! # JMAP
//!
//! The JMAP backend: session opening, authentication and the mailbox
//! watch.
//!
//! The watch polls `Email/changes` and resolves the ids it names through
//! `Email/get`, keeping the ones inside the watched mailbox. The push
//! subscription (RFC 8620 §7.2) only replaces the interval with a
//! wake-up, leaving that reconciliation untouched.

use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Read, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use anyhow::{Context, Result, anyhow};
use base64::{Engine, prelude::BASE64_STANDARD};
use io_jmap::{
    client::JmapClientStd,
    coroutine::{JmapCoroutine, JmapCoroutineState},
    rfc8620::event_source::{
        JmapCloseAfter,
        subscribe::{JmapEventSource, JmapEventSourceYield},
    },
    rfc8621::{
        email::{
            JmapEmail, JmapEmailProperty,
            get::JmapEmailGetOptions,
            query::{JmapEmailFilter, JmapEmailQueryOptions},
        },
        mailbox::JmapMailboxRole,
    },
};
use log::{debug, trace};
use pimalaya_config::secret::SecretResolver;
use pimalaya_stream::{retry::Retry, stream::Stream, tls::Tls};
use secrecy::{ExposeSecret, SecretString};
use url::Url;

use crate::{
    config::{JmapAuthConfig, JmapConfig},
    event::{ItemSummary, WatchDomain, WatchEvent},
};

/// How long the watch waits between two polls.
const POLL_INTERVAL: Duration = Duration::from_secs(30);
/// Per-read scratch buffer for the event stream.
const READ_BUF: usize = 8 * 1024;
/// The JMAP type a mail watch subscribes to.
const EMAIL_TYPE: &str = "Email";
/// How long it sleeps at a time, so a shutdown is noticed promptly.
const POLL_STEP: Duration = Duration::from_millis(200);

/// Opens a JMAP session against the configured server, resolving its
/// credential through `resolver`, so a caller opening several backends of
/// one account spawns each distinct credential command once.
pub fn open(config: &JmapConfig, resolver: &mut SecretResolver) -> Result<(JmapClientStd, Url)> {
    let mut tls: Tls = config.tls.clone().into();
    tls.rustls.alpn = vec![String::from("http/1.1")];

    let url = parse_server(&config.server)?;
    let auth = http_auth(config.auth.clone(), resolver)?;
    let mut client = JmapClientStd::connect(&url, &tls, auth)?;

    // NOTE: io-jmap arms a five-second read deadline to wake a caller up,
    // which pimalaya-stream retries away for a minute by default. Handing
    // the failures back is what bounds a poll against a server that
    // stopped answering, and so how long a Ctrl+C waits.
    if let Some(stream) = client.stream.as_any_mut().downcast_mut::<Stream>() {
        stream.retry = Retry::Never;
    }

    client.session_get(&url)?;

    Ok((client, url))
}

/// Renders the configuration as the `Authorization` header value io-jmap
/// presents on every request.
///
/// `resolver` spawns each distinct credential command once, so a caller
/// opening several backends of one account unlocks their store once.
pub fn http_auth(config: JmapAuthConfig, resolver: &mut SecretResolver) -> Result<SecretString> {
    Ok(match config {
        JmapAuthConfig::Header(token) => resolver.resolve(token)?,
        JmapAuthConfig::Bearer { token } => {
            let token = resolver.resolve(token)?;
            format!("Bearer {}", token.expose_secret()).into()
        }
        JmapAuthConfig::Basic { username, password } => {
            let credentials = format!("{username}:{}", resolver.resolve(password)?.expose_secret());
            let encoded = BASE64_STANDARD.encode(credentials.into_bytes());
            format!("Basic {encoded}").into()
        }
    })
}

/// Parses a JMAP server string into a URL.
///
/// A bare authority is discovered through `GET /.well-known/jmap`, a full
/// URL points straight at the session endpoint.
pub fn parse_server(server: &str) -> Result<Url> {
    match Url::parse(server) {
        Ok(url) => Ok(url),
        Err(url::ParseError::RelativeUrlWithoutBase) => {
            Ok(Url::parse(&format!("https://{server}"))?)
        }
        Err(err) => Err(err.into()),
    }
}

/// Watches `collection` by polling, until `shutdown` is set.
///
/// Every round asks `Email/changes` what moved since the state it last
/// saw; a round that finds the state unmoved costs one request.
pub fn watch_poll(
    config: &JmapConfig,
    collection: &str,
    interval: Option<Duration>,
    resolve: bool,
    shutdown: &Arc<AtomicBool>,
    mut on_event: impl FnMut(WatchEvent, Option<ItemSummary>),
) -> Result<()> {
    let interval = interval.unwrap_or(POLL_INTERVAL);
    let (mut client, mailbox_id, mut known, mut state) = arm(config, collection)?;

    while !shutdown.load(Ordering::SeqCst) {
        if !sleep(interval, shutdown) {
            break;
        }

        // NOTE: the connection slept as long as the interval, which a
        // server is free to have found long enough to close, so a failed
        // round is given a fresh one before the session is given up.
        let outcome = round(
            &mut client,
            &mailbox_id,
            &mut known,
            &mut state,
            resolve,
            &mut on_event,
        );

        if let Err(err) = outcome {
            debug!("jmap round failed, reconnecting: {err:#}");
            reconnect(&mut client, config)?;
            round(
                &mut client,
                &mailbox_id,
                &mut known,
                &mut state,
                resolve,
                &mut on_event,
            )?;
        }
    }

    Ok(())
}

/// Watches `collection` over an EventSource stream, until `shutdown` is
/// set.
///
/// Asked to close after the first state change (RFC 8620 §7.3
/// `closeafter=state`), the stream leaves the loop looking like an IMAP
/// IDLE. It holds its own connection, being the one the server hangs up.
pub fn watch_push(
    config: &JmapConfig,
    collection: &str,
    ping: u64,
    resolve: bool,
    shutdown: &Arc<AtomicBool>,
    mut on_event: impl FnMut(WatchEvent, Option<ItemSummary>),
) -> Result<()> {
    let (mut client, mailbox_id, mut known, mut state) = arm(config, collection)?;

    while !shutdown.load(Ordering::SeqCst) {
        if !subscribe(&mut client, config, ping, shutdown)? {
            continue;
        }

        round(
            &mut client,
            &mailbox_id,
            &mut known,
            &mut state,
            resolve,
            &mut on_event,
        )?;
    }

    Ok(())
}

/// Dials a fresh connection, keeping the session already read.
///
/// The session carries the API URL and the account id, and does not move
/// when the transport does, so it is not read again.
fn reconnect(client: &mut JmapClientStd, config: &JmapConfig) -> Result<()> {
    let (fresh, _url) = open(config, &mut SecretResolver::new())?;
    client.set_stream(fresh.stream);
    debug!("reconnected the jmap client");

    Ok(())
}

/// Opens the session and reads the collection as it stands, which is what
/// a later change is a change against.
fn arm(config: &JmapConfig, collection: &str) -> Result<(JmapClientStd, String, Known, String)> {
    let (mut client, _url) = open(config, &mut SecretResolver::new())?;
    let mailbox_id = resolve_mailbox(&mut client, collection)?;

    let known = baseline(&mut client, &mailbox_id)?;
    let state = client
        .email_get(Vec::new(), get_options(false))
        .context("cannot read the initial email state")?
        .new_state;

    debug!("watching jmap collection with {} messages", known.len());

    Ok((client, mailbox_id, known, state))
}

/// Reads what moved since `state`, reports it, and advances `state`.
fn round(
    client: &mut JmapClientStd,
    mailbox_id: &str,
    known: &mut Known,
    state: &mut String,
    resolve: bool,
    on_event: &mut impl FnMut(WatchEvent, Option<ItemSummary>),
) -> Result<()> {
    let changes = client
        .email_changes(state.clone(), Default::default())
        .context("cannot read email changes")?;

    if changes.new_state == *state {
        return Ok(());
    }

    trace!("jmap changes: {changes:?}");

    let touched: Vec<String> = changes
        .created
        .iter()
        .chain(changes.updated.iter())
        .cloned()
        .collect();

    // NOTE: nothing is reported until every request the round makes has
    // answered, so a round failing part way leaves the state and the
    // picture where they were, and can simply be run again.
    let mut reported = Vec::new();

    for id in &changes.destroyed {
        if known.contains_key(id) {
            reported.push((
                WatchEvent::ItemRemoved {
                    domain: WatchDomain::Message,
                    id: id.clone(),
                },
                None,
            ));
        }
    }

    let fetched = if touched.is_empty() {
        Vec::new()
    } else {
        client
            .email_get(touched, get_options(resolve))
            .context("cannot resolve changed emails")?
            .emails
    };

    for id in &changes.destroyed {
        known.remove(id);
    }

    for email in fetched {
        // NOTE: the envelope rides the same response the reconciliation
        // reads, so an arrival costs no second request.
        let summary = resolve.then(|| summarize(&email));

        for event in reconcile(known, mailbox_id, email) {
            let summary = matches!(event, WatchEvent::ItemAdded { .. })
                .then(|| summary.clone())
                .flatten();
            reported.push((event, summary));
        }
    }

    for (event, summary) in reported {
        on_event(event, summary);
    }

    *state = changes.new_state;

    Ok(())
}

/// Folds an `Email/get` result into what an arrival hook templates on.
fn summarize(email: &JmapEmail) -> ItemSummary {
    let mut summary = ItemSummary {
        subject: email.subject.clone(),
        date: email.received_at.clone(),
        ..Default::default()
    };

    if let Some(from) = email.from.as_ref().and_then(|from| from.first()) {
        summary.from_name = from.name.clone();
        summary.from_addr = Some(from.email.clone());
    }

    if let Some(to) = email.to.as_ref().and_then(|to| to.first()) {
        summary.to_name = to.name.clone();
        summary.to_addr = Some(to.email.clone());
    }

    summary
}

/// Holds an EventSource subscription until the server reports a state
/// change, and says whether one arrived.
///
/// A frame with an empty `changed` map is the server's keep-alive, and a
/// read that times out is the wakeup this loop arms to look at the
/// shutdown flag: neither is news.
fn subscribe(
    client: &mut JmapClientStd,
    config: &JmapConfig,
    ping: u64,
    shutdown: &Arc<AtomicBool>,
) -> Result<bool> {
    let session = client
        .session()
        .ok_or_else(|| anyhow!("The JMAP session was not read"))?;
    let mut coroutine = JmapEventSource::new(
        session,
        &client.http_auth,
        &[EMAIL_TYPE],
        ping,
        JmapCloseAfter::State,
        shutdown.clone(),
    )?;

    // NOTE: the subscription is what the server hangs up on, so it holds
    // a connection of its own and leaves the client's to the next round.
    let (mut stream, _url) = open(config, &mut SecretResolver::new())?;
    let mut buf = [0u8; READ_BUF];
    let mut arg: Option<Vec<u8>> = None;
    let mut changed = false;

    loop {
        match coroutine.resume(arg.take().as_deref()) {
            JmapCoroutineState::Yielded(JmapEventSourceYield::Frame(frame)) => {
                if !frame.changed.is_empty() {
                    trace!("jmap state change: {frame:?}");
                    changed = true;
                }
            }
            JmapCoroutineState::Yielded(JmapEventSourceYield::WantsRead) => {
                if shutdown.load(Ordering::SeqCst) {
                    return Ok(false);
                }

                match stream.stream.read(&mut buf) {
                    Ok(0) => return Ok(changed),
                    Ok(read) => arg = Some(buf[..read].to_vec()),
                    Err(err) if is_timeout(&err) => continue,
                    Err(err) => return Err(err).context("read failed"),
                }
            }
            JmapCoroutineState::Yielded(JmapEventSourceYield::WantsWrite(bytes)) => {
                stream.stream.write_all(&bytes).context("write failed")?;
            }
            JmapCoroutineState::Complete(Ok(())) => return Ok(changed),
            JmapCoroutineState::Complete(Err(err)) => return Err(err.into()),
        }
    }
}

/// Whether an I/O error is the read deadline expiring, which on a
/// quiet stream is a wakeup rather than a failure.
fn is_timeout(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    )
}
/// Reconciles one resolved email against what the watch knows, and reports
/// what moved.
///
/// A message leaving the watched mailbox is a removal, as an IMAP move out
/// of it is: the watch reports what the mailbox holds, not the account.
fn reconcile(known: &mut Known, mailbox_id: &str, email: JmapEmail) -> Vec<WatchEvent> {
    let Some(id) = email.id else {
        return Vec::new();
    };

    let inside = email
        .mailbox_ids
        .as_ref()
        .is_some_and(|ids| ids.get(mailbox_id).copied().unwrap_or(false));

    if !inside {
        return match known.remove(&id) {
            Some(_) => vec![WatchEvent::ItemRemoved {
                domain: WatchDomain::Message,
                id,
            }],
            None => Vec::new(),
        };
    }

    let keywords = render_keywords(email.keywords.as_ref());

    let Some(before) = known.insert(id.clone(), keywords.clone()) else {
        return vec![WatchEvent::ItemAdded {
            domain: WatchDomain::Message,
            id,
        }];
    };

    let mut events = Vec::new();

    // NOTE: one event per keyword, so a hook knows which flag it fired
    // for.
    for flag in keywords.difference(&before) {
        events.push(WatchEvent::FlagAdded {
            domain: WatchDomain::Message,
            id: id.clone(),
            flag: flag.clone(),
        });
    }

    for flag in before.difference(&keywords) {
        events.push(WatchEvent::FlagRemoved {
            domain: WatchDomain::Message,
            id: id.clone(),
            flag: flag.clone(),
        });
    }

    events
}

/// Lists what the watched mailbox holds, so a later change has
/// something to be a change against.
fn baseline(client: &mut JmapClientStd, mailbox_id: &str) -> Result<Known> {
    let filter = JmapEmailFilter {
        in_mailbox: Some(mailbox_id.to_string()),
        ..Default::default()
    };
    let opts = JmapEmailQueryOptions {
        filter: Some(filter.into()),
        properties: Some(vec![JmapEmailProperty::Id, JmapEmailProperty::Keywords]),
        ..Default::default()
    };

    let listed = client
        .email_query(opts)
        .context("cannot list the watched mailbox")?;

    Ok(listed
        .emails
        .into_iter()
        .filter_map(|email| Some((email.id?, render_keywords(email.keywords.as_ref()))))
        .collect())
}

/// Resolves a mailbox name into the id every other call speaks.
///
/// The name matches case-insensitively, then falls back to the
/// special-use role, so `INBOX` finds the inbox on a server naming it in
/// another language.
fn resolve_mailbox(client: &mut JmapClientStd, mailbox: &str) -> Result<String> {
    let listed = client
        .mailbox_get(Default::default())
        .context("cannot list mailboxes")?;

    let by_name = listed.mailboxes.iter().find(|candidate| {
        candidate
            .name
            .as_deref()
            .is_some_and(|name| name.eq_ignore_ascii_case(mailbox))
    });

    let found = by_name.or_else(|| {
        mailbox.eq_ignore_ascii_case("INBOX").then(|| {
            listed
                .mailboxes
                .iter()
                .find(|candidate| candidate.role == Some(JmapMailboxRole::Inbox))
        })?
    });

    found
        .and_then(|mailbox| mailbox.id.clone())
        .ok_or_else(|| anyhow!("Mailbox `{mailbox}` not found on the JMAP server"))
}

/// The `Email/get` properties each call needs: ids only for a state
/// probe, plus what a change is judged against otherwise.
fn get_options(envelope: bool) -> JmapEmailGetOptions {
    let mut properties = vec![
        JmapEmailProperty::Id,
        JmapEmailProperty::MailboxIds,
        JmapEmailProperty::Keywords,
    ];

    // NOTE: the envelope is asked for only when a hook consumes one, the
    // rule IMAP resolves an arrival under, except that here it costs
    // properties on a request already being made rather than a second
    // connection.
    if envelope {
        properties.push(JmapEmailProperty::Subject);
        properties.push(JmapEmailProperty::ReceivedAt);
        properties.push(JmapEmailProperty::From);
        properties.push(JmapEmailProperty::To);
    }

    JmapEmailGetOptions {
        properties: Some(properties),
        ..Default::default()
    }
}

/// Renders JMAP keywords under the names a hook filter matches, so a
/// filter written for IMAP fires here too.
fn render_keywords(keywords: Option<&BTreeMap<String, bool>>) -> BTreeSet<String> {
    let Some(keywords) = keywords else {
        return BTreeSet::new();
    };

    keywords
        .iter()
        .filter(|(_, set)| **set)
        .map(|(keyword, _)| match keyword.as_str() {
            "$seen" => String::from("Seen"),
            "$flagged" => String::from("Flagged"),
            "$answered" => String::from("Answered"),
            "$draft" => String::from("Draft"),
            "$forwarded" => String::from("Passed"),
            keyword => keyword.to_string(),
        })
        .collect()
}

/// Sleeps `total` in small steps, returning false as soon as a
/// shutdown is requested.
fn sleep(total: Duration, shutdown: &Arc<AtomicBool>) -> bool {
    let mut left = total;

    while left > Duration::ZERO {
        if shutdown.load(Ordering::SeqCst) {
            return false;
        }

        let step = left.min(POLL_STEP);
        thread::sleep(step);
        left -= step;
    }

    !shutdown.load(Ordering::SeqCst)
}

/// What the watch knows of the collection: a message id to its keywords.
type Known = BTreeMap<String, BTreeSet<String>>;

#[cfg(test)]
mod tests {
    use io_jmap::rfc8621::email::JmapEmailAddress;

    use crate::jmap::*;

    /// The envelope a hook templates against comes out of the response the
    /// reconciliation reads, so an arrival costs no second request.
    #[test]
    fn an_envelope_is_read_from_the_round_s_own_response() {
        let email = JmapEmail {
            id: Some(String::from("M1")),
            subject: Some(String::from("Investment Funding")),
            received_at: Some(String::from("2026-08-22T12:58:23Z")),
            from: Some(vec![JmapEmailAddress {
                name: Some(String::from("Robert Daniels")),
                email: String::from("robert@example.org"),
            }]),
            to: Some(vec![JmapEmailAddress {
                name: None,
                email: String::from("alice@example.org"),
            }]),
            ..Default::default()
        };

        let summary = summarize(&email);
        assert_eq!(Some(String::from("Investment Funding")), summary.subject);
        assert_eq!(Some(String::from("2026-08-22T12:58:23Z")), summary.date);
        assert_eq!(Some(String::from("Robert Daniels")), summary.from_name);
        assert_eq!(Some(String::from("robert@example.org")), summary.from_addr);
        // NOTE: a recipient with no personal name still has an address,
        // which is what the combined `$recipient` falls back to.
        assert_eq!(None, summary.to_name);
        assert_eq!(Some(String::from("alice@example.org")), summary.to_addr);
    }

    /// An arrival on an account with no arrival hook is not resolved,
    /// so the envelope properties are not even asked for.
    #[test]
    fn the_envelope_is_asked_for_only_when_a_hook_wants_it() {
        // NOTE: JmapEmailProperty carries no PartialEq, so the list is
        // read as it renders.
        let bare = format!("{:?}", get_options(false).properties);
        assert!(!bare.contains("Subject"), "got {bare}");
        assert!(bare.contains("MailboxIds"), "got {bare}");

        let resolved = format!("{:?}", get_options(true).properties);
        assert!(resolved.contains("Subject"), "got {resolved}");
        assert!(resolved.contains("From"), "got {resolved}");
        assert!(resolved.contains("To"), "got {resolved}");
        assert!(resolved.contains("ReceivedAt"), "got {resolved}");
    }
}
