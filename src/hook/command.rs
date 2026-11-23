//! # Hook commands
//!
//! Runs the commands hooks fire off the threads that watch, so a slow
//! command never holds a watch, an alarm or a shutdown.
//!
//! A command is spawned with no stdin, its stdout on carillon's stderr,
//! and reaped by a thread of its own. One instance of a command runs at a
//! time: firings meanwhile wait in order, each with its own variables, a
//! firing equal to one already waiting dropped, and at most [`QUEUE`]
//! waiting. A command run under a [`Timeout`] is sent SIGTERM once it
//! passes, then SIGKILL after [`Timeout::GRACE`], and reported as timed
//! out. On shutdown every running command is sent SIGTERM and given a
//! grace to end, then SIGKILL. See [`crate::hook`] for what a firing
//! hands here.

use std::{
    collections::{BTreeMap, VecDeque},
    io, mem,
    process::{Child, Command, ExitStatus, Stdio},
    sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Error, Result, anyhow};
use log::{debug, warn};
use pimalaya_config::command::Timeout;

use crate::hook::HookJob;

/// How often the reaper looks at the running commands, while one runs.
const REAP_STEP: Duration = Duration::from_millis(100);
/// How many firings of one command may wait for it to end.
const QUEUE: usize = 1000;

/// The commands the hooks of a watch fired, running or waiting to.
///
/// Cloning it shares the commands; the reaper ends once the last clone
/// is dropped.
#[derive(Clone)]
pub struct HookCommands {
    shared: Arc<Shared>,
    /// Dropped with the last clone, ending the reaper.
    _owner: Arc<Owner>,
}

/// What the reaper and the firings share.
#[derive(Default)]
struct Shared {
    state: Mutex<State>,
    /// Rung when a command starts or the last [`HookCommands`] is
    /// dropped, ending the reaper's wait while no command runs.
    ring: Condvar,
}

/// The commands running and waiting.
#[derive(Default)]
struct State {
    /// The command running under each key, with the runs waiting on it.
    running: BTreeMap<String, Running>,
    /// Whether a shutdown was requested, after which nothing is spawned
    /// and nothing reported.
    stopping: bool,
    /// Whether the last [`HookCommands`] was dropped, ending the reaper.
    closed: bool,
}

/// Held by every clone of a [`HookCommands`], closing the shared state
/// once the last one is dropped.
struct Owner(Arc<Shared>);

impl Drop for Owner {
    fn drop(&mut self) {
        self.0.lock().closed = true;
        self.0.ring.notify_all();
    }
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A command running, and the runs waiting for it to end.
struct Running {
    child: Child,
    /// When the run is sent SIGTERM, none for a run without a timeout.
    deadline: Option<Instant>,
    /// When the timeout sent it SIGTERM.
    expired: Option<Instant>,
    /// The timeout it runs under.
    timeout: Option<Timeout>,
    /// Where the failure of the run going on goes.
    report: Report,
    /// The runs waiting, in the order they were fired.
    pending: VecDeque<(HookJob, Report)>,
    /// How many firings the full queue dropped.
    dropped: usize,
}

/// Where a command's failure goes.
type Report = Box<dyn Fn(Error) + Send>;

impl HookCommands {
    /// Starts the reaper, no command running.
    pub fn start() -> Result<Self> {
        let shared = Arc::new(Shared::default());
        let reaped = shared.clone();

        thread::Builder::new()
            .name(String::from("hooks"))
            .spawn(move || reap(&reaped))?;

        Ok(Self {
            _owner: Arc::new(Owner(shared.clone())),
            shared,
        })
    }

