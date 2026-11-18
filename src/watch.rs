//! # Watch
//!
//! The `carillon watch` command, watching accounts and firing their hooks
//! on every change until a termination signal.
//!
//! Bare `watch` takes every configured account at once, one thread each
//! under a single shared shutdown, `-a/--account` narrowing it to one.
//! What an account watches is its own collection, so there is nothing to
//! override on the command line.
//!
//! One account is one thread, and that thread is [`watch_account`], so
//! backend selection, reconnect backoff and envelope resolution all fail
//! inside it: a failure ends the session, never the process, and one
//! unreachable server cannot stop the other accounts watching.
//!
//! What the threads observe reaches the command's own thread as events,
//! which alone prints them, then the result once a signal asked it to
//! stop: pimalaya-cli's `Shutdown` turns the first signal into a request
//! the command reads, a second one ending the process. A backend the
//! build leaves out fails the session of its account coded
//! `backend-not-built` and stops that account for the run, no retry
//! building it; with no account left, the command fails with it.

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, RecvTimeoutError},
    },
    thread,
    time::{Duration, Instant},
};

#[cfg(alarm)]
use std::sync::{Mutex, PoisonError};

use anyhow::{Result, bail};
#[cfg(alarm)]
use ceichas::{timer::CeichasTimer, waker::CeichasWaker};
use clap::Parser;
#[cfg(feature = "imap")]
use io_imap::watch::ImapMailboxWatchState;
#[cfg(any(alarm, feature = "imap"))]
use log::warn;
use log::{debug, error, info};
use pimalaya_cli::{printer::Printer, signal::Shutdown};
use pimalaya_config::toml::TomlConfig;

#[cfg(api)]
use crate::config::api::ApiWatchConfig;
#[cfg(feature = "gcal")]
use crate::gcal;
#[cfg(feature = "gmail")]
use crate::gmail;
#[cfg(feature = "gpeople")]
use crate::gpeople;
#[cfg(feature = "msgraph")]
use crate::msgraph;
#[cfg(feature = "pimdir")]
use crate::pimdir;
#[cfg(alarm)]
use crate::{
    alarm::{self, AlarmShared},
    config::ItemHook,
    hook::{self, HookCollection},
};
use crate::{
    backend::Backend,
    config::{AccountConfig, Config},
    error::{self, ErrorCode},
    event::{ItemSummary, WatchEvent},
    hook::command::HookCommands,
    stream::{WatchEvents, WatchMethod, WatchOutput, WatchStop},
};
#[cfg(dav)]
use crate::{
    config::dav::DavWatchConfig,
    dav::{self, DavKind, DavTarget},
};
#[cfg(feature = "imap")]
use crate::{
    config::imap::{IdleWatchConfig, ImapWatchConfig},
    imap::{self, Resolver},
};
#[cfg(feature = "jmap")]
use crate::{
    config::jmap::{JmapWatchConfig, PushWatchConfig},
    jmap,
};
#[cfg(feature = "maildir")]
use crate::{config::maildir::MaildirWatchConfig, maildir};

/// Reconnect backoff floor.
const INITIAL_BACKOFF: Duration = Duration::from_secs(2);
/// Reconnect backoff ceiling.
const MAX_BACKOFF: Duration = Duration::from_secs(300);
/// A session that lived at least this long resets the backoff.
const HEALTHY_THRESHOLD: Duration = Duration::from_secs(60);
/// Backoff sleep granularity, so a shutdown is noticed promptly.
const BACKOFF_STEP: Duration = Duration::from_millis(200);
/// How often the command reads the signal while no event comes.
const SIGNAL_STEP: Duration = Duration::from_millis(100);
/// How long the hook commands still running at a shutdown are given to end
/// once sent SIGTERM.
const HOOK_GRACE: Duration = Duration::from_secs(1);

/// Watch accounts and fire their hooks on every change.
///
/// What each account watches is its own collection, so there is no
/// flag to override it: watching something else is another account.
#[derive(Debug, Parser)]
pub struct WatchCommand;

