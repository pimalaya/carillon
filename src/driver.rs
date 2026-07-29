//! The blocking driver for carillon-core's I/O-free watch.
//!
//! carillon-core does no I/O: its `CarillonImapWatch` coroutine returns what
//! to do next. This opens a TLS stream, pumps the coroutine over it
//! (performing each read and write it requests), and on each surfaced change
//! mints an event and fires the hook, reconnecting with capped backoff until
//! shutdown. This is the whole I/O side for the CLI, one thread per watch.
//!
//! The ring is content-free. When a watch has a notify hook, the driver
//! resolves the change locally through [`crate::resolve`] (its own
//! connection) so the notification can name the new message; a cmd-only watch
//! resolves nothing.

use std::{
    io::{Read, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow, bail};
use carillon_core::{
    backend::CarillonImapBackend,
    credential::CarillonCredential,
    event::{CarillonEvent, CarillonSource},
    imap::{CarillonImapWatch, CarillonImapWatchProgress as Progress},
};
use io_imap::types::mailbox::Mailbox;
use log::{debug, info, warn};

use crate::{
    config::{HookConfig, ImapConfig},
    hook::{self, HookContext},
    net,
    resolve::{self, CarillonImapMessage},
};

/// Reconnect backoff floor.
const INITIAL_BACKOFF: Duration = Duration::from_secs(2);
/// Reconnect backoff ceiling.
const MAX_BACKOFF: Duration = Duration::from_secs(300);
/// A connection that lived at least this long resets the backoff.
const HEALTHY_THRESHOLD: Duration = Duration::from_secs(60);
/// Read timeout, so a blocking IDLE read returns often enough to notice a
/// shutdown request.
const READ_TIMEOUT: Duration = Duration::from_secs(1);
/// Per-read scratch buffer. Only mailbox status is read, never a body.
const READ_BUF: usize = 8 * 1024;

/// Runs one account's watch until `shutdown` is set: connect, pump, then
/// reconnect with backoff.
pub fn run(
    account: &str,
    imap: ImapConfig,
    mailbox: String,
    hook: HookConfig,
    shutdown: Arc<AtomicBool>,
) -> Result<()> {
    let (host, port) = parse_server(&imap.server)?;
    let mut backoff = INITIAL_BACKOFF;

    while !shutdown.load(Ordering::SeqCst) {
        let started = Instant::now();
        match watch_once(account, &imap, &host, port, &mailbox, &hook, &shutdown) {
            Ok(()) => debug!("[{account}] session ended; reconnecting"),
            Err(err) => warn!("[{account}] session lost: {err:#}"),
        }
        if shutdown.load(Ordering::SeqCst) {
            break;
        }
        if started.elapsed() >= HEALTHY_THRESHOLD {
            backoff = INITIAL_BACKOFF;
        }
        if !sleep_backoff(&mut backoff, &shutdown) {
            break;
        }
    }
    Ok(())
}

/// Opens one connection and pumps the coroutine until it stops, the
/// connection drops, or shutdown is requested. The credential is resolved
/// fresh here so its residency is minimal.
fn watch_once(
    account: &str,
    imap: &ImapConfig,
    host: &str,
    port: u16,
    mailbox: &str,
    hook: &HookConfig,
    shutdown: &Arc<AtomicBool>,
) -> Result<()> {
    let credential = resolve_credential(imap)?;
    let mut stream = net::connect(host, port, Some(READ_TIMEOUT))?;

    let backend = CarillonImapBackend {
        host: host.to_string(),
        port,
        login: imap.login.clone(),
        mailbox: mailbox.to_string(),
    };
    let mut watch = CarillonImapWatch::new(backend, credential, shutdown.clone());
    info!("[{account}] watching {mailbox}");

    // Enrich the notify consumer only; a cmd-only watch resolves nothing.
    let enrich = hook.notify.is_some();
    let watched = Mailbox::try_from(mailbox.to_string())
        .map_err(|_| anyhow!("invalid mailbox name: {mailbox}"))?;
    // Watermark for the new-message range; resets per connection.
    let mut resolver_uid_next: Option<u32> = None;

    let mut buf = [0u8; READ_BUF];
    let mut pending: Option<usize> = None;
    loop {
        let input = pending.take().map(|n| &buf[..n]);
        match watch.resume(input) {
            Progress::WantsRead => loop {
                if shutdown.load(Ordering::SeqCst) {
                    return Ok(());
                }
                match stream.read(&mut buf) {
                    Ok(0) => bail!("connection closed by peer"),
                    Ok(n) => {
                        pending = Some(n);
                        break;
                    }
                    Err(err) if net::is_timeout(&err) => continue,
                    Err(err) => return Err(err).context("read failed"),
                }
            },
            Progress::WantsWrite(bytes) => {
                if shutdown.load(Ordering::SeqCst) {
                    return Ok(());
                }
                stream.write_all(&bytes).context("write failed")?;
            }
            Progress::Changed(state) => {
                let ring = CarillonEvent::new(
                    new_id(),
                    now_secs(),
                    account,
                    CarillonSource::Imap,
                    mailbox,
                    Some(state),
                );
                let new_messages = if enrich {
                    match resolve_new(imap, host, port, &watched, resolver_uid_next, shutdown) {
                        Ok((messages, uid_next)) => {
                            resolver_uid_next = uid_next.or(resolver_uid_next);
                            messages
                        }
                        Err(err) => {
                            warn!("[{account}] cannot resolve new messages: {err:#}");
                            Vec::new()
                        }
                    }
                } else {
                    Vec::new()
                };
                hook::run(
                    hook,
                    &HookContext {
                        account,
                        mailbox,
                        source: ring.source.as_str(),
                        state: ring.state.as_deref().unwrap_or_default(),
                    },
                    &new_messages,
                );
            }
            Progress::Done(result) => return Ok(result?),
        }
    }
}

/// Resolves the newly-arrived messages on a fresh connection, resolving the
/// credential again so its residency stays minimal.
fn resolve_new(
    imap: &ImapConfig,
    host: &str,
    port: u16,
    mailbox: &Mailbox<'static>,
    since_uid: Option<u32>,
    shutdown: &Arc<AtomicBool>,
) -> Result<(Vec<CarillonImapMessage>, Option<u32>)> {
    let credential = resolve_credential(imap)?;
    resolve::resolve_new_messages(
        &imap.login,
        host,
        port,
        &credential,
        mailbox,
        since_uid,
        shutdown,
    )
}

/// Resolves the account credential: a password (`LOGIN`) or a bearer token
/// (`OAUTHBEARER`). Fresh each connect.
fn resolve_credential(imap: &ImapConfig) -> Result<CarillonCredential> {
    if let Some(password) = &imap.password {
        return Ok(CarillonCredential::Password(password.clone().get()?));
    }
    if let Some(token) = &imap.token {
        return Ok(CarillonCredential::Bearer(token.clone().get()?));
    }
    bail!("account imap config has neither `password` nor `token`")
}

/// Splits `host` or `host:port`, defaulting to the IMAPS port.
fn parse_server(server: &str) -> Result<(String, u16)> {
    match server.rsplit_once(':') {
        Some((host, port)) => {
            let port = port
                .parse()
                .with_context(|| format!("invalid port in server `{server}`"))?;
            Ok((host.to_string(), port))
        }
        None => Ok((server.to_string(), 993)),
    }
}

/// Sleeps the current backoff in small steps so a shutdown is noticed
/// promptly, then doubles it toward the ceiling. Returns false if shutdown
/// was requested.
fn sleep_backoff(backoff: &mut Duration, shutdown: &Arc<AtomicBool>) -> bool {
    let step = Duration::from_millis(200);
    let mut left = *backoff;
    while left > Duration::ZERO {
        if shutdown.load(Ordering::SeqCst) {
            return false;
        }
        let chunk = left.min(step);
        thread::sleep(chunk);
        left -= chunk;
    }
    *backoff = (*backoff * 2).min(MAX_BACKOFF);
    !shutdown.load(Ordering::SeqCst)
}

/// A distinct event id. The CLI does not sign or dedup deliveries, so a
/// monotonic hex stamp suffices; the server mints a random id instead.
fn new_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    format!("{nanos:032x}")
}

/// The current Unix time in seconds, or 0 before the epoch.
fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}
