//! # carillon
//!
//! Watches PIM accounts and fires local hooks on every change. One
//! account is one thread; nothing is stored between runs.
//!
//! ## Layout
//!
//! The frontend: main dispatches, cli declares the parser and meets
//! the invocations that find no configuration, config parses the TOML
//! accounts and renders a generated one back, watch, check and
//! configure are the three commands, driver supervises one account
//! (backend selection, reconnect backoff, credential per attempt),
//! hook fires the notification and the command.
//!
//! The wizard: one prompt takes an email address, io-pim-discovery
//! turns it into the services reachable from it, and the module of the
//! chosen backend prompts its credential and opens the connection.
//! That connection is both the test and what the account is completed
//! from: the collection a DAV server holds, and the method its server
//! can actually be watched with. What is done with the account, a file
//! to create, a block to append or a document on stdout, is configure's.
//!
//! The backends: imap, jmap, maildir and dav, each behind its cargo
//! feature, each learning about changes its own way and reporting them
//! in the one vocabulary event defines, together with whatever it
//! already knows about the change. How a change is learned is the
//! protocol crate's job (io-imap, io-jmap, io-maildir, io-webdav);
//! what to do about it is this crate's. The dav module serves the two
//! configured DAV backends, CalDAV and CardDAV being one poll over
//! collections that differ in what they hold.
//!
//! The hooks live under their backend rather than on the account,
//! since which events exist and which variables a template can use are
//! both the backend's; what reaches hook is the one hook an event
//! resolved to, so the runner stays one shape.

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
mod wizard;

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
    cli.execute(printer)
}
