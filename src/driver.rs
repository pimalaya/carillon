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

use crate::{
    backend::Backend,
    config::{AccountConfig, HooksConfig, WatchConfig},
    event::WatchEvent,
    hook,
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
    let hooks = config.hooks.clone();
    let collection = config.collection.clone();
    let mut backoff = INITIAL_BACKOFF;

    while !shutdown.load(Ordering::SeqCst) {
        let started = Instant::now();

        let outcome = watch_once(account, &config, &collection, backend, &hooks, &shutdown);

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
/// A backend refuses a method it cannot honour rather than quietly
/// using one it can: a watch silently downgraded to a poll is how
/// someone ends up wondering why their mail arrives a minute late.
fn watch_once(
    account: &str,
    config: &AccountConfig,
    collection: &str,
    backend: Backend,
    hooks: &HooksConfig,
    shutdown: &Arc<AtomicBool>,
) -> Result<()> {
    let watch = config.watch.as_ref();
    let poll = watch.and_then(WatchConfig::poll_interval);

    #[cfg(feature = "imap")]
    if backend.allows_imap() {
        if let Some(imap) = &config.imap {
            let mut resolver = crate::imap::Resolver::new(imap, collection, shutdown);
            let mut on_event = |event: WatchEvent| {
                let summary = resolve_added(account, hooks, &event, &mut resolver);
                hook::run(hooks, &event, collection, summary.as_ref());
            };

            return match watch {
                None | Some(WatchConfig::Idle(_)) => {
                    info!("[{account}] watching `{collection}` over imap, idling");
                    crate::imap::watch_idle(imap, collection, shutdown, &mut on_event)
                }
                Some(WatchConfig::Poll(_)) => {
                    info!("[{account}] watching `{collection}` over imap, polling");
                    crate::imap::watch_poll(imap, collection, poll, shutdown, &mut on_event)
                }
                Some(other) => bail!(unsupported("imap", other, "idle or poll")),
            };
        }

        if backend == Backend::Imap {
            bail!("account has no `imap` config block");
        }
    }

    #[cfg(feature = "jmap")]
    if backend.allows_jmap() {
        if let Some(jmap) = &config.jmap {
            let mut on_event = |event: WatchEvent| hook::run(hooks, &event, collection, None);

            return match watch {
                None | Some(WatchConfig::Push(_)) => {
                    let ping = match watch {
                        Some(WatchConfig::Push(push)) => push.ping,
                        _ => crate::config::PushWatchConfig::default().ping,
                    };
                    info!("[{account}] watching `{collection}` over jmap, pushed");
                    crate::jmap::watch_push(jmap, collection, ping, shutdown, &mut on_event)
                }
                Some(WatchConfig::Poll(_)) => {
                    info!("[{account}] watching `{collection}` over jmap, polling");
                    crate::jmap::watch_poll(jmap, collection, poll, shutdown, &mut on_event)
                }
                Some(other) => bail!(unsupported("jmap", other, "push or poll")),
            };
        }

        if backend == Backend::Jmap {
            bail!("account has no `jmap` config block");
        }
    }

    #[cfg(feature = "maildir")]
    if backend.allows_maildir() {
        if let Some(maildir) = &config.maildir {
            if let Some(other) = watch.filter(|watch| watch.poll_interval().is_none()) {
                bail!(unsupported("maildir", other, "poll"));
            }

            info!("[{account}] watching `{collection}` over maildir, polling");
            let mut on_event = |event: WatchEvent| hook::run(hooks, &event, collection, None);
            return crate::maildir::watch(maildir, collection, poll, shutdown, &mut on_event);
        }

        if backend == Backend::Maildir {
            bail!("account has no `maildir` config block");
        }
    }

    #[cfg(feature = "dav")]
    if backend.allows_dav() {
        if let Some(dav) = &config.dav {
            if let Some(other) = watch.filter(|watch| watch.poll_interval().is_none()) {
                bail!(unsupported("dav", other, "poll"));
            }

            info!("[{account}] watching `{collection}` over dav, polling");
            let mut on_event = |event: WatchEvent| hook::run(hooks, &event, collection, None);
            return crate::dav::watch(dav, collection, poll, shutdown, &mut on_event);
        }

        if backend == Backend::Dav {
            bail!("account has no `dav` config block");
        }
    }

    bail!(
        "account has no usable backend block (expected one of `imap`, `jmap`, `maildir`, \
         `dav`); use `-b/--backend` to pin a specific one"
    )
}

/// The message a backend gives back when asked for a method it does
/// not have.
fn unsupported(backend: &str, watch: &WatchConfig, available: &str) -> String {
    format!(
        "the {backend} backend cannot watch with `{}`; it offers {available}",
        watch.name(),
    )
}
/// Resolves an arrival, only when `on-item-added` is configured to
/// consume one. Anything else costs nothing.
#[cfg(feature = "imap")]
fn resolve_added(
    account: &str,
    hooks: &HooksConfig,
    event: &WatchEvent,
    resolver: &mut crate::imap::Resolver<'_>,
) -> Option<crate::event::ItemSummary> {
    let WatchEvent::ItemAdded { id } = event else {
        return None;
    };

    hooks.on_item_added.as_ref()?;

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
