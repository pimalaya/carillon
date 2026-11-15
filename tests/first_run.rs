//! # First run
//!
//! The built binary run with and without a configuration, its stdin and
//! stderr not a terminal, so it answers as a script sees it: a bare
//! invocation prints the help over a configuration, and like any command
//! finding none fails naming the path it looked at and the documented
//! sample, the welcome left to a person at a terminal, and nothing offers
//! to write one.

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
    run_with(home, args, &[])
}

/// Runs the binary as [`run`] does, with the environment variables `vars`
/// set.
fn run_with(home: &Path, args: &[&str], vars: &[(&str, &Path)]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_carillon"))
        .args(args)
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home)
        .env_remove("XDG_CONFIG_DIRS")
        .env_remove("RUST_LOG")
        .envs(vars.iter().copied())
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
/// the sample and at nothing writing one, with no welcome: stderr is no
/// terminal.
fn assert_missing(output: &Output, path: &Path) {
    let stdout = text(&output.stdout);
    let stderr = text(&output.stderr);

    assert_eq!(output.status.code(), Some(1), "{stdout}");
    let missing = format!("No configuration found at {}", path.display());
    assert!(stdout.contains(&missing), "{stdout}");
    assert!(stdout.contains("config.sample.toml"), "{stdout}");
    assert!(!stdout.contains("configure"), "{stdout}");
    assert!(!stderr.contains("Welcome to Carillon"), "{stderr}");
}

#[test]
fn a_bare_invocation_prints_the_help_when_a_configuration_exists() {
    let (home, path) = home(Some("[accounts.a]\ndefault = true\n"));
    let output = run(home.path(), &["-c", path.to_str().unwrap()]);

    assert!(output.status.success(), "{}", text(&output.stdout));
    assert!(text(&output.stdout).contains("Usage: carillon"));
}

/// A bare `carillon` finding no configuration fails as any command does,
/// as himalaya's does, where it printed the help and exited 0.
#[test]
fn a_bare_invocation_without_a_configuration_names_the_path_and_the_sample() {
    let (home, path) = home(None);
    let output = run(home.path(), &["-c", path.to_str().unwrap()]);

    assert_missing(&output, &path);
    assert!(!text(&output.stdout).contains("Usage: carillon"));
    assert!(!path.exists());

    let output = run(home.path(), &["--json"]);
    let default = home.path().join("carillon").join("config.toml");

    assert_eq!(output.status.code(), Some(1));
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let error = json["error"].as_str().unwrap();
    assert!(error.contains(&default.display().to_string()), "{error}");
    assert!(output.stderr.is_empty(), "{}", text(&output.stderr));
}

/// A configuration that does not parse is still one: the help prints, its
/// error left to the first command reading it.
#[test]
fn a_bare_invocation_prints_the_help_over_a_broken_configuration() {
    let (home, path) = home(Some("[accounts.a\n"));
    let output = run(home.path(), &["-c", path.to_str().unwrap()]);

    assert!(output.status.success(), "{}", text(&output.stdout));
    assert!(text(&output.stdout).contains("Usage: carillon"));
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
    assert!(output.stderr.is_empty(), "{}", text(&output.stderr));
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

#[test]
fn a_config_path_holding_a_colon_stays_whole() {
    let (home, _) = home(None);
    let path = home.path().join("a:b.toml");
    let output = run(home.path(), &["-c", path.to_str().unwrap(), "check"]);

    assert_missing(&output, &path);
}

#[test]
fn the_former_config_variable_is_not_read() {
    let (home, path) = home(Some("[accounts.a]\ndefault = true\n"));
    let output = run_with(home.path(), &["check"], &[("CARILLON_CONFIG", &path)]);
    let default = home.path().join("carillon").join("config.toml");

    assert_missing(&output, &default);
}
