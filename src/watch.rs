//! `mirador watch` command: watches accounts and fires their hooks on
//! every change until Ctrl+C.
//!
//! Bare `watch` watches every configured account at once, one thread
//! each under a single shared shutdown; `-a/--account` narrows it to
//! one. Each account's mailbox comes from its own config, so accounts
//! watching different mailboxes need no flag; `-m/--mailbox` overrides
//! it when a single account is watched.

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
    backend::Backend,
    config::{AccountConfig, Config},
    driver,
};

/// The mailbox watched when neither the flag nor the account says.
const DEFAULT_MAILBOX: &str = "INBOX";

/// Watch accounts and fire their hooks on every change.
#[derive(Debug, Parser)]
pub struct WatchCommand {
    /// Mailbox to watch. Overrides the account's `mailbox` setting;
    /// falls back to `INBOX` when neither is provided. Only valid when
    /// a single account is watched, since accounts watch their own.
    #[arg(long, short)]
    pub mailbox: Option<String>,
}

impl WatchCommand {
    pub fn execute(
        self,
        _printer: &mut impl Printer,
        config_paths: &[PathBuf],
        account_name: Option<&str>,
        backend: Backend,
    ) -> Result<()> {
        let mut config = Config::load(config_paths)?;
        let selected = select_accounts(&mut config, account_name)?;

        if self.mailbox.is_some() && selected.len() > 1 {
            bail!("--mailbox needs a single account; narrow with --account");
        }

        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_for_ctrlc = shutdown.clone();
        ctrlc::set_handler(move || {
            println!("received Ctrl+C, shutting down the watcher…");
            shutdown_for_ctrlc.store(true, Ordering::SeqCst);
        })?;

        let mut handles = Vec::with_capacity(selected.len());

        for (name, account) in selected {
            let mailbox = self
                .mailbox
                .clone()
                .or_else(|| account.mailbox.clone())
                .unwrap_or_else(|| String::from(DEFAULT_MAILBOX));
            let shutdown = shutdown.clone();

            info!("watching `{mailbox}` on account `{name}`");

            let handle = thread::Builder::new().name(name.clone()).spawn(move || {
                if let Err(err) = driver::run(&name, account, mailbox, backend, shutdown) {
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

/// Selects the accounts to watch: the named one, or every configured
/// account when no name is given.
fn select_accounts(
    config: &mut Config,
    account_name: Option<&str>,
) -> Result<Vec<(String, AccountConfig)>> {
    if let Some(name) = account_name {
        let Some(entry) = config.accounts.remove_entry(name) else {
            bail!("account `{name}` not found in config");
        };

        return Ok(vec![entry]);
    }

    if config.accounts.is_empty() {
        bail!("no accounts configured");
    }

    let mut all: Vec<_> = config.accounts.drain().collect();
    all.sort_by(|(left, _), (right, _)| left.cmp(right));

    Ok(all)
}
