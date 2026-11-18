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
    io::Write,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use base64::{Engine, prelude::BASE64_STANDARD};
use io_jmap::{
    calendars::calendar_event::{
        get::JmapCalendarEventGetOptions,
        query::{JmapCalendarEventFilter, JmapCalendarEventQueryOptions},
    },
    client::{JmapClientStd, JmapClientStdConnectOptions, JmapClientStdError},
    coroutine::{JmapCoroutine, JmapCoroutineState},
    rfc8620::{
        changes::JmapChangesOutput,
        event_source::{
            JmapCloseAfter, JmapStateChange,
            subscribe::{
                JmapEventSource, JmapEventSourceError, JmapEventSourceOptions, JmapEventSourceYield,
            },
        },
        send::JmapSendError,
        session_get::JmapSessionGetError,
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
    config::{
        jmap::{JmapAuthConfig, JmapConfig},
        parse_url,
        proxy::ProxyConfig,
    },
    error::{self, ErrorCode},
    event::{ItemSummary, WatchDomain, WatchEvent},
    poll,
    read::{self, ReadOutcome},
};

/// How long the watch waits between two polls.
const POLL_INTERVAL: Duration = Duration::from_secs(30);
/// Per-read scratch buffer for the event stream.
const READ_BUF: usize = 8 * 1024;
/// How many items one baseline page asks for, at most.
const PAGE: u64 = 500;
/// How many times a baseline is listed before a collection that keeps
/// moving under it fails the session.
const LISTINGS: usize = 3;
/// How many pings an event stream may miss, nothing read, before it is taken
/// for dead and opened again.
const PINGS_MISSED: u64 = 3;
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
/// URL points straight at the session endpoint. A host that is not ASCII is
/// refused, naming its Punycode form ([`parse_url`]).
pub fn parse_server(server: &str) -> Result<Url> {
    match Url::parse(server) {
        Ok(url) => Ok(url),
        Err(url::ParseError::RelativeUrlWithoutBase) => parse_url(&format!("https://{server}")),
        Err(_) => parse_url(server),
    }
}

/// Watches every configured collection by polling, until `shutdown` is
/// set.
///
/// Every round asks each domain's `/changes` what moved since the state
/// it last saw; a round that finds the states unmoved costs one request
/// per domain.
///
/// `on_open` is called once the session is up and its collections read,
/// right before the first wait.
pub fn watch_poll(
    config: &JmapConfig,
    interval: Option<Duration>,
    resolve: bool,
    shutdown: &Arc<AtomicBool>,
    on_open: impl FnOnce(),
    mut on_event: impl FnMut(WatchEvent, Option<ItemSummary>),
) -> Result<()> {
    let interval = interval.unwrap_or(POLL_INTERVAL);
    let (mut client, mut watched) = arm(config)?;
    on_open();

    while !shutdown.load(Ordering::SeqCst) {
        if !poll::sleep(interval, shutdown) {
            break;
        }

        rounds(
            &mut client,
            config,
            &mut watched,
            resolve,
            shutdown,
            &mut on_event,
        )?;
    }

    Ok(())
}

/// Watches every configured collection over one EventSource stream, until
/// `shutdown` is set.
///
/// The stream is held until it pushes a state the watch does not already
/// hold, then dropped for a round, which leaves the loop looking like an
/// IMAP IDLE. It holds its own connection, the client's being the
/// round's.
///
/// `on_open` is called once the session is up and its collections read,
/// right before the first wait.
pub fn watch_push(
    config: &JmapConfig,
    ping: u64,
    resolve: bool,
    shutdown: &Arc<AtomicBool>,
    on_open: impl FnOnce(),
    mut on_event: impl FnMut(WatchEvent, Option<ItemSummary>),
) -> Result<()> {
    let (mut client, mut watched) = arm(config)?;
    on_open();
    let mut last_event_id = None;

    while !shutdown.load(Ordering::SeqCst) {
        let news = subscribe(config, &watched, ping, &mut last_event_id, shutdown)?;

        if !news {
            continue;
        }

        rounds(
            &mut client,
            config,
            &mut watched,
            resolve,
            shutdown,
            &mut on_event,
        )?;
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

/// Runs one round per watched collection, each retried once on a fresh
/// session ([`poll::retried`]), its credential resolved again.
fn rounds(
    client: &mut JmapClientStd,
    config: &JmapConfig,
    watched: &mut [Watched],
    resolve: bool,
    shutdown: &AtomicBool,
    on_event: &mut impl FnMut(WatchEvent, Option<ItemSummary>),
) -> Result<()> {
    for watched in watched {
        poll::retried(
            client,
            shutdown,
            || Ok(open(config, &mut SecretResolver::new())?.0),
            |client| watched.round(client, resolve, on_event),
        )?;
    }

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
    ///
    /// The state is read before the listing, so an item moving meanwhile is
    /// reported by the first round rather than lost between the two.
    fn arm(client: &mut JmapClientStd, domain: WatchDomain, name: &str) -> Result<Self> {
        let (id, known, state) = match domain {
            WatchDomain::Message => {
                let id = resolve_mailbox(client, name)?;
                let state = client
                    .email_get(Vec::new(), get_options(false))
                    .context("cannot read the initial email state")?
                    .new_state;
                let known = baseline_mailbox(client, &id)?;
                (id, known, state)
            }
            WatchDomain::Card => {
                let id = resolve_addressbook(client, name)?;
                let opts = JmapContactCardGetOptions {
                    ids: Some(Vec::new()),
                    properties: Some(vec![String::from("id")]),
                };
                let state = client
                    .contact_card_get(opts)
                    .context("cannot read the initial contact state")?
                    .new_state;
                let known = baseline_addressbook(client, &id)?;
                (id, known, state)
            }
            WatchDomain::Event => {
                let id = resolve_calendar(client, name)?;
                let opts = JmapCalendarEventGetOptions {
                    ids: Some(Vec::new()),
                    properties: Some(vec![String::from("id")]),
                    ..Default::default()
                };
                let state = client
                    .calendar_event_get(opts)
                    .context("cannot read the initial event state")?
                    .new_state;
                let known = baseline_calendar(client, &id)?;
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

    /// Whether a pushed state change names a state of this collection's
    /// type other than the one the picture was last read at.
    fn moved(&self, change: &JmapStateChange) -> bool {
        change.changed.values().any(|states| {
            states
                .get(self.type_name())
                .is_some_and(|state| *state != self.state)
        })
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

/// Holds an EventSource subscription until the server pushes a state the
/// watch does not hold, and says whether one arrived.
///
/// News is the state itself rather than the event: a server may push
/// the states as they stand the moment the stream opens (Fastmail does),
/// and a frame with an empty `changed` map is its keep-alive. The stream
/// is asked to stay open (`closeafter=no`) and dropped on news, since
/// not every server ends it after a state event when asked to. A read
/// that times out is the wakeup this loop arms to look at the shutdown
/// flag. A stream that sent nothing, its pings included, for
/// [`PINGS_MISSED`] pings is taken for dead and handed back as ended, to
/// be opened again.
///
/// `last_event_id` is the id of the last event read, sent back so the
/// server pushes what moved between two subscriptions (RFC 8620 §7.3).
///
/// The subscription opens a session of its own, its credential resolved
/// again, and leaves the client's connection to the next round.
fn subscribe(
    config: &JmapConfig,
    watched: &[Watched],
    ping: u64,
    last_event_id: &mut Option<String>,
    shutdown: &Arc<AtomicBool>,
) -> Result<bool> {
    let (mut source, _url) = open(config, &mut SecretResolver::new())?;
    let session = source
        .session()
        .ok_or_else(|| anyhow!("The JMAP session was not read"))?;
    let types: Vec<&str> = watched.iter().map(Watched::type_name).collect();
    let opts = JmapEventSourceOptions {
        last_event_id: last_event_id.clone(),
    };
    let mut coroutine = JmapEventSource::new(
        session,
        &source.http_auth,
        &types,
        ping,
        JmapCloseAfter::No,
        shutdown.clone(),
        opts,
    )?;

    let silence = match ping {
        0 => Duration::MAX,
        ping => Duration::from_secs(ping * PINGS_MISSED),
    };
    let mut buf = [0u8; READ_BUF];
    let mut arg: Option<Vec<u8>> = None;

    loop {
        match coroutine.resume(arg.take().as_deref()) {
            JmapCoroutineState::Yielded(JmapEventSourceYield::Frame { change, id }) => {
                trace!("jmap state change: {change:?}");

                if let Some(id) = id.filter(|id| !id.is_empty()) {
                    *last_event_id = Some(id);
                }

                if watched.iter().any(|watched| watched.moved(&change)) {
                    return Ok(true);
                }
            }
            JmapCoroutineState::Yielded(JmapEventSourceYield::WantsRead) => {
                match read::read(&mut source.stream, &mut buf, shutdown, silence)
                    .context("read failed")?
                {
                    ReadOutcome::Bytes(read) => arg = Some(buf[..read].to_vec()),
                    ReadOutcome::Closed | ReadOutcome::Shutdown => return Ok(false),
                    ReadOutcome::Silent => {
                        debug!("jmap event stream missed {PINGS_MISSED} pings, reopening it");
                        return Ok(false);
                    }
                }
            }
            JmapCoroutineState::Yielded(JmapEventSourceYield::WantsWrite(bytes)) => {
                source.stream.write_all(&bytes).context("write failed")?;
            }
            JmapCoroutineState::Complete(Ok(())) => return Ok(false),
            JmapCoroutineState::Complete(Err(err)) => return Err(err.into()),
        }
    }
}

/// The code of a JMAP refusal in an error chain, read from the HTTP status
/// the session, a method call or the event source was answered with: a
/// 403 is `permission-denied`, a 429 `rate-limited`, a 507
/// `quota-exceeded`. Carillon only reads, so no `SetError` arises.
pub(crate) fn refusal_code(cause: &(dyn std::error::Error + 'static)) -> Option<ErrorCode> {
    let status = if let Some(err) = cause.downcast_ref::<JmapSendError>() {
        match err {
            JmapSendError::HttpStatus(status) | JmapSendError::Request { status, .. } => *status,
            _ => return None,
        }
    } else if let Some(err) = cause.downcast_ref::<JmapClientStdError>() {
        match err {
            JmapClientStdError::SessionGet(JmapSessionGetError::HttpStatus(status)) => *status,
            _ => return None,
        }
    } else {
        match cause.downcast_ref::<JmapEventSourceError>()? {
            JmapEventSourceError::HttpStatus(status) => *status,
            _ => return None,
        }
    };

    match status {
        403 => Some(ErrorCode::PermissionDenied),
        429 => Some(ErrorCode::RateLimited),
        507 => Some(ErrorCode::QuotaExceeded),
        _ => None,
    }
}

/// Lists what the watched mailbox holds, with each message's keywords.
fn baseline_mailbox(client: &mut JmapClientStd, mailbox_id: &str) -> Result<Known> {
    baseline(client, |client, position, limit| {
        let filter = JmapEmailFilter {
            in_mailbox: Some(mailbox_id.to_string()),
            ..Default::default()
        };
        let opts = JmapEmailQueryOptions {
            filter: Some(filter.into()),
            position: Some(position),
            limit: Some(limit),
            properties: Some(vec![JmapEmailProperty::Id, JmapEmailProperty::Keywords]),
            ..Default::default()
        };

        let listed = client
            .email_query(opts)
            .context("cannot list the watched mailbox")?;

        Ok(Page {
            held: (listed.emails.len() + listed.not_found.len()) as u64,
            total: listed.total,
            members: listed
                .emails
                .into_iter()
                .filter_map(|email| Some((email.id?, render_keywords(email.keywords.as_ref()))))
                .collect(),
        })
    })
}

/// Lists what the watched addressbook holds.
fn baseline_addressbook(client: &mut JmapClientStd, addressbook_id: &str) -> Result<Known> {
    baseline(client, |client, position, limit| {
        let filter = JmapContactCardFilter {
            in_address_book: Some(addressbook_id.to_string()),
            ..Default::default()
        };
        let opts = JmapContactCardQueryOptions {
            filter: Some(filter),
            position: Some(position),
            limit: Some(limit),
            properties: Some(vec![String::from("id")]),
            ..Default::default()
        };

        let listed = client
            .contact_card_query(opts)
            .context("cannot list the watched addressbook")?;

        Ok(Page {
            held: (listed.cards.len() + listed.not_found.len()) as u64,
            total: listed.total,
            members: listed
                .cards
                .into_iter()
                .filter_map(|card| Some((card.id?, BTreeSet::new())))
                .collect(),
        })
    })
}

/// Lists what the watched calendar holds, one id per series.
fn baseline_calendar(client: &mut JmapClientStd, calendar_id: &str) -> Result<Known> {
    baseline(client, |client, position, limit| {
        let filter = JmapCalendarEventFilter {
            in_calendar: Some(calendar_id.to_string()),
            ..Default::default()
        };
        let opts = JmapCalendarEventQueryOptions {
            filter: Some(filter),
            position: Some(position),
            limit: Some(limit),
            properties: Some(vec![String::from("id")]),
            ..Default::default()
        };

        let listed = client
            .calendar_event_query(opts)
            .context("cannot list the watched calendar")?;

        Ok(Page {
            held: (listed.events.len() + listed.not_found.len()) as u64,
            total: listed.total,
            members: listed
                .events
                .into_iter()
                .filter_map(|event| Some((event.id?, BTreeSet::new())))
                .collect(),
        })
    })
}

/// Lists a collection page by page through `list`, given a position and a
/// limit, until a page comes back empty or `total` items were read.
///
/// An item leaving between two pages shifts the next ones up and hides
/// one, so a listing whose count does not end as the server's `total` is
/// read again from the start, [`LISTINGS`] times at most.
fn baseline(
    client: &mut JmapClientStd,
    mut list: impl FnMut(&mut JmapClientStd, u64, u64) -> Result<Page>,
) -> Result<Known> {
    let limit = client
        .session()
        .and_then(|session| session.core_capability().max_objects_in_get)
        .unwrap_or(PAGE)
        .clamp(1, PAGE);

    for listing in 1..=LISTINGS {
        let mut known = Known::new();
        let mut position = 0;

        let total = loop {
            let page = list(client, position, limit)?;

            if page.held == 0 {
                break page.total;
            }

            position += page.held;
            known.extend(page.members);

            if page.total.is_some_and(|total| position >= total) {
                break page.total;
            }
        };

        match total {
            Some(total) if known.len() as u64 != total => {
                debug!(
                    "jmap listing {listing} read {} of {total} items",
                    known.len()
                );
            }
            _ => return Ok(known),
        }
    }

    bail!("The watched collection kept moving while it was listed {LISTINGS} times")
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

    found.and_then(|mailbox| mailbox.id.clone()).ok_or_else(|| {
        let message = format!("Mailbox `{mailbox}` not found on the JMAP server");
        error::not_found(WatchDomain::Message, message)
    })
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
        .ok_or_else(|| {
            let message = format!("Addressbook `{addressbook}` not found on the JMAP server");
            error::not_found(WatchDomain::Card, message)
        })
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
        .ok_or_else(|| {
            let message = format!("Calendar `{calendar}` not found on the JMAP server");
            error::not_found(WatchDomain::Event, message)
        })
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

/// One page of a baseline listing.
struct Page {
    /// The ids the page named, the ones gone by the time they were read
    /// included, which is how far the next page starts.
    held: u64,
    /// How many items the collection holds, when the server counted them.
    total: Option<u64>,
    /// The items read, each with its keywords.
    members: Vec<(String, BTreeSet<String>)>,
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::{BufRead, BufReader, Read},
        net::{SocketAddr, TcpListener, TcpStream},
        path::Path,
        sync::{Mutex, mpsc},
        thread,
    };

    use io_jmap::rfc8621::email::JmapEmailAddress;
    use tempfile::TempDir;

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

    /// Fastmail pushes the states as they stand when a stream opens, so a
    /// state the watch already holds is not news, nor is another type's.
    #[test]
    fn only_a_state_the_watch_does_not_hold_is_news() {
        let mut cards = watched(WatchDomain::Card);
        cards.state = String::from("S1");

        let change = |kind: &str, state: &str| JmapStateChange {
            r#type: String::from("StateChange"),
            changed: [(
                String::from("u1"),
                [(String::from(kind), String::from(state))].into(),
            )]
            .into(),
        };

        assert!(!cards.moved(&change("ContactCard", "S1")));
        assert!(!cards.moved(&change("Email", "S2")));
        assert!(cards.moved(&change("ContactCard", "S2")));
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

    /// The failure opening a session meets on a server answering its first
    /// request with `status`.
    fn refused(status: &'static str) -> anyhow::Error {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut reader = BufReader::new(stream.try_clone().unwrap());

            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line.trim_end().is_empty() {
                    break;
                }
            }

            let answer = format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\n\r\n");
            let _ = stream.write_all(answer.as_bytes());
        });

        let config: JmapConfig = toml::from_str(&format!(
            "mailbox = \"INBOX\"\nserver = \"http://{addr}/jmap/session\"\nauth.header.raw = \"token\"\n"
        ))
        .unwrap();

        open(&config, &mut SecretResolver::new()).err().unwrap()
    }

    #[test]
    fn a_session_refused_with_403_or_429_carries_its_code() {
        let denied = refused("403 Forbidden");
        let limited = refused("429 Too Many Requests");

        assert_eq!(
            Some(ErrorCode::PermissionDenied),
            crate::error::code(&denied),
            "{denied:#}"
        );
        assert_eq!(
            Some(ErrorCode::RateLimited),
            crate::error::code(&limited),
            "{limited:#}"
        );
    }

    /// What the loopback JMAP server of [`serve`] holds.
    #[derive(Default)]
    struct Mock {
        /// The one bearer token taken, any other answered 401.
        token: String,
        /// How many messages the inbox holds, `e1` to `e<held>`, the
        /// Email state being `s<held>`.
        held: u64,
        /// The most ids one `Email/query` answers, none when 0.
        cap: u64,
        /// Whether a message arrives right after the listing reaches the
        /// end of the inbox.
        arrive: bool,
        /// Whether the event stream sends its head and then nothing, as a
        /// stream gone dead reads.
        silent: bool,
    }

    /// A loopback JMAP server answering from `mock`: the session, the inbox,
    /// its messages, their changes, and an event stream pushing the Email
    /// state `s<held + 1>`.
    fn serve(mock: Arc<Mutex<Mock>>) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else {
                    return;
                };
                let mock = mock.clone();
                thread::spawn(move || answer(stream, &mock, addr));
            }
        });

        addr
    }

    /// Answers the requests of one connection of [`serve`].
    fn answer(mut stream: TcpStream, mock: &Mutex<Mock>, addr: SocketAddr) {
        let mut reader = BufReader::new(stream.try_clone().unwrap());

        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                return;
            }

            let path = line.split(' ').nth(1).unwrap_or_default().to_string();
            let mut auth = String::new();
            let mut len = 0;

            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                let header = header.trim_end();
                if header.is_empty() {
                    break;
                }

                let (name, value) = header.split_once(':').unwrap();
                match name.to_ascii_lowercase().as_str() {
                    "authorization" => auth = value.trim().to_string(),
                    "content-length" => len = value.trim().parse().unwrap(),
                    _ => (),
                }
            }

            let mut body = vec![0; len];
            reader.read_exact(&mut body).unwrap();
            let mut mock = mock.lock().unwrap();

            if auth != format!("Bearer {}", mock.token) {
                let refusal = "HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\n\r\n";
                stream.write_all(refusal.as_bytes()).unwrap();
                continue;
            }

            if path.starts_with("/es") {
                let state = mock.held + 1;
                let silent = mock.silent;
                drop(mock);
                let frame = format!(
                    "event: state\ndata: {{\"@type\":\"StateChange\",\
                     \"changed\":{{\"A1\":{{\"Email\":\"s{state}\"}}}}}}\n\n"
                );
                let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\
                            Transfer-Encoding: chunked\r\n\r\n";
                let chunk = format!("{head}{:x}\r\n{frame}\r\n", frame.len());
                let chunk = if silent { String::from(head) } else { chunk };
                stream.write_all(chunk.as_bytes()).unwrap();
                let _ = reader.read_line(&mut String::new());
                return;
            }

            let json = if body.is_empty() {
                session(addr)
            } else {
                let request: serde_json::Value = serde_json::from_slice(&body).unwrap();
                let mut ids = serde_json::Value::Null;
                let mut responses = Vec::new();

                for call in request["methodCalls"].as_array().unwrap() {
                    let name = call[0].as_str().unwrap();
                    let args = mock.call(name, &call[1], &mut ids);
                    responses.push(serde_json::json!([name, args, call[2]]));
                }

                serde_json::json!({ "methodResponses": responses, "sessionState": "sess1" })
            };

            let json = json.to_string();
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
                json.len()
            );
            stream.write_all(head.as_bytes()).unwrap();
            stream.write_all(json.as_bytes()).unwrap();
        }
    }

    impl Mock {
        /// Answers one method call, `ids` holding what the last
        /// `Email/query` listed for a back-reference to read.
        fn call(
            &mut self,
            name: &str,
            args: &serde_json::Value,
            ids: &mut serde_json::Value,
        ) -> serde_json::Value {
            let state = format!("s{}", self.held);

            match name {
                "Mailbox/get" => serde_json::json!({
                    "accountId": "A1", "state": "m1", "notFound": [],
                    "list": [{ "id": "mb1", "name": "Inbox", "role": "inbox" }],
                }),
                "Email/query" => {
                    let position = args["position"].as_u64().unwrap_or(0);
                    let mut limit = args["limit"].as_u64().unwrap_or(self.held);
                    if self.cap > 0 {
                        limit = limit.min(self.cap);
                    }

                    let total = self.held;
                    let end = (position + limit).min(total);
                    *ids = (position + 1..=end).map(|i| format!("e{i}")).collect();

                    if self.arrive && end >= total {
                        self.arrive = false;
                        self.held += 1;
                    }

                    serde_json::json!({
                        "accountId": "A1", "queryState": "q", "canCalculateChanges": false,
                        "position": position, "total": total, "ids": ids,
                    })
                }
                "Email/get" => {
                    let wanted = if args.get("#ids").is_some() {
                        ids.clone()
                    } else {
                        args["ids"].clone()
                    };
                    let list: Vec<_> = wanted
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|id| {
                            serde_json::json!({
                                "id": id, "mailboxIds": { "mb1": true }, "keywords": {},
                            })
                        })
                        .collect();

                    serde_json::json!({
                        "accountId": "A1", "state": state, "list": list, "notFound": [],
                    })
                }
                "Email/changes" => {
                    let since = args["sinceState"].as_str().unwrap();
                    let since: u64 = since[1..].parse().unwrap();
                    let created: Vec<_> =
                        (since + 1..=self.held).map(|i| format!("e{i}")).collect();

                    serde_json::json!({
                        "accountId": "A1", "oldState": format!("s{since}"), "newState": state,
                        "hasMoreChanges": false, "created": created, "updated": [], "destroyed": [],
                    })
                }
                _ => serde_json::json!({ "type": "unknownMethod" }),
            }
        }
    }

    /// The session [`serve`] answers.
    fn session(addr: SocketAddr) -> serde_json::Value {
        serde_json::json!({
            "username": "u",
            "state": "sess1",
            "apiUrl": format!("http://{addr}/api"),
            "downloadUrl": format!("http://{addr}/dl/{{accountId}}/{{blobId}}/{{name}}?type={{type}}"),
            "uploadUrl": format!("http://{addr}/ul/{{accountId}}"),
            "eventSourceUrl": format!("http://{addr}/es?types={{types}}&closeafter={{closeafter}}&ping={{ping}}"),
            "capabilities": {
                "urn:ietf:params:jmap:core": {
                    "maxSizeUpload": 1, "maxConcurrentUpload": 1,
                    "maxSizeRequest": 1, "maxConcurrentRequests": 1,
                    "maxCallsInRequest": 16, "maxObjectsInGet": 500,
                    "maxObjectsInSet": 500, "collationAlgorithms": [],
                },
                "urn:ietf:params:jmap:mail": {},
            },
            "accounts": {
                "A1": {
                    "name": "u", "isPersonal": true, "isReadOnly": false,
                    "accountCapabilities": { "urn:ietf:params:jmap:mail": {} },
                },
            },
            "primaryAccounts": { "urn:ietf:params:jmap:mail": "A1" },
        })
    }

    /// An account on [`serve`] whose token command reads `token`.
    fn on_mock(addr: SocketAddr, token: &Path) -> JmapConfig {
        toml::from_str(&format!(
            "mailbox = \"INBOX\"\nserver = \"http://{addr}/session\"\n\
             auth.bearer.token.command = [\"cat\", \"{}\"]\n",
            token.display()
        ))
        .unwrap()
    }

    /// A session opened on [`serve`] answering from `mock`, its token `T1`.
    fn opened(mock: Mock) -> (JmapClientStd, JmapConfig, Arc<Mutex<Mock>>, TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let token = dir.path().join("token");
        fs::write(&token, "T1").unwrap();
        let mock = Arc::new(Mutex::new(Mock {
            token: String::from("T1"),
            ..mock
        }));

        let config = on_mock(serve(mock.clone()), &token);
        let (client, _url) = open(&config, &mut SecretResolver::new()).unwrap();

        (client, config, mock, dir)
    }

    /// A session opened on `T1`, the server then taking `T2` alone, as a
    /// token rotating while the watch holds it.
    fn expired() -> (JmapClientStd, JmapConfig, TempDir) {
        let held = Mock {
            held: 1,
            ..Default::default()
        };
        let (client, config, mock, dir) = opened(held);

        fs::write(dir.path().join("token"), "T2").unwrap();
        mock.lock().unwrap().token = String::from("T2");

        (client, config, dir)
    }

    #[test]
    fn a_round_retried_presents_the_credential_resolved_again() {
        let (mut client, config, _dir) = expired();
        let mut watched = [watched(WatchDomain::Message)];
        watched[0].state = String::from("s1");
        let shutdown = AtomicBool::new(false);

        let round = rounds(
            &mut client,
            &config,
            &mut watched,
            false,
            &shutdown,
            &mut |_, _| (),
        );

        round.unwrap();
    }

    #[test]
    fn a_subscription_presents_the_credential_resolved_again() {
        let (_client, config, _dir) = expired();
        let mut watched = watched(WatchDomain::Message);
        watched.state = String::from("s1");
        let shutdown = Arc::new(AtomicBool::new(false));

        let news = subscribe(&config, &[watched], 0, &mut None, &shutdown);

        assert!(news.unwrap());
    }

    #[test]
    fn a_baseline_reads_every_page_the_server_answers() {
        let capped = Mock {
            held: 5,
            cap: 2,
            ..Default::default()
        };
        let (mut client, _config, _mock, _dir) = opened(capped);

        let watched = Watched::arm(&mut client, WatchDomain::Message, "INBOX").unwrap();

        assert_eq!(5, watched.known.len());
    }

    #[test]
    fn an_arrival_while_the_baseline_is_listed_is_reported() {
        let arriving = Mock {
            held: 3,
            arrive: true,
            ..Default::default()
        };
        let (mut client, _config, _mock, _dir) = opened(arriving);
        let mut watched = Watched::arm(&mut client, WatchDomain::Message, "INBOX").unwrap();
        let mut events = Vec::new();

        watched
            .round(&mut client, false, &mut |event, _| events.push(event))
            .unwrap();

        let added = WatchEvent::ItemAdded {
            domain: WatchDomain::Message,
            id: String::from("e4"),
        };
        assert_eq!(vec![added], events);
    }

    #[test]
    fn a_listing_short_of_its_total_is_read_again() {
        let (mut client, _config, _mock, _dir) = opened(Mock::default());
        let mut listings = 0;

        let known = baseline(&mut client, |_, position, _| {
            if position == 0 {
                listings += 1;
            }

            let ids: &[&str] = match (listings, position) {
                (1, 0) => &["e1"],
                (1, _) => &[],
                (_, 0) => &["e1", "e2"],
                _ => &[],
            };

            Ok(Page {
                held: ids.len() as u64,
                total: Some(2),
                members: ids
                    .iter()
                    .map(|id| (id.to_string(), BTreeSet::new()))
                    .collect(),
            })
        })
        .unwrap();

        assert_eq!(2, listings);
        assert_eq!(2, known.len());
    }

    #[test]
    fn an_event_stream_missing_its_pings_is_opened_again() {
        let silent = Mock {
            held: 1,
            silent: true,
            ..Default::default()
        };
        let (_client, config, _mock, _dir) = opened(silent);
        let (sent, got) = mpsc::channel();

        thread::spawn(move || {
            let mut watched = watched(WatchDomain::Message);
            watched.state = String::from("s1");
            let shutdown = Arc::new(AtomicBool::new(false));
            let news = subscribe(&config, &[watched], 1, &mut None, &shutdown);
            sent.send(news.map_err(|err| err.to_string())).unwrap();
        });

        let news = got.recv_timeout(Duration::from_secs(15)).unwrap();
        assert_eq!(Ok(false), news);
    }
}
