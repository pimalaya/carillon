//! JMAP backend: session opening, authentication and the mailbox watch.
//!
//! The watch polls `Email/changes` and resolves the ids it names
//! through `Email/get`, keeping the ones inside the watched mailbox. A
//! push subscription (RFC 8620 §7.2) would replace the interval with a
//! wake-up and leave the reconciliation below untouched.

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
use pimalaya_stream::{retry::Retry, stream::Stream, tls::Tls};
use secrecy::{ExposeSecret, SecretString};
use url::Url;

use crate::{
    config::{JmapAuthConfig, JmapConfig},
    event::{WatchDomain, WatchEvent},
};

/// How long the watch waits between two polls.
const POLL_INTERVAL: Duration = Duration::from_secs(30);
/// Per-read scratch buffer for the event stream.
const READ_BUF: usize = 8 * 1024;
/// The JMAP type a mail watch subscribes to.
const EMAIL_TYPE: &str = "Email";
/// How long it sleeps at a time, so a shutdown is noticed promptly.
const POLL_STEP: Duration = Duration::from_millis(200);

/// Opens a JMAP session against the configured server.
pub fn open(config: &JmapConfig) -> Result<(JmapClientStd, Url)> {
    let mut tls: Tls = config.tls.clone().into();
    tls.rustls.alpn = vec![String::from("http/1.1")];

    let url = parse_server(&config.server)?;
    let auth = http_auth(config.auth.clone())?;
    let mut client = JmapClientStd::connect(&url, &tls, auth)?;

    // NOTE: io-jmap arms a five-second read deadline so a caller can be
    // woken up, but pimalaya-stream retries that wakeup away for a
    // minute by default. Handing the failures back is what bounds a
    // poll against a server that stopped answering, and therefore how
    // long a Ctrl+C waits.
    if let Some(stream) = client.stream.as_any_mut().downcast_mut::<Stream>() {
        stream.retry = Retry::Never;
    }

    client.session_get(&url)?;

    Ok((client, url))
}

/// Renders the configured authentication as the HTTP `Authorization`
/// header value io-jmap presents on every request.
pub fn http_auth(config: JmapAuthConfig) -> Result<SecretString> {
    Ok(match config {
        JmapAuthConfig::Header(token) => token.get()?,
        JmapAuthConfig::Bearer { token } => {
            let token = token.get()?;
            format!("Bearer {}", token.expose_secret()).into()
        }
        JmapAuthConfig::Basic { username, password } => {
            let credentials = format!("{username}:{}", password.get()?.expose_secret());
            let encoded = BASE64_STANDARD.encode(credentials.into_bytes());
            format!("Basic {encoded}").into()
        }
    })
}

/// Parses a JMAP server string into a URL.
///
/// Accepts a bare authority, discovered through
/// `GET /.well-known/jmap`, or a full URL pointing straight at the
/// session endpoint.
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
    shutdown: &Arc<AtomicBool>,
    mut on_event: impl FnMut(WatchEvent),
) -> Result<()> {
    let interval = interval.unwrap_or(POLL_INTERVAL);
    let (mut client, mailbox_id, mut known, mut state) = arm(config, collection)?;

    while !shutdown.load(Ordering::SeqCst) {
        if !sleep(interval, shutdown) {
            break;
        }

        round(
            &mut client,
            &mailbox_id,
            &mut known,
            &mut state,
            &mut on_event,
        )?;
    }

    Ok(())
}

/// Watches `collection` over an EventSource stream, until `shutdown`
/// is set.
///
/// The server is asked to close the stream after the first state
/// change (RFC 8620 §7.3 `closeafter=state`), which frees the socket
/// for the `Email/changes` round that follows and leaves the loop
/// looking like an IMAP IDLE: subscribe, wait, read what moved,
/// subscribe again.
pub fn watch_push(
    config: &JmapConfig,
    collection: &str,
    ping: u64,
    shutdown: &Arc<AtomicBool>,
    mut on_event: impl FnMut(WatchEvent),
) -> Result<()> {
    let (mut client, mailbox_id, mut known, mut state) = arm(config, collection)?;

    while !shutdown.load(Ordering::SeqCst) {
        if !subscribe(&mut client, ping, shutdown)? {
            continue;
        }

        round(
            &mut client,
            &mailbox_id,
            &mut known,
            &mut state,
            &mut on_event,
        )?;
    }

    Ok(())
}

