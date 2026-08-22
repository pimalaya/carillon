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

use crate::{backend::Backend, config::AccountConfig, event::WatchEvent, hook};
#[cfg(feature = "dav")]
use crate::{
    config::DavWatchConfig,
    dav::{self, DavKind},
};
#[cfg(feature = "maildir")]
use crate::{config::MaildirWatchConfig, maildir};
#[cfg(feature = "imap")]
use crate::{
    config::{IdleWatchConfig, ImapWatchConfig},
    event::ItemSummary,
    imap::{self, Resolver},
};
#[cfg(feature = "jmap")]
use crate::{
    config::{JmapWatchConfig, PushWatchConfig},
    jmap,
};

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
    let mut backoff = INITIAL_BACKOFF;

    while !shutdown.load(Ordering::SeqCst) {
        let started = Instant::now();

        let outcome = watch_once(account, &config, backend, &shutdown);

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
    backend: Backend,
    shutdown: &Arc<AtomicBool>,
) -> Result<()> {
    #[cfg(feature = "imap")]
    if backend.allows_imap() {
        if let Some(imap_config) = &config.imap {
            let collection = imap_config.collection();
            let mut resolver = Resolver::new(imap_config, collection.value, shutdown);
            let mut on_event = |event: WatchEvent| {
                let Some(hook) = imap_config.hook.get(&event) else {
                    return;
                };

                let summary = resolve_added(account, &event, &mut resolver);
                hook::run(hook, &event, imap_config.collection(), summary.as_ref());
            };

            return match &imap_config.watch {
                Some(ImapWatchConfig::Poll(poll)) => {
                    info!(
                        "[{account}] watching `{}` over imap, polling",
                        collection.value
                    );
                    imap::watch_poll(
                        imap_config,
                        collection.value,
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

                    info!(
                        "[{account}] watching `{}` over imap, idling",
                        collection.value
                    );
                    imap::watch_idle(
                        imap_config,
                        collection.value,
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
        if let Some(jmap_config) = &config.jmap {
            let collection = jmap_config.collection();
            let mut on_event = |event: WatchEvent| {
                if let Some(hook) = jmap_config.hook.get(&event) {
                    hook::run(hook, &event, jmap_config.collection(), None);
                }
            };

            return match &jmap_config.watch {
                Some(JmapWatchConfig::Poll(poll)) => {
                    info!(
                        "[{account}] watching `{}` over jmap, polling",
                        collection.value
                    );
                    jmap::watch_poll(
                        jmap_config,
                        collection.value,
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
                    info!(
                        "[{account}] watching `{}` over jmap, pushed",
                        collection.value
                    );
                    jmap::watch_push(jmap_config, collection.value, ping, shutdown, &mut on_event)
                }
            };
        }

        if backend == Backend::Jmap {
            bail!("account has no `jmap` config block");
        }
    }

    #[cfg(feature = "maildir")]
    if backend.allows_maildir() {
        if let Some(maildir_config) = &config.maildir {
            let collection = maildir_config.collection();
            let MaildirWatchConfig::Poll(poll) = maildir_config
                .watch
                .clone()
                .unwrap_or(MaildirWatchConfig::Poll(Default::default()));

            info!(
                "[{account}] watching `{}` over maildir, polling",
                collection.value
            );
            let mut on_event = |event: WatchEvent| {
                if let Some(hook) = maildir_config.hook.get(&event) {
                    hook::run(hook, &event, maildir_config.collection(), None);
                }
            };
            return maildir::watch(
                maildir_config,
                collection.value,
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
        if let Some(caldav_config) = &config.caldav {
            let collection = caldav_config.collection();
            info!(
                "[{account}] watching `{}` over caldav, polling",
                collection.value
            );
            let mut on_event = |event: WatchEvent| {
                if let Some(hook) = caldav_config.hook.get(&event) {
                    hook::run(hook, &event, caldav_config.collection(), None);
                }
            };
            return dav::watch(
                caldav_config.server(),
                DavKind::Calendar(caldav_config.hook.domains()),
                collection.value,
                dav_interval(&caldav_config.watch),
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
        if let Some(carddav_config) = &config.carddav {
            let collection = carddav_config.collection();
            info!(
                "[{account}] watching `{}` over carddav, polling",
                collection.value
            );
            let mut on_event = |event: WatchEvent| {
                if let Some(hook) = carddav_config.hook.get(&event) {
                    hook::run(hook, &event, carddav_config.collection(), None);
                }
            };
            return dav::watch(
                carddav_config.server(),
                DavKind::Addressbook,
                collection.value,
                dav_interval(&carddav_config.watch),
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
        if let Some(dav_config) = &config.dav {
            let collection = dav_config.collection();
            info!(
                "[{account}] watching `{}` over dav, polling",
                collection.value
            );
            let mut on_event = |event: WatchEvent| {
                if let Some(hook) = dav_config.hook.get(&event) {
                    hook::run(hook, &event, dav_config.collection(), None);
                }
            };
            return dav::watch(
                dav_config.server(),
                DavKind::Plain,
                collection.value,
                dav_interval(&dav_config.watch),
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
    resolver: &mut Resolver<'_>,
) -> Option<ItemSummary> {
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
