//! # Accounts
//!
//! The built binary taking the account a command runs against, under
//! `--json`: `-a` names an account, `default` being a name like any
//! other, and no `-a` takes the one marked default. Each way of failing
//! carries its own code. No account here declares a backend, since the
//! account is resolved before any is opened. Then the `account` group:
//! `account list` reading every account without opening one, and `check`
//! still running `account check`.

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

/// The commands taking one account or every one, `check` the hidden
/// former name of `account check`.
const COMMANDS: [&str; 3] = ["account check", "check", "watch"];

/// `args` then the words of `command`.
fn with(args: &[&'static str], command: &'static str) -> Vec<&'static str> {
    args.iter().copied().chain(command.split(' ')).collect()
}

#[test]
fn a_name_no_account_carries_is_account_missing() {
    let toml = "[accounts.perso]\ndefault = true\n\n[accounts.work]\n";

    for command in COMMANDS {
        let json = failure(&run(toml, &with(&["-a", "wrok"], command)));
        let error = json["error"].as_str().unwrap();

        assert_eq!(json["code"], "account-missing", "{command}: {json}");
        assert!(error.contains("`wrok`"), "{command}: {error}");
        assert!(error.contains("perso, work"), "{command}: {error}");
    }
}

#[test]
fn default_names_the_account_called_default_not_the_one_marked() {
    let toml = "[accounts.perso]\ndefault = true\n";

    for command in COMMANDS {
        let json = failure(&run(toml, &with(&["-a", "default"], command)));
        let error = json["error"].as_str().unwrap();

        assert_eq!(json["code"], "account-missing", "{command}: {json}");
        assert!(error.contains("omit `-a`"), "{command}: {error}");
    }
}

#[test]
fn an_account_called_default_is_taken_by_its_name() {
    let toml = "[accounts.perso]\ndefault = true\n\n[accounts.default]\n";
    let output = run(toml, &["-a", "default", "account", "check"]);
    let json = failure(&output);
    let error = json["error"].as_str().unwrap();

    // NOTE: the account declares no backend, which is the failure once
    // it was taken, rather than its name.
    assert_eq!(json["code"], Value::Null, "{json}");
    assert!(error.contains("No backend matching"), "{error}");
}

#[test]
fn no_account_marked_default_is_account_default_missing() {
    let json = failure(&run("[accounts.perso]\n", &["account", "check"]));

    assert_eq!(json["code"], "account-default-missing", "{json}");
}

#[test]
fn several_accounts_marked_default_are_account_default_several() {
    let toml = "[accounts.perso]\ndefault = true\n\n[accounts.work]\ndefault = true\n";
    let json = failure(&run(toml, &["account", "check"]));
    let error = json["error"].as_str().unwrap();

    assert_eq!(json["code"], "account-default-several", "{json}");
    assert!(error.contains("perso, work"), "{error}");
}

/// Two accounts, `work` the default declaring a Maildir and a CalDAV
/// block, neither opened by a listing.
const TWO: &str = "[accounts.work]\ndefault = true\nmaildir.mailbox = \".\"\n\
                   maildir.root = \"/nonexistent\"\n\
                   caldav.server = \"https://dav.example.org\"\n\
                   caldav.calendar = \"work\"\n\n[accounts.perso]\n";

/// The names of `built` that this build serves, or leaves out.
fn split(built: &[(&str, bool)], served: bool) -> Value {
    let names = built.iter().filter(|(_, b)| *b == served);
    names.map(|(name, _)| Value::from(*name)).collect()
}

/// `account list` prints every account sorted by name, the backends this
/// build serves under `backends` and the ones it leaves out under
/// `notBuilt`, in the order `--backend auto` tries them.
#[test]
fn account_list_prints_every_account_and_its_backends() {
    let output = run(TWO, &["account", "list"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(0), "{stdout}");

    let json: Value = serde_json::from_str(&stdout).unwrap();
    let declared = [
        ("maildir", cfg!(feature = "maildir")),
        ("caldav", cfg!(feature = "dav")),
    ];
    let expected = serde_json::json!({"accounts": [
        {"name": "perso", "default": false, "backends": [], "notBuilt": []},
        {
            "name": "work",
            "default": true,
            "backends": split(&declared, true),
            "notBuilt": split(&declared, false),
        },
    ]});
    assert_eq!(json, expected);
}

/// The text is the family's: one line per account and no header, a
/// backend left out marked, `default` on the default account.
#[cfg(all(feature = "maildir", feature = "dav"))]
#[test]
fn account_list_prints_the_familys_columns() {
    let home = TempDir::new().unwrap();
    let path = home.path().join("config.toml");
    fs::write(&path, TWO).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_carillon"))
        .args(["-c", path.to_str().unwrap(), "account", "ls"])
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", home.path())
        .env_remove("XDG_CONFIG_DIRS")
        .env_remove("RUST_LOG")
        .stdin(Stdio::null())
        .output()
        .unwrap();

    let expected = "\
perso  -               -
work   maildir,caldav  default
";
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected);
}

/// A listing takes no account, so it reads a configuration that marks
/// none default, which `account check` refuses.
#[test]
fn account_list_takes_no_account() {
    let output = run("[accounts.perso]\n[accounts.work]\n", &["account", "list"]);
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert_eq!(output.status.code(), Some(0), "{json}");
    assert_eq!(json["accounts"].as_array().unwrap().len(), 2, "{json}");
}

/// `check` prints what `account check` does, and the help lists the
/// group alone.
#[test]
fn check_runs_account_check_unlisted() {
    let toml = "[accounts.perso]\ndefault = true\n";
    let check = run(toml, &["check"]);
    let account_check = run(toml, &["account", "check"]);

    assert_eq!(check.status.code(), account_check.status.code());
    assert_eq!(check.stdout, account_check.stdout);
    assert!(!check.stdout.is_empty());

    let help = run(toml, &["--help"]);
    let help = String::from_utf8_lossy(&help.stdout);
    assert!(help.contains("\n  account "), "{help}");
    assert!(!help.contains("\n  check "), "{help}");
}

/// The group and its list read himalaya's abouts, carillon's own
/// paragraph kept under the group's, and `ls` lists as `list`.
#[test]
fn the_account_abouts_are_the_familys() {
    let toml = "[accounts.perso]\ndefault = true\n";

    let help = run(toml, &["account", "--help"]);
    let help = String::from_utf8_lossy(&help.stdout);
    assert!(
        help.starts_with(
            "Manage accounts defined in the TOML configuration file.\n\n\
             An account is a named group of backend settings"
        ),
        "{help}"
    );

    let help = run(toml, &["account", "-h"]);
    let help = String::from_utf8_lossy(&help.stdout);
    assert!(
        help.contains("\n  list   List all accounts declared in the configuration [alias: ls]\n"),
        "{help}"
    );

    let list = run(toml, &["account", "list"]);
    let ls = run(toml, &["account", "ls"]);
    assert_eq!(ls.status.code(), Some(0));
    assert_eq!(ls.stdout, list.stdout);
}
