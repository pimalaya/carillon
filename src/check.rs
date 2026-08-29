//! # Check
//!
//! The `carillon check` command, opening the backends an account declares
//! so a credential or connectivity error surfaces before the first watch.
//!
//! Mirrors `himalaya account check`: each backend `--backend` allows is
//! tried in turn, and the result collected into a per-backend report.

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
use crate::{backend::Backend, cli::load_config};
#[cfg(feature = "dav")]
use crate::{config::DavServer, dav};
#[cfg(feature = "imap")]
use crate::{config::ImapConfig, imap};
#[cfg(feature = "jmap")]
use crate::{config::JmapConfig, jmap};

/// Validate the account configuration.
///
/// Every backend `--backend` allows on the account, `-a` naming it or the
/// default one, is opened the way a watch would, so a bad credential or
/// an unreachable server is reported here rather than at the first change.
#[derive(Debug, Parser)]
pub struct CheckCommand;

impl CheckCommand {
    /// Opens each allowed backend of the account and reports on it.
    pub fn execute(
        self,
        printer: &mut impl Printer,
        config_paths: &[PathBuf],
        account_name: Option<&str>,
        backend: Backend,
    ) -> Result<()> {
        let mut config = load_config(printer, config_paths)?;

        let (name, account_config) = config.take_account(account_name)?.ok_or_else(|| {
            anyhow!(
                "No default account found, name one with `-a <NAME>` or mark one with `default = true`"
            )
        })?;

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
    // NOTE: opening proves the transport, and one report the credential
    // and that the collection is there, as a first poll would.
    let shutdown = Arc::new(AtomicBool::new(false));
    let result = dav::probe(server, collection, &shutdown);

    BackendCheck::from(backend, result)
}

/// What `check` reports: one account, and one line per backend tried.
#[derive(Clone, Debug, Serialize)]
pub struct CheckReport {
    /// The account that was checked.
    pub account: String,
    /// One entry per backend `--backend` allowed on it.
    pub backends: Vec<BackendCheck>,
}

/// The outcome of opening one backend.
#[derive(Clone, Debug, Serialize)]
pub struct BackendCheck {
    /// The backend name, as the configuration spells it.
    pub backend: &'static str,
    /// Whether it opened.
    pub ok: bool,
    /// Why it did not, rendered with its causes.
    pub error: Option<String>,
}

impl BackendCheck {
    /// Folds one backend's outcome into its report entry.
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
