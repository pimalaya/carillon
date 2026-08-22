//! Per-account supervisor: opens a watch, dispatches what it reports
//! to the hooks, and reopens it when the connection drops.
//!
//! One account is one thread, holding everything that can fail per
//! account: backend selection, reconnect backoff, envelope
//! resolution. A failure ends the session, never the process, so one
//! unreachable server cannot stop the other accounts watching.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::{Result, bail};
use log::{debug, info, warn};

#[cfg(feature = "dav")]
use crate::config::DavWatchConfig;
#[cfg(feature = "maildir")]
use crate::config::MaildirWatchConfig;
#[cfg(feature = "imap")]
use crate::config::{IdleWatchConfig, ImapWatchConfig};
#[cfg(feature = "jmap")]
use crate::config::{JmapWatchConfig, PushWatchConfig};
use crate::{backend::Backend, config::AccountConfig, event::WatchEvent, hook};

/// Reconnect backoff floor.
const INITIAL_BACKOFF: Duration = Duration::from_secs(2);
/// Reconnect backoff ceiling.
const MAX_BACKOFF: Duration = Duration::from_secs(300);
/// A session that lived at least this long resets the backoff.
const HEALTHY_THRESHOLD: Duration = Duration::from_secs(60);
/// Backoff sleep granularity, so a shutdown is noticed promptly.
const BACKOFF_STEP: Duration = Duration::from_millis(200);

/// Watches one account until `shutdown` is set, reopening the watch
/// with a capped backoff whenever a session ends.
pub fn run(
    account: &str,
    config: AccountConfig,
    backend: Backend,
    shutdown: Arc<AtomicBool>,
) -> Result<()> {
    let collection = config.collection.clone();
    let mut backoff = INITIAL_BACKOFF;

    while !shutdown.load(Ordering::SeqCst) {
        let started = Instant::now();

        let outcome = watch_once(account, &config, &collection, backend, &shutdown);

        // NOTE: neither an asked-for ending nor a failure raced on the
        // way out is news, and nothing is reopened either way.
        if shutdown.load(Ordering::SeqCst) {
            if let Err(err) = outcome {
                debug!("[{account}] session ended while shutting down: {err:#}");
            }

            break;
        }

        match outcome {
            Ok(()) => debug!("[{account}] session ended, reopening"),
            Err(err) => warn!("[{account}] session lost: {err:#}"),
        }

        // NOTE: a session that stayed up is evidence the server is
        // healthy, so the next failure starts from the floor rather
        // than inheriting an old outage's backoff.
        if started.elapsed() >= HEALTHY_THRESHOLD {
            backoff = INITIAL_BACKOFF;
        }

        if !sleep_backoff(&mut backoff, &shutdown) {
            break;
        }
    }

    Ok(())
}

