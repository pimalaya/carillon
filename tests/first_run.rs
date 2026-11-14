//! # First run
//!
//! The built binary run with and without a configuration, its stdin and
//! stderr not a terminal, so it answers as a script sees it: a bare
//! invocation prints the help, a command finding no configuration fails
//! naming the path it looked at and the documented sample, and nothing
//! offers to write one.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

use serde_json::Value;
use tempfile::TempDir;

/// Runs the binary with `args` in `home`, which holds no configuration of
/// its own.
fn run(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_carillon"))
        .args(args)
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home)
        .env_remove("XDG_CONFIG_DIRS")
        .env_remove("CARILLON_CONFIG")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

/// A home and the path of a configuration file in it holding `toml`, or
/// none when `toml` is `None`.
fn home(toml: Option<&str>) -> (TempDir, PathBuf) {
    let home = TempDir::new().unwrap();
    let path = home.path().join("config.toml");

    if let Some(toml) = toml {
        fs::write(&path, toml).unwrap();
    }

    (home, path)
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Asserts `output` is the missing configuration at `path`, pointing at
/// the sample and at nothing writing one.
fn assert_missing(output: &Output, path: &Path) {
    let stdout = text(&output.stdout);

    assert_eq!(output.status.code(), Some(1), "{stdout}");
    let missing = format!("No configuration found at {}", path.display());
    assert!(stdout.contains(&missing), "{stdout}");
    assert!(stdout.contains("config.sample.toml"), "{stdout}");
    assert!(!stdout.contains("configure"), "{stdout}");
}

#[test]
fn a_bare_invocation_prints_the_help_when_a_configuration_exists() {
    let (home, path) = home(Some("[accounts.a]\ndefault = true\n"));
    let output = run(home.path(), &["-c", path.to_str().unwrap()]);

    assert!(output.status.success(), "{}", text(&output.stdout));
    assert!(text(&output.stdout).contains("Usage: carillon"));
}

#[test]
fn a_bare_invocation_prints_the_help_without_a_configuration() {
    let (home, path) = home(None);
    let output = run(home.path(), &["-c", path.to_str().unwrap()]);

    assert!(output.status.success(), "{}", text(&output.stdout));
    assert!(text(&output.stdout).contains("Usage: carillon"));
    assert!(!path.exists());
}

#[test]
fn a_command_without_configuration_names_the_given_path_and_the_sample() {
    let (home, path) = home(None);
    let output = run(home.path(), &["-c", path.to_str().unwrap(), "watch"]);

    assert_missing(&output, &path);
    assert!(!path.exists());
}

#[test]
fn a_command_without_configuration_names_the_default_path() {
    let (home, _) = home(None);
    let output = run(home.path(), &["check"]);
    let path = home.path().join("carillon").join("config.toml");

    assert_missing(&output, &path);
    assert!(!path.exists());
}

#[test]
fn a_json_caller_gets_the_missing_configuration_as_the_failure_object() {
    let (home, path) = home(None);
    let args = ["--json", "-c", path.to_str().unwrap(), "watch"];
    let output = run(home.path(), &args);

    assert_eq!(output.status.code(), Some(1));
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let error = json["error"].as_str().unwrap();
    assert!(error.contains(&path.display().to_string()), "{error}");
    assert!(error.contains("config.sample.toml"), "{error}");
    assert!(!error.contains("configure"), "{error}");
}

#[test]
fn a_broken_configuration_is_an_error_rather_than_absent() {
    let (home, path) = home(Some("[accounts.a\n"));
    let output = run(home.path(), &["-c", path.to_str().unwrap(), "check"]);
    let stdout = text(&output.stdout);

    assert_eq!(output.status.code(), Some(1), "{stdout}");
    assert!(!stdout.contains("No configuration found"), "{stdout}");
}

#[test]
fn the_configure_command_is_gone() {
    let (home, _) = home(None);

    for command in ["configure", "wizard"] {
        let output = run(home.path(), &[command]);
        assert_eq!(output.status.code(), Some(2), "{command}");
    }
}
