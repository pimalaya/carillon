//! # Carillon
//!
//! A CLI watching PIM accounts and firing local hooks on every change and
//! every alarm. One account is one thread, its alarms another; nothing is
//! stored between runs.
//!
//! ## Layout
//!
//! The frontend: main dispatches, cli declares the parser and meets a bare
//! invocation with the help, or with the missing configuration as any
//! command does, config parses the TOML accounts and fails naming the file
//! and the documented sample when there is none, watch and check are the
//! two commands, hook fires the notification and
//! the command, json_schema names the schema of what each command prints,
//! error the stable codes a failure carries, each backend reading its own
//! protocol's refusals into them.
//!
//! A command exits 0 on success, a watch stopped by a signal included, 1
//! on a failure, printed under `--json` as the failure object with its
//! code, 2 on a command line the grammar refuses (`usage-invalid`), and 3
//! on a `check` that printed its report with a failed backend.
//! No configuration is ever written: an account is written by hand from
//! config.sample.toml, which a person at a terminal missing one is
//! welcomed to by pimalaya-cli's welcome, a script or a JSON caller
//! getting the failure alone.
//!
//! watch carries the runtime: it selects the accounts, spawns one thread
//! each, and each thread opens the session its backend calls for,
//! forwards what it reports to the hooks, and reopens it after a capped
//! backoff, resolving the credential again on every attempt. What a lost
//! IMAP or Gmail session knew is handed to the next one, which reports
//! what moved meanwhile. stream holds what it prints: one event per thing
//! a session observed, then one result once a signal stopped it.
//!
//! The backends: imap, jmap, maildir, dav, msgraph, gmail, gcal, gpeople
//! and pimdir, each behind its cargo feature, learning about changes its
//! own way and reporting them in the one vocabulary event defines. How a
//! change is learned is the protocol crate's; what to do about it is
//! here. poll holds the sleep every polled backend waits on, and picture
//! what a REST change feed (a Graph delta link, a Google sync token) is
//! reconciled against.
//!
//! The dav module serves the two configured DAV backends, CalDAV and
//! CardDAV being one poll over collections that differ in what they hold.
//!
//! alarm fires the alarms of a calendar: CalDAV and pimdir read the bodies
//! of their members into its book, and a thread per account, living
//! across sessions, fires `on-alarm-trigger` when the wall clock reaches
//! a trigger. pimdir reads a local store and reports no change, its one
//! hook being the alarm's.
//!
//! The hooks live under their backend rather than on the account, since
//! which events exist and which variables a template can use are both the
//! backend's; what reaches hook is the one hook an event resolved to, so
//! the runner stays one shape.

// NOTE: a build enabling no backend reporting changes, none or pimdir
// alone, still compiles the change vocabulary and the hook runner that no
// backend then feeds. It watches no change, and gating every item of both
// on the backend features would cost more noise than the dead code it
// removes.
#![cfg_attr(
    not(backend),
    allow(dead_code, unused_imports, unused_mut, unused_variables)
)]

#[cfg(alarm)]
mod alarm;
mod backend;
mod check;
mod cli;
mod config;
#[cfg(feature = "dav")]
mod dav;
mod error;
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
#[cfg(feature = "pimdir")]
mod pimdir;
#[cfg(any(
    feature = "jmap",
    feature = "maildir",
    feature = "dav",
    feature = "pimdir",
    api
))]
mod poll;
#[cfg(all(test, api))]
mod scripted;
mod stream;
mod watch;

use std::{path::PathBuf, process};

use anyhow::Result;
use pimalaya_cli::{
    clap::{args::LogFlags, parse},
    error::{EXIT_FAILURE, ErrorReport, with_code},
    log::Logger,
    printer::StdoutPrinter,
    welcome::{is_wanted, shipped_sample},
};
use pimalaya_config::toml::ConfigMissingError;

use crate::{
    backend::Backend,
    cli::{Cli, Command},
    error::ErrorCode,
};

fn main() {
    let cli = parse::<Cli>();
    let mut printer = StdoutPrinter::new(&cli.json);

    // NOTE: a watch prints its events to a pipe too, a status bar reading
    // them as they come.
    if matches!(cli.command, Some(Command::Watch(_))) {
        printer = printer.streaming();
    }
    let config_paths = &cli.config.paths;
    let account_name = cli.account.name.as_deref();

    let result = execute(
        &mut printer,
        cli.command,
        &cli.log,
        config_paths,
        account_name,
        cli.backend,
    );

    // A person missing a configuration meets the welcome, a script or a
    // JSON caller the error.
    if let Err(err) = &result
        && let Some(missing) = err.downcast_ref::<ConfigMissingError>()
        && is_wanted(&printer)
    {
        let sample = shipped_sample("carillon");
        let welcome = cli::welcome(&missing.path, config_paths, sample.as_deref());
        eprint!("{welcome}");
        process::exit(EXIT_FAILURE);
    }

    ErrorReport::<ErrorCode>::eval_coded(
        &mut printer,
        result.map_err(|err| with_code(err, error::code)),
    );
}

/// Runs the parsed command, or meets a bare `carillon`.
fn execute(
    printer: &mut StdoutPrinter,
    command: Option<Command>,
    log: &LogFlags,
    config_paths: &[PathBuf],
    account_name: Option<&str>,
    backend: Backend,
) -> Result<()> {
    Logger::try_init(log)?;

    let Some(command) = command else {
        return cli::meet_bare_invocation(config_paths);
    };

    command.execute(printer, config_paths, account_name, backend)
}
