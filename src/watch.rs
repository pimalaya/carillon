//! `carillon watch` command: watches accounts and fires their hooks on every
//! change until Ctrl+C. Bare `watch` watches every configured account at
//! once, one thread each; a cumulative `-a/--account` narrows to a subset.

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

use anyhow::{Result, bail};
use clap::Parser;
use log::{error, info};
use pimalaya_cli::printer::Printer;

use crate::{
    config::{AccountConfig, Config},
    driver,
};

/// Watch accounts and fire their hooks on every change.
#[derive(Debug, Parser)]
pub struct WatchCommand {
    /// Accounts to watch, by name. Watches every configured account when none
    /// are given. Each account's mailbox comes from its own config.
    #[arg(value_name = "ACCOUNT")]
    pub accounts: Vec<String>,
}

impl WatchCommand {
    pub fn execute(self, _printer: &mut impl Printer, config_paths: &[PathBuf]) -> Result<()> {
        let mut config = Config::load(config_paths)?;
        let selected = select_accounts(&mut config, &self.accounts)?;

        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_for_ctrlc = shutdown.clone();
        ctrlc::set_handler(move || {
            println!("received Ctrl+C, shutting down the watcher…");
            shutdown_for_ctrlc.store(true, Ordering::SeqCst);
        })?;

        let mut handles = Vec::with_capacity(selected.len());
        for (name, account) in selected {
            let mailbox = account
                .mailbox
                .clone()
                .unwrap_or_else(|| String::from("INBOX"));
            let shutdown = shutdown.clone();
            info!("watching `{mailbox}` on account `{name}`");
            let handle = thread::Builder::new().name(name.clone()).spawn(move || {
                if let Err(err) = driver::run(&name, account.imap, mailbox, account.hook, shutdown)
                {
                    error!("[{name}] watch stopped: {err:#}");
                }
            })?;
            handles.push(handle);
        }

        info!("press Ctrl+C to exit");
        for handle in handles {
            let _ = handle.join();
        }
        Ok(())
    }
}

/// Selects the accounts to watch: the named subset when `names` is non-empty
/// (erroring on any unknown name), otherwise every configured account.
fn select_accounts(config: &mut Config, names: &[String]) -> Result<Vec<(String, AccountConfig)>> {
    if names.is_empty() {
        if config.accounts.is_empty() {
            bail!("no accounts configured");
        }
        let mut all: Vec<_> = config.accounts.drain().collect();
        all.sort_by(|(left, _), (right, _)| left.cmp(right));
        return Ok(all);
    }

    let mut selected = Vec::with_capacity(names.len());
    for name in names {
        match config.accounts.remove_entry(name) {
            Some(entry) => selected.push(entry),
            None => bail!("account `{name}` not found in config"),
        }
    }
    Ok(selected)
}
