//! # carillon
//!
//! A self-hostable daemon that watches PIM accounts and fires local hooks
//! on each change. It is the lightweight frontend of Carillon: it drives an
//! I/O-free watch coroutine with a blocking socket, one thread per watch,
//! without the server's network-and-trust apparatus.
//!
//! ## Shape
//!
//! Standard Pimalaya CLI layout, in two halves.
//!
//! The watch itself is I/O-free: imap is the coroutine (greet,
//! authenticate, EXAMINE, IDLE, re-EXAMINE, ring when the state token
//! advanced), backend the source it dials, credential the resolved secret
//! it presents, event the content-free ring it produces, error its failure
//! type. Nothing there touches a socket, which is what keeps a future
//! async driver a driver rather than a second watcher.
//!
//! The frontend brings the I/O: main parses the CLI and dispatches; cli
//! declares the parser and subcommands; config parses the TOML accounts;
//! watch is the `watch` command; driver is the blocking pump, owning
//! transport, reconnect and credential resolution; resolve fetches
//! new-message envelopes on its own connection to enrich notifications;
//! net is the shared TLS connect; hook runs the desktop notification and
//! the shell command.
//!
//! The daemon hosts only outbound sources, so it never needs a public
//! endpoint.

mod backend;
mod cli;
mod config;
mod credential;
mod driver;
mod error;
mod event;
mod hook;
mod imap;
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
