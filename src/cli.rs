//! # Parser
//!
//! The top-level CLI parser and subcommand dispatcher, what a bare
//! `carillon` meets, and the welcome a person missing a configuration
//! meets.

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
use pimalaya_config::toml::ConfigMissingError;

use crate::{
    backend::Backend, check::CheckCommand, config::Config, json_schema, watch::WatchCommand,
};

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
    /// The command to run, a bare `carillon` showing this help.
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
    #[command(alias = "completions")]
    Completion(CompletionCommand),
    #[command(alias = "manuals")]
    Manual(ManualCommand),
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
            Self::Completion(cmd) => cmd.execute(printer, Cli::command()),
            Self::Manual(cmd) => cmd.execute(printer, Cli::command()),
            Self::JsonSchema(cmd) => cmd.execute(printer, json_schema::registry()),
        }
    }
}

/// Meets a bare `carillon`: the help when a configuration exists, the
/// missing configuration otherwise, which a person meets as the welcome.
pub fn meet_bare_invocation(config_paths: &[PathBuf]) -> Result<()> {
    // NOTE: a broken file counts as a configuration, its error left to
    // the first command reading it.
    if let Err(err) = Config::load(config_paths)
        && err.is::<ConfigMissingError>()
    {
        return Err(err);
    }

    Cli::command().print_help()?;
    Ok(())
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

    use clap::CommandFactory;

    use crate::cli::{Cli, welcome};

    /// The standard commands come in himalaya's order, with the abouts
    /// pimalaya-cli gives them in every binary of the family.
    #[test]
    fn the_standard_commands_are_the_familys() {
        let command = Cli::command();
        let names: Vec<_> = command.get_subcommands().map(|c| c.get_name()).collect();
        let about = |name: &str| {
            let subcommand = command.find_subcommand(name).unwrap();
            subcommand.get_about().unwrap().to_string()
        };

        let standard = ["completion", "manual", "json-schema"];
        assert_eq!(standard, names[2..]);
        assert_eq!(
            "Generate completion script for the given shell(s)",
            about("completion")
        );
        assert_eq!(
            "Generate manual page for the given command(s)",
            about("manual")
        );
        assert_eq!(
            "Generate JSON Schema of the given command(s) JSON output",
            about("json-schema")
        );
    }

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
