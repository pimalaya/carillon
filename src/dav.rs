//! # WebDAV
//!
//! The WebDAV backend: an RFC 6578 `sync-collection` poll over a DAV
//! collection, whichever domain it holds.
//!
//! CalDAV and CardDAV are WebDAV, so one poll serves both DAV backends;
//! what differs is what a member is called, and that is [`DavKind`]. An
//! addressbook holds cards, known before the first request; a calendar
//! holds events, tasks or both, so it is asked when the watch starts.
//!
//! The report asks for `getetag` only, so a poll carries no vCard and no
//! VEVENT. Created and updated members are reported together, so the
//! backend keeps an href to etag picture and reads the difference: an
//! unseen href is an arrival, a known one whose etag moved is an edit.
//!
//! Each member's domain is remembered beside its etag, a removal having
//! only an href left to be recognised by.

use std::{
    collections::BTreeMap,
    error,
    io::{self, Read, Write},
    mem,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use anyhow::{Context, Error, Result, anyhow, bail};
use io_http::{
    client::HttpClientStd, rfc6750::bearer::HttpAuthBearer, rfc7617::basic::HttpAuthBasic,
};
use io_webdav::{
    client::WebdavClientStd,
    coroutine::{WebdavCoroutine, WebdavCoroutineState, WebdavYield},
    rfc4791::calendar::{SUPPORTED_CALENDAR_COMPONENT_SET, home_set::CaldavCalendarHomeSet},
    rfc4918::{
        DAV, GETETAG, WebdavAuth, WebdavMultistatus, WebdavProperty,
        coroutine::WebdavRedirectYield, follow_redirects::WebdavFollowRedirectsError,
        propfind::WebdavPropfind, send::WebdavSendError,
    },
    rfc5397::current_user_principal::WebdavCurrentUserPrincipal,
    rfc6352::addressbook::home_set::CarddavAddressbookHomeSet,
    rfc6578::sync_collection::{
        WebdavSyncChange, WebdavSyncCollection, WebdavSyncCollectionError,
        WebdavSyncCollectionOptions, WebdavSyncDelta,
    },
};
use log::{debug, trace, warn};
use pimalaya_config::secret::SecretResolver;
use pimalaya_stream::{
    retry::Retry,
    stream::{Stream, TcpConnectOptions, TlsConnectOptions},
};
use secrecy::ExposeSecret;
use url::Url;

use crate::{
    config::{DavAuthConfig, DavServer, ProxyConfig},
    event::{ItemSummary, WatchDomain, WatchEvent},
    poll,
};

/// How long the watch waits between two reports, unless the config
/// says otherwise.
const POLL_INTERVAL: Duration = Duration::from_secs(60);
/// How long a poll may sit in a read before looking at the shutdown flag.
const READ_TIMEOUT: Duration = Duration::from_secs(1);
/// Per-read scratch buffer.
const READ_BUF: usize = 8 * 1024;

/// `DAV:getcontenttype` (RFC 4918 §15.5), which a CalDAV server spells
/// with the `component` parameter RFC 4791 §10.1 allows.
// NOTE: belongs upstream beside io-webdav's own GETETAG; declared here
// until it lands there.
const GETCONTENTTYPE: WebdavProperty = WebdavProperty {
    ns: DAV,
    local: "getcontenttype",
};

/// What the watched collection holds, which is what its members are
/// called.
pub enum DavKind {
    /// A CalDAV calendar, carrying the domains its hooks name so that
    /// a calendar not holding one of them can say so.
    Calendar(Vec<WatchDomain>),
    /// A CardDAV addressbook, whose members are all cards.
    Addressbook,
}

impl DavKind {
    /// The home set a collection of this kind hangs under.
    pub fn home(&self) -> DavHome {
        match self {
            Self::Calendar(_) => DavHome::Calendars,
            Self::Addressbook => DavHome::Addressbooks,
        }
    }
}

/// The home set a collection named by its id is looked up under.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DavHome {
    /// The calendar home set (RFC 4791 §6.2.1).
    Calendars,
    /// The addressbook home set (RFC 6352 §7.1.1).
    Addressbooks,
}

/// Opens a connection to the configured server, resolving its credential
/// through `resolver`, so a caller opening several backends of one account
/// spawns each distinct credential command once.
///
/// The stream carries a read deadline and hands back the failures that
/// only mean "not ready yet", so a poll against a server that stopped
/// answering ends at the next deadline rather than holding the thread.
pub fn open(config: DavServer<'_>, resolver: &mut SecretResolver) -> Result<WebdavClientStd> {
    let url = Url::parse(config.server)
        .with_context(|| format!("Invalid DAV server URL `{}`", config.server))?;
    let host = url
        .host_str()
        .ok_or_else(|| anyhow!("DAV server URL `{url}` has no host"))?
        .to_string();

    let alpn = match config.alpn {
        Some(alpn) => alpn.to_vec(),
        None => HttpClientStd::default_alpn(),
    };
    let tls = config.tls.clone().into_tls(alpn);
    let proxy = ProxyConfig::resolve(config.proxy.cloned(), resolver)?;

    let stream = match url.scheme() {
        "http" => {
            let port = url.port().unwrap_or(80);
            let opts = TcpConnectOptions {
                proxy,
                retry: Retry::Never,
            };
            Stream::connect_tcp(&host, port, opts)?
        }
        "https" => {
            let port = url.port().unwrap_or(443);
            let opts = TlsConnectOptions {
                tls,
                proxy,
                retry: Retry::Never,
            };
            Stream::connect_tls(&host, port, opts)?
        }
        scheme => bail!("Unsupported DAV scheme `{scheme}`, expected http or https"),
    };

    stream.set_read_timeout(Some(READ_TIMEOUT))?;

    debug!("opened dav connection");
    trace!("server: {url}");

    let auth = auth(config.auth, resolver)?;

    Ok(WebdavClientStd::new(stream, auth, url))
}

