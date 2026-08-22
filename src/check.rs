//! `mirador check` command: opens the configured backend(s) so
//! credential and connectivity errors surface before the first real
//! `watch` run. Mirrors `himalaya account check`: each backend
//! allowed by `--backend` is tried in turn, and the result is
//! collected into a per-backend report.

use std::{fmt, path::PathBuf};

use anyhow::{Result, bail};
use clap::Parser;
use pimalaya_config::toml::TomlConfig;
use serde::Serialize;

use crate::{
    backend::Backend,
    config::{AccountConfig, Config},
};

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
        printer: &mut impl pimalaya_cli::printer::Printer,
        config_paths: &[PathBuf],
        account_name: Option<&str>,
        backend: Backend,
    ) -> Result<()> {
        let mut config = Config::load(config_paths)?;

        let (name, account_config) = config
            .take_account(account_name)?
            .ok_or_else(|| anyhow::anyhow!("Cannot find account"))?;

        let mut report = CheckReport {
            account: name,
            backends: Vec::new(),
        };

        #[cfg(feature = "imap")]
        if backend.allows_imap()
            && let Some(imap_config) = account_config.imap.clone()
        {
            report
                .backends
                .push(check_imap(&account_config, imap_config));
        }

        #[cfg(feature = "jmap")]
        if backend.allows_jmap()
            && let Some(jmap_config) = account_config.jmap.clone()
        {
            report
                .backends
                .push(check_jmap(&account_config, jmap_config));
        }

        #[cfg(feature = "maildir")]
        if backend.allows_maildir()
            && let Some(maildir_config) = account_config.maildir.clone()
        {
            report
                .backends
                .push(check_maildir(&account_config, maildir_config));
        }

        #[cfg(feature = "dav")]
        if backend.allows_dav()
            && let Some(dav_config) = account_config.dav.clone()
        {
            report.backends.push(check_dav(&account_config, dav_config));
        }

        if report.backends.is_empty() {
            bail!("No backend matching `{backend}` is configured for this account");
        }

        printer.out(report)
    }
}

#[cfg(feature = "imap")]
fn check_imap(
    _account_config: &AccountConfig,
    imap_config: crate::config::ImapConfig,
) -> BackendCheck {
    // NOTE: opening the session is the check: it runs the same
    // transport, greeting and authentication a watch would.
    let result = crate::imap::open(&imap_config).map(|_| ());

    BackendCheck::from("imap", result)
}

#[cfg(feature = "jmap")]
fn check_jmap(
    _account_config: &AccountConfig,
    jmap_config: crate::config::JmapConfig,
) -> BackendCheck {
    let result = crate::jmap::open(&jmap_config).map(|_| ());

    BackendCheck::from("jmap", result)
}

#[cfg(feature = "maildir")]
fn check_maildir(
    _account_config: &AccountConfig,
    maildir_config: crate::config::MaildirConfig,
) -> BackendCheck {
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
fn check_dav(account_config: &AccountConfig, dav_config: crate::config::DavConfig) -> BackendCheck {
    // NOTE: opening proves the transport, and one report proves the
    // credential and that the collection is really there, which is
    // what a watch would find out on its first poll.
    let shutdown = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let result = crate::dav::probe(&dav_config, &account_config.collection, &shutdown);

    BackendCheck::from("dav", result)
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