    /// Runs `job` under `key`, or once the runs before it under that key
    /// ended, its failure handed to `report`.
    pub fn submit(&self, key: String, job: HookJob, report: impl Fn(Error) + Send + 'static) {
        let mut state = self.lock();

        if state.stopping {
            debug!("shutting down, hook command not run");
            return;
        }

        let Some(running) = state.running.get_mut(&key) else {
            start(
                &mut state,
                key,
                VecDeque::from([(job, Box::new(report) as Report)]),
            );
            self.shared.ring.notify_all();
            return;
        };

        if running.pending.iter().any(|(pending, _)| *pending == job) {
            debug!("the same hook command run already waits, dropping this one");
        } else if running.pending.len() >= QUEUE {
            running.dropped += 1;
        } else {
            debug!("hook command still running, its next run waits");
            running.pending.push_back((job, Box::new(report)));
        }
    }

    /// Sends SIGTERM to every running command and drops the runs waiting,
    /// waits up to `grace` for them to end, then sends SIGKILL to the
    /// others and reaps them.
    pub fn stop(&self, grace: Duration) {
        let deadline = Instant::now() + grace;

        {
            let mut state = self.lock();
            state.stopping = true;

            for running in state.running.values_mut() {
                if running.dropped > 0 {
                    warn!(
                        "dropped {} runs of a hook command, {QUEUE} already waiting",
                        running.dropped
                    );
                }

                running.pending.clear();
                signal(&mut running.child, Signal::Term);
            }
        }

        while !self.lock().running.is_empty() && Instant::now() < deadline {
            thread::sleep(REAP_STEP);
        }

        let left = mem::take(&mut self.lock().running);

        if !left.is_empty() {
            warn!("killing {} hook commands still running", left.len());
        }

        for mut running in left.into_values() {
            signal(&mut running.child, Signal::Kill);

            if let Err(err) = running.child.wait() {
                debug!("cannot reap hook command: {err}");
            }
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.shared.lock()
    }
}

impl Running {
    /// Sends SIGTERM once the timeout passed, and SIGKILL once its grace
    /// passed too.
    fn expire(&mut self, now: Instant) {
        if let Some(expired) = self.expired {
            if now >= expired + Timeout::GRACE {
                signal(&mut self.child, Signal::Kill);
            }
        } else if self.deadline.is_some_and(|deadline| now >= deadline) {
            signal(&mut self.child, Signal::Term);
            self.expired = Some(now);
        }
    }

