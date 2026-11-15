//! # Parser
//!
//! The top-level CLI parser and subcommand dispatcher, and the welcome a
//! person missing a configuration meets. A bare `carillon` is met in main.

use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use pimalaya_cli::{
    clap::{
        args::{AccountFlag, ConfigFlags, JsonFlag, LogFlags},
        commands::{
            completion::CompletionCommand, json_schema::JsonSchemaCommand, manual::ManualCommand,
        },
    },
    footer, long_version,
    printer::Printer,
    welcome::Welcome,
};

use crate::{backend::Backend, check::CheckCommand, json_schema, watch::WatchCommand};

/// The sample configuration as a raw file, for `curl` to fetch.
const SAMPLE_RAW_URL: &str =
    "https://raw.githubusercontent.com/pimalaya/carillon/master/config.sample.toml";

/// Top-level CLI: global flags and subcommand dispatch.
#[derive(Parser, Debug)]
#[command(name = env!("CARGO_PKG_NAME"))]
#[command(author, version, about)]
#[command(long_version = long_version!())]
#[command(after_help = footer!())]
#[command(propagate_version = true, infer_subcommands = true)]
pub struct Cli {
    /// The command to run.
    ///
    /// Omitted, a bare `carillon` shows this help.
    #[command(subcommand)]
    pub command: Option<Command>,
    #[command(flatten)]
    pub config: ConfigFlags,
    #[command(flatten)]
    pub account: AccountFlag,
    /// Force a specific backend.
    ///
    /// One connection is opened per `watch`, so this picks the block an
    /// account declaring several is watched over. `auto`, the default,
    /// takes the first configured, in the order IMAP, JMAP, Maildir,
    /// CalDAV, CardDAV, Microsoft Graph, Gmail, Google Calendar, Google
    /// People.
    #[arg(short, long, global = true, default_value_t)]
    pub backend: Backend,
    #[command(flatten)]
    pub json: JsonFlag,
    #[command(flatten)]
    pub log: LogFlags,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Watch the accounts, fire their hooks and print every change until
    /// a termination signal.
    Watch(WatchCommand),
    /// Validate the account configuration against each allowed backend.
    Check(CheckCommand),
    /// Generate man pages into the given directory.
    #[command(arg_required_else_help = true)]
    #[command(alias = "manuals")]
    Manual(ManualCommand),
    /// Generate shell completion scripts into the given directory.
    #[command(arg_required_else_help = true)]
    #[command(alias = "completions")]
    Completion(CompletionCommand),
    /// Generate the JSON Schema of a command's `--json` output.
    #[command(alias = "json-schemas")]
    JsonSchema(JsonSchemaCommand),
}

impl Command {
    /// Runs the subcommand against the account and backend the global
    /// flags name.
    pub fn execute(
        self,
        printer: &mut impl Printer,
        config_paths: &[PathBuf],
        account_name: Option<&str>,
        backend: Backend,
    ) -> Result<()> {
        match self {
            Self::Watch(cmd) => cmd.execute(printer, config_paths, account_name, backend),
            Self::Check(cmd) => cmd.execute(printer, config_paths, account_name, backend),
            Self::Manual(cmd) => cmd.execute(printer, Cli::command()),
            Self::Completion(cmd) => cmd.execute(printer, Cli::command()),
            Self::JsonSchema(cmd) => cmd.execute(printer, json_schema::registry()),
        }
    }
}

/// The welcome a person meets when no configuration exists at `path`:
/// the commands creating it from `sample`, the shipped sample
/// configuration, or downloading it without one, then `check`.
pub fn welcome<'a>(
    path: &'a Path,
    config_paths: &'a [PathBuf],
    sample: Option<&'a Path>,
) -> Welcome<'a> {
    Welcome {
        bin: "carillon",
        intro: "Welcome to Carillon, the CLI to watch PIM collection changes.",
        path,
        config_paths,
        sample,
        sample_url: SAMPLE_RAW_URL,
        next: "Edit it, keep one backend and the hooks you want, then check the account:",
        command: "check",
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::cli::welcome;

    #[test]
    #[cfg(not(windows))]
    fn the_welcome_downloads_the_raw_sample_then_checks() {
        let paths = [PathBuf::from("/nonexistent/carillon/config.toml")];
        let welcome = welcome(&paths[0], &paths, None).to_string();

        let curl = "curl -sSL --create-dirs -o '/nonexistent/carillon/config.toml' \
                    https://raw.githubusercontent.com/pimalaya/carillon/master/config.sample.toml";
        assert!(welcome.contains(curl), "{welcome}");
        let check = "\n  carillon -c '/nonexistent/carillon/config.toml' check\n";
        assert!(welcome.contains(check), "{welcome}");
    }

    #[test]
    #[cfg(not(windows))]
    fn the_welcome_copies_the_shipped_sample() {
        let paths = [PathBuf::from("/nonexistent/carillon/config.toml")];
        let sample = Path::new("/usr/share/doc/carillon/config.sample.toml");
        let welcome = welcome(&paths[0], &paths, Some(sample)).to_string();

        let copy = "install -m 644 '/usr/share/doc/carillon/config.sample.toml' \
                    '/nonexistent/carillon/config.toml'";
        assert!(welcome.contains(copy), "{welcome}");
        assert!(!welcome.contains("curl"), "{welcome}");
    }
}
