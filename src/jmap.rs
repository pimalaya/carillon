//! JMAP backend: session opening, authentication and the mailbox watch.
//!
//! JMAP has no held connection here. The watch polls `Email/changes`,
//! which answers with the ids created, updated and destroyed since the
//! state the client last saw, and resolves those ids through
//! `Email/get` to keep the ones inside the watched mailbox. A push
//! subscription (RFC 8620 §7.2) would replace the interval with a
//! wake-up; the change reconciliation below would not move.

use std::{
    collections::{BTreeMap, BTreeSet},
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
    event::WatchEvent,
};

/// How long the watch waits between two polls.
const POLL_INTERVAL: Duration = Duration::from_secs(30);
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

/// Watches `mailbox` until `shutdown` is set, calling `on_event` for
/// every change.
///
/// JMAP has no held connection here: the watch polls `Email/changes`,
/// which answers with the ids created, updated and destroyed since the
/// state it last saw. A poll that finds the state unmoved costs one
/// request and nothing else.
///
/// The mailbox is a filter, not a channel: `Email/changes` reports the
/// whole account, so the ids it names are resolved through `Email/get`
/// and kept only when they belong to the watched mailbox. That
/// resolution also carries the keywords, which is what turns an update
/// into a flag event.
pub fn watch(
    config: &JmapConfig,
    mailbox: &str,
    shutdown: &Arc<AtomicBool>,
    mut on_event: impl FnMut(WatchEvent),
) -> Result<()> {
    let (mut client, _url) = open(config)?;
    let mailbox_id = resolve_mailbox(&mut client, mailbox)?;

    let mut known = baseline(&mut client, &mailbox_id)?;
    let mut state = client
        .email_get(Vec::new(), get_options(false))
        .context("cannot read the initial email state")?
        .new_state;

    debug!("watching jmap mailbox with {} messages", known.len());

    while !shutdown.load(Ordering::SeqCst) {
        if !sleep(POLL_INTERVAL, shutdown) {
            break;
        }

        let changes = client
            .email_changes(state.clone(), Default::default())
            .context("cannot read email changes")?;

        if changes.new_state == state {
            continue;
        }

        trace!("jmap changes: {changes:?}");

        for id in &changes.destroyed {
            if known.remove(id).is_some() {
                on_event(WatchEvent::MessageRemoved { id: id.clone() });
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
                for event in reconcile(&mut known, &mailbox_id, email) {
                    on_event(event);
                }
            }
        }

        state = changes.new_state;
    }

    Ok(())
}

/// Reconciles one resolved email against what the watch knows, and
/// reports what moved.
///
/// A message leaving the watched mailbox is a removal here, the same
/// way an IMAP move out of the mailbox is: the watch reports what the
/// mailbox holds, not what the account holds.
fn reconcile(
    known: &mut BTreeMap<String, BTreeSet<String>>,
    mailbox_id: &str,
    email: JmapEmail,
) -> Vec<WatchEvent> {
    let Some(id) = email.id else {
        return Vec::new();
    };

    let inside = email
        .mailbox_ids
        .as_ref()
        .is_some_and(|ids| ids.get(mailbox_id).copied().unwrap_or(false));

    if !inside {
        return match known.remove(&id) {
            Some(_) => vec![WatchEvent::MessageRemoved { id }],
            None => Vec::new(),
        };
    }

    let keywords = render_keywords(email.keywords.as_ref());

    let Some(before) = known.insert(id.clone(), keywords.clone()) else {
        return vec![WatchEvent::MessageAdded { id }];
    };

    let mut events = Vec::new();

    let added: BTreeSet<String> = keywords.difference(&before).cloned().collect();
    if !added.is_empty() {
        events.push(WatchEvent::FlagsAdded {
            id: id.clone(),
            flags: added,
        });
    }

    let removed: BTreeSet<String> = before.difference(&keywords).cloned().collect();
    if !removed.is_empty() {
        events.push(WatchEvent::FlagsRemoved { id, flags: removed });
    }

    events
}

/// Lists what the watched mailbox holds, so a later change has
/// something to be a change against.
fn baseline(
    client: &mut JmapClientStd,
    mailbox_id: &str,
) -> Result<BTreeMap<String, BTreeSet<String>>> {
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
