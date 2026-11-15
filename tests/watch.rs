//! # Watch output
//!
//! The built binary watching a local Maildir, its stdout piped: it prints
//! one event per line as it observes them, then, once SIGINT asks, one
//! result line and exits 0. Offline: the Maildir is a temporary
//! directory, and the signal is sent by `kill`.

#![cfg(all(unix, feature = "maildir"))]

use std::{
    fs,
    io::{BufRead, BufReader},
    path::Path,
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};

use serde_json::Value;
use tempfile::TempDir;

/// How long a line may take to come.
const PATIENCE: Duration = Duration::from_secs(20);

/// A watch running in the background, its stdout read line by line.
struct Watch {
    child: Child,
    lines: Receiver<String>,
}

impl Watch {
    /// Starts `carillon <args> watch` over the configuration `toml`.
    fn start(home: &Path, toml: &str, args: &[&str]) -> Self {
        let config = home.join("config.toml");
        fs::write(&config, toml).unwrap();

        let mut child = Command::new(env!("CARGO_BIN_EXE_carillon"))
            .args(args)
            .args(["-c", config.to_str().unwrap(), "watch"])
            .env("HOME", home)
            .env("XDG_CONFIG_HOME", home)
            .env_remove("XDG_CONFIG_DIRS")
            .env_remove("RUST_LOG")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(fs::File::create(home.join("stderr")).unwrap())
            .spawn()
            .unwrap();

        let stdout = child.stdout.take().unwrap();
        let (sender, lines) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if sender.send(line).is_err() {
                    break;
                }
            }
        });

        Self { child, lines }
    }

    /// The next line, failing after [`PATIENCE`].
    fn line(&self) -> String {
        self.lines.recv_timeout(PATIENCE).expect("a line in time")
    }

    /// The next line as JSON.
    fn json(&self) -> Value {
        serde_json::from_str(&self.line()).unwrap()
    }

    /// Sends SIGINT, then returns every line left and the exit status.
    fn interrupt(mut self) -> (Vec<String>, Option<i32>) {
        let pid = self.child.id().to_string();
        let killed = Command::new("kill").args(["-INT", &pid]).status().unwrap();
        assert!(killed.success());

        let status = self.child.wait().unwrap();
        let rest = self.lines.iter().collect();
        (rest, status.code())
    }
}

/// A Maildir root holding its three directories.
fn maildir(home: &Path) -> String {
    let root = home.join("mail");
    for dir in ["cur", "new", "tmp"] {
        fs::create_dir_all(root.join(dir)).unwrap();
    }
    root.display().to_string()
}

#[test]
fn a_json_watch_prints_events_then_one_result_on_sigint() {
    let home = TempDir::new().unwrap();
    let root = maildir(home.path());
    let toml = format!(
        "[accounts.a]\nmaildir.mailbox = \".\"\nmaildir.root = {root:?}\n\
         maildir.watch.poll.interval = 1\n\
         maildir.hooks.on-message-add.command = \"exit 3\"\n"
    );
    let watch = Watch::start(home.path(), &toml, &["--json"]);

    let opened = watch.json();
    assert_eq!(opened["event"], "session-opened", "{opened}");
    assert_eq!(opened["account"], "a", "{opened}");
    assert_eq!(opened["backend"], "maildir", "{opened}");
    assert_eq!(opened["method"], "poll", "{opened}");
    assert_eq!(opened["collections"][0]["name"], "mailbox", "{opened}");

    // NOTE: the first listing is the baseline, so the message lands once
    // it was taken.
    thread::sleep(Duration::from_secs(2));
    let message = format!("{root}/new/1700000000.1.carillon");
    fs::write(&message, "Subject: hi\r\n\r\nhi\r\n").unwrap();

    let changed = watch.json();
    assert_eq!(changed["event"], "changed", "{changed}");
    assert_eq!(changed["collection"]["value"], ".", "{changed}");
    assert_eq!(changed["change"]["kind"], "item-added", "{changed}");
    assert_eq!(changed["change"]["domain"], "message", "{changed}");

    let failed = watch.json();
    assert_eq!(failed["event"], "hook-failed", "{failed}");
    assert_eq!(failed["reaction"], "command", "{failed}");
    assert_eq!(failed["code"], Value::Null, "{failed}");
    assert!(failed["error"].as_str().unwrap().contains("3"), "{failed}");

    let (rest, status) = watch.interrupt();
    assert_eq!(status, Some(0), "{rest:?}");
    let (result, events) = rest.split_last().expect("a result line");
    for event in events {
        let event: Value = serde_json::from_str(event).unwrap();
        assert!(event.get("event").is_some(), "{event}");
    }
    let result: Value = serde_json::from_str(result).unwrap();
    assert_eq!(result, serde_json::json!({ "stoppedBy": "signal" }));
}

