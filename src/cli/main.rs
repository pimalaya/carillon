// This file is part of Mirador, a CLI to watch mailbox changes.
//
// Copyright (C) 2024-2026  soywod <pimalaya.org@posteo.net>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Top-level CLI parser and subcommand dispatcher.

use std::path::PathBuf;

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use pimalaya_cli::{
    clap::{
        args::{AccountFlag, JsonFlag, LogFlags},
        commands::{CompletionCommand, ManualCommand},
        parsers::path_parser,
    },
    long_version,
    printer::Printer,
};

use crate::{
    backend::Backend,
    cli::{check::CheckCommand, watch::WatchCommand},
};

/// Top-level CLI: global flags and subcommand dispatch.
#[derive(Parser, Debug)]
#[command(name = env!("CARGO_PKG_NAME"))]
#[command(author, version, about)]
#[command(long_version = long_version!())]
#[command(propagate_version = true, infer_subcommands = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Override the default configuration file path.
    ///
    /// Paths are shell-expanded then canonicalized; multiple paths
    /// may be delimited by `:` and are merged left-to-right.
    #[arg(short, long = "config", global = true, env = "MIRADOR_CONFIG")]
    #[arg(value_name = "PATH", value_parser = path_parser, value_delimiter = ':')]
    pub config_paths: Vec<PathBuf>,

    #[command(flatten)]
    pub account: AccountFlag,

    /// Force a specific backend.
    ///
    /// Mirador only opens one connection per `watch`, so when more
    /// than one of `imap`, `jmap`, `maildir` is declared on the
    /// account this flag picks which one is used. With `auto`
    /// (default), the priority order is IMAP, then JMAP, then
    /// Maildir; the first configured block wins.
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
    /// Generate man pages into the given directory.
    #[command(arg_required_else_help = true)]
    Manuals(ManualCommand),
    /// Generate shell completion scripts into the given directory.
    #[command(arg_required_else_help = true)]
    Completions(CompletionCommand),
}

impl Command {
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
            Self::Manuals(cmd) => cmd.execute(printer, Cli::command()),
            Self::Completions(cmd) => cmd.execute(printer, Cli::command()),
        }
    }
}
