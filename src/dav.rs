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
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use anyhow::{Context, Error, Result, anyhow, bail};
use io_http::{rfc6750::bearer::HttpAuthBearer, rfc7617::basic::HttpAuthBasic};
use io_webdav::{
    client::WebdavClientStd,
    coroutine::{WebdavCoroutine, WebdavCoroutineState, WebdavYield},
    rfc4791::calendar::SUPPORTED_CALENDAR_COMPONENT_SET,
    rfc4918::{
        DAV, GETETAG, WebdavAuth, WebdavMultistatus, WebdavProperty, propfind::WebdavPropfind,
    },
    rfc6578::sync_collection::{WebdavSyncCollection, WebdavSyncCollectionError, WebdavSyncDelta},
};
use log::{debug, trace, warn};
use pimalaya_stream::{
    retry::Retry,
    stream::{Stream, TcpConnectOptions, TlsConnectOptions},
    tls::Tls,
};
use secrecy::ExposeSecret;
use url::Url;

use crate::{
    config::{DavAuthConfig, DavServer},
    event::{ItemSummary, WatchDomain, WatchEvent},
};

/// How long the watch waits between two reports, unless the config
/// says otherwise.
const POLL_INTERVAL: Duration = Duration::from_secs(60);
/// How long a poll may sit in a read before looking at the shutdown flag.
const READ_TIMEOUT: Duration = Duration::from_secs(1);
/// How long the watch sleeps at a time, so a shutdown is noticed promptly.
const POLL_STEP: Duration = Duration::from_millis(200);
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

/// Opens a connection to the configured server.
///
/// The stream carries a read deadline and hands back the failures that
/// only mean "not ready yet", so a poll against a server that stopped
/// answering ends at the next deadline rather than holding the thread.
pub fn open(config: DavServer<'_>) -> Result<WebdavClientStd> {
    let url = Url::parse(config.server)
        .with_context(|| format!("Invalid DAV server URL `{}`", config.server))?;
    let host = url
        .host_str()
        .ok_or_else(|| anyhow!("DAV server URL `{url}` has no host"))?
        .to_string();

    let mut tls: Tls = config.tls.clone().into();
    tls.rustls.alpn = vec![String::from("http/1.1")];

    let stream = match url.scheme() {
        "http" => {
            let port = url.port().unwrap_or(80);
            let opts = TcpConnectOptions {
                retry: Retry::Never,
                ..Default::default()
            };
            Stream::connect_tcp(&host, port, opts)?
        }
        "https" => {
            let port = url.port().unwrap_or(443);
            let opts = TlsConnectOptions {
                tls,
                retry: Retry::Never,
                ..Default::default()
            };
            Stream::connect_tls(&host, port, opts)?
        }
        scheme => bail!("Unsupported DAV scheme `{scheme}`, expected http or https"),
    };

    stream.set_read_timeout(Some(READ_TIMEOUT))?;

    debug!("opened dav connection");
    trace!("server: {url}");

    let auth = auth(config.auth)?;

    Ok(WebdavClientStd::new(stream, auth, url))
}

/// Watches the configured collection until `shutdown` is set, calling
/// `on_event` for every change.
///
/// The first report runs with no token: it enumerates the collection and
/// is the baseline, so it reports nothing. Everything after is read
/// against that picture.
pub fn watch(
    config: DavServer<'_>,
    kind: DavKind,
    collection: &str,
    interval: Option<Duration>,
    shutdown: &Arc<AtomicBool>,
    mut on_event: impl FnMut(WatchEvent, Option<ItemSummary>),
) -> Result<()> {
    let mut client = open(config)?;
    let collection = path(&client, collection);
    let interval = interval.unwrap_or(POLL_INTERVAL);
    let mut domains = Domains::resolve(&mut client, &collection, kind, shutdown)?;

    let (mut known, mut token) = baseline(&mut client, &collection, &mut domains, shutdown)?;
    debug!("watching dav collection with {} members", known.len());

    while !shutdown.load(Ordering::SeqCst) {
        if !sleep(interval, shutdown) {
            break;
        }

        // NOTE: a truncated report means the server stopped early and the
        // rest waits behind the token it just handed back, so it is
        // drained now rather than at the next interval.
        loop {
            let delta = match sync(&mut client, &collection, token.as_deref(), shutdown) {
                Ok(delta) => delta,
                // NOTE: a rejected token means the server's history no
                // longer reaches that far, so enumerating again is the
                // only answer, and a re-baseline is not news.
                Err(err) if is_invalid_token(&err) => {
                    warn!("dav sync token rejected, re-enumerating the collection");
                    let (fresh, next) = baseline(&mut client, &collection, &mut domains, shutdown)?;
                    known = fresh;
                    token = next;
                    break;
                }
                Err(err) => return Err(err),
            };

            let truncated = delta.truncated;

            for event in reconcile(
                &mut client,
                &mut known,
                &domains,
                delta,
                &mut token,
                shutdown,
            ) {
                on_event(event, None);
            }

            if !truncated || shutdown.load(Ordering::SeqCst) {
                break;
            }
        }
    }

    Ok(())
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
    /// recognised by the `component` parameter of its content type.
    Mixed,
}

