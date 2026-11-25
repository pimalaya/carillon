//! # Exit contract
//!
//! The built binary's exit statuses beyond success and failure: a command
//! line the grammar refuses exits 2, its failure object coded
//! `usage-invalid` under `--json`, and an `account check` that printed its
//! report with a failed backend exits 3.

use std::{
    path::Path,
    process::{Command, Output, Stdio},
};

use serde_json::Value;
use tempfile::TempDir;

/// Runs the binary with `args` in `home`, its stdin closed.
fn run(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_carillon"))
        .args(args)
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home)
        .env_remove("XDG_CONFIG_DIRS")
        .env_remove("RUST_LOG")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn a_refused_command_line_under_json_prints_the_usage_failure_object() {
    let home = TempDir::new().unwrap();
    let output = run(home.path(), &["--json", "watch", "--nope"]);
    let stdout = text(&output.stdout);

    assert_eq!(output.status.code(), Some(2), "{stdout}");
    let json: Value = serde_json::from_str(&stdout).unwrap();
    let error = json["error"].as_str().unwrap();
    assert_eq!(json["code"], "usage-invalid", "{json}");
    assert!(error.starts_with("unexpected argument '--nope'"), "{error}");
    assert_eq!(json["sources"], Value::Array(Vec::new()), "{json}");
    assert_eq!(json["backtrace"], Value::Null, "{json}");
    assert!(output.stderr.is_empty(), "{}", text(&output.stderr));
}

#[test]
fn a_refused_command_line_without_json_is_clap_rendering_on_stderr() {
    let home = TempDir::new().unwrap();
    let output = run(home.path(), &["watch", "--nope"]);
    let stderr = text(&output.stderr);

    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(output.stdout.is_empty(), "{}", text(&output.stdout));
    assert!(stderr.starts_with("error: unexpected argument"), "{stderr}");
}

/// A Maildir account whose root does not exist, checked under `json`
/// when it is set.
#[cfg(feature = "maildir")]
fn check_missing_maildir(json: bool) -> Output {
    let home = TempDir::new().unwrap();
    let config = home.path().join("config.toml");
    let missing = home.path().join("missing");
    let toml = format!(
        "[accounts.a]\ndefault = true\nmaildir.mailbox = \".\"\nmaildir.root = {:?}\n",
        missing.display().to_string(),
    );
    std::fs::write(&config, toml).unwrap();

    let config = config.to_str().unwrap();
    let args = ["-c", config, "--backend", "maildir", "account", "check"];
    let mut args = Vec::from(args);
    if json {
        args.insert(0, "--json");
    }

    run(home.path(), &args)
}

#[cfg(feature = "maildir")]
#[test]
fn a_failed_check_prints_its_report_and_exits_3() {
    let output = check_missing_maildir(true);
    let stdout = text(&output.stdout);

    assert_eq!(output.status.code(), Some(3), "{stdout}");
    let json: Value = serde_json::from_str(&stdout).unwrap();
    let row = &json["accounts"][0]["backends"][0];
    assert_eq!(row["backend"], "maildir", "{json}");
    assert_eq!(row["ok"], false, "{json}");
    assert_eq!(row["code"], "mailbox-not-found", "{json}");
    assert!(row["error"].as_str().unwrap().contains("missing"), "{json}");
    assert_eq!(stdout.lines().count(), 1, "{stdout}");
}

#[cfg(feature = "maildir")]
#[test]
fn a_failed_check_in_text_prints_its_report_alone_and_exits_3() {
    let output = check_missing_maildir(false);
    let stdout = text(&output.stdout);

    assert_eq!(output.status.code(), Some(3), "{stdout}");
    assert!(
        stdout.starts_with("Account:  a\nmaildir:  FAIL ("),
        "{stdout}"
    );
    assert!(!stdout.contains("Error:"), "{stdout}");
}

/// The row of `account check` over a pimdir store stamped `draft`.
#[cfg(feature = "pimdir")]
fn check_store_of_draft(draft: &str) -> Value {
    let home = TempDir::new().unwrap();
    let root = home.path().join("pimdir");
    let store = io_pimdir::client::PimdirStore::open(&root).unwrap();
    store.ensure_collection("cal", "text/calendar").unwrap();
    drop(store);
    rusqlite::Connection::open(root.join("pimdir.db"))
        .unwrap()
        .execute("UPDATE store_meta SET format_version = ?1", [draft])
        .unwrap();

    let config = home.path().join("config.toml");
    let toml = format!(
        "[accounts.a]\ndefault = true\npimdir.calendar = \"cal\"\npimdir.root = {:?}\npimdir.hooks.on-alarm-trigger.command = \"true\"\n",
        root.display().to_string(),
    );
    std::fs::write(&config, toml).unwrap();

    let config = config.to_str().unwrap();
    let args = [
        "--json",
        "-c",
        config,
        "--backend",
        "pimdir",
        "account",
        "check",
    ];
    let output = run(home.path(), &args);
    let stdout = text(&output.stdout);

    assert_eq!(output.status.code(), Some(3), "{stdout}");
    let json: Value = serde_json::from_str(&stdout).unwrap();
    let row = json["accounts"][0]["backends"][0].clone();
    assert_eq!(row["backend"], "pimdir", "{json}");
    assert_eq!(row["ok"], false, "{json}");
    assert_eq!(row["code"], "store-version", "{json}");
    row
}

/// A pimdir store of draft-15, the draft io-pimdir 0.19.1 wrote and
/// users hold, is refused with io-pimdir's code, its message naming
/// carillon and the draft it reads.
#[cfg(feature = "pimdir")]
#[test]
fn a_check_over_a_store_of_another_draft_is_coded_store_version() {
    let row = check_store_of_draft("storage-draft-15");
    let error = row["error"].as_str().unwrap();

    assert!(error.contains("storage-draft-15"), "{row}");
    assert!(error.contains("carillon reads"), "{row}");
    assert!(error.contains(io_pimdir::sql::FORMAT_VERSION), "{row}");
    assert!(!error.contains("this program"), "{row}");
}

/// A pimdir store of a later draft asks to upgrade carillon.
#[cfg(feature = "pimdir")]
#[test]
fn a_check_over_a_store_of_a_later_draft_asks_to_upgrade_carillon() {
    let row = check_store_of_draft("storage-draft-99");
    let error = row["error"].as_str().unwrap();

    assert!(error.contains("upgrade carillon"), "{row}");
}

/// `manual`, `completion` and `json-schema` given no argument are a
/// command line clap refuses, exit 2, coded `usage-invalid` under `--json`.
#[test]
fn a_standard_command_with_no_argument_is_refused_by_clap() {
    let home = TempDir::new().unwrap();

    for command in ["manual", "completion", "json-schema"] {
        let output = run(home.path(), &[command]);
        let stderr = text(&output.stderr);

        assert_eq!(output.status.code(), Some(2), "{command}: {stderr}");
        assert!(stderr.contains("--dir"), "{command}: {stderr}");
        assert!(output.stdout.is_empty(), "{}", text(&output.stdout));

        let output = run(home.path(), &["--json", command]);
        let stdout = text(&output.stdout);

        assert_eq!(output.status.code(), Some(2), "{command}: {stdout}");
        let json: Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(json["code"], "usage-invalid", "{json}");
        let error = json["error"].as_str().unwrap();
        assert!(error.contains("--dir"), "{command}: {error}");
    }
}
