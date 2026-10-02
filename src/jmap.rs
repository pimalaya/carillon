//! # JMAP
//!
//! The JMAP backend: session opening, authentication and the watch of
//! every domain the account configures.
//!
//! Each domain is a [`Watched`] collection: a mailbox read through
//! `Email/changes` (RFC 8621), an addressbook through
//! `ContactCard/changes` (RFC 9610), a calendar through
//! `CalendarEvent/changes` (JMAP for Calendars). A round asks each what
//! moved and resolves the ids it names through the matching `/get`,
//! keeping the ones inside the watched collection. All of them share one
//! session and one connection, and the push subscription (RFC 8620 §7.2)
//! is one stream covering every type, which only replaces the interval
//! with a wake-up.

use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Read, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use anyhow::{Context, Result, anyhow};
use base64::{Engine, prelude::BASE64_STANDARD};
use io_jmap::{
    calendars::calendar_event::{
        get::JmapCalendarEventGetOptions,
        query::{JmapCalendarEventFilter, JmapCalendarEventQueryOptions},
    },
    client::{JmapClientStd, JmapClientStdConnectOptions},
    coroutine::{JmapCoroutine, JmapCoroutineState},
    rfc8620::{
        changes::JmapChangesOutput,
        event_source::{
            JmapCloseAfter,
            subscribe::{JmapEventSource, JmapEventSourceYield},
        },
    },
    rfc8621::{
        email::{
            JmapEmail, JmapEmailProperty,
            get::JmapEmailGetOptions,
            query::{JmapEmailFilter, JmapEmailQueryOptions},
        },
        mailbox::JmapMailboxRole,
    },
    rfc9610::contact_card::{
        get::JmapContactCardGetOptions,
        query::{JmapContactCardFilter, JmapContactCardQueryOptions},
    },
};
use log::{debug, trace};
use pimalaya_config::secret::SecretResolver;
use pimalaya_stream::{retry::Retry, stream::Stream};
use secrecy::{ExposeSecret, SecretString};
use url::Url;

use crate::{
    config::{JmapAuthConfig, JmapConfig, ProxyConfig},
    event::{ItemSummary, WatchDomain, WatchEvent},
    poll,
};