/// Watches the configured collection until `shutdown` is set, calling
/// `on_event` for every change.
///
/// The first enumeration is the baseline, so it reports nothing.
/// Everything after is read against that picture. A round that fails is
/// run once more on a fresh connection before the session is given up,
/// the idle connection being one a server is free to close between two
/// polls.
pub fn watch(
    config: DavServer<'_>,
    kind: DavKind,
    collection: &str,
    interval: Option<Duration>,
    shutdown: &Arc<AtomicBool>,
    mut on_event: impl FnMut(WatchEvent, Option<ItemSummary>),
) -> Result<()> {
    let mut client = open(config, &mut SecretResolver::new())?;
    let collection = locate(&mut client, kind.home(), collection, shutdown)?;
    let interval = interval.unwrap_or(POLL_INTERVAL);
    let domains = Domains::resolve(&mut client, &collection, kind, shutdown)?;
    let mut watched = Watched::arm(&mut client, collection, domains, shutdown)?;

    while !shutdown.load(Ordering::SeqCst) {
        if !poll::sleep(interval, shutdown) {
            break;
        }

        if let Err(err) = watched.round(&mut client, shutdown, &mut on_event) {
            if shutdown.load(Ordering::SeqCst) {
                break;
            }

            debug!("dav round failed, reconnecting: {err:#}");
            client = open(config, &mut SecretResolver::new())?;
            watched.round(&mut client, shutdown, &mut on_event)?;
        }
    }

    Ok(())
}

/// The collection the watch holds a picture of, and how it reads it.
struct Watched {
    /// The request path of the collection.
    collection: String,
    /// How a member's domain is decided.
    domains: Domains,
    /// What the collection holds, as of `token`.
    known: Known,
    /// The sync token the next report reads from, `None` before one was
    /// handed back and always on a server listing instead.
    token: Option<String>,
    /// Whether the server lacks `sync-collection`, so that every round
    /// lists the collection and reads it against the picture.
    listing: bool,
}

impl Watched {
    /// Enumerates the collection, which is the baseline a later change
    /// is a change against, learning on the way whether the server has
    /// `sync-collection` at all.
    fn arm(
        client: &mut WebdavClientStd,
        collection: String,
        domains: Domains,
        shutdown: &Arc<AtomicBool>,
    ) -> Result<Self> {
        let mut watched = Self {
            collection,
            domains,
            known: Known::new(),
            token: None,
            listing: false,
        };

        let snapshot = watched.snapshot(client, shutdown)?;
        watched.rebase(client, snapshot, shutdown)?;

        debug!(
            "watching dav collection with {} members{}",
            watched.known.len(),
            if watched.listing { ", listing" } else { "" }
        );

        Ok(watched)
    }

    /// Reads what moved since the last round and reports it.
    ///
    /// A truncated report means the server stopped early and the rest
    /// waits behind the token it just handed back, so it is drained now
    /// rather than at the next interval. Each report is applied whole, so
    /// a round failing part way leaves the picture and the token at the
    /// last report it completed, and can simply be run again.
    fn round(
        &mut self,
        client: &mut WebdavClientStd,
        shutdown: &Arc<AtomicBool>,
        on_event: &mut impl FnMut(WatchEvent, Option<ItemSummary>),
    ) -> Result<()> {
        if self.listing {
            let snapshot = self.snapshot(client, shutdown)?;

            for event in self.rebase(client, snapshot, shutdown)? {
                on_event(event, None);
            }

            return Ok(());
        }

        loop {
            let delta = match sync(client, &self.collection, self.token.as_deref(), shutdown) {
                Ok(delta) => delta,
                // NOTE: a rejected token means the server's history no
                // longer reaches that far, so the collection is
                // enumerated again and read against the picture, the gap
                // reporting only what actually differs.
                Err(err) if is_invalid_token(&err) => {
                    warn!("dav sync token rejected, enumerating the collection again");
                    let snapshot = self.snapshot(client, shutdown)?;

                    for event in self.rebase(client, snapshot, shutdown)? {
                        on_event(event, None);
                    }

                    return Ok(());
                }
                Err(err) => return Err(err),
            };

            let truncated = delta.truncated;

            for event in reconcile(
                client,
                &mut self.known,
                &self.domains,
                delta,
                &mut self.token,
                shutdown,
            ) {
                on_event(event, None);
            }

            if !truncated || shutdown.load(Ordering::SeqCst) {
                return Ok(());
            }
        }
    }