/// Opens the session and reads the collection as it stands, which is
/// what a later change is a change against.
fn arm(config: &JmapConfig, collection: &str) -> Result<(JmapClientStd, String, Known, String)> {
    let (mut client, _url) = open(config)?;
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
    on_event: &mut impl FnMut(WatchEvent),
) -> Result<()> {
    let changes = client
        .email_changes(state.clone(), Default::default())
        .context("cannot read email changes")?;

    if changes.new_state == *state {
        return Ok(());
    }

    trace!("jmap changes: {changes:?}");

    for id in &changes.destroyed {
        if known.remove(id).is_some() {
            on_event(WatchEvent::ItemRemoved {
                domain: WatchDomain::Message,
                id: id.clone(),
            });
        }
    }

    let touched: Vec<String> = changes
        .created
        .iter()
        .chain(changes.updated.iter())
        .cloned()
        .collect();

    if !touched.is_empty() {
        let fetched = client
            .email_get(touched, get_options(true))
            .context("cannot resolve changed emails")?;

        for email in fetched.emails {
            for event in reconcile(known, mailbox_id, email) {
                on_event(event);
            }
        }
    }

    *state = changes.new_state;

    Ok(())
}

/// Holds an EventSource subscription until the server reports a state
/// change, and says whether one arrived.
///
/// A frame with an empty `changed` map is the server's keep-alive, and
/// a read that times out is the wakeup this loop arms to look at the
/// shutdown flag: neither is news.
fn subscribe(client: &mut JmapClientStd, ping: u64, shutdown: &Arc<AtomicBool>) -> Result<bool> {
    let session = client
        .session()
        .ok_or_else(|| anyhow!("the JMAP session was not read"))?;
    let mut coroutine = JmapEventSource::new(
        session,
        &client.http_auth,
        &[EMAIL_TYPE],
        ping,
        JmapCloseAfter::State,
        shutdown.clone(),
    )?;

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

                match client.stream.read(&mut buf) {
                    Ok(0) => return Ok(changed),
                    Ok(read) => arg = Some(buf[..read].to_vec()),
                    Err(err) if is_timeout(&err) => continue,
                    Err(err) => return Err(err).context("read failed"),
                }
            }
            JmapCoroutineState::Yielded(JmapEventSourceYield::WantsWrite(bytes)) => {
                client.stream.write_all(&bytes).context("write failed")?;
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
/// Reconciles one resolved email against what the watch knows, and
/// reports what moved.
///
/// A message leaving the watched mailbox is a removal here, the same
/// way an IMAP move out of the mailbox is: the watch reports what the
/// mailbox holds, not what the account holds.
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

    // NOTE: one event per keyword, so a hook always knows which
    // flag it fired for.
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
/// Matches the name case-insensitively, then falls back to the
/// special-use role, so `INBOX` finds the inbox on a server that names
/// it in another language.
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
        .ok_or_else(|| anyhow!("mailbox `{mailbox}` not found on the JMAP server"))
}

/// The `Email/get` properties each call needs: ids only for a state
/// probe, plus what a change is judged against otherwise.
fn get_options(resolving: bool) -> JmapEmailGetOptions {
    let mut properties = vec![JmapEmailProperty::Id];

    if resolving {
        properties.push(JmapEmailProperty::MailboxIds);
        properties.push(JmapEmailProperty::Keywords);
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

/// What the watch knows of the collection: a message id to its
/// keywords.
type Known = BTreeMap<String, BTreeSet<String>>;
