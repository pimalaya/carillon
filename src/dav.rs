//! WebDAV backend: a `sync-collection` poll over any DAV collection.
//!
//! CalDAV and CardDAV are WebDAV, so one backend watches a calendar, an
//! addressbook or a plain collection: RFC 6578 answers "what changed
//! since this token" with the members that moved and a fresh token,
//! which is the whole watch. Nothing out there implements a push a
//! client can subscribe to without a public endpoint, so the interval
//! is the state of the art rather than a shortcut.
//!
//! The report asks for `getetag` and nothing else, so a poll never
//! carries a vCard or a VEVENT: the watch says an item moved, and a
//! hook that wants its content goes and reads it.
//!
//! Because RFC 6578 reports created and updated members together, the
//! backend keeps an href to etag picture of the collection and reads
//! the difference: an href it has never seen is an arrival, a known one
//! whose etag moved is an edit. That edit is the event mail never
//! needed, a contact being mutable where a message is not.

use std::{
    collections::BTreeMap,
    io::{self, Read, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use io_http::{rfc6750::bearer::HttpAuthBearer, rfc7617::basic::HttpAuthBasic};
use io_webdav::{
    client::WebdavClientStd,
    coroutine::{WebdavCoroutine, WebdavCoroutineState, WebdavYield},
    rfc4918::{GETETAG, WebdavAuth},
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
    config::{DavAuthConfig, DavConfig},
    event::WatchEvent,
};

/// How long a poll may sit in a read before looking at the shutdown
/// flag again.
const READ_TIMEOUT: Duration = Duration::from_secs(1);
/// How long the watch sleeps at a time between polls, so a shutdown is
/// noticed promptly.
const POLL_STEP: Duration = Duration::from_millis(200);
/// Per-read scratch buffer.
const READ_BUF: usize = 8 * 1024;

/// Opens a connection to the configured collection.
///
/// The stream carries a read deadline and hands back the failures that
/// only mean "not ready yet", so a poll against a server that stopped
/// answering ends at the next deadline instead of holding the thread
/// for as long as the transport would.
pub fn open(config: &DavConfig) -> Result<(WebdavClientStd, String)> {
    let url = Url::parse(&config.server)
        .with_context(|| format!("invalid DAV collection URL `{}`", config.server))?;
    let host = url
        .host_str()
        .ok_or_else(|| anyhow!("DAV collection URL `{url}` has no host"))?
        .to_string();
    let collection = url.path().to_string();

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
        scheme => bail!("unsupported DAV scheme `{scheme}`, expected http or https"),
    };

    stream.set_read_timeout(Some(READ_TIMEOUT))?;

    debug!("opened dav connection");
    trace!("collection: {url}");

    let auth = auth(&config.auth)?;

    Ok((WebdavClientStd::new(stream, auth, url), collection))
}

/// Watches the configured collection until `shutdown` is set, calling
/// `on_event` for every change.
///
/// The first report runs without a token: it enumerates the collection
/// and is the baseline, so it reports nothing. Everything after it is
/// read against that picture.
pub fn watch(
    config: &DavConfig,
    shutdown: &Arc<AtomicBool>,
    mut on_event: impl FnMut(WatchEvent),
) -> Result<()> {
    let (mut client, collection) = open(config)?;
    let interval = Duration::from_secs(config.poll);

    let (mut known, mut token) = baseline(&mut client, &collection, shutdown)?;
    debug!("watching dav collection with {} members", known.len());

    while !shutdown.load(Ordering::SeqCst) {
        if !sleep(interval, shutdown) {
            break;
        }

        // NOTE: a truncated report means the server stopped early and
        // the rest is waiting behind the token it just handed back, so
        // it is drained now rather than at the next interval.
        loop {
            let delta = match sync(&mut client, &collection, token.as_deref(), shutdown) {
                Ok(delta) => delta,
                // NOTE: a rejected token is the server saying its
                // history no longer reaches back that far. The only
                // answer is to enumerate again, and a re-baseline is
                // not news, so nothing is reported from it.
                Err(err) if is_invalid_token(&err) => {
                    warn!("dav sync token rejected, re-enumerating the collection");
                    let (fresh, next) = baseline(&mut client, &collection, shutdown)?;
                    known = fresh;
                    token = next;
                    break;
                }
                Err(err) => return Err(err),
            };

            let truncated = delta.truncated;

            for event in reconcile(&mut known, delta, &mut token) {
                on_event(event);
            }

            if !truncated || shutdown.load(Ordering::SeqCst) {
                break;
            }
        }
    }

    Ok(())
}

/// What the watch knows of the collection: an href to its etag.
type Known = BTreeMap<String, Option<String>>;

/// Reads a delta against what the watch knows, and reports what moved.
fn reconcile(
    known: &mut Known,
    delta: WebdavSyncDelta,
    token: &mut Option<String>,
) -> Vec<WatchEvent> {
    let mut events = Vec::new();

    for href in delta.vanished {
        if known.remove(&href).is_some() {
            events.push(WatchEvent::ItemRemoved { id: href });
        }
    }

    for change in delta.changed {
        match known.insert(change.href.clone(), change.etag.clone()) {
            // NOTE: an href never seen before is an arrival.
            None => events.push(WatchEvent::ItemAdded { id: change.href }),
            // NOTE: RFC 6578 does not say whether a member was created
            // or updated, so a known href is an edit, and only when its
            // etag actually moved: a server is free to re-report a
            // member that did not change.
            Some(before) if before != change.etag => {
                events.push(WatchEvent::ItemChanged { id: change.href })
            }
            Some(_) => {}
        }
    }

    if delta.sync_token.is_some() {
        *token = delta.sync_token;
    }

    events
}

/// Enumerates the collection, so a later report has something to be a
/// difference against.
fn baseline(
    client: &mut WebdavClientStd,
    collection: &str,
    shutdown: &Arc<AtomicBool>,
) -> Result<(Known, Option<String>)> {
    let delta = sync(client, collection, None, shutdown)?;

    let known = delta
        .changed
        .into_iter()
        .map(|change| (change.href, change.etag))
        .collect();

    Ok((known, delta.sync_token))
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

    let mut buf = [0u8; READ_BUF];
    let mut arg: Option<Vec<u8>> = None;

    loop {
        match coroutine.resume(arg.take().as_deref()) {
            WebdavCoroutineState::Yielded(WebdavYield::WantsRead) => loop {
                if shutdown.load(Ordering::SeqCst) {
                    bail!("shutting down");
                }

                match client.stream.read(&mut buf) {
                    Ok(0) => bail!("connection closed by peer"),
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
            WebdavCoroutineState::Complete(Ok(delta)) => {
                trace!("dav sync delta: {delta:?}");
                return Ok(delta);
            }
            WebdavCoroutineState::Complete(Err(err)) => return Err(err.into()),
        }
    }
}

/// Builds the credential presented on every request.
fn auth(config: &DavAuthConfig) -> Result<WebdavAuth> {
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
fn is_invalid_token(err: &anyhow::Error) -> bool {
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

/// Opens the collection and runs one report, which is what `check`
/// needs: it proves the transport, the credential and that the
/// collection is where the config says.
pub fn probe(config: &DavConfig, shutdown: &Arc<AtomicBool>) -> Result<()> {
    let (mut client, collection) = open(config)?;
    sync(&mut client, &collection, None, shutdown)?;

    Ok(())
}