    /// Enumerates every member of the collection with its etag: a
    /// `sync-collection` from no token, drained when truncated, or a
    /// `PROPFIND` listing on a server that has no such report.
    ///
    /// The token it ends on is only kept once the enumeration answered,
    /// so a failure leaves the one the picture was taken at.
    fn snapshot(
        &mut self,
        client: &mut WebdavClientStd,
        shutdown: &Arc<AtomicBool>,
    ) -> Result<Snapshot> {
        if self.listing {
            return list(client, &self.collection, shutdown).map(Snapshot::from);
        }

        let mut changes = Vec::new();
        let mut token: Option<String> = None;

        loop {
            let delta = match sync(client, &self.collection, token.as_deref(), shutdown) {
                Ok(delta) => delta,
                // NOTE: RFC 6578 is an extension, so a server may refuse
                // the report outright (RFC 3253 §3.6). The collection is
                // then listed on every round instead, which costs a full
                // listing each time but still tells an edit by its etag.
                Err(err) if token.is_none() && is_unsupported_report(&err) => {
                    debug!("dav server has no sync-collection, listing the collection instead");
                    self.listing = true;
                    self.token = None;
                    return list(client, &self.collection, shutdown).map(Snapshot::from);
                }
                Err(err) => return Err(err),
            };

            changes.extend(delta.changed);
            token = delta.sync_token;

            if !delta.truncated || token.is_none() || shutdown.load(Ordering::SeqCst) {
                let complete = !delta.truncated;
                self.token = token;
                return Ok(Snapshot { changes, complete });
            }
        }
    }

    /// Reads a full enumeration against the picture, which it replaces,
    /// and hands back what differs.
    ///
    /// A mixed calendar reads the content type of every member in one
    /// `PROPFIND` when the enumeration holds one it has not seen, rather
    /// than one request per member.
    fn rebase(
        &mut self,
        client: &mut WebdavClientStd,
        snapshot: Snapshot,
        shutdown: &Arc<AtomicBool>,
    ) -> Result<Vec<WatchEvent>> {
        let unseen = snapshot
            .changes
            .iter()
            .any(|change| !self.known.contains_key(&change.href));
        let types = match self.domains {
            Domains::Mixed(_) if unseen => content_types(client, &self.collection, shutdown)?,
            _ => BTreeMap::new(),
        };

        Ok(rebase(&mut self.known, &self.domains, &types, snapshot))
    }
}

/// A full enumeration of the collection.
struct Snapshot {
    /// Every member listed, with its etag.
    changes: Vec<WebdavSyncChange>,
    /// Whether the server listed them all, a truncated one saying nothing
    /// of the members it left out.
    complete: bool,
}

impl From<WebdavSyncDelta> for Snapshot {
    fn from(delta: WebdavSyncDelta) -> Self {
        Self {
            complete: !delta.truncated,
            changes: delta.changed,
        }
    }
}

/// Reads a full enumeration against `known`, which it replaces, and
/// returns what differs: a member never seen is an arrival, a known one
/// whose etag moved an edit, and one no longer listed a removal, under
/// the domain remembered for it. A truncated enumeration reports no
/// removal, since it does not say what it left out.
fn rebase(
    known: &mut Known,
    domains: &Domains,
    types: &BTreeMap<String, String>,
    snapshot: Snapshot,
) -> Vec<WatchEvent> {
    let mut fresh = Known::new();
    let mut events = Vec::new();

    for change in snapshot.changes {
        match known.remove(&change.href) {
            Some((before, domain)) => {
                if before != change.etag {
                    events.push(WatchEvent::ItemChanged {
                        domain,
                        id: change.href.clone(),
                    });
                }

                fresh.insert(change.href, (change.etag, domain));
            }
            None => {
                let domain = domains.read(types.get(&change.href).map(String::as_str));

                events.push(WatchEvent::ItemAdded {
                    domain,
                    id: change.href.clone(),
                });
                fresh.insert(change.href, (change.etag, domain));
            }
        }
    }

    for (href, (etag, domain)) in mem::take(known) {
        if snapshot.complete {
            events.push(WatchEvent::ItemRemoved { domain, id: href });
        } else {
            fresh.insert(href, (etag, domain));
        }
    }

    *known = fresh;

    events
}

/// What the watch knows of the collection: an href to its etag and the
/// domain it turned out to hold.
type Known = BTreeMap<String, (Option<String>, WatchDomain)>;

/// How a member's domain is decided, once the collection has been asked
/// what it holds.
enum Domains {
    /// Every member is the same thing, which an addressbook and a
    /// single-component calendar both are, and which costs nothing.
    Fixed(WatchDomain),
    /// A calendar holding several components, where a member has to be
    /// recognised by the `component` parameter of its content type, or
    /// else taken for the domain carried here.
    Mixed(WatchDomain),
}

