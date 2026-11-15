//! # Account resolution
//!
//! The built binary taking the account a command runs against, under
//! `--json`: `-a` names an account, `default` being a name like any
//! other, and no `-a` takes the one marked default. Each way of failing
//! carries its own code. No account here declares a backend, since the
//! account is resolved before any is opened.

use std::{
    fs,
    path::Path,
    process::{Command, Output, Stdio},
};

use serde_json::Value;
use tempfile::TempDir;

/// Runs the binary with `--json`, the configuration `toml` and `args`.
fn run(toml: &str, args: &[&str]) -> Output {
    let home = TempDir::new().unwrap();
    let path = home.path().join("config.toml");
    fs::write(&path, toml).unwrap();

    run_in(home.path(), &path, args)
}

fn run_in(home: &Path, config: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_carillon"))
        .args(["--json", "-c", config.to_str().unwrap()])
        .args(args)
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home)
        .env_remove("XDG_CONFIG_DIRS")
        .env_remove("RUST_LOG")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

/// The failure object `output` printed, asserting it exited 1.
fn failure(output: &Output) -> Value {
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(1), "{stdout}");
    serde_json::from_str(&stdout).unwrap()
}

#[test]
fn a_name_no_account_carries_is_account_missing() {
    let toml = "[accounts.perso]\ndefault = true\n\n[accounts.work]\n";

    for command in ["check", "watch"] {
        let json = failure(&run(toml, &["-a", "wrok", command]));
        let error = json["error"].as_str().unwrap();

        assert_eq!(json["code"], "account-missing", "{command}: {json}");
        assert!(error.contains("`wrok`"), "{command}: {error}");
        assert!(error.contains("perso, work"), "{command}: {error}");
    }
}

#[test]
fn default_names_the_account_called_default_not_the_one_marked() {
    let toml = "[accounts.perso]\ndefault = true\n";

    for command in ["check", "watch"] {
        let json = failure(&run(toml, &["-a", "default", command]));
        let error = json["error"].as_str().unwrap();

        assert_eq!(json["code"], "account-missing", "{command}: {json}");
        assert!(error.contains("omit `-a`"), "{command}: {error}");
    }
}

#[test]
fn an_account_called_default_is_taken_by_its_name() {
    let toml = "[accounts.perso]\ndefault = true\n\n[accounts.default]\n";
    let output = run(toml, &["-a", "default", "check"]);
    let json = failure(&output);
    let error = json["error"].as_str().unwrap();

    // NOTE: the account declares no backend, which is the failure once
    // it was taken, rather than its name.
    assert_eq!(json["code"], Value::Null, "{json}");
    assert!(error.contains("No backend matching"), "{error}");
}

#[test]
fn no_account_marked_default_is_account_default_missing() {
    let json = failure(&run("[accounts.perso]\n", &["check"]));

    assert_eq!(json["code"], "account-default-missing", "{json}");
}

#[test]
fn several_accounts_marked_default_are_account_default_several() {
    let toml = "[accounts.perso]\ndefault = true\n\n[accounts.work]\ndefault = true\n";
    let json = failure(&run(toml, &["check"]));
    let error = json["error"].as_str().unwrap();

    assert_eq!(json["code"], "account-default-several", "{json}");
    assert!(error.contains("perso, work"), "{error}");
}