impl Domains {
    /// Asks the collection what it holds, which only a calendar has to be
    /// asked.
    ///
    /// A calendar advertising one component answers for every member at
    /// once. Hooks naming a component it does not hold are refused here,
    /// a hook that could never fire being a configuration error.
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
                Ok(Self::Mixed)
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
            Self::Mixed => match content_type(client, href, shutdown) {
                Ok(content_type) => match component(content_type.as_deref()) {
                    Some(domain) => Some(domain),
                    None => {
                        warn!("dav member `{href}` names no component, skipping");
                        None
                    }
                },
                Err(err) => {
                    warn!("cannot read the content type of dav member `{href}`: {err:#}");
                    None
                }
            },
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

/// Enumerates the collection, so a later report has something to be a
/// difference against.
///
/// A mixed calendar reads every member's content type in one PROPFIND
/// rather than one request per member, so holding two kinds of thing is
/// paid for once, at startup.
fn baseline(
    client: &mut WebdavClientStd,
    collection: &str,
    domains: &mut Domains,
    shutdown: &Arc<AtomicBool>,
) -> Result<(Known, Option<String>)> {
    let delta = sync(client, collection, None, shutdown)?;

    let members = match domains {
        Domains::Fixed(_) => BTreeMap::new(),
        Domains::Mixed => content_types(client, collection, shutdown)?,
    };

    let known = delta
        .changed
        .into_iter()
        .filter_map(|change| {
            let domain = match domains {
                Domains::Fixed(domain) => *domain,
                Domains::Mixed => component(members.get(&change.href).map(String::as_str))?,
            };

            Some((change.href, (change.etag, domain)))
        })
        .collect();

    Ok((known, delta.sync_token))
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
    );

    let delta = pump(client, &mut coroutine, shutdown)?;
    trace!("dav sync delta: {delta:?}");

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

/// Builds the credential presented on every request.
pub fn auth(config: &DavAuthConfig) -> Result<WebdavAuth> {
    Ok(match config {
        DavAuthConfig::None => WebdavAuth::None,
        DavAuthConfig::Basic { username, password } => WebdavAuth::Basic(HttpAuthBasic {
            username: username.clone(),
            password: password.clone().get()?,
        }),
        DavAuthConfig::Bearer { token } => {
            let token = token.clone().get()?;
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

/// Opens the collection and runs one report, which is what `check` needs.
///
/// It proves the transport, the credential, and that the collection is
/// where the configuration says.
pub fn probe(config: DavServer<'_>, collection: &str, shutdown: &Arc<AtomicBool>) -> Result<()> {
    let mut client = open(config)?;
    let collection = path(&client, collection);
    sync(&mut client, &collection, None, shutdown)?;

    Ok(())
}

/// Resolves the account's collection into the request path.
///
/// An absolute path is taken as it stands, anything else read under the
/// server URL's own path: `server` names the DAV root, the collection
/// what to watch under it.
fn path(client: &WebdavClientStd, collection: &str) -> String {
    if collection.starts_with('/') {
        return collection.to_string();
    }

    let base = client.base_url.path().trim_end_matches('/');

    format!("{base}/{}", collection.trim_start_matches('/'))
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
}