impl Domains {
    /// Asks the collection what it holds, which only a calendar has to be
    /// asked.
    ///
    /// A calendar advertising one component answers for every member at
    /// once. Hooks naming a component it does not hold are refused here,
    /// a hook that could never fire being a configuration error.
    ///
    /// A member of a mixed calendar whose content type names no component,
    /// which RFC 4791 §10.1 allows, is taken for the one domain the hooks
    /// name, or for an event when they name both.
    fn resolve(
        client: &mut WebdavClientStd,
        collection: &str,
        kind: DavKind,
        shutdown: &Arc<AtomicBool>,
    ) -> Result<Self> {
        let wanted = match kind {
            DavKind::Addressbook => return Ok(Self::Fixed(WatchDomain::Card)),
            DavKind::Calendar(wanted) => wanted,
        };

        let held = components(client, collection, shutdown)?;

        // NOTE: a server not answering the property leaves the hooks as
        // the only statement of what the calendar holds.
        let held = if held.is_empty() {
            wanted.clone()
        } else {
            held
        };

        for domain in &wanted {
            if !held.contains(domain) {
                bail!(
                    "calendar `{collection}` holds no {}, so its hooks can never fire",
                    match domain {
                        WatchDomain::Task => "VTODO",
                        _ => "VEVENT",
                    }
                );
            }
        }

        match held.as_slice() {
            [domain] => {
                debug!("calendar holds one component");
                Ok(Self::Fixed(*domain))
            }
            _ => {
                debug!("calendar holds several components, reading content types");
                let fallback = match wanted.as_slice() {
                    [domain] => *domain,
                    _ => WatchDomain::Event,
                };
                Ok(Self::Mixed(fallback))
            }
        }
    }

    /// The domain of a member the watch has not seen before, read from
    /// the collection when one read is not enough to answer for all.
    fn of(
        &self,
        client: &mut WebdavClientStd,
        href: &str,
        shutdown: &Arc<AtomicBool>,
    ) -> Option<WatchDomain> {
        match self {
            Self::Fixed(domain) => Some(*domain),
            Self::Mixed(_) => match content_type(client, href, shutdown) {
                Ok(content_type) => Some(self.read(content_type.as_deref())),
                Err(err) => {
                    warn!("cannot read the content type of dav member `{href}`: {err:#}");
                    None
                }
            },
        }
    }

    /// The domain a member's content type names, or the fallback.
    fn read(&self, content_type: Option<&str>) -> WatchDomain {
        match self {
            Self::Fixed(domain) => *domain,
            Self::Mixed(fallback) => component(content_type).unwrap_or_else(|| {
                trace!("content type `{content_type:?}` names no component");
                *fallback
            }),
        }
    }
}

/// Reads a delta against what the watch knows, and reports what moved.
fn reconcile(
    client: &mut WebdavClientStd,
    known: &mut Known,
    domains: &Domains,
    delta: WebdavSyncDelta,
    token: &mut Option<String>,
    shutdown: &Arc<AtomicBool>,
) -> Vec<WatchEvent> {
    let mut events = Vec::new();

    for href in delta.vanished {
        // NOTE: a vanished member is gone, so what it was can only come
        // from what the watch remembered of it.
        if let Some((_etag, domain)) = known.remove(&href) {
            events.push(WatchEvent::ItemRemoved { domain, id: href });
        }
    }

    for change in delta.changed {
        match known.get(&change.href) {
            // NOTE: an href never seen before is an arrival, and the one
            // place a member's domain has to be worked out.
            None => {
                let Some(domain) = domains.of(client, &change.href, shutdown) else {
                    continue;
                };

                known.insert(change.href.clone(), (change.etag, domain));
                events.push(WatchEvent::ItemAdded {
                    domain,
                    id: change.href,
                });
            }
            // NOTE: RFC 6578 does not say whether a member was created or
            // updated, so a known href is an edit, and only when its etag
            // moved: a server may re-report an unchanged member.
            Some((before, domain)) => {
                let domain = *domain;
                let moved = *before != change.etag;

                known.insert(change.href.clone(), (change.etag, domain));

                if moved {
                    events.push(WatchEvent::ItemChanged {
                        domain,
                        id: change.href,
                    });
                }
            }
        }
    }

    if delta.sync_token.is_some() {
        *token = delta.sync_token;
    }

    events
}

/// Reads the components a CalDAV calendar holds, mapped onto the domains
/// its hooks are named after.
///
/// An empty answer is a server not carrying the property, not a calendar
/// holding nothing.
fn components(
    client: &mut WebdavClientStd,
    collection: &str,
    shutdown: &Arc<AtomicBool>,
) -> Result<Vec<WatchDomain>> {
    let multistatus = propfind(
        client,
        collection,
        0,
        &[SUPPORTED_CALENDAR_COMPONENT_SET],
        shutdown,
    )?;

    let mut domains = Vec::new();

    for entry in &multistatus.responses {
        let Some(prop) = entry.prop(SUPPORTED_CALENDAR_COMPONENT_SET) else {
            continue;
        };

        for child in &prop.children {
            let domain = match child.name.as_deref() {
                Some(name) if name.eq_ignore_ascii_case("VEVENT") => WatchDomain::Event,
                Some(name) if name.eq_ignore_ascii_case("VTODO") => WatchDomain::Task,
                // NOTE: VJOURNAL, VFREEBUSY and VTIMEZONE name no hook,
                // so a calendar holding them holds nothing to report.
                _ => continue,
            };

            if !domains.contains(&domain) {
                domains.push(domain);
            }
        }
    }

    trace!("calendar components: {domains:?}");

    Ok(domains)
}

/// Reads the content type of every member of the collection, keyed by
/// href.
fn content_types(
    client: &mut WebdavClientStd,
    collection: &str,
    shutdown: &Arc<AtomicBool>,
) -> Result<BTreeMap<String, String>> {
    let multistatus = propfind(client, collection, 1, &[GETCONTENTTYPE], shutdown)?;

    Ok(multistatus
        .responses
        .iter()
        .filter_map(|entry| {
            let text = entry.text(GETCONTENTTYPE)?;
            Some((entry.href.clone(), text.to_string()))
        })
        .collect())
}

