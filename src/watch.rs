//! `carillon watch` command: opens the account's IMAP connection, drives
//! carillon-core's watch coroutine over a blocking socket, and fires the
//! account hook on every change until Ctrl+C.

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use anyhow::{Result, bail};
use clap::Parser;
use log::info;
use pimalaya_cli::printer::Printer;
use pimalaya_config::toml::TomlConfig;

use crate::{config::Config, driver};

/// Watch a mailbox and fire the account hook on every change.
#[derive(Debug, Parser)]
pub struct WatchCommand {
    /// Mailbox to watch. Overrides the account's `mailbox`; falls back to
    /// INBOX when neither is set.
    #[arg(long, short)]
    pub mailbox: Option<String>,
}

impl WatchCommand {
    pub fn execute(
        self,
        _printer: &mut impl Printer,
        config_paths: &[PathBuf],
        account_name: Option<&str>,
    ) -> Result<()> {
        let mut config = Config::load(config_paths)?;

        let Some((name, account)) = config.take_account(account_name)? else {
            bail!("cannot find account");
        };

        let mailbox = self
            .mailbox
            .or(account.mailbox.clone())
            .unwrap_or_else(|| String::from("INBOX"));

        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_for_ctrlc = shutdown.clone();
        ctrlc::set_handler(move || {
            println!("received Ctrl+C, shutting down the watcher…");
            shutdown_for_ctrlc.store(true, Ordering::SeqCst);
        })?;

        info!("watching `{mailbox}` on account `{name}` (press Ctrl+C to exit)");
        driver::run(&name, account.imap, mailbox, account.hook, shutdown)
    }
}