    /// Why the run that ended with `status` failed, if it did.
    fn failure(&self, status: ExitStatus) -> Option<Error> {
        if let (Some(_), Some(Timeout(timeout))) = (self.expired, self.timeout) {
            Some(anyhow!("Hook command timed out after {timeout:?}"))
        } else if status.success() {
            None
        } else {
            Some(anyhow!("Hook command exited with {status}"))
        }
    }
}

/// Reaps the commands that ended, starting the run waiting first on
/// each, until the last [`HookCommands`] is dropped.
///
/// While no command runs it waits on the ring; while one runs it looks
/// every [`REAP_STEP`].
fn reap(shared: &Shared) {
    loop {
        let mut state = shared.lock();

        while state.running.is_empty() && !state.closed {
            state = shared
                .ring
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }

        if state.closed {
            return;
        }

        let mut ended = Vec::new();

        for (key, running) in state.running.iter_mut() {
            match running.child.try_wait() {
                Ok(None) => running.expire(Instant::now()),
                Ok(Some(status)) => ended.push((key.clone(), running.failure(status))),
                Err(err) => ended.push((key.clone(), Some(err.into()))),
            }
        }

        for (key, err) in ended {
            let Some(running) = state.running.remove(&key) else {
                continue;
            };

            if state.stopping {
                continue;
            }

            if let Some(err) = err {
                (running.report)(err);
            }

            if running.dropped > 0 {
                warn!(
                    "dropped {} runs of a hook command, {QUEUE} already waiting",
                    running.dropped
                );
            }

            start(&mut state, key, running.pending);
        }

        drop(state);
        thread::sleep(REAP_STEP);
    }
}

/// Spawns the first run of `pending` that spawns under `key`, the others
/// waiting on it, reporting each that cannot.
fn start(state: &mut State, key: String, mut pending: VecDeque<(HookJob, Report)>) {
    while let Some((job, report)) = pending.pop_front() {
        match spawn(&job) {
            Ok(child) => {
                let running = Running {
                    child,
                    deadline: job.timeout.map(|Timeout(timeout)| Instant::now() + timeout),
                    expired: None,
                    timeout: job.timeout,
                    report,
                    pending,
                    dropped: 0,
                };
                state.running.insert(key, running);
                return;
            }
            Err(err) => report(Error::new(err).context("Cannot spawn hook command")),
        }
    }
}

/// Spawns `job` with no stdin, its stdout on carillon's stderr and its
/// variables in its environment.
fn spawn(job: &HookJob) -> io::Result<Child> {
    let mut command = job.command.to_command();
    command
        .envs(job.vars.iter().map(|(name, value)| (*name, value.as_str())))
        .stdin(Stdio::null())
        .stdout(io::stderr());
    group(&mut command);
    command.spawn()
}

/// The signals a timeout or a shutdown sends a command.
#[derive(Clone, Copy, Debug)]
enum Signal {
    /// Asks it to end.
    Term,
    /// Ends it.
    Kill,
}

/// Puts the command in a process group of its own, so a shutdown signals
/// the shell and what it started alike, and a terminal's Ctrl+C reaches
/// carillon alone.
#[cfg(unix)]
fn group(command: &mut Command) {
    use std::os::unix::process::CommandExt;

    command.process_group(0);
}

/// Leaves the command in carillon's process group, the only one there is.
#[cfg(not(unix))]
fn group(_command: &mut Command) {}

/// Sends `signal` to the process group `child` leads, not yet reaped, so
/// it names no other process.
#[cfg(unix)]
fn signal(child: &mut Child, signal: Signal) {
    let Ok(group) = libc::pid_t::try_from(child.id()) else {
        return;
    };
    let number = match signal {
        Signal::Term => libc::SIGTERM,
        Signal::Kill => libc::SIGKILL,
    };

    // SAFETY: kill(2) takes no pointer; the group is our unreaped child's.
    if unsafe { libc::kill(-group, number) } != 0 {
        let err = io::Error::last_os_error();
        debug!("cannot send {signal:?} to hook command: {err}");
    }
}

/// Kills `child` whatever `signal` asks, the platform having no SIGTERM.
#[cfg(not(unix))]
fn signal(child: &mut Child, signal: Signal) {
    if let Err(err) = child.kill() {
        debug!("cannot send {signal:?} to hook command: {err}");
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::{fs, sync::mpsc};

    use pimalaya_config::command::CommandConfig;

    use crate::hook::command::*;

    fn job(line: &str) -> HookJob {
        HookJob {
            command: CommandConfig::Shell(line.to_string()),
            timeout: None,
            vars: Vec::new(),
        }
    }

    /// A job appending `n` to `log`, then sleeping a moment.
    fn logging(log: &std::path::Path, n: usize) -> HookJob {
        let line = format!("echo $n >> '{}'; sleep 0.2", log.display());
        HookJob {
            command: CommandConfig::Shell(line),
            timeout: None,
            vars: vec![("n", n.to_string())],
        }
    }

    /// Waits up to five seconds for `done`.
    fn wait(done: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);

        while !done() {
            assert!(Instant::now() < deadline, "timed out");
            thread::sleep(REAP_STEP);
        }
    }

    #[test]
    fn a_failing_command_is_reported_once_it_ends() {
        let commands = HookCommands::start().unwrap();
        let (sent, got) = mpsc::channel();

        commands.submit(String::from("a"), job("exit 3"), move |err| {
            sent.send(err.to_string()).unwrap();
        });

        let err = got.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(err.contains("exited with"), "{err}");
    }

    #[test]
    fn every_firing_while_a_command_runs_runs_after_it_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("log");
        let commands = HookCommands::start().unwrap();

        for n in [1, 2, 3, 3] {
            commands.submit(String::from("a"), logging(&log, n), |_| ());
        }

        wait(|| commands.lock().running.is_empty());
        assert_eq!("1\n2\n3\n", fs::read_to_string(&log).unwrap());
    }