/// Reads the content type of one member.
fn content_type(
    client: &mut WebdavClientStd,
    href: &str,
    shutdown: &Arc<AtomicBool>,
) -> Result<Option<String>> {
    let multistatus = propfind(client, href, 0, &[GETCONTENTTYPE], shutdown)?;

    Ok(multistatus
        .responses
        .iter()
        .find_map(|entry| entry.text(GETCONTENTTYPE))
        .map(ToString::to_string))
}

/// Reads a content type's `component` parameter (RFC 4791 §10.1), which
/// tells a VEVENT from a VTODO without reading either.
fn component(content_type: Option<&str>) -> Option<WatchDomain> {
    let value = content_type?
        .split(';')
        .skip(1)
        .map(str::trim)
        .find_map(|param| param.strip_prefix("component="))?
        .trim_matches('"');

    match value {
        value if value.eq_ignore_ascii_case("VEVENT") => Some(WatchDomain::Event),
        value if value.eq_ignore_ascii_case("VTODO") => Some(WatchDomain::Task),
        _ => None,
    }
}

/// Runs one PROPFIND over the open connection.
fn propfind(
    client: &mut WebdavClientStd,
    path: &str,
    depth: u8,
    props: &[WebdavProperty],
    shutdown: &Arc<AtomicBool>,
) -> Result<WebdavMultistatus> {
    let mut coroutine = WebdavPropfind::new(
        &client.base_url,
        client.auth(),
        &client.user_agent,
        path,
        depth,
        props,
    );

    pump(client, &mut coroutine, shutdown)
}

/// Runs one `sync-collection` report over the open connection.
fn sync(
    client: &mut WebdavClientStd,
    collection: &str,
    token: Option<&str>,
    shutdown: &Arc<AtomicBool>,
) -> Result<WebdavSyncDelta> {
    let mut coroutine = WebdavSyncCollection::new(
        &client.base_url,
        client.auth(),
        &client.user_agent,
        collection,
        token,
        &[GETETAG],
        WebdavSyncCollectionOptions::default(),
    );

    let delta = pump(client, &mut coroutine, shutdown)?;
    trace!("dav sync delta: {delta:?}");

    Ok(delta)
}

/// Lists every member of the collection with its etag through a
/// `PROPFIND`, what a server with no `sync-collection` offers instead.
fn list(
    client: &mut WebdavClientStd,
    collection: &str,
    shutdown: &Arc<AtomicBool>,
) -> Result<WebdavSyncDelta> {
    let mut coroutine = WebdavSyncCollection::new(
        &client.base_url,
        client.auth(),
        &client.user_agent,
        collection,
        None,
        &[GETETAG],
        WebdavSyncCollectionOptions { fallback: true },
    );

    let delta = pump(client, &mut coroutine, shutdown)?;
    trace!("dav listing: {delta:?}");

    if delta.truncated {
        warn!("dav server truncated the listing of {collection}");
    }

    Ok(delta)
}

/// Pumps one coroutine over the client's stream, checking the shutdown
/// flag between reads.
fn pump<C, T, E>(
    client: &mut WebdavClientStd,
    coroutine: &mut C,
    shutdown: &Arc<AtomicBool>,
) -> Result<T>
where
    C: WebdavCoroutine<Yield = WebdavYield, Return = Result<T, E>>,
    E: error::Error + Send + Sync + 'static,
{
    let mut buf = [0u8; READ_BUF];
    let mut arg: Option<Vec<u8>> = None;

    loop {
        match coroutine.resume(arg.take().as_deref()) {
            WebdavCoroutineState::Yielded(WebdavYield::WantsRead) => loop {
                if shutdown.load(Ordering::SeqCst) {
                    bail!("Shutting down");
                }

                match client.stream.read(&mut buf) {
                    Ok(0) => bail!("Connection closed by peer"),
                    Ok(read) => {
                        arg = Some(buf[..read].to_vec());
                        break;
                    }
                    Err(err) if is_timeout(&err) => continue,
                    Err(err) => return Err(err).context("read failed"),
                }
            },
            WebdavCoroutineState::Yielded(WebdavYield::WantsWrite(bytes)) => {
                client.stream.write_all(&bytes).context("write failed")?;
            }
            WebdavCoroutineState::Complete(Ok(value)) => return Ok(value),
            WebdavCoroutineState::Complete(Err(err)) => return Err(err.into()),
        }
    }
}

/// Builds the credential presented on every request, resolving it through
/// `resolver` rather than spawning its command again.
pub fn auth(config: &DavAuthConfig, resolver: &mut SecretResolver) -> Result<WebdavAuth> {
    Ok(match config {
        DavAuthConfig::None => WebdavAuth::None,
        DavAuthConfig::Basic { username, password } => WebdavAuth::Basic(HttpAuthBasic {
            username: username.clone(),
            password: resolver.resolve(password.clone())?,
        }),
        DavAuthConfig::Bearer { token } => {
            let token = resolver.resolve(token.clone())?;
            WebdavAuth::Bearer(HttpAuthBearer::new(token.expose_secret()))
        }
    })
}

/// Whether the failure is the server refusing the sync token, which
/// asks for an enumeration rather than a retry.
fn is_invalid_token(err: &Error) -> bool {
    err.downcast_ref::<WebdavSyncCollectionError>()
        .is_some_and(|err| matches!(err, WebdavSyncCollectionError::InvalidSyncToken))
}

