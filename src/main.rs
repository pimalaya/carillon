//! # Carillon
//!
//! A CLI watching PIM accounts and firing local hooks on every change.
//! One account is one thread; nothing is stored between runs.
//!
//! ## Layout
//!
//! The frontend: main dispatches, cli declares the parser and meets the
//! invocations that find no configuration, config parses the TOML
//! accounts and renders a generated one back, watch, check and configure
//! are the three commands, hook fires the notification and the command,
//! json_schema names the schema of what each command prints.
//!
//! watch carries the runtime: it selects the accounts, spawns one thread
//! each, and each thread opens the session its backend calls for,
//! forwards what it reports to the hooks, and reopens it after a capped
//! backoff, resolving the credential again on every attempt.
//!
//! The wizard: one prompt takes an email address, io-pim-discovery turns
//! it into the services reachable from it, and the module of the chosen
//! backend prompts its credential and opens the connection.
//!
//! That connection is both the test and what the account is completed
//! from: the collection a DAV server holds, and the method its server can
//! actually be watched with. What is done with the account, a file to
//! create, a block to append or a document on stdout, is configure's.
//!
//! The backends: imap, jmap, maildir, dav, msgraph, gmail, gcal and
//! gpeople, each behind its cargo feature, learning about changes its own
//! way and reporting them in the one vocabulary event defines. How a
//! change is learned is the protocol crate's; what to do about it is
//! here. poll holds the sleep every polled backend waits on, and picture
//! what a REST change feed (a Graph delta link, a Google sync token) is
//! reconciled against.
//!
//! The dav module serves the two configured DAV backends, CalDAV and
//! CardDAV being one poll over collections that differ in what they hold.
//!
//! The hooks live under their backend rather than on the account, since
//! which events exist and which variables a template can use are both the
//! backend's; what reaches hook is the one hook an event resolved to, so
//! the runner stays one shape.

// NOTE: a build enabling no backend still compiles the change vocabulary
// and the hook runner that no backend then feeds. It watches nothing, and
// gating every item of both on the four backend features would cost more
// noise than the dead code it removes.
#![cfg_attr(
    not(backend),
    allow(dead_code, unused_imports, unused_mut, unused_variables)
)]

mod backend;
mod check;
mod cli;
mod config;
#[cfg(feature = "dav")]
mod dav;
mod event;
#[cfg(feature = "gcal")]
mod gcal;
#[cfg(feature = "gmail")]
mod gmail;
#[cfg(feature = "gpeople")]
mod gpeople;
mod hook;
#[cfg(feature = "imap")]
mod imap;
#[cfg(feature = "jmap")]
mod jmap;
mod json_schema;
#[cfg(feature = "maildir")]
mod maildir;
#[cfg(feature = "msgraph")]
mod msgraph;
#[cfg(any(feature = "msgraph", feature = "gcal", feature = "gpeople"))]
mod picture;
#[cfg(any(feature = "jmap", feature = "maildir", feature = "dav", api))]
mod poll;
mod watch;
#[cfg(feature = "wizard")]
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