impl WatchCommand {
    /// Spawns one thread per selected account, prints what they observe,
    /// and once a termination signal asks, stops them and prints why.
    pub fn execute(
        self,
        printer: &mut impl Printer,
        config_paths: &[PathBuf],
        account_name: Option<&str>,
        backend: Backend,
    ) -> Result<()> {
        let mut config = Config::load(config_paths)?;
        let selected = select_accounts(&mut config, account_name)?;

        let signal = Shutdown::install()?;
        let shutdown = Arc::new(AtomicBool::new(false));
        #[cfg(alarm)]
        let wakers = AlarmWakers::default();
        let commands = HookCommands::start()?;
        let (sender, receiver) = mpsc::channel();
        let mut handles = Vec::with_capacity(selected.len());

        for (name, account) in selected {
            let shutdown = shutdown.clone();
            let events = WatchEvents::new(name.clone(), sender.clone(), commands.clone());
            #[cfg(alarm)]
            let wakers = wakers.clone();

            let handle = thread::Builder::new().name(name.clone()).spawn(move || {
                watch_account(
                    &name,
                    account,
                    backend,
                    shutdown,
                    &events,
                    #[cfg(alarm)]
                    wakers,
                )
            })?;

            handles.push(handle);
        }

        // NOTE: the threads hold the only senders left, so the channel
        // closes once every one of them ended.
        drop(sender);
        info!("press Ctrl+C to exit");

        let mut printed = Ok(());
        let mut stopping = false;

        loop {
            match receiver.recv_timeout(SIGNAL_STEP) {
                Ok(event) if printed.is_ok() => printed = printer.event(event),
                Ok(_) | Err(RecvTimeoutError::Timeout) => (),
                Err(RecvTimeoutError::Disconnected) => break,
            }

            // NOTE: a stdout no one reads any more stops the watch as a
            // signal does, and its failure is the command's.
            if !stopping && (signal.is_requested() || printed.is_err()) {
                stopping = true;
                info!("shutting down the watcher");
                shutdown.store(true, Ordering::SeqCst);

                // NOTE: rung after the flag is set, so a woken alarm
                // thread reads it and stops.
                #[cfg(alarm)]
                for waker in wakers.lock().unwrap_or_else(PoisonError::into_inner).iter() {
                    if let Err(err) = waker.wake() {
                        warn!("cannot wake an alarm thread: {err}");
                    }
                }

                commands.stop(HOOK_GRACE);
            }
        }

        let mut stopped = Vec::new();

        for handle in handles {
            match handle.join() {
                Ok(Ok(())) => (),
                Ok(Err(err)) => stopped.push(err),
                Err(_) => error!("an account thread panicked"),
            }
        }

        printed?;

        // NOTE: every account stopped on its own, a backend the build
        // leaves out, so nothing is left to watch.
        if !stopping {
            match stopped.into_iter().next() {
                Some(err) => return Err(err),
                None => bail!("Every account thread ended before a shutdown was requested"),
            }
        }

        printer.out(WatchOutput {
            stopped_by: WatchStop::Signal,
        })
    }
}

