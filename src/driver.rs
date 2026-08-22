//! Per-account supervisor: opens a watch, dispatches what it reports to
//! the account's hooks, and reopens it when the connection drops.
//!
//! One account is one thread, and everything that can fail per account
//! is held here: the backend selection, the reconnect backoff, and the
//! resolution of an arrival into the envelope a hook templates against.
//! A failure ends the session, never the process, so one unreachable
//! server cannot stop the other accounts from watching.

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
    config::{AccountConfig, HooksConfig},
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
    mailbox: String,
    backend: Backend,
    shutdown: Arc<AtomicBool>,
) -> Result<()> {
    let hooks = config.hooks.clone();
    let mut backoff = INITIAL_BACKOFF;

    while !shutdown.load(Ordering::SeqCst) {
        let started = Instant::now();

        let outcome = watch_once(account, &config, &mailbox, backend, &hooks, &shutdown);

        // NOTE: a session ending because it was asked to is not news,
        // and a failure it raced on the way out is not a failure worth
        // warning about; either way nothing is reopened.
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
        // healthy, so the next failure starts over from the floor
        // rather than inheriting the backoff of an old outage.
        if started.elapsed() >= HEALTHY_THRESHOLD {
            backoff = INITIAL_BACKOFF;
        }

        if !sleep_backoff(&mut backoff, &shutdown) {
            break;
        }
    }

    Ok(())
}

/// Runs one watch session against the account's active backend.
fn watch_once(
    account: &str,
    config: &AccountConfig,
    mailbox: &str,
    backend: Backend,
    hooks: &HooksConfig,
    shutdown: &Arc<AtomicBool>,
) -> Result<()> {
    #[cfg(feature = "imap")]
    if backend.allows_imap() {
        if let Some(imap) = &config.imap {
            info!("[{account}] watching `{mailbox}` over imap");
            let mut resolver = crate::imap::Resolver::new(imap, mailbox, shutdown);
            let mut on_event = |event: WatchEvent| {
                let summary = resolve_added(account, hooks, &event, &mut resolver);
                hook::run(hooks, &event, mailbox, summary.as_ref());
            };
            return crate::imap::watch(imap, mailbox, shutdown, &mut on_event);
        }

        if backend == Backend::Imap {
            bail!("account has no `imap` config block");
        }
    }

    #[cfg(feature = "jmap")]
    if backend.allows_jmap() {
        if let Some(jmap) = &config.jmap {
            info!("[{account}] watching `{mailbox}` over jmap");
            let mut on_event = |event: WatchEvent| hook::run(hooks, &event, mailbox, None);
            return crate::jmap::watch(jmap, mailbox, shutdown, &mut on_event);
        }

        if backend == Backend::Jmap {
            bail!("account has no `jmap` config block");
        }
    }

    #[cfg(feature = "maildir")]
    if backend.allows_maildir() {
        if let Some(maildir) = &config.maildir {
            info!("[{account}] watching `{mailbox}` over maildir");
            let mut on_event = |event: WatchEvent| hook::run(hooks, &event, mailbox, None);
            return crate::maildir::watch(maildir, mailbox, shutdown, &mut on_event);
        }

        if backend == Backend::Maildir {
            bail!("account has no `maildir` config block");
        }
    }

    #[cfg(feature = "dav")]
    if backend.allows_dav() {
        if let Some(dav) = &config.dav {
            info!("[{account}] watching `{}` over dav", dav.server);
            let mut on_event = |event: WatchEvent| hook::run(hooks, &event, mailbox, None);
            return crate::dav::watch(dav, shutdown, &mut on_event);
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

/// Resolves the envelope of an arrival, but only when an
/// `on-message-added` hook is configured to consume one. Every other
/// event, and every account without that hook, costs nothing.
#[cfg(feature = "imap")]
fn resolve_added(
    account: &str,
    hooks: &HooksConfig,
    event: &WatchEvent,
    resolver: &mut crate::imap::Resolver<'_>,
) -> Option<crate::event::MessageSummary> {
    let WatchEvent::ItemAdded { id } = event else {
        return None;
    };

    hooks.on_item_added.as_ref()?;

    match resolver.summary(id) {
        Ok(summary) => Some(summary),
        Err(err) => {
            warn!("[{account}] cannot resolve message `{id}`: {err:#}");
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