    #[test]
    fn a_full_queue_drops_the_firings_past_it() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("log");
        let commands = HookCommands::start().unwrap();

        commands.submit(String::from("a"), job("sleep 30"), |_| ());
        for n in 0..QUEUE + 2 {
            commands.submit(String::from("a"), logging(&log, n), |_| ());
        }

        {
            let state = commands.lock();
            let running = &state.running["a"];
            assert_eq!(QUEUE, running.pending.len());
            assert_eq!(2, running.dropped);
        }

        commands.stop(Duration::ZERO);
        assert!(!log.exists());
    }

    #[test]
    fn a_command_reads_no_stdin() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out");
        let commands = HookCommands::start().unwrap();
        let line = format!("cat; echo done > '{}'", out.display());

        commands.submit(String::from("a"), job(&line), |_| ());

        wait(|| out.exists());
    }

    #[test]
    fn a_shutdown_terminates_what_runs_within_its_grace() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out");
        let commands = HookCommands::start().unwrap();
        let line = format!(
            "trap 'echo term > \"{}\"; exit 0' TERM; sleep 30 & wait",
            out.display()
        );

        commands.submit(String::from("a"), job(&line), |_| ());
        wait(|| !commands.lock().running.is_empty());
        thread::sleep(Duration::from_millis(300));

        let stopping = Instant::now();
        commands.stop(Duration::from_secs(2));

        assert!(stopping.elapsed() < Duration::from_secs(2));
        assert_eq!("term\n", fs::read_to_string(&out).unwrap());
    }

    #[test]
    fn a_shutdown_kills_what_outlives_its_grace() {
        let commands = HookCommands::start().unwrap();

        commands.submit(String::from("a"), job("trap '' TERM; sleep 30"), |_| ());
        wait(|| !commands.lock().running.is_empty());
        thread::sleep(Duration::from_millis(300));
        let pid = commands.lock().running["a"].child.id().to_string();

        let stopping = Instant::now();
        commands.stop(Duration::from_millis(500));

        assert!(stopping.elapsed() < Duration::from_secs(2));
        let alive = Command::new("kill").args(["-0", &pid]).status().unwrap();
        assert!(!alive.success(), "{pid} still alive");
    }

    fn timed(line: &str, millis: u64) -> HookJob {
        HookJob {
            timeout: Some(Timeout(Duration::from_millis(millis))),
            ..job(line)
        }
    }

    #[test]
    fn a_command_past_its_timeout_is_terminated_and_reported() {
        let commands = HookCommands::start().unwrap();
        let (sent, got) = mpsc::channel();
        let line = "trap 'exit 0' TERM; sleep 30 & wait";

        commands.submit(String::from("a"), timed(line, 300), move |err| {
            sent.send(err.to_string()).unwrap();
        });

        let err = got.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(err.contains("timed out after 300ms"), "{err}");
    }

    #[test]
    fn a_command_outliving_its_timeout_grace_is_killed() {
        let commands = HookCommands::start().unwrap();
        let (sent, got) = mpsc::channel();
        let started = Instant::now();

        commands.submit(
            String::from("a"),
            timed("trap '' TERM; sleep 30", 100),
            move |err| sent.send(err.to_string()).unwrap(),
        );

        let err = got.recv_timeout(Timeout::GRACE * 2).unwrap();
        assert!(
            started.elapsed() >= Timeout::GRACE,
            "killed before its grace"
        );
        assert!(err.contains("timed out after 100ms"), "{err}");
    }
}
