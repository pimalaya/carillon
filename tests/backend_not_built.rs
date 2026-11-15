//! # A backend the build leaves out
//!
//! The built binary reading a configuration that declares a backend its
//! build leaves out: the block loads, a misspelt key beside it is still
//! refused, and `check` reaching it fails its row coded
//! `backend-not-built`, naming the cargo feature. These run on a build
//! without the `imap` feature, such as
//! `cargo test --no-default-features --features rustls-ring,maildir`.

#![cfg(not(feature = "imap"))]

use std::{
    fs,
    process::{Command, Output, Stdio},
};

use serde_json::Value;
use tempfile::TempDir;

/// Runs the binary with `--json`, the configuration `toml` and `args`.
fn run(toml: &str, args: &[&str]) -> Output {
    let home = TempDir::new().unwrap();
    let path = home.path().join("config.toml");
    fs::write(&path, toml).unwrap();

    Command::new(env!("CARGO_BIN_EXE_carillon"))
        .args(["--json", "-c", path.to_str().unwrap()])
        .args(args)
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", home.path())
        .env_remove("XDG_CONFIG_DIRS")
        .env_remove("RUST_LOG")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// An IMAP account, its block holding keys this build cannot check.
const IMAP: &str = "[accounts.a]\ndefault = true\nimap.mailbox = \"INBOX\"\n\
                    imap.server = \"imap.example.org\"\nimap.anything = 1\n";

#[test]
fn a_check_reaching_it_fails_its_row_naming_the_feature() {
    let output = run(IMAP, &["check"]);
    let stdout = text(&output.stdout);

    assert_eq!(output.status.code(), Some(3), "{stdout}");
    let json: Value = serde_json::from_str(&stdout).unwrap();
    let row = &json["backends"][0];
    let error = row["error"].as_str().unwrap();
    assert_eq!(row["backend"], "imap", "{json}");
    assert_eq!(row["ok"], false, "{json}");
    assert_eq!(row["code"], "backend-not-built", "{json}");
    assert!(error.contains("`--features imap`"), "{error}");
}

#[test]
fn a_backend_pinned_away_from_it_leaves_it_unread() {
    let output = run(IMAP, &["--backend", "jmap", "check"]);
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert_eq!(output.status.code(), Some(1), "{json}");
    assert_eq!(json["code"], Value::Null, "{json}");
    assert!(json["error"].as_str().unwrap().contains("`jmap`"), "{json}");
}

#[test]
fn a_misspelt_backend_is_still_refused() {
    let output = run(
        "[accounts.a]\ndefault = true\nimapp.mailbox = \"INBOX\"\n",
        &["check"],
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let error = json.to_string();

    assert_eq!(output.status.code(), Some(1), "{json}");
    assert!(error.contains("unknown field `imapp`"), "{error}");
}

/// A watch over every account loses the one whose backend is left out
/// once, a `session-lost` event coded `backend-not-built`, never retries
/// it, and goes on watching the others.
#[cfg(all(unix, feature = "maildir"))]
#[test]
fn a_watch_over_every_account_loses_that_account_alone() {
    use std::{
        io::{BufRead, BufReader},
        process::Stdio,
    };

    let home = TempDir::new().unwrap();
    let root = home.path().join("mail");
    for dir in ["cur", "new", "tmp"] {
        fs::create_dir_all(root.join(dir)).unwrap();
    }
    let toml = format!(
        "{IMAP}\n[accounts.b]\nmaildir.mailbox = \".\"\nmaildir.root = {:?}\n",
        root.display().to_string(),
    );
    let config = home.path().join("config.toml");
    fs::write(&config, toml).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_carillon"))
        .args(["--json", "-c", config.to_str().unwrap(), "watch"])
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", home.path())
        .env_remove("XDG_CONFIG_DIRS")
        .env_remove("RUST_LOG")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();

    let mut events: Vec<Value> = Vec::new();
    while events.len() < 2 {
        let line = lines.next().unwrap().unwrap();
        events.push(serde_json::from_str(&line).unwrap());
    }

    // NOTE: past the first backoff, which would have retried it.
    std::thread::sleep(std::time::Duration::from_secs(3));

    let pid = child.id().to_string();
    let killed = Command::new("kill").args(["-INT", &pid]).status().unwrap();
    assert!(killed.success());
    assert_eq!(child.wait().unwrap().code(), Some(0));

    let lost = events.iter().find(|e| e["event"] == "session-lost");
    let lost = lost.expect("a lost session");
    assert_eq!(lost["account"], "a", "{lost}");
    assert_eq!(lost["code"], "backend-not-built", "{lost}");
    let opened = events.iter().find(|e| e["event"] == "session-opened");
    assert_eq!(opened.expect("an opened session")["account"], "b");

    let rest: Vec<String> = lines.map(Result::unwrap).collect();
    let result: Value = serde_json::from_str(rest.last().unwrap()).unwrap();
    assert_eq!(result["stoppedBy"], "signal", "{rest:?}");
    assert!(
        !rest.iter().any(|line| line.contains("session-lost")),
        "{rest:?}"
    );
}

/// A watch whose every account is left out of the build has nothing to
/// watch: it prints each loss, then the failure object, and exits 1.
#[test]
fn a_watch_with_nothing_left_fails_backend_not_built() {
    let output = run(IMAP, &["watch"]);
    let stdout = text(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();

    assert_eq!(output.status.code(), Some(1), "{stdout}");
    assert_eq!(lines.len(), 2, "{stdout}");
    let lost: Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(lost["event"], "session-lost", "{lost}");
    assert_eq!(lost["code"], "backend-not-built", "{lost}");
    let failure: Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(failure["code"], "backend-not-built", "{failure}");
    assert!(
        failure["error"]
            .as_str()
            .unwrap()
            .contains("--features imap")
    );
}
