//! # carillon
//!
//! A self-hostable daemon that watches PIM accounts and fires local hooks
//! on each change. It is the lightweight frontend of Carillon: it hosts
//! carillon-core's I/O-free watch coroutine and drives it with a blocking
//! socket, one thread per watch, without the server's network-and-trust
//! apparatus.
//!
//! ## Shape
//!
//! Standard Pimalaya CLI layout. main parses the CLI and dispatches; cli
//! declares the parser and subcommands; config parses the TOML accounts;
//! watch is the `watch` command; driver is the blocking pump over
//! carillon-core's coroutine; resolve fetches new-message envelopes locally
//! to enrich notifications; net is the shared TLS connect; hook runs the
//! desktop notification and shell command. The daemon hosts only outbound
//! sources, so it never needs a public endpoint.

mod cli;
mod config;
mod driver;
mod hook;
mod net;
mod resolve;
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
    cli.command.execute(printer, config_paths)
}
