//! # Parser
//!
//! The top-level CLI parser and subcommand dispatcher.
//!
//! Nothing works without a configuration, so the two invocations that can
//! find none, a bare `carillon` and a command needing an account, both
//! offer to generate one rather than failing on it.

use std::{
    io::{IsTerminal, stdin},
    path::{Path, PathBuf},
};

use anyhow::{Result, bail};
use clap::{CommandFactory, Parser, Subcommand};
use pimalaya_cli::{
    clap::{
        args::{AccountFlag, JsonFlag, LogFlags},
        commands::{CompletionCommand, JsonSchemaCommand, ManualCommand},
        parsers::path_parser,
    },
    footer, long_version,
    printer::Printer,
    prompt,
};
use pimalaya_config::toml::TomlConfig;

use crate::{
    backend::Backend,
    check::CheckCommand,
    config::{CONFIG_SAMPLE_URL, Config},
    json_schema,
    watch::WatchCommand,
    wizard::{self, configure::ConfigureCommand},
};

/// Top-level CLI: global flags and subcommand dispatch.
#[derive(Parser, Debug)]
#[command(name = env!("CARGO_PKG_NAME"))]
#[command(author, version, about)]
#[command(long_about = concat!(
    "CLI to watch PIM collection changes.\n\n",
    "First time here? Run `carillon` with no command: it offers to generate an ",
    "account discovered from your email address, which `carillon configure` does ",
    "again later. Everything discovery does not cover is written by hand.",
))]
#[command(long_version = long_version!())]
#[command(after_help = footer!())]
#[command(propagate_version = true, infer_subcommands = true)]
pub struct Cli {
    /// The command to run.
    ///
    /// Omitted, a bare `carillon` offers to generate a configuration when
    /// it finds none, and shows this help otherwise.
    #[command(subcommand)]
    pub command: Option<Command>,
    /// Override the default configuration file path.
    ///
    /// Paths are shell-expanded then canonicalized; multiple paths may be
    /// delimited by `:` and are merged left-to-right.
    #[arg(short, long = "config", global = true, env = "CARILLON_CONFIG")]
    #[arg(value_name = "PATH", value_parser = path_parser, value_delimiter = ':')]
    pub config_paths: Vec<PathBuf>,
    #[command(flatten)]
    pub account: AccountFlag,
    /// Force a specific backend.
    ///
    /// One connection is opened per `watch`, so this picks the block an
    /// account declaring several is watched over. `auto`, the default,
    /// takes the first configured, in the order IMAP, JMAP, Maildir,
    /// CalDAV, CardDAV.
    #[arg(short, long, global = true, default_value_t)]
    pub backend: Backend,
    #[command(flatten)]
    pub json: JsonFlag,
    #[command(flatten)]
    pub log: LogFlags,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Watch a mailbox and fire hooks on every change until Ctrl+C.
    Watch(WatchCommand),
    /// Validate the account configuration against each allowed backend.
    Check(CheckCommand),
    /// Configure an account interactively.
    #[command(visible_alias = "wizard")]
    Configure(ConfigureCommand),
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

impl Cli {
    /// Runs the parsed command, or meets a bare invocation.
    ///
    /// A missing configuration raises the offer, anything else the help,
    /// which is also what a script or a JSON caller gets. A broken file
    /// counts as a configuration, so the offer never writes over one.
    pub fn execute(self, printer: &mut impl Printer) -> Result<()> {
        let config_paths = self.config_paths.as_ref();
        let account_name = self.account.name.as_deref();

        let Some(command) = self.command else {
            let configured = Config::from_paths_or_default(config_paths)
                .ok()
                .flatten()
                .is_some();

            if !configured && !printer.is_json() && stdin().is_terminal() {
                let path = Config::target_path(config_paths)?;

                // NOTE: a bare invocation has nothing to run after the
                // offer, so a declined one falls back to the help.
                if offer_configuration(printer, config_paths, &path)? {
                    return Ok(());
                }
            }

            Cli::command().print_help()?;

            return Ok(());
        };

        command.execute(printer, config_paths, account_name, self.backend)
    }
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
            Self::Configure(cmd) => cmd.execute(printer, config_paths),
            Self::Manual(cmd) => cmd.execute(printer, Cli::command()),
            Self::Completion(cmd) => cmd.execute(printer, Cli::command()),
            Self::JsonSchema(cmd) => cmd.execute(printer, json_schema::schemas()),
        }
    }
}

/// Loads the configuration a command runs against.
///
/// A missing configuration is met with the wizard rather than an error:
/// accepting gives the command a chance to work, declining leaves it to
/// fail on the configuration it still has not got.
pub fn load_config(printer: &mut impl Printer, config_paths: &[PathBuf]) -> Result<Config> {
    if let Some(config) = Config::load(config_paths)? {
        return Ok(config);
    }

    // NOTE: the target path is where `-c` pointed, so a mistyped path
    // shows up as itself rather than as a generic first run.
    let path = Config::target_path(config_paths)?;

    // NOTE: nobody answers a prompt in a script or a systemd unit, and a
    // JSON consumer wants a failure it can read, so both fail below.
    if !printer.is_json() && stdin().is_terminal() {
        offer_configuration(printer, config_paths, &path)?;
    }

    // NOTE: the wizard also prints the account instead of writing it, so
    // having run it proves nothing: the configuration is looked up again.
    match Config::load(config_paths)? {
        Some(config) => Ok(config),
        None => bail!(
            "No configuration found at {}, run `carillon configure` to generate one or write it by hand: {CONFIG_SAMPLE_URL}",
            path.display(),
        ),
    }
}

/// Welcomes, then offers a first configuration, saying whether it ran.
///
/// Raised from the two places nothing can happen without a configuration:
/// a bare invocation, and a command needing an account. It is a hook
/// rather than a gate, so what a decline leads to is the caller's.
fn offer_configuration(
    printer: &mut impl Printer,
    config_paths: &[PathBuf],
    path: &Path,
) -> Result<bool> {
    wizard::configure::print_welcome(path);

    if !prompt::bool("Create a configuration with a default account?", true)? {
        return Ok(false);
    }

    ConfigureCommand.execute(printer, config_paths)?;

    Ok(true)
}
