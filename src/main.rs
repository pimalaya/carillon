//! # carillon
//!
//! Watches PIM accounts and fires local hooks on every change. One
//! account is one thread; nothing is stored between runs.
//!
//! ## Layout
//!
//! The frontend: main dispatches, cli declares the parser, config
//! parses the TOML accounts, watch and check are the two commands,
//! driver supervises one account (backend selection, reconnect
//! backoff, credential per attempt), hook fires the notification and
//! the command.
//!
//! The backends: imap, jmap, maildir and dav, each behind its cargo
//! feature, each learning about changes its own way and reporting them
//! in the one vocabulary event defines. How a change is learned is the
//! protocol crate's job (io-imap, io-jmap, io-maildir, io-webdav);
//! what to do about it is this crate's.

mod backend;
mod check;
mod cli;
mod config;
#[cfg(feature = "dav")]
mod dav;
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