/// Whether an I/O error is the read deadline expiring, which is a
/// wakeup rather than a failure.
fn is_timeout(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    )
}

/// Opens the collection and runs one report, which is what `check` needs.
///
/// It proves the transport, the credential, and that the collection is
/// where the configuration says. A server with no `sync-collection` is
/// listed instead, as the watch would.
pub fn probe(
    config: DavServer<'_>,
    home: DavHome,
    collection: &str,
    shutdown: &Arc<AtomicBool>,
    resolver: &mut SecretResolver,
) -> Result<()> {
    let mut client = open(config, resolver)?;
    let collection = locate(&mut client, home, collection, shutdown)?;

    match sync(&mut client, &collection, None, shutdown) {
        Err(err) if is_unsupported_report(&err) => {
            list(&mut client, &collection, shutdown)?;
        }
        result => {
            result?;
        }
    }

    Ok(())
}

/// Resolves the account's collection into the request path.
///
/// An absolute path is taken as it stands. A bare name, one path segment,
/// is the collection's id under the home set the server's principal
/// names (RFC 4791 §6.2.1, RFC 6352 §7.1.1), which is the id a listing of
/// the account gives; a server naming no home set reads it under its own
/// path instead. Any other relative path is read under the server URL's
/// own path.
fn locate(
    client: &mut WebdavClientStd,
    home: DavHome,
    collection: &str,
    shutdown: &Arc<AtomicBool>,
) -> Result<String> {
    if collection.starts_with('/') || collection.trim_matches('/').contains('/') {
        return Ok(path(&client.base_url, collection));
    }

    match home_set(client, home, shutdown) {
        Ok(Some(home)) => {
            debug!("dav home set: {home}");
            Ok(under(&home, collection))
        }
        Ok(None) => {
            debug!("dav server names no home set, reading `{collection}` under its path");
            Ok(path(&client.base_url, collection))
        }
        Err(err) => {
            debug!(
                "cannot discover the dav home set, reading `{collection}` under the server path: {err:#}"
            );
            Ok(path(&client.base_url, collection))
        }
    }
}

/// The path of a collection given relative to the server URL, or as an
/// absolute path.
fn path(base: &Url, collection: &str) -> String {
    if collection.starts_with('/') {
        return collection.to_string();
    }

    under(base, collection)
}

/// The path of `id` under `base`, the way io-webdav composes one under a
/// home set.
fn under(base: &Url, id: &str) -> String {
    let base = base.path().trim_end_matches('/');
    let id = id.trim_matches('/');

    format!("{base}/{id}")
}

/// Discovers the home set of the account: the principal first (RFC
/// 5397), then the calendar or addressbook home set it names.
///
/// `None` is a server that answered without naming one.
fn home_set(
    client: &mut WebdavClientStd,
    home: DavHome,
    shutdown: &Arc<AtomicBool>,
) -> Result<Option<Url>> {
    let mut principal =
        WebdavCurrentUserPrincipal::new(&client.base_url, client.auth(), &client.user_agent);
    let Some(principal) = pump_redirect(client, &mut principal, shutdown)? else {
        return Ok(None);
    };

    let path = principal.path().to_string();
    trace!("dav principal: {path}");

    match home {
        DavHome::Calendars => {
            let mut coroutine = CaldavCalendarHomeSet::new(
                &client.base_url,
                client.auth(),
                &client.user_agent,
                &path,
            );
            pump_redirect(client, &mut coroutine, shutdown)
        }
        DavHome::Addressbooks => {
            let mut coroutine = CarddavAddressbookHomeSet::new(
                &client.base_url,
                client.auth(),
                &client.user_agent,
                &path,
            );
            pump_redirect(client, &mut coroutine, shutdown)
        }
    }
}

/// Pumps one discovery coroutine over the client's stream, checking the
/// shutdown flag between reads.
///
/// A redirect is not followed: the stream is one connection, so the
/// server it points at is what `server` should name instead.
fn pump_redirect<C>(
    client: &mut WebdavClientStd,
    coroutine: &mut C,
    shutdown: &Arc<AtomicBool>,
) -> Result<Option<Url>>
where
    C: WebdavCoroutine<
            Yield = WebdavRedirectYield,
            Return = Result<Option<Url>, WebdavFollowRedirectsError>,
        >,
{
    let mut buf = [0u8; READ_BUF];
    let mut arg: Option<Vec<u8>> = None;

    loop {
        match coroutine.resume(arg.take().as_deref()) {
            WebdavCoroutineState::Yielded(WebdavRedirectYield::WantsRead) => loop {
                if shutdown.load(Ordering::SeqCst) {
                    bail!("Shutting down");
                }

                match client.stream.read(&mut buf) {
                    Ok(0) => bail!("Connection closed by peer"),
                    Ok(read) => {
                        arg = Some(buf[..read].to_vec());
                        break;
                    }
                    Err(err) if is_timeout(&err) => continue,
                    Err(err) => return Err(err).context("read failed"),
                }
            },
            WebdavCoroutineState::Yielded(WebdavRedirectYield::WantsWrite(bytes)) => {
                client.stream.write_all(&bytes).context("write failed")?;
            }
            WebdavCoroutineState::Yielded(WebdavRedirectYield::WantsRedirect { url, .. }) => {
                bail!("DAV server redirected discovery to {url}");
            }
            WebdavCoroutineState::Complete(Ok(url)) => return Ok(url),
            WebdavCoroutineState::Complete(Err(err)) => return Err(err.into()),
        }
    }
}

