//! # Help
//!
//! The built binary's help, printed as written as pimalaya-cli's
//! terminal.md asks: no colour, each paragraph on one line whatever the
//! terminal width, the terminal folding a long line itself.

use std::process::{Command, Stdio};

use tempfile::TempDir;

/// On a 60-column terminal (`COLUMNS`, which clap reads when stdout is no
/// terminal) asked for colour (`CLICOLOR_FORCE`), a paragraph longer than
/// 60 columns prints whole on one line, with no escape sequence.
#[test]
fn the_help_is_printed_as_written() {
    let home = TempDir::new().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_carillon"))
        .arg("--help")
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", home.path())
        .env_remove("XDG_CONFIG_DIRS")
        .env_remove("RUST_LOG")
        .env("COLUMNS", "60")
        .env("LINES", "24")
        .env("CLICOLOR_FORCE", "1")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let help = String::from_utf8_lossy(&output.stdout);

    assert_eq!(output.status.code(), Some(0), "{help}");
    let paragraph = "\n          One connection is opened per `watch`, so this picks the \
                     block an account declaring several is watched over. `auto`, the \
                     default, takes the first configured, in the order IMAP, JMAP, \
                     Maildir, CalDAV, CardDAV, Microsoft Graph, Gmail, Google Calendar, \
                     Google People, pimdir.\n";
    assert!(help.contains(paragraph), "{help}");
    assert!(!help.contains('\x1b'), "{help}");
}
