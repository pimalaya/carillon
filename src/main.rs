//! Binary entry point: parses the CLI, configures logging and dispatches
//! the requested subcommand.

mod backend;
mod check;
mod cli;
mod config;
mod driver;
mod event;
mod hook;
#[cfg(feature = "imap")]
mod imap;
#[cfg(feature = "jmap")]
mod jmap;
#[cfg(feature = "maildir")]
mod maildir;
mod watch;

use anyhow::Result;
use clap::Parser;
use pimalaya_cli::{error::ErrorReport, log::Logger, printer::StdoutPrinter};

use crate::cli::Cli;

fn main() {
    let cli = Cli::parse();
    let mut printer = StdoutPrinter::new(&cli.json);
    let result = execute(&mut printer, cli);
    ErrorReport::eval(&mut printer, result);
}

fn execute(printer: &mut StdoutPrinter, cli: Cli) -> Result<()> {
    Logger::try_init(&cli.log)?;
    let config_paths = cli.config_paths.as_ref();
    let account_name = cli.account.name.as_deref();
    cli.command
        .execute(printer, config_paths, account_name, cli.backend)
}