/// Whether the failure is the server refusing `sync-collection` itself
/// (RFC 3253 §3.6), which asks for a listing rather than a retry.
fn is_unsupported_report(err: &Error) -> bool {
    err.chain().any(|cause| {
        matches!(
            cause.downcast_ref::<WebdavSyncCollectionError>(),
            Some(WebdavSyncCollectionError::UnsupportedReport)
                | Some(WebdavSyncCollectionError::Send(
                    WebdavSendError::UnsupportedReport { .. }
                ))
        )
    })
}

#[cfg(test)]
mod tests {
    use crate::dav::*;

    #[test]
    fn a_caldav_content_type_names_its_component() {
        assert_eq!(
            Some(WatchDomain::Event),
            component(Some("text/calendar; charset=utf-8; component=vevent"))
        );
        assert_eq!(
            Some(WatchDomain::Task),
            component(Some("text/calendar; component=\"VTODO\""))
        );
    }

    #[test]
    fn a_content_type_with_no_component_names_nothing() {
        assert_eq!(None, component(Some("text/calendar; charset=utf-8")));
        assert_eq!(None, component(Some("text/vcard")));
        assert_eq!(None, component(None));
        // NOTE: a component carillon has no hook for reads the same as
        // no component at all.
        assert_eq!(None, component(Some("text/calendar; component=VJOURNAL")));
    }

    fn change(href: &str, etag: &str) -> WebdavSyncChange {
        WebdavSyncChange {
            href: href.to_string(),
            etag: Some(etag.to_string()),
        }
    }

    fn snapshot(changes: Vec<WebdavSyncChange>, complete: bool) -> Snapshot {
        Snapshot { changes, complete }
    }

    #[test]
    fn a_full_enumeration_reports_what_differs() {
        let domains = Domains::Fixed(WatchDomain::Event);
        let types = BTreeMap::new();
        let mut known = Known::new();

        let events = rebase(
            &mut known,
            &domains,
            &types,
            snapshot(vec![change("/c/a.ics", "1"), change("/c/b.ics", "1")], true),
        );
        assert_eq!(2, events.len());

        let events = rebase(
            &mut known,
            &domains,
            &types,
            snapshot(vec![change("/c/a.ics", "2"), change("/c/c.ics", "1")], true),
        );
        assert_eq!(
            vec![
                WatchEvent::ItemChanged {
                    domain: WatchDomain::Event,
                    id: String::from("/c/a.ics"),
                },
                WatchEvent::ItemAdded {
                    domain: WatchDomain::Event,
                    id: String::from("/c/c.ics"),
                },
                WatchEvent::ItemRemoved {
                    domain: WatchDomain::Event,
                    id: String::from("/c/b.ics"),
                },
            ],
            events
        );
    }

    /// A truncated listing says nothing of what it left out.
    #[test]
    fn a_truncated_enumeration_reports_no_removal() {
        let domains = Domains::Fixed(WatchDomain::Card);
        let types = BTreeMap::new();
        let mut known = Known::new();

        rebase(
            &mut known,
            &domains,
            &types,
            snapshot(vec![change("/b/a.vcf", "1"), change("/b/b.vcf", "1")], true),
        );
        let events = rebase(
            &mut known,
            &domains,
            &types,
            snapshot(vec![change("/b/a.vcf", "1")], false),
        );

        assert!(events.is_empty(), "got {events:?}");
        assert_eq!(2, known.len());
    }

    #[test]
    fn a_collection_path_is_read_under_its_base() {
        let base = Url::parse("https://dav.example.org/dav/calendars/alice/").unwrap();

        assert_eq!("/dav/calendars/alice/work", under(&base, "work"));
        assert_eq!("/dav/calendars/alice/work", under(&base, "/work/"));
        assert_eq!("/elsewhere/", path(&base, "/elsewhere/"));
        assert_eq!("/dav/calendars/alice/a/b", path(&base, "a/b"));
    }

    /// A DAV server answering discovery, holding one calendar at
    /// `/cal/alice/work` and implementing no `sync-collection`.
    mod fake {
        use std::{
            io::{BufRead, BufReader, Read, Write},
            net::{TcpListener, TcpStream},
            sync::{Arc, Mutex},
            thread,
        };

        /// The members the calendar lists, as href and etag.
        pub type Members = Arc<Mutex<Vec<(String, String)>>>;
        /// The requests the server answered, as method and path.
        pub type Seen = Arc<Mutex<Vec<String>>>;

        /// Starts the server, returning its address.
        pub fn start(members: Members, seen: Seen) -> String {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap().to_string();

            thread::spawn(move || {
                for stream in listener.incoming() {
                    let Ok(stream) = stream else { return };
                    let (members, seen) = (members.clone(), seen.clone());
                    thread::spawn(move || serve(stream, members, seen));
                }
            });

            addr
        }

