//! `mirador watch` command: opens the configured backend, watches
//! the chosen mailbox via the new
//! [`io_email::client::EmailClientStd::watch_mailbox`] shared API,
//! and fires per-event hooks until Ctrl+C.
//!
//! Threading model: io-email's `watch_mailbox` is **blocking**, so
//! we spawn it on a dedicated thread and stream events back through
//! an `std::sync::mpsc` channel. The main thread reads the channel,
//! dispatches hooks, and watches the shutdown flag.

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, RecvTimeoutError},
    },
    thread,
    time::Duration,
};

use anyhow::{Result, bail};
use clap::Parser;
use io_email::envelope::event::WatchEvent;
use log::{info, trace};
use pimalaya_cli::printer::Printer;
use pimalaya_config::toml::TomlConfig;

use crate::{
    backend::Backend,
    client,
    config::Config,
    config::HooksConfig,
    hook::{FlagsContext, MessageContext, run_flags_hook, run_message_hook},
};

const POLL_TICK: Duration = Duration::from_millis(500);

/// Watch a mailbox and execute hooks on every change.
#[derive(Debug, Parser)]
pub struct WatchCommand {
    /// Mailbox to watch. Overrides the account's `mailbox` setting;
    /// falls back to `INBOX` when neither is provided.
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

        let Some((name, account)) = config.take_account(account_name)? else {
            bail!("Cannot find account");
        };

        let mailbox = self
            .mailbox
            .or(account.mailbox.clone())
            .unwrap_or_else(|| "INBOX".into());

        let hooks = account.hooks.clone();

        info!("watching mailbox `{mailbox}` on account `{name}`");

        let mut client = client::open(account, backend)?;

        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_for_ctrlc = shutdown.clone();
        ctrlc::set_handler(move || {
            println!("Received SIGINT, shutting down the watcher…");
            shutdown_for_ctrlc.store(true, Ordering::SeqCst)
        })?;

        let (tx, rx) = mpsc::channel();

        // NOTE: spawn the blocking watch_mailbox on its own thread;
        // main thread consumes events through `rx`.
        let mailbox_for_worker = mailbox.clone();
        let shutdown_for_worker = shutdown.clone();
        let worker = thread::spawn(move || -> Result<()> {
            client.watch_mailbox(&mailbox_for_worker, shutdown_for_worker, tx)?;
            Ok(())
        });

        println!("Watching `{mailbox}` on account `{name}` (press Ctrl+C to exit)");

        loop {
            if shutdown.load(Ordering::SeqCst) {
                break;
            }

            match rx.recv_timeout(POLL_TICK) {
                Ok(event) => dispatch_event(&hooks, &event),
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }

        // NOTE: wait for the worker to wind down cleanly. It either
        // observed `shutdown` and returned `Ok(())`, or it errored
        // out and we surface that to the caller.
        match worker.join() {
            Ok(Ok(())) => Ok(()),
            Ok(Err(err)) => Err(err),
            Err(_) => bail!("Watch worker panicked"),
        }
    }
}

fn dispatch_event(hooks: &HooksConfig, event: &WatchEvent) {
    trace!("dispatch event: {event:?}");

    match event {
        WatchEvent::EnvelopeAdded { mailbox, envelope } => {
            if let Some(hook) = &hooks.on_message_added {
                run_message_hook(
                    hook,
                    &MessageContext {
                        mailbox,
                        id: &envelope.id,
                        envelope: Some(envelope),
                    },
                );
            }
        }
        WatchEvent::EnvelopeRemoved { mailbox, id } => {
            if let Some(hook) = &hooks.on_message_removed {
                run_message_hook(
                    hook,
                    &MessageContext {
                        mailbox,
                        id,
                        envelope: None,
                    },
                );
            }
        }
        WatchEvent::FlagsAdded { mailbox, id, flags } => {
            if let Some(hook) = &hooks.on_flags_added {
                run_flags_hook(hook, &FlagsContext { mailbox, id, flags });
            }
        }
        WatchEvent::FlagsRemoved { mailbox, id, flags } => {
            if let Some(hook) = &hooks.on_flags_removed {
                run_flags_hook(hook, &FlagsContext { mailbox, id, flags });
            }
        }
        WatchEvent::KeepAlive => {}
    }
}