/// How long the watch waits between two polls.
const POLL_INTERVAL: Duration = Duration::from_secs(30);
/// Per-read scratch buffer for the event stream.
const READ_BUF: usize = 8 * 1024;
/// Opens a JMAP session against the configured server, resolving its
/// credential through `resolver`, so a caller opening several backends of
/// one account spawns each distinct credential command once.
pub fn open(config: &JmapConfig, resolver: &mut SecretResolver) -> Result<(JmapClientStd, Url)> {
    let alpn = config
        .alpn
        .clone()
        .unwrap_or_else(JmapClientStd::default_alpn);
    let tls = config.tls.clone().into_tls(alpn);

    let url = parse_server(&config.server)?;
    let auth = http_auth(config.auth.clone(), resolver)?;
    let opts = JmapClientStdConnectOptions {
        tls,
        proxy: ProxyConfig::resolve(config.proxy.clone(), resolver)?,
    };

    let mut client = JmapClientStd::connect(&url, auth, opts)?;

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

/// Watches every configured collection by polling, until `shutdown` is
/// set.
///
/// Every round asks each domain's `/changes` what moved since the state
/// it last saw; a round that finds the states unmoved costs one request
/// per domain.
pub fn watch_poll(
    config: &JmapConfig,
    interval: Option<Duration>,
    resolve: bool,
    shutdown: &Arc<AtomicBool>,
    mut on_event: impl FnMut(WatchEvent, Option<ItemSummary>),
) -> Result<()> {
    let interval = interval.unwrap_or(POLL_INTERVAL);
    let (mut client, mut watched) = arm(config)?;

    while !shutdown.load(Ordering::SeqCst) {
        if !poll::sleep(interval, shutdown) {
            break;
        }

        rounds(&mut client, config, &mut watched, resolve, &mut on_event)?;
    }

    Ok(())
}

/// Watches every configured collection over one EventSource stream, until
/// `shutdown` is set.
///
/// Asked to close after the first state change (RFC 8620 §7.3
/// `closeafter=state`), the stream leaves the loop looking like an IMAP
/// IDLE. It holds its own connection, being the one the server hangs up.
pub fn watch_push(
    config: &JmapConfig,
    ping: u64,
    resolve: bool,
    shutdown: &Arc<AtomicBool>,
    mut on_event: impl FnMut(WatchEvent, Option<ItemSummary>),
) -> Result<()> {
    let (mut client, mut watched) = arm(config)?;
    let types: Vec<&str> = watched.iter().map(Watched::type_name).collect();

    while !shutdown.load(Ordering::SeqCst) {
        if !subscribe(&mut client, config, &types, ping, shutdown)? {
            continue;
        }

        rounds(&mut client, config, &mut watched, resolve, &mut on_event)?;
    }

    Ok(())
}

/// Opens the session and reads every configured collection as it stands.
fn arm(config: &JmapConfig) -> Result<(JmapClientStd, Vec<Watched>)> {
    let (mut client, _url) = open(config, &mut SecretResolver::new())?;
    let mut watched = Vec::new();

    for (domain, collection) in config.collections() {
        watched.push(Watched::arm(&mut client, domain, collection.value)?);
    }

    Ok((client, watched))
}

/// Runs one round per watched collection.
///
/// The connection may have sat idle as long as the interval or the
/// stream, which a server is free to have found long enough to close, so
/// a failed round is given a fresh one before the session is given up.
fn rounds(
    client: &mut JmapClientStd,
    config: &JmapConfig,
    watched: &mut [Watched],
    resolve: bool,
    on_event: &mut impl FnMut(WatchEvent, Option<ItemSummary>),
) -> Result<()> {
    for watched in watched {
        if let Err(err) = watched.round(client, resolve, on_event) {
            debug!("jmap round failed, reconnecting: {err:#}");
            reconnect(client, config)?;
            watched.round(client, resolve, on_event)?;
        }
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

/// One collection the watch holds a picture of, in one domain.
struct Watched {
    /// What the collection holds, which picks the methods it is read
    /// through and names the events it reports.
    domain: WatchDomain,
    /// The id of the collection, which every call speaks.
    id: String,
    /// What the collection holds, as of `state`.
    known: Known,
    /// The state the next `/changes` reads from.
    state: String,
}

/// What a round reads of one item, whatever its domain.
#[derive(Debug, Default)]
struct Member {
    /// The item's id.
    id: String,
    /// Whether the item belongs to the watched collection.
    inside: bool,
    /// The item's keywords, under the names a hook filter matches. Only
    /// mail has any.
    keywords: BTreeSet<String>,
    /// The envelope an arrival hook templates on, when one was asked for.
    summary: Option<ItemSummary>,
}

impl Watched {
    /// Resolves `name` into its collection and reads what it holds, which
    /// is what a later change is a change against.
    fn arm(client: &mut JmapClientStd, domain: WatchDomain, name: &str) -> Result<Self> {
        let (id, known, state) = match domain {
            WatchDomain::Message => {
                let id = resolve_mailbox(client, name)?;
                let known = baseline_mailbox(client, &id)?;
                let state = client
                    .email_get(Vec::new(), get_options(false))
                    .context("cannot read the initial email state")?
                    .new_state;
                (id, known, state)
            }
            WatchDomain::Card => {
                let id = resolve_addressbook(client, name)?;
                let known = baseline_addressbook(client, &id)?;
                let opts = JmapContactCardGetOptions {
                    ids: Some(Vec::new()),
                    properties: Some(vec![String::from("id")]),
                };
                let state = client
                    .contact_card_get(opts)
                    .context("cannot read the initial contact state")?
                    .new_state;
                (id, known, state)
            }
            WatchDomain::Event => {
                let id = resolve_calendar(client, name)?;
                let known = baseline_calendar(client, &id)?;
                let opts = JmapCalendarEventGetOptions {
                    ids: Some(Vec::new()),
                    properties: Some(vec![String::from("id")]),
                    ..Default::default()
                };
                let state = client
                    .calendar_event_get(opts)
                    .context("cannot read the initial event state")?
                    .new_state;
                (id, known, state)
            }
            WatchDomain::Task => unreachable!("JMAP has no task type"),
        };

        debug!(
            "watching jmap {} `{name}` with {} items",
            domain.collection_name(),
            known.len()
        );

        Ok(Self {
            domain,
            id,
            known,
            state,
        })
    }

    /// The JMAP type the event stream subscribes to for this collection.
    fn type_name(&self) -> &'static str {
        match self.domain {
            WatchDomain::Message => "Email",
            WatchDomain::Card => "ContactCard",
            WatchDomain::Event | WatchDomain::Task => "CalendarEvent",
        }
    }

    /// Reads what moved since `state`, reports it, and advances `state`.
    fn round(
        &mut self,
        client: &mut JmapClientStd,
        resolve: bool,
        on_event: &mut impl FnMut(WatchEvent, Option<ItemSummary>),
    ) -> Result<()> {
        let changes = self.changes(client)?;

        if changes.new_state == self.state {
            return Ok(());
        }

        trace!("jmap changes: {changes:?}");

        let touched: Vec<String> = changes
            .created
            .iter()
            .chain(changes.updated.iter())
            .cloned()
            .collect();

        // NOTE: nothing is reported until every request the round makes
        // has answered, so a round failing part way leaves the state and
        // the picture where they were, and can simply be run again.
        let mut reported = Vec::new();

        for id in &changes.destroyed {
            if self.known.contains_key(id) {
                let event = WatchEvent::ItemRemoved {
                    domain: self.domain,
                    id: id.clone(),
                };
                reported.push((event, None));
            }
        }

        let fetched = if touched.is_empty() {
            Vec::new()
        } else {
            self.fetch(client, touched, resolve)?
        };

        for id in &changes.destroyed {
            self.known.remove(id);
        }

        for mut member in fetched {
            // NOTE: the envelope rides the same response the
            // reconciliation reads, so an arrival costs no second request.
            let summary = member.summary.take();

            for event in self.reconcile(member) {
                let summary = matches!(event, WatchEvent::ItemAdded { .. })
                    .then(|| summary.clone())
                    .flatten();
                reported.push((event, summary));
            }
        }

        for (event, summary) in reported {
            on_event(event, summary);
        }

        self.state = changes.new_state;

        Ok(())
    }

    /// Asks this domain's `/changes` what moved since `state`.
    fn changes(&self, client: &mut JmapClientStd) -> Result<JmapChangesOutput> {
        let state = self.state.clone();

        Ok(match self.domain {
            WatchDomain::Message => client
                .email_changes(state, Default::default())
                .context("cannot read email changes")?,
            WatchDomain::Card => client
                .contact_card_changes(state, Default::default())
                .context("cannot read contact changes")?,
            WatchDomain::Event | WatchDomain::Task => client
                .calendar_event_changes(state, Default::default())
                .context("cannot read event changes")?,
        })
    }

    /// Reads `ids` through this domain's `/get`, keeping what membership
    /// and, for mail, keywords and the envelope need.
    fn fetch(
        &self,
        client: &mut JmapClientStd,
        ids: Vec<String>,
        resolve: bool,
    ) -> Result<Vec<Member>> {
        let inside = |collections: &BTreeMap<String, bool>| {
            collections.get(&self.id).copied().unwrap_or(false)
        };

        Ok(match self.domain {
            WatchDomain::Message => client
                .email_get(ids, get_options(resolve))
                .context("cannot resolve changed emails")?
                .emails
                .into_iter()
                .filter_map(|email| {
                    Some(Member {
                        inside: email.mailbox_ids.as_ref().is_some_and(inside),
                        keywords: render_keywords(email.keywords.as_ref()),
                        summary: resolve.then(|| summarize(&email)),
                        id: email.id?,
                    })
                })
                .collect(),
            WatchDomain::Card => {
                let opts = JmapContactCardGetOptions {
                    ids: Some(ids),
                    properties: Some(vec![String::from("id"), String::from("addressBookIds")]),
                };

                client
                    .contact_card_get(opts)
                    .context("cannot resolve changed contacts")?
                    .cards
                    .into_iter()
                    .filter_map(|card| {
                        Some(Member {
                            inside: inside(&card.address_book_ids),
                            id: card.id?,
                            ..Default::default()
                        })
                    })
                    .collect()
            }
            WatchDomain::Event | WatchDomain::Task => {
                let opts = JmapCalendarEventGetOptions {
                    ids: Some(ids),
                    properties: Some(vec![String::from("id"), String::from("calendarIds")]),
                    ..Default::default()
                };

                client
                    .calendar_event_get(opts)
                    .context("cannot resolve changed events")?
                    .events
                    .into_iter()
                    .filter_map(|event| {
                        Some(Member {
                            inside: inside(&event.calendar_ids),
                            id: event.id?,
                            ..Default::default()
                        })
                    })
                    .collect()
            }
        })
    }

    /// Reconciles one resolved item against the picture, and reports what
    /// moved.
    ///
    /// An item leaving the watched collection is a removal, as an IMAP
    /// move out of a mailbox is: the watch reports what the collection
    /// holds, not the account. A known item still inside is an edit for a
    /// card or an event, which `/changes` named because its content moved,
    /// and a keyword delta for a message, which is immutable.
    fn reconcile(&mut self, member: Member) -> Vec<WatchEvent> {
        let Member {
            id,
            inside,
            keywords,
            ..
        } = member;
        let domain = self.domain;

        if !inside {
            return match self.known.remove(&id) {
                Some(_) => vec![WatchEvent::ItemRemoved { domain, id }],
                None => Vec::new(),
            };
        }

        let Some(before) = self.known.insert(id.clone(), keywords.clone()) else {
            return vec![WatchEvent::ItemAdded { domain, id }];
        };

        if domain != WatchDomain::Message {
            return vec![WatchEvent::ItemChanged { domain, id }];
        }

        let mut events = Vec::new();

        // NOTE: one event per keyword, so a hook knows which flag it fired
        // for.
        for flag in keywords.difference(&before) {
            events.push(WatchEvent::FlagAdded {
                domain,
                id: id.clone(),
                flag: flag.clone(),
            });
        }

        for flag in before.difference(&keywords) {
            events.push(WatchEvent::FlagRemoved {
                domain,
                id: id.clone(),
                flag: flag.clone(),
            });
        }

        events
    }
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
    types: &[&str],
    ping: u64,
    shutdown: &Arc<AtomicBool>,
) -> Result<bool> {
    let session = client
        .session()
        .ok_or_else(|| anyhow!("The JMAP session was not read"))?;
    let mut coroutine = JmapEventSource::new(
        session,
        &client.http_auth,
        types,
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

/// Lists what the watched mailbox holds, with each message's keywords.
fn baseline_mailbox(client: &mut JmapClientStd, mailbox_id: &str) -> Result<Known> {
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

/// Lists what the watched addressbook holds.
fn baseline_addressbook(client: &mut JmapClientStd, addressbook_id: &str) -> Result<Known> {
    let filter = JmapContactCardFilter {
        in_address_book: Some(addressbook_id.to_string()),
        ..Default::default()
    };
    let opts = JmapContactCardQueryOptions {
        filter: Some(filter),
        properties: Some(vec![String::from("id")]),
        ..Default::default()
    };

    let listed = client
        .contact_card_query(opts)
        .context("cannot list the watched addressbook")?;

    Ok(listed
        .cards
        .into_iter()
        .filter_map(|card| Some((card.id?, BTreeSet::new())))
        .collect())
}

/// Lists what the watched calendar holds, one id per series.
fn baseline_calendar(client: &mut JmapClientStd, calendar_id: &str) -> Result<Known> {
    let filter = JmapCalendarEventFilter {
        in_calendar: Some(calendar_id.to_string()),
        ..Default::default()
    };
    let opts = JmapCalendarEventQueryOptions {
        filter: Some(filter),
        properties: Some(vec![String::from("id")]),
        ..Default::default()
    };

    let listed = client
        .calendar_event_query(opts)
        .context("cannot list the watched calendar")?;

    Ok(listed
        .events
        .into_iter()
        .filter_map(|event| Some((event.id?, BTreeSet::new())))
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

/// Resolves an addressbook, by name case-insensitively or by id.
fn resolve_addressbook(client: &mut JmapClientStd, addressbook: &str) -> Result<String> {
    let listed = client
        .address_book_get(Default::default())
        .context("cannot list addressbooks")?;

    listed
        .address_books
        .into_iter()
        .map(|candidate| (candidate.id, candidate.name))
        .find_map(|(id, name)| matches_collection(id, name, addressbook))
        .ok_or_else(|| anyhow!("Addressbook `{addressbook}` not found on the JMAP server"))
}

/// Resolves a calendar, by name case-insensitively or by id.
fn resolve_calendar(client: &mut JmapClientStd, calendar: &str) -> Result<String> {
    let listed = client
        .calendar_get(Default::default())
        .context("cannot list calendars")?;

    listed
        .calendars
        .into_iter()
        .map(|candidate| (candidate.id, candidate.name))
        .find_map(|(id, name)| matches_collection(id, name, calendar))
        .ok_or_else(|| anyhow!("Calendar `{calendar}` not found on the JMAP server"))
}

/// The id of a listed collection, when `wanted` names it by id or by
/// name.
fn matches_collection(id: Option<String>, name: Option<String>, wanted: &str) -> Option<String> {
    let id = id?;
    let named = name.is_some_and(|name| name.eq_ignore_ascii_case(wanted));

    (id == wanted || named).then_some(id)
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

/// What the watch knows of a collection: an item id to its keywords,
/// empty for anything but mail.
type Known = BTreeMap<String, BTreeSet<String>>;

#[cfg(test)]
mod tests {
    use io_jmap::rfc8621::email::JmapEmailAddress;

    use crate::jmap::*;

    fn watched(domain: WatchDomain) -> Watched {
        Watched {
            domain,
            id: String::from("C1"),
            known: Known::new(),
            state: String::new(),
        }
    }

    fn member(id: &str, inside: bool) -> Member {
        Member {
            id: String::from(id),
            inside,
            ..Default::default()
        }
    }

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

    /// A card is mutable, so a known one named again is an edit, where a
    /// message named again only moved its keywords.
    #[test]
    fn a_known_card_named_again_is_an_edit() {
        let mut cards = watched(WatchDomain::Card);
        let changed = WatchEvent::ItemChanged {
            domain: WatchDomain::Card,
            id: String::from("A"),
        };

        assert_eq!(
            vec![WatchEvent::ItemAdded {
                domain: WatchDomain::Card,
                id: String::from("A"),
            }],
            cards.reconcile(member("A", true)),
        );
        assert_eq!(vec![changed], cards.reconcile(member("A", true)));

        let mut mail = watched(WatchDomain::Message);
        mail.reconcile(member("M", true));
        assert!(mail.reconcile(member("M", true)).is_empty());
    }

    /// An event moved to another calendar leaves the watched one, and one
    /// moved in arrives, whatever the account still holds.
    #[test]
    fn an_event_crossing_the_calendar_is_an_arrival_or_a_removal() {
        let mut events = watched(WatchDomain::Event);

        assert!(events.reconcile(member("E", false)).is_empty());
        assert_eq!(
            vec![WatchEvent::ItemAdded {
                domain: WatchDomain::Event,
                id: String::from("E"),
            }],
            events.reconcile(member("E", true)),
        );
        assert_eq!(
            vec![WatchEvent::ItemRemoved {
                domain: WatchDomain::Event,
                id: String::from("E"),
            }],
            events.reconcile(member("E", false)),
        );
    }

    #[test]
    fn a_collection_matches_by_name_or_by_id() {
        let listed = || (Some(String::from("b1")), Some(String::from("Personal")));

        let (id, name) = listed();
        assert_eq!(
            Some(String::from("b1")),
            matches_collection(id, name, "personal")
        );

        let (id, name) = listed();
        assert_eq!(Some(String::from("b1")), matches_collection(id, name, "b1"));

        let (id, name) = listed();
        assert_eq!(None, matches_collection(id, name, "Work"));
    }
}
