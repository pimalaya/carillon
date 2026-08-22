//! `carillon watch` command: watches accounts and fires their hooks on
//! every change until Ctrl+C.
//!
//! Bare `watch` watches every configured account at once, one thread
//! each under a single shared shutdown; `-a/--account` narrows it to
//! one. What an account watches is its own collection, so there is
//! nothing to override on the command line.
//!
//! One account is one thread, and that thread is [`watch_account`]:
//! everything that can fail per account, meaning backend selection,
//! reconnect backoff and envelope resolution, happens inside it. A
//! failure ends the session, never the process, so one unreachable
//! server cannot stop the other accounts watching.

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::{Result, bail};
use clap::Parser;
use log::{debug, error, info, warn};
use pimalaya_cli::printer::Printer;

use crate::{
    backend::Backend,
    cli::load_config,
    config::{AccountConfig, Config},
    event::{ItemSummary, WatchEvent},
    hook,
};
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

/// Watch accounts and fire their hooks on every change.
///
/// What each account watches is its own collection, so there is no
/// flag to override it: watching something else is another account.
#[derive(Debug, Parser)]
pub struct WatchCommand;

impl WatchCommand {
    pub fn execute(
        self,
        printer: &mut impl Printer,
        config_paths: &[PathBuf],
        account_name: Option<&str>,
        backend: Backend,
    ) -> Result<()> {
        let mut config = load_config(printer, config_paths)?;
        let selected = select_accounts(&mut config, account_name)?;

        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_for_ctrlc = shutdown.clone();
        ctrlc::set_handler(move || {
            println!("received Ctrl+C, shutting down the watcher…");
            shutdown_for_ctrlc.store(true, Ordering::SeqCst);
        })?;

        let mut handles = Vec::with_capacity(selected.len());

        for (name, account) in selected {
            let shutdown = shutdown.clone();

            let handle = thread::Builder::new().name(name.clone()).spawn(move || {
                if let Err(err) = watch_account(&name, account, backend, shutdown) {
                    error!("[{name}] watch stopped: {err:#}");
                }
            })?;

            handles.push(handle);
        }

        info!("press Ctrl+C to exit");

        for handle in handles {
            let _ = handle.join();
        }

        Ok(())
    }
}

/// Selects the accounts to watch: the named one, or every configured
/// account when no name is given.
fn select_accounts(
    config: &mut Config,
    account_name: Option<&str>,
) -> Result<Vec<(String, AccountConfig)>> {
    if let Some(name) = account_name {
        let Some(entry) = config.accounts.remove_entry(name) else {
            let mut names: Vec<&str> = config.accounts.keys().map(String::as_str).collect();
            names.sort_unstable();

            bail!(
                "Account `{name}` not found, the configuration holds: {}",
                names.join(", "),
            );
        };

        return Ok(vec![entry]);
    }

    if config.accounts.is_empty() {
        bail!("No accounts configured");
    }

    let mut all: Vec<_> = config.accounts.drain().collect();
    all.sort_by(|(left, _), (right, _)| left.cmp(right));

    Ok(all)
}

/// Watches one account until `shutdown` is set, reopening the watch
/// with a capped backoff whenever a session ends.
fn watch_account(
    account: &str,
    config: AccountConfig,
    backend: Backend,
    shutdown: Arc<AtomicBool>,
) -> Result<()> {
    let mut backoff = INITIAL_BACKOFF;

    while !shutdown.load(Ordering::SeqCst) {
        let started = Instant::now();

        let outcome = watch_session(account, &config, backend, &shutdown);

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
fn watch_session(
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
            let mut on_event = |event: WatchEvent, summary: Option<ItemSummary>| {
                let Some(hook) = imap_config.hook.get(&event) else {
                    return;
                };

                // NOTE: an IMAP delta names a UID and nothing else, so
                // what the hook wants is read on a second connection.
                let summary = summary.or_else(|| resolve_added(account, &event, &mut resolver));
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
            bail!("Account has no `imap` config block");
        }
    }

    #[cfg(feature = "jmap")]
    if backend.allows_jmap() {
        if let Some(jmap_config) = &config.jmap {
            let collection = jmap_config.collection();
            let resolve = jmap_config.hook.on_message_added.is_some();
            let mut on_event = |event: WatchEvent, summary: Option<ItemSummary>| {
                if let Some(hook) = jmap_config.hook.get(&event) {
                    hook::run(hook, &event, jmap_config.collection(), summary.as_ref());
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
                        resolve,
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
                    jmap::watch_push(
                        jmap_config,
                        collection.value,
                        ping,
                        resolve,
                        shutdown,
                        &mut on_event,
                    )
                }
            };
        }

        if backend == Backend::Jmap {
            bail!("Account has no `jmap` config block");
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
            let mut on_event = |event: WatchEvent, _summary: Option<ItemSummary>| {
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
            bail!("Account has no `maildir` config block");
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
            let mut on_event = |event: WatchEvent, _summary: Option<ItemSummary>| {
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
            bail!("Account has no `caldav` config block");
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
            let mut on_event = |event: WatchEvent, _summary: Option<ItemSummary>| {
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
            bail!("Account has no `carddav` config block");
        }
    }

    bail!(
        "Account has no usable backend block (expected one of `imap`, `jmap`, `maildir`, \
         `caldav`, `carddav`); use `-b/--backend` to pin a specific one"
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