/// Runs one watch session against the account's active backend, with
/// the method that backend was asked for.
///
/// Which methods exist, and which events have hooks, are both the
/// backend's, declared in its own config, so anything it cannot do was
/// already refused when the file was read: nothing here can be asked
/// for the impossible.
fn watch_once(
    account: &str,
    config: &AccountConfig,
    collection: &str,
    backend: Backend,
    shutdown: &Arc<AtomicBool>,
) -> Result<()> {
    #[cfg(feature = "imap")]
    if backend.allows_imap() {
        if let Some(imap) = &config.imap {
            let mut resolver = crate::imap::Resolver::new(imap, collection, shutdown);
            let mut on_event = |event: WatchEvent| {
                let Some(hook) = imap.hook.get(&event) else {
                    return;
                };

                let summary = resolve_added(account, &event, &mut resolver);
                hook::run(hook, &event, collection, summary.as_ref());
            };

            return match &imap.watch {
                Some(ImapWatchConfig::Poll(poll)) => {
                    info!("[{account}] watching `{collection}` over imap, polling");
                    crate::imap::watch_poll(
                        imap,
                        collection,
                        poll.interval(),
                        shutdown,
                        &mut on_event,
                    )
                }
                watch => {
                    let idle = match watch {
                        Some(ImapWatchConfig::Idle(idle)) => idle.clone(),
                        _ => IdleWatchConfig::default(),
                    };

                    info!("[{account}] watching `{collection}` over imap, idling");
                    crate::imap::watch_idle(
                        imap,
                        collection,
                        idle.timeout(),
                        shutdown,
                        &mut on_event,
                    )
                }
            };
        }

        if backend == Backend::Imap {
            bail!("account has no `imap` config block");
        }
    }

    #[cfg(feature = "jmap")]
    if backend.allows_jmap() {
        if let Some(jmap) = &config.jmap {
            let mut on_event = |event: WatchEvent| {
                if let Some(hook) = jmap.hook.get(&event) {
                    hook::run(hook, &event, collection, None);
                }
            };

            return match &jmap.watch {
                Some(JmapWatchConfig::Poll(poll)) => {
                    info!("[{account}] watching `{collection}` over jmap, polling");
                    crate::jmap::watch_poll(
                        jmap,
                        collection,
                        poll.interval(),
                        shutdown,
                        &mut on_event,
                    )
                }
                watch => {
                    let ping = match watch {
                        Some(JmapWatchConfig::Push(push)) => push.ping,
                        _ => PushWatchConfig::default().ping,
                    };
                    info!("[{account}] watching `{collection}` over jmap, pushed");
                    crate::jmap::watch_push(jmap, collection, ping, shutdown, &mut on_event)
                }
            };
        }

        if backend == Backend::Jmap {
            bail!("account has no `jmap` config block");
        }
    }

    #[cfg(feature = "maildir")]
    if backend.allows_maildir() {
        if let Some(maildir) = &config.maildir {
            let MaildirWatchConfig::Poll(poll) = maildir
                .watch
                .clone()
                .unwrap_or(MaildirWatchConfig::Poll(Default::default()));

            info!("[{account}] watching `{collection}` over maildir, polling");
            let mut on_event = |event: WatchEvent| {
                if let Some(hook) = maildir.hook.get(&event) {
                    hook::run(hook, &event, collection, None);
                }
            };
            return crate::maildir::watch(
                maildir,
                collection,
                poll.interval(),
                shutdown,
                &mut on_event,
            );
        }

        if backend == Backend::Maildir {
            bail!("account has no `maildir` config block");
        }
    }

    #[cfg(feature = "dav")]
    if backend.allows_caldav() {
        if let Some(caldav) = &config.caldav {
            info!("[{account}] watching `{collection}` over caldav, polling");
            let mut on_event = |event: WatchEvent| {
                if let Some(hook) = caldav.hook.get(&event) {
                    hook::run(hook, &event, collection, None);
                }
            };
            return crate::dav::watch(
                caldav.server(),
                crate::dav::DavKind::Calendar(caldav.hook.domains()),
                collection,
                dav_interval(&caldav.watch),
                shutdown,
                &mut on_event,
            );
        }

        if backend == Backend::Caldav {
            bail!("account has no `caldav` config block");
        }
    }

    #[cfg(feature = "dav")]
    if backend.allows_carddav() {
        if let Some(carddav) = &config.carddav {
            info!("[{account}] watching `{collection}` over carddav, polling");
            let mut on_event = |event: WatchEvent| {
                if let Some(hook) = carddav.hook.get(&event) {
                    hook::run(hook, &event, collection, None);
                }
            };
            return crate::dav::watch(
                carddav.server(),
                crate::dav::DavKind::Addressbook,
                collection,
                dav_interval(&carddav.watch),
                shutdown,
                &mut on_event,
            );
        }

        if backend == Backend::Carddav {
            bail!("account has no `carddav` config block");
        }
    }

    #[cfg(feature = "dav")]
    if backend.allows_dav() {
        if let Some(dav) = &config.dav {
            info!("[{account}] watching `{collection}` over dav, polling");
            let mut on_event = |event: WatchEvent| {
                if let Some(hook) = dav.hook.get(&event) {
                    hook::run(hook, &event, collection, None);
                }
            };
            return crate::dav::watch(
                dav.server(),
                crate::dav::DavKind::Plain,
                collection,
                dav_interval(&dav.watch),
                shutdown,
                &mut on_event,
            );
        }

        if backend == Backend::Dav {
            bail!("account has no `dav` config block");
        }
    }

    bail!(
        "account has no usable backend block (expected one of `imap`, `jmap`, `maildir`, \
         `caldav`, `carddav`, `dav`); use `-b/--backend` to pin a specific one"
    )
}

/// The poll interval a DAV backend was configured with, WebDAV having
/// the one method whatever domain it holds.
#[cfg(feature = "dav")]
fn dav_interval(watch: &Option<DavWatchConfig>) -> Option<Duration> {
    let DavWatchConfig::Poll(poll) = watch
        .clone()
        .unwrap_or(DavWatchConfig::Poll(Default::default()));

    poll.interval()
}

/// Resolves an arrival, the caller having already found the hook that
/// will consume one. Anything else costs nothing.
#[cfg(feature = "imap")]
fn resolve_added(
    account: &str,
    event: &WatchEvent,
    resolver: &mut crate::imap::Resolver<'_>,
) -> Option<crate::event::ItemSummary> {
    let WatchEvent::ItemAdded { id, .. } = event else {
        return None;
    };

    match resolver.summary(id) {
        Ok(summary) => Some(summary),
        Err(err) => {
            warn!("[{account}] cannot resolve item `{id}`: {err:#}");
            None
        }
    }
}

/// Sleeps the current backoff in small steps so a shutdown is noticed
/// promptly, then doubles it toward the ceiling. Returns false when a
/// shutdown was requested.
fn sleep_backoff(backoff: &mut Duration, shutdown: &Arc<AtomicBool>) -> bool {
    let mut left = *backoff;

    while left > Duration::ZERO {
        if shutdown.load(Ordering::SeqCst) {
            return false;
        }

        let step = left.min(BACKOFF_STEP);
        thread::sleep(step);
        left -= step;
    }

    *backoff = (*backoff * 2).min(MAX_BACKOFF);

    !shutdown.load(Ordering::SeqCst)
}
