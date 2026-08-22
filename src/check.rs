//! `carillon check` command: opens the configured backend(s) so
//! credential and connectivity errors surface before the first real
//! `watch` run. Mirrors `himalaya account check`: each backend
//! allowed by `--backend` is tried in turn, and the result is
//! collected into a per-backend report.

#[cfg(feature = "dav")]
use std::sync::{Arc, atomic::AtomicBool};
use std::{fmt, path::PathBuf};

use anyhow::{Result, anyhow, bail};
use clap::Parser;
use pimalaya_cli::printer::Printer;
use pimalaya_config::toml::TomlConfig;
use serde::Serialize;

#[cfg(feature = "maildir")]
use crate::config::MaildirConfig;
use crate::{backend::Backend, config::Config};
#[cfg(feature = "dav")]
use crate::{config::DavServer, dav};
#[cfg(feature = "imap")]
use crate::{config::ImapConfig, imap};
#[cfg(feature = "jmap")]
use crate::{config::JmapConfig, jmap};

/// Validate the account configuration.
///
/// Loads the TOML configuration, picks the active account (via the
/// global `--account` flag or the default), and checks each backend
/// allowed by `--backend`. The check tries to instantiate a client
/// per backend, which exercises the same handshake / authentication
/// paths the other commands would take.
#[derive(Debug, Parser)]
pub struct CheckCommand;

impl CheckCommand {
    pub fn execute(
        self,
        printer: &mut impl Printer,
        config_paths: &[PathBuf],
        account_name: Option<&str>,
        backend: Backend,
    ) -> Result<()> {
        let mut config = Config::load(config_paths)?;

        let (name, account_config) = config
            .take_account(account_name)?
            .ok_or_else(|| anyhow!("Cannot find account"))?;

        let mut report = CheckReport {
            account: name,
            backends: Vec::new(),
        };

        #[cfg(feature = "imap")]
        if backend.allows_imap()
            && let Some(imap_config) = account_config.imap.clone()
        {
            report.backends.push(check_imap(imap_config));
        }

        #[cfg(feature = "jmap")]
        if backend.allows_jmap()
            && let Some(jmap_config) = account_config.jmap.clone()
        {
            report.backends.push(check_jmap(jmap_config));
        }

        #[cfg(feature = "maildir")]
        if backend.allows_maildir()
            && let Some(maildir_config) = account_config.maildir.clone()
        {
            report.backends.push(check_maildir(maildir_config));
        }

        #[cfg(feature = "dav")]
        if backend.allows_caldav()
            && let Some(caldav_config) = account_config.caldav.clone()
        {
            report.backends.push(check_dav(
                "caldav",
                caldav_config.server(),
                &caldav_config.calendar,
            ));
        }

        #[cfg(feature = "dav")]
        if backend.allows_carddav()
            && let Some(carddav_config) = account_config.carddav.clone()
        {
            report.backends.push(check_dav(
                "carddav",
                carddav_config.server(),
                &carddav_config.addressbook,
            ));
        }

        #[cfg(feature = "dav")]
        if backend.allows_dav()
            && let Some(dav_config) = account_config.dav.clone()
        {
            report.backends.push(check_dav(
                "dav",
                dav_config.server(),
                &dav_config.collection,
            ));
        }

        if report.backends.is_empty() {
            bail!("No backend matching `{backend}` is configured for this account");
        }

        printer.out(report)
    }
}

#[cfg(feature = "imap")]
fn check_imap(imap_config: ImapConfig) -> BackendCheck {
    // NOTE: opening the session is the check: it runs the same
    // transport, greeting and authentication a watch would.
    let result = imap::open(&imap_config).map(|_| ());

    BackendCheck::from("imap", result)
}

#[cfg(feature = "jmap")]
fn check_jmap(jmap_config: JmapConfig) -> BackendCheck {
    let result = jmap::open(&jmap_config).map(|_| ());

    BackendCheck::from("jmap", result)
}

#[cfg(feature = "maildir")]
fn check_maildir(maildir_config: MaildirConfig) -> BackendCheck {
    let result = (|| -> Result<()> {
        if !maildir_config.root.is_dir() {
            bail!(
                "Maildir root `{}` does not exist or is not a directory",
                maildir_config.root.display()
            );
        }
        Ok(())
    })();

    BackendCheck::from("maildir", result)
}

#[cfg(feature = "dav")]
fn check_dav(backend: &'static str, server: DavServer<'_>, collection: &str) -> BackendCheck {
    // NOTE: opening proves the transport, and one report proves the
    // credential and that the collection is really there, which is
    // what a watch would find out on its first poll.
    let shutdown = Arc::new(AtomicBool::new(false));
    let result = dav::probe(server, collection, &shutdown);

    BackendCheck::from(backend, result)
}

#[derive(Clone, Debug, Serialize)]
pub struct CheckReport {
    pub account: String,
    pub backends: Vec<BackendCheck>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BackendCheck {
    pub backend: &'static str,
    pub ok: bool,
    pub error: Option<String>,
}

impl BackendCheck {
    fn from(backend: &'static str, result: Result<()>) -> Self {
        match result {
            Ok(()) => Self {
                backend,
                ok: true,
                error: None,
            },
            Err(err) => Self {
                backend,
                ok: false,
                error: Some(format!("{err:#}")),
            },
        }
    }
}

impl fmt::Display for CheckReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Account: {}", self.account)?;
        for check in &self.backends {
            match &check.error {
                None => writeln!(f, "  {}: OK", check.backend)?,
                Some(err) => writeln!(f, "  {}: FAIL ({err})", check.backend)?,
            }
        }
        Ok(())
    }
}