        fn serve(stream: TcpStream, members: Members, seen: Seen) {
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut stream = stream;

            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 {
                    return;
                }
                let mut parts = line.split_whitespace();
                let method = parts.next().unwrap_or_default().to_string();
                let path = parts.next().unwrap_or_default().to_string();

                let mut length = 0;
                loop {
                    let mut header = String::new();
                    reader.read_line(&mut header).unwrap();
                    let header = header.trim_end();
                    if header.is_empty() {
                        break;
                    }
                    if let Some((name, value)) = header.split_once(':')
                        && name.eq_ignore_ascii_case("content-length")
                    {
                        length = value.trim().parse().unwrap();
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                let body = String::from_utf8_lossy(&body);

                seen.lock().unwrap().push(format!("{method} {path}"));

                let (status, xml) = answer(&method, &path, &body, &members);
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/xml; charset=utf-8\r\nContent-Length: {}\r\n\r\n{xml}",
                    xml.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
            }
        }

        fn answer(
            method: &str,
            path: &str,
            body: &str,
            members: &Members,
        ) -> (&'static str, String) {
            const OPEN: &str = r#"<?xml version="1.0" encoding="utf-8"?><d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">"#;
            const CLOSE: &str = "</d:multistatus>";

            let one = |href: &str, prop: &str| {
                format!(
                    "{OPEN}<d:response><d:href>{href}</d:href><d:propstat><d:prop>{prop}</d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>{CLOSE}"
                )
            };

            match method {
                "REPORT" => ("501 Not Implemented", String::new()),
                "PROPFIND" if body.contains("current-user-principal") => (
                    "207 Multi-Status",
                    one(
                        path,
                        "<d:current-user-principal><d:href>/principals/alice/</d:href></d:current-user-principal>",
                    ),
                ),
                "PROPFIND" if body.contains("calendar-home-set") => (
                    "207 Multi-Status",
                    one(
                        path,
                        "<c:calendar-home-set><d:href>/cal/alice/</d:href></c:calendar-home-set>",
                    ),
                ),
                "PROPFIND" if body.contains("supported-calendar-component-set") => (
                    "207 Multi-Status",
                    one(
                        path,
                        r#"<c:supported-calendar-component-set><c:comp name="VEVENT"/></c:supported-calendar-component-set>"#,
                    ),
                ),
                "PROPFIND" if path.trim_end_matches('/') == "/cal/alice/work" => {
                    let mut xml = String::from(OPEN);
                    xml.push_str("<d:response><d:href>/cal/alice/work/</d:href><d:propstat><d:prop><d:getetag/></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>");
                    for (href, etag) in members.lock().unwrap().iter() {
                        xml.push_str(&format!(
                            "<d:response><d:href>{href}</d:href><d:propstat><d:prop><d:getetag>\"{etag}\"</d:getetag></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>"
                        ));
                    }
                    xml.push_str(CLOSE);
                    ("207 Multi-Status", xml)
                }
                _ => ("404 Not Found", String::new()),
            }
        }
    }

    fn client(addr: &str) -> WebdavClientStd {
        let stream = std::net::TcpStream::connect(addr).unwrap();
        stream.set_read_timeout(Some(READ_TIMEOUT)).unwrap();
        let url = Url::parse(&format!("http://{addr}/")).unwrap();

        WebdavClientStd::new(stream, WebdavAuth::None, url)
    }

    /// The id a listing of the account gives is what a configuration
    /// names, and a server with no `sync-collection` is listed instead.
    #[test]
    fn a_calendar_named_by_its_id_is_watched_by_listing() {
        let members: fake::Members = Default::default();
        let seen: fake::Seen = Default::default();
        members
            .lock()
            .unwrap()
            .push((String::from("/cal/alice/work/a.ics"), String::from("1")));
        let addr = fake::start(members.clone(), seen.clone());
        let shutdown = Arc::new(AtomicBool::new(false));
        let mut client = client(&addr);

        let collection = locate(&mut client, DavHome::Calendars, "work", &shutdown).unwrap();
        assert_eq!("/cal/alice/work", collection);

        let kind = DavKind::Calendar(vec![WatchDomain::Event]);
        let domains = Domains::resolve(&mut client, &collection, kind, &shutdown).unwrap();
        let mut watched = Watched::arm(&mut client, collection, domains, &shutdown).unwrap();
        assert!(watched.listing);
        assert_eq!(1, watched.known.len());

        {
            let mut members = members.lock().unwrap();
            members[0].1 = String::from("2");
            members.push((String::from("/cal/alice/work/b.ics"), String::from("1")));
        }

        let mut events = Vec::new();
        watched
            .round(&mut client, &shutdown, &mut |event, _| events.push(event))
            .unwrap();
        assert_eq!(
            vec![
                WatchEvent::ItemChanged {
                    domain: WatchDomain::Event,
                    id: String::from("/cal/alice/work/a.ics"),
                },
                WatchEvent::ItemAdded {
                    domain: WatchDomain::Event,
                    id: String::from("/cal/alice/work/b.ics"),
                },
            ],
            events
        );

        members.lock().unwrap().remove(0);
        let mut events = Vec::new();
        watched
            .round(&mut client, &shutdown, &mut |event, _| events.push(event))
            .unwrap();
        assert_eq!(
            vec![WatchEvent::ItemRemoved {
                domain: WatchDomain::Event,
                id: String::from("/cal/alice/work/a.ics"),
            }],
            events
        );

        let seen = seen.lock().unwrap();
        assert!(
            seen.contains(&String::from("PROPFIND /principals/alice/")),
            "{seen:?}"
        );
    }
}