/// The accounts to watch: the one named, or every configured one.
fn select_accounts(
    config: &mut Config,
    account_name: Option<&str>,
) -> Result<Vec<(String, AccountConfig)>> {
    if account_name.is_some() {
        return Ok(vec![config.take_account(account_name)?]);
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
///
/// A backend the build leaves out stops the account for the run, its
/// failure handed back: no reopening can build it.
fn watch_account(
    account: &str,
    config: AccountConfig,
    backend: Backend,
    shutdown: Arc<AtomicBool>,
    events: &WatchEvents,
    #[cfg(alarm)] wakers: AlarmWakers,
) -> Result<()> {
    let mut stopped = Ok(());
    let mut backoff = INITIAL_BACKOFF;
    #[cfg(alarm)]
    let mut alarms = AlarmRunner::new(wakers);
    #[cfg(any(feature = "imap", feature = "gmail"))]
    let mut resume = Resume::default();

    while !shutdown.load(Ordering::SeqCst) {
        let started = Instant::now();

        let outcome = watch_session(
            account,
            &config,
            backend,
            &shutdown,
            events,
            #[cfg(alarm)]
            &mut alarms,
            #[cfg(any(feature = "imap", feature = "gmail"))]
            &mut resume,
        );

        // NOTE: neither an asked-for ending nor a failure raced on the
        // way out is news, and nothing is reopened either way.
        if shutdown.load(Ordering::SeqCst) {
            if let Err(err) = outcome {
                debug!("[{account}] session ended while shutting down: {err:#}");
            }

            break;
        }

        // NOTE: the event carries the code, which tells a refused account
        // from a network blip.
        match outcome {
            Ok(()) => debug!("[{account}] session ended, reopening"),
            Err(err) => {
                events.lost(&err);

                if matches!(
                    error::code(&err),
                    Some(ErrorCode::BackendNotBuilt | ErrorCode::CapabilityMissing)
                ) {
                    stopped = Err(err);
                    break;
                }
            }
        }

        // NOTE: a session that stayed up is evidence the server is
        // healthy, so the next failure starts from the floor rather than
        // inheriting an old outage's backoff.
        if started.elapsed() >= HEALTHY_THRESHOLD {
            backoff = INITIAL_BACKOFF;
        }

        if !sleep_backoff(&mut backoff, &shutdown) {
            break;
        }
    }

    #[cfg(alarm)]
    alarms.join(account);

    stopped
}

/// Runs one session against the account's active backend, with the method
/// that backend was asked for.
///
/// Which methods exist and which events have hooks are both declared in
/// the backend's own config, so anything it cannot do was already refused
/// when the file was read.
fn watch_session(
    #[cfg_attr(not(any(backend, feature = "pimdir")), allow(unused_variables))] account: &str,
    config: &AccountConfig,
    backend: Backend,
    shutdown: &Arc<AtomicBool>,
    events: &WatchEvents,
    #[cfg(alarm)] alarms: &mut AlarmRunner,
    #[cfg(any(feature = "imap", feature = "gmail"))] resume: &mut Resume,
) -> Result<()> {
    #[cfg(feature = "imap")]
    if backend.allows_imap() {
        if let Some(imap_config) = &config.imap {
            let collection = imap_config.collection();
            let mut resolver = Resolver::new(imap_config, collection.value, shutdown);
            let mut on_event = |event: WatchEvent, summary: Option<ItemSummary>| {
                let hook = imap_config.hooks.get(&event);

                // NOTE: an IMAP delta names a UID and nothing else, so
                // what the hook wants is read on a second connection.
                let summary = match hook {
                    Some(_) => summary.or_else(|| resolve_added(account, &event, &mut resolver)),
                    None => None,
                };
                events.react(hook, &event, imap_config.collection(), summary.as_ref());
            };

            return match &imap_config.watch {
                Some(ImapWatchConfig::Poll(poll)) => imap::watch_poll(
                    imap_config,
                    collection.value,
                    poll.interval(),
                    &mut resume.imap,
                    shutdown,
                    || events.opened("imap", WatchMethod::Poll, vec![collection]),
                    &mut on_event,
                ),
                watch => {
                    let idle = match watch {
                        Some(ImapWatchConfig::Idle(idle)) => idle.clone(),
                        _ => IdleWatchConfig::default(),
                    };

                    imap::watch_idle(
                        imap_config,
                        collection.value,
                        idle.timeout(),
                        &mut resume.imap,
                        shutdown,
                        || events.opened("imap", WatchMethod::Idle, vec![collection]),
                        &mut on_event,
                    )
                }
            };
        }

        if backend == Backend::Imap {
            return Err(error::backend_missing(account, "imap"));
        }
    }

    #[cfg(not(feature = "imap"))]
    if backend.allows_imap() && config.imap.is_some() {
        return Err(error::backend_not_built("imap"));
    }

    #[cfg(feature = "jmap")]
    if backend.allows_jmap() {
        if let Some(jmap_config) = &config.jmap {
            let collections: Vec<_> = jmap_config
                .collections()
                .into_iter()
                .map(|(_, collection)| collection)
                .collect();
            let resolve = jmap_config.hooks.on_message_add.is_some();

            // NOTE: one JMAP account watches up to three domains, so the
            // collection a hook templates against follows the event.
            let mut on_event = |event: WatchEvent, summary: Option<ItemSummary>| {
                if let Some(collection) = jmap_config.collection(event.domain()) {
                    let hook = jmap_config.hooks.get(&event);
                    events.react(hook, &event, collection, summary.as_ref());
                }
            };

            return match &jmap_config.watch {
                Some(JmapWatchConfig::Poll(poll)) => jmap::watch_poll(
                    jmap_config,
                    poll.interval(),
                    resolve,
                    shutdown,
                    || events.opened("jmap", WatchMethod::Poll, collections),
                    &mut on_event,
                ),
                watch => {
                    let ping = match watch {
                        Some(JmapWatchConfig::Push(push)) => push.ping.as_secs(),
                        _ => PushWatchConfig::default().ping.as_secs(),
                    };
                    let on_open = || events.opened("jmap", WatchMethod::Push, collections);
                    jmap::watch_push(jmap_config, ping, resolve, shutdown, on_open, &mut on_event)
                }
            };
        }

        if backend == Backend::Jmap {
            return Err(error::backend_missing(account, "jmap"));
        }
    }

    #[cfg(not(feature = "jmap"))]
    if backend.allows_jmap() && config.jmap.is_some() {
        return Err(error::backend_not_built("jmap"));
    }

    #[cfg(feature = "maildir")]
    if backend.allows_maildir() {
        if let Some(maildir_config) = &config.maildir {
            let collection = maildir_config.collection();
            let MaildirWatchConfig::Poll(poll) = maildir_config
                .watch
                .clone()
                .unwrap_or(MaildirWatchConfig::Poll(Default::default()));

            let mut on_event = |event: WatchEvent, _summary: Option<ItemSummary>| {
                let hook = maildir_config.hooks.get(&event);
                events.react(hook, &event, maildir_config.collection(), None);
            };
            return maildir::watch(
                maildir_config,
                collection.value,
                poll.interval(),
                shutdown,
                || events.opened("maildir", WatchMethod::Poll, vec![collection]),
                &mut on_event,
            );
        }

        if backend == Backend::Maildir {
            return Err(error::backend_missing(account, "maildir"));
        }
    }

    #[cfg(not(feature = "maildir"))]
    if backend.allows_maildir() && config.maildir.is_some() {
        return Err(error::backend_not_built("maildir"));
    }

    #[cfg(feature = "caldav")]
    if backend.allows_caldav() {
        if let Some(caldav_config) = &config.caldav {
            let collection = caldav_config.collection();
            let mut on_event = |event: WatchEvent, _summary: Option<ItemSummary>| {
                let hook = caldav_config.hooks.get(&event);
                events.react(hook, &event, caldav_config.collection(), None);
            };
            let shared = match &caldav_config.hooks.on_alarm_trigger {
                Some(hook) => Some(alarms.ensure(account, hook, collection, shutdown, events)?),
                None => None,
            };
            return dav::watch(
                caldav_config.server(),
                DavTarget {
                    collection: collection.value,
                    kind: DavKind::Calendar(caldav_config.hooks.domains()),
                    alarms: shared,
                },
                dav_interval(&caldav_config.watch),
                shutdown,
                || events.opened("caldav", WatchMethod::Poll, vec![collection]),
                &mut on_event,
            );
        }

        if backend == Backend::Caldav {
            return Err(error::backend_missing(account, "caldav"));
        }
    }

    #[cfg(not(feature = "caldav"))]
    if backend.allows_caldav() && config.caldav.is_some() {
        return Err(error::backend_not_built("caldav"));
    }

    #[cfg(feature = "carddav")]
    if backend.allows_carddav() {
        if let Some(carddav_config) = &config.carddav {
            let collection = carddav_config.collection();
            let mut on_event = |event: WatchEvent, _summary: Option<ItemSummary>| {
                let hook = carddav_config.hooks.get(&event);
                events.react(hook, &event, carddav_config.collection(), None);
            };
            return dav::watch(
                carddav_config.server(),
                DavTarget {
                    collection: collection.value,
                    kind: DavKind::Addressbook,
                    #[cfg(feature = "caldav")]
                    alarms: None,
                },
                dav_interval(&carddav_config.watch),
                shutdown,
                || events.opened("carddav", WatchMethod::Poll, vec![collection]),
                &mut on_event,
            );
        }

        if backend == Backend::Carddav {
            return Err(error::backend_missing(account, "carddav"));
        }
    }

    #[cfg(not(feature = "carddav"))]
    if backend.allows_carddav() && config.carddav.is_some() {
        return Err(error::backend_not_built("carddav"));
    }

    #[cfg(feature = "msgraph")]
    if backend.allows_msgraph() {
        if let Some(msgraph_config) = &config.msgraph {
            let collections: Vec<_> = msgraph_config
                .collections()
                .into_iter()
                .map(|(_, collection)| collection)
                .collect();
            let resolve = msgraph_config.hooks.on_message_add.is_some();

            // NOTE: one Graph account watches up to three domains, so the
            // collection a hook templates against follows the event.
            let mut on_event = |event: WatchEvent, summary: Option<ItemSummary>| {
                if let Some(collection) = msgraph_config.collection(event.domain()) {
                    let hook = msgraph_config.hooks.get(&event);
                    events.react(hook, &event, collection, summary.as_ref());
                }
            };

            return msgraph::watch(
                msgraph_config,
                ApiWatchConfig::interval(&msgraph_config.watch),
                resolve,
                shutdown,
                || events.opened("msgraph", WatchMethod::Poll, collections),
                &mut on_event,
            );
        }

        if backend == Backend::Msgraph {
            return Err(error::backend_missing(account, "msgraph"));
        }
    }

    #[cfg(not(feature = "msgraph"))]
    if backend.allows_msgraph() && config.msgraph.is_some() {
        return Err(error::backend_not_built("msgraph"));
    }

    #[cfg(feature = "gmail")]
    if backend.allows_gmail() {
        if let Some(gmail_config) = &config.gmail {
            let collection = gmail_config.collection();
            let resolve = gmail_config.hooks.on_message_add.is_some();
            let mut on_event = |event: WatchEvent, summary: Option<ItemSummary>| {
                let hook = gmail_config.hooks.get(&event);
                events.react(hook, &event, gmail_config.collection(), summary.as_ref());
            };

            return gmail::watch(
                gmail_config,
                ApiWatchConfig::interval(&gmail_config.watch),
                resolve,
                &mut resume.gmail,
                shutdown,
                || events.opened("gmail", WatchMethod::Poll, vec![collection]),
                &mut on_event,
            );
        }

        if backend == Backend::Gmail {
            return Err(error::backend_missing(account, "gmail"));
        }
    }

    #[cfg(not(feature = "gmail"))]
    if backend.allows_gmail() && config.gmail.is_some() {
        return Err(error::backend_not_built("gmail"));
    }

    #[cfg(feature = "gcal")]
    if backend.allows_gcal() {
        if let Some(gcal_config) = &config.gcal {
            let collection = gcal_config.collection();
            let mut on_event = |event: WatchEvent, _summary: Option<ItemSummary>| {
                let hook = gcal_config.hooks.get(&event);
                events.react(hook, &event, gcal_config.collection(), None);
            };

            return gcal::watch(
                gcal_config,
                ApiWatchConfig::interval(&gcal_config.watch),
                shutdown,
                || events.opened("gcal", WatchMethod::Poll, vec![collection]),
                &mut on_event,
            );
        }

        if backend == Backend::Gcal {
            return Err(error::backend_missing(account, "gcal"));
        }
    }

    #[cfg(not(feature = "gcal"))]
    if backend.allows_gcal() && config.gcal.is_some() {
        return Err(error::backend_not_built("gcal"));
    }

    #[cfg(feature = "gpeople")]
    if backend.allows_gpeople() {
        if let Some(gpeople_config) = &config.gpeople {
            let collection = gpeople_config.collection();
            let mut on_event = |event: WatchEvent, _summary: Option<ItemSummary>| {
                let hook = gpeople_config.hooks.get(&event);
                events.react(hook, &event, gpeople_config.collection(), None);
            };

            return gpeople::watch(
                gpeople_config,
                ApiWatchConfig::interval(&gpeople_config.watch),
                shutdown,
                || events.opened("gpeople", WatchMethod::Poll, vec![collection]),
                &mut on_event,
            );
        }

        if backend == Backend::Gpeople {
            return Err(error::backend_missing(account, "gpeople"));
        }
    }

    #[cfg(not(feature = "gpeople"))]
    if backend.allows_gpeople() && config.gpeople.is_some() {
        return Err(error::backend_not_built("gpeople"));
    }

    #[cfg(feature = "pimdir")]
    if backend.allows_pimdir() {
        if let Some(pimdir_config) = &config.pimdir {
            let collection = pimdir_config.collection();
            let Some(hook) = &pimdir_config.hooks.on_alarm_trigger else {
                bail!("Account has no `pimdir.hooks.on-alarm-trigger`, so nothing to watch for");
            };

            let shared = alarms.ensure(account, hook, collection, shutdown, events)?;
            let on_open = || events.opened("pimdir", WatchMethod::Poll, vec![collection]);
            return pimdir::watch(pimdir_config, shutdown, shared, on_open);
        }

        if backend == Backend::Pimdir {
            return Err(error::backend_missing(account, "pimdir"));
        }
    }

    #[cfg(not(feature = "pimdir"))]
    if backend.allows_pimdir() && config.pimdir.is_some() {
        return Err(error::backend_not_built("pimdir"));
    }

    bail!(
        "Account has no usable backend block (expected one of `imap`, `jmap`, `maildir`, \
         `caldav`, `carddav`, `msgraph`, `gmail`, `gcal`, `gpeople`, `pimdir`); use \
         `-b/--backend` to pin a specific one"
    )
}

/// What a session leaves for the next one of its account to resume
/// from, so a lost connection hides nothing that moved meanwhile.
#[cfg(any(feature = "imap", feature = "gmail"))]
#[derive(Default)]
struct Resume {
    /// What the IMAP watch knew of its mailbox.
    #[cfg(feature = "imap")]
    imap: Option<ImapMailboxWatchState>,
    /// How far the Gmail history was read.
    #[cfg(feature = "gmail")]
    gmail: Option<String>,
}

/// The thread firing an account's alarms, beside its watch.
///
/// It lives as long as the account, not the session: a session lost to
/// a dropped connection leaves the alarms it read firing on time while
/// the next one connects, and that one replaces them with what it reads.
#[cfg(alarm)]
struct AlarmRunner {
    /// Where the thread's waker is left for a shutdown to ring.
    wakers: AlarmWakers,
    started: Option<(Arc<AlarmShared>, thread::JoinHandle<()>)>,
}

/// The waker of every alarm thread, which a shutdown rings once its flag
/// is set: a thread blocks until its next firing, which may be hours
/// away, and would otherwise not notice.
#[cfg(alarm)]
type AlarmWakers = Arc<Mutex<Vec<CeichasWaker>>>;

#[cfg(alarm)]
impl AlarmRunner {
    /// No thread yet, its waker to be left in `wakers`.
    fn new(wakers: AlarmWakers) -> Self {
        Self {
            wakers,
            started: None,
        }
    }

    /// The alarms of this account, starting its thread on the first
    /// session to fire `hook` for the alarms of `collection`, each firing
    /// reported to `events`.
    fn ensure(
        &mut self,
        account: &str,
        hook: &ItemHook,
        collection: HookCollection<'_>,
        shutdown: &Arc<AtomicBool>,
        events: &WatchEvents,
    ) -> Result<&AlarmShared> {
        if let Some(started) = self.started.take() {
            let (shared, _) = self.started.insert(started);
            return Ok(shared);
        }

        let timer = CeichasTimer::new(Default::default())?;
        debug!("[{account}] alarms wait on {:?}", timer.backend());

        // NOTE: left before the thread first reads the flag, so a
        // shutdown racing the start either stops it before its first
        // wait or rings this waker.
        self.wakers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(timer.waker());

        let shared = Arc::new(AlarmShared::new(timer.waker()));
        let hook = hook.clone();
        let name = collection.name;
        let value = collection.value.to_owned();
        let thread_shared = shared.clone();
        let thread_shutdown = shutdown.clone();
        let events = events.clone();

        let handle = thread::Builder::new()
            .name(format!("{account}-alarms"))
            .spawn(move || {
                alarm::run(&thread_shared, timer, &thread_shutdown, |firing| {
                    let collection = HookCollection {
                        name,
                        value: &value,
                    };
                    let fired = hook::run_alarm(&hook, collection, firing);
                    events.alarm(collection, firing, fired);
                })
            })?;

        let (shared, _) = self.started.insert((shared, handle));
        Ok(shared)
    }

    /// Waits for the thread, which ends once a shutdown is requested.
    fn join(self, account: &str) {
        if let Some((_, handle)) = self.started
            && handle.join().is_err()
        {
            warn!("[{account}] alarm thread panicked");
        }
    }
}

/// The poll interval a DAV backend was configured with, WebDAV having
/// the one method whatever domain it holds.
#[cfg(dav)]
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

/// Sleeps the current backoff, then doubles it toward the ceiling.
///
/// The sleep runs in small steps so a shutdown is noticed promptly, and
/// returns false as soon as one is requested.
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
