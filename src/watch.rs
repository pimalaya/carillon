//! `carillon watch` command: watches accounts and fires their hooks on
//! every change until Ctrl+C.
//!
//! Bare `watch` watches every configured account at once, one thread
//! each under a single shared shutdown; `-a/--account` narrows it to
//! one. What an account watches is its own `collection`, so there is
//! nothing to override on the command line.

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
    cli::load_config,
    config::{AccountConfig, Config},
    driver,
};

/// Watch accounts and fire their hooks on every change.
///
/// What each account watches is its own `collection`, so there is no
/// flag to override it: watching something else is another account.
#[derive(Debug, Parser)]
pub struct WatchCommand;

impl WatchCommand {
    pub fn execute(
        self,
        printer: &mut impl Printer,
        config_paths: &[PathBuf],
        account_name: Option<&str>,
        backend: Backend,
    ) -> Result<()> {
        let mut config = load_config(printer, config_paths)?;
        let selected = select_accounts(&mut config, account_name)?;

        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_for_ctrlc = shutdown.clone();
        ctrlc::set_handler(move || {
            println!("received Ctrl+C, shutting down the watcher…");
            shutdown_for_ctrlc.store(true, Ordering::SeqCst);
        })?;

        let mut handles = Vec::with_capacity(selected.len());

        for (name, account) in selected {
            let shutdown = shutdown.clone();

            let handle = thread::Builder::new().name(name.clone()).spawn(move || {
                if let Err(err) = driver::run(&name, account, backend, shutdown) {
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