#[test]
fn a_text_watch_streams_its_events_to_a_pipe() {
    let home = TempDir::new().unwrap();
    let root = maildir(home.path());
    let toml = format!("[accounts.a]\nmaildir.mailbox = \".\"\nmaildir.root = {root:?}\n");
    let watch = Watch::start(home.path(), &toml, &[]);

    let opened = watch.line();
    assert_eq!(opened, "[a] watching mailbox `.` over maildir, polling");

    let (rest, status) = watch.interrupt();
    assert_eq!(status, Some(0), "{rest:?}");
    assert_eq!(
        rest.last().unwrap(),
        "Watch stopped by a signal",
        "{rest:?}"
    );
}

/// A hook command printing to stdout reaches the watch's stderr, so its
/// stdout holds the events and the result alone.
#[test]
fn a_hook_printing_to_stdout_reaches_stderr() {
    let home = TempDir::new().unwrap();
    let root = maildir(home.path());
    let toml = format!(
        "[accounts.a]\nmaildir.mailbox = \".\"\nmaildir.root = {root:?}\n\
         maildir.watch.poll.interval = 1\n\
         maildir.hooks.on-message-add.command = \"echo hook-said-hi\"\n"
    );
    let watch = Watch::start(home.path(), &toml, &["--json"]);

    assert_eq!(watch.json()["event"], "session-opened");
    thread::sleep(Duration::from_secs(2));
    fs::write(format!("{root}/new/1700000000.1.carillon"), "\r\n").unwrap();
    assert_eq!(watch.json()["event"], "changed");

    // NOTE: the hook runs after its event is sent, so give it a moment.
    thread::sleep(Duration::from_millis(500));
    let (rest, status) = watch.interrupt();
    assert_eq!(status, Some(0), "{rest:?}");
    for line in &rest {
        let value: Value = serde_json::from_str(line).expect(line);
        assert!(value.is_object(), "{line}");
    }

    let stderr = fs::read_to_string(home.path().join("stderr")).unwrap();
    assert!(stderr.contains("hook-said-hi"), "{stderr}");
}

/// A session failing before it watches anything, here a mailbox the
/// Maildir does not hold, is reported lost and never open.
#[test]
fn a_session_that_cannot_open_is_lost_and_never_opened() {
    let home = TempDir::new().unwrap();
    let root = maildir(home.path());
    let toml = format!("[accounts.a]\nmaildir.mailbox = \"missing\"\nmaildir.root = {root:?}\n");
    let watch = Watch::start(home.path(), &toml, &["--json"]);

    let lost = watch.json();
    assert_eq!(lost["event"], "session-lost", "{lost}");
    assert_eq!(lost["account"], "a", "{lost}");
    assert_eq!(lost["code"], "mailbox-not-found", "{lost}");
    assert!(
        lost["error"].as_str().unwrap().contains("missing"),
        "{lost}"
    );

    // NOTE: past the first backoff, so the reopening is refused too.
    thread::sleep(Duration::from_secs(3));
    let (rest, status) = watch.interrupt();
    assert_eq!(status, Some(0), "{rest:?}");
    assert!(
        !rest.iter().any(|line| line.contains("session-opened")),
        "{rest:?}"
    );
}
