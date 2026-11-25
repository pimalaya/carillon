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
#[cfg(feature = "imap")]
use std::{
    thread,
    time::{Duration, Instant},
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
        ("caldav", cfg!(feature = "caldav")),
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
#[cfg(all(feature = "maildir", feature = "caldav"))]
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

/// `-b` naming a backend the account has no block for is refused as
/// himalaya refuses its `--backend`, coded `capability-missing`: `account
/// check` fails, and `watch` stops the account rather than retrying it.
#[cfg(feature = "imap")]
#[test]
fn a_pinned_backend_the_account_lacks_is_capability_missing() {
    let home = TempDir::new().unwrap();
    let root = home.path().join("mail");
    for dir in ["cur", "new", "tmp"] {
        fs::create_dir_all(root.join(dir)).unwrap();
    }
    let path = home.path().join("config.toml");
    let toml = format!(
        "[accounts.a]\ndefault = true\nmaildir.mailbox = \".\"\nmaildir.root = {:?}\n",
        root.display().to_string()
    );
    fs::write(&path, toml).unwrap();

    let json = failure(&run_in(
        home.path(),
        &path,
        &["-b", "imap", "account", "check"],
    ));
    assert_eq!(json["code"], "capability-missing", "{json}");
    assert_eq!(json["error"], "Account `a` has no `imap` block", "{json}");

    let mut watch = Command::new(env!("CARGO_BIN_EXE_carillon"))
        .args([
            "--json",
            "-c",
            path.to_str().unwrap(),
            "-b",
            "imap",
            "watch",
        ])
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", home.path())
        .env_remove("RUST_LOG")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while watch.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            watch.kill().unwrap();
            panic!("the watch kept retrying");
        }
        thread::sleep(Duration::from_millis(100));
    }

    let output = watch.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(1), "{stdout}");
    let lines: Vec<Value> = stdout
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines[0]["event"], "session-lost", "{stdout}");
    assert_eq!(lines[0]["code"], "capability-missing", "{stdout}");
    assert_eq!(
        lines.last().unwrap()["code"],
        "capability-missing",
        "{stdout}"
    );
}

/// Each network row names the TLS its backend resolved, the account's
/// table filling what the backend's leaves unset, the client certificate
/// as one; a local backend's row names none.
#[cfg(all(feature = "imap", feature = "maildir"))]
#[test]
fn a_check_row_names_the_tls_its_backend_inherited() {
    let home = TempDir::new().unwrap();
    let root = home.path().join("mail");
    for dir in ["cur", "new", "tmp"] {
        fs::create_dir_all(root.join(dir)).unwrap();
    }

    let path = home.path().join("config.toml");
    let toml = format!(
        "[accounts.a]\ndefault = true\n\
         tls.rustls.crypto = \"ring\"\n\
         tls.cert = \"/etc/carillon/ca.pem\"\n\
         tls.client = {{ cert = \"/etc/carillon/me.pem\", key = \"/etc/carillon/me.key\" }}\n\
         imap.server = \"imaps://127.0.0.1:1\"\nimap.mailbox = \"INBOX\"\n\
         imap.tls.cert = \"/etc/carillon/imap.pem\"\n\
         maildir.mailbox = \".\"\nmaildir.root = {:?}\n",
        root.display().to_string()
    );
    fs::write(&path, toml).unwrap();

    let output = run_in(home.path(), &path, &["account", "check"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(3), "{stdout}");

    let json: Value = serde_json::from_str(&stdout).unwrap();
    let account = &json["accounts"][0];
    assert_eq!(account["account"], "a", "{json}");

    let tls = serde_json::json!({
        "provider": null,
        "crypto": "ring",
        "cert": "/etc/carillon/imap.pem",
        "clientCert": "/etc/carillon/me.pem",
    });
    assert_eq!(account["backends"][0]["backend"], "imap", "{json}");
    assert_eq!(account["backends"][0]["tls"], tls, "{json}");

    assert_eq!(account["backends"][1]["backend"], "maildir", "{json}");
    assert_eq!(account["backends"][1]["ok"], true, "{json}");
    assert_eq!(account["backends"][1]["tls"], Value::Null, "{json}");
}

/// The maildir row of an `account check` over the mailbox `mailbox` of a
/// root holding `dirs`, asserting the check printed its report.
#[cfg(feature = "maildir")]
fn maildir_check(dirs: &[&str], mailbox: &str) -> (Option<i32>, Value) {
    let home = TempDir::new().unwrap();
    let root = home.path().join("mail");
    fs::create_dir_all(&root).unwrap();
    for dir in dirs {
        fs::create_dir_all(root.join(dir)).unwrap();
    }

    let path = home.path().join("config.toml");
    let toml = format!(
        "[accounts.a]\ndefault = true\nmaildir.mailbox = {mailbox:?}\nmaildir.root = {:?}\n",
        root.display().to_string()
    );
    fs::write(&path, toml).unwrap();

    let output = run_in(home.path(), &path, &["account", "check"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: Value = serde_json::from_str(&stdout).unwrap();
    (
        output.status.code(),
        json["accounts"][0]["backends"][0].clone(),
    )
}

/// A maildir row opens the mailbox as a watch would: a root lacking the
/// Maildir directories fails it, where a check passed on any directory
/// and the watch then lost its session.
#[cfg(feature = "maildir")]
#[test]
fn a_maildir_check_fails_on_a_root_that_is_no_maildir() {
    let (status, row) = maildir_check(&[], "INBOX");
    assert_eq!(status, Some(3), "{row}");
    assert_eq!(row["backend"], "maildir", "{row}");
    assert_eq!(row["ok"], false, "{row}");

    let (status, row) = maildir_check(&["cur", "new", "tmp"], "INBOX");
    assert_eq!(status, Some(0), "{row}");
    assert_eq!(row["ok"], true, "{row}");
}

/// A maildir row naming a mailbox the root does not hold fails coded
/// `mailbox-not-found`, as the watch's `session-lost` does.
#[cfg(feature = "maildir")]
#[test]
fn a_maildir_check_codes_a_missing_mailbox() {
    let (status, row) = maildir_check(&["cur", "new", "tmp"], "Archive");
    assert_eq!(status, Some(3), "{row}");
    assert_eq!(row["ok"], false, "{row}");
    assert_eq!(row["code"], "mailbox-not-found", "{row}");
}
