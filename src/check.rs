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
    use io_imap::client::ImapClientStd;
    use pimalaya_stream::{sasl::Sasl, tls::Tls};

    use crate::config::resolve_auto_id_params;

    let result = (|| -> Result<()> {
        let mut tls: Tls = imap_config.tls.clone().into();
        tls.rustls.alpn = vec!["imap".into()];
        let server = crate::client::parse_imap_server(&imap_config.server)?;
        let sasl: Option<Sasl> = imap_config
            .sasl
            .clone()
            .and_then(|cfg| {
                let host = server.host_str()?;
                let port = server.port_or_known_default()?;
                Some(cfg.try_into_sasl(host, port))
            })
            .transpose()?;
        let auto_id = resolve_auto_id_params(&imap_config.id)?;
        let _ = ImapClientStd::connect(&server, &tls, imap_config.starttls, sasl, auto_id)?;
        Ok(())
    })();

    BackendCheck::from("imap", result)
}

#[cfg(feature = "jmap")]
fn check_jmap(
    _account_config: &AccountConfig,
    jmap_config: crate::config::JmapConfig,
) -> BackendCheck {
    use io_jmap::client::JmapClientStd;
    use pimalaya_stream::tls::Tls;

    let result = (|| -> Result<()> {
        let mut tls: Tls = jmap_config.tls.clone().into();
        tls.rustls.alpn = vec!["http/1.1".into()];
        let http_auth = crate::client::jmap_http_auth(jmap_config.auth.clone())?;
        let url = crate::client::parse_jmap_server(&jmap_config.server)?;
        let mut client = JmapClientStd::connect(&url, &tls, http_auth)?;
        client.session_get(&url)?;
        Ok(())
    })();

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
