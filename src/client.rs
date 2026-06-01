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

//! Backend client construction.
//!
//! [`open`] picks the active backend from the [`Backend`] CLI flag and
//! the protocol blocks declared on the [`AccountConfig`], then returns
//! a fresh [`EmailClientStd`] with one slot registered. Priority under
//! `Backend::Auto`: IMAP, then JMAP, then Maildir; the first
//! configured-and-allowed block wins.

use anyhow::{Result, bail};
#[cfg(feature = "jmap")]
use base64::{Engine, prelude::BASE64_STANDARD};
use io_email::client::EmailClientStd;
#[cfg(feature = "maildir")]
use io_email::maildir::client::MaildirClient;
#[cfg(feature = "imap")]
use pimalaya_stream::sasl::Sasl;
#[cfg(any(feature = "imap", feature = "jmap"))]
use pimalaya_stream::tls::Tls;
#[cfg(feature = "jmap")]
use secrecy::{ExposeSecret, SecretString};
#[cfg(any(feature = "imap", feature = "jmap"))]
use url::Url;

#[cfg(feature = "jmap")]
use crate::config::JmapAuthConfig;
use crate::{backend::Backend, config::AccountConfig};

/// Opens the protocol client for `account` under the active
/// [`Backend`] selection. Returns a fresh [`EmailClientStd`] with one
/// slot registered (the rest are unset because mirador's watch
/// channel is per-protocol).
pub fn open(account: AccountConfig, backend: Backend) -> Result<EmailClientStd> {
    #[cfg(feature = "imap")]
    if backend.allows_imap() {
        if let Some(config) = account.imap {
            return open_imap(config);
        }
        if backend == Backend::Imap {
            bail!("Account has no `imap` config block");
        }
    }

    #[cfg(feature = "jmap")]
    if backend.allows_jmap() {
        if let Some(config) = account.jmap {
            return open_jmap(config);
        }
        if backend == Backend::Jmap {
            bail!("Account has no `jmap` config block");
        }
    }

    #[cfg(feature = "maildir")]
    if backend.allows_maildir() {
        if let Some(config) = account.maildir {
            return open_maildir(config);
        }
        if backend == Backend::Maildir {
            bail!("Account has no `maildir` config block");
        }
    }

    bail!(
        "Account has no usable backend block (expected one of `imap`, `jmap`, `maildir`); \
         use `-b/--backend` to pin a specific one"
    );
}

#[cfg(feature = "imap")]
fn open_imap(config: crate::config::ImapConfig) -> Result<EmailClientStd> {
    use crate::config::resolve_auto_id_params;

    let mut tls: Tls = config.tls.into();
    tls.rustls.alpn = vec!["imap".into()];
    let server = parse_imap_server(&config.server)?;
    let sasl: Option<Sasl> = config
        .sasl
        .and_then(|cfg| {
            let host = server.host_str()?;
            let port = server.port_or_known_default()?;
            Some(cfg.try_into_sasl(host, port))
        })
        .transpose()?;
    let auto_id = resolve_auto_id_params(&config.id)?;

    let mut client =
        EmailClientStd::new().connect_imap(&server, &tls, config.starttls, sasl, auto_id)?;
    if let Some(imap) = client.imap.as_mut() {
        // NOTE: mirador owns the connection for the lifetime of the
        // watch (IDLE + QRESYNC); auto_select would just cause an
        // extra SELECT on every coroutine entry.
        imap.auto_select = false;
    }
    Ok(client)
}

#[cfg(feature = "jmap")]
fn open_jmap(config: crate::config::JmapConfig) -> Result<EmailClientStd> {
    let mut tls: Tls = config.tls.into();
    tls.rustls.alpn = vec!["http/1.1".into()];

    let http_auth = jmap_http_auth(config.auth)?;
    let url = parse_jmap_server(&config.server)?;
    Ok(EmailClientStd::new().connect_jmap(&url, &tls, http_auth)?)
}

/// Converts a [`JmapAuthConfig`] into the pre-formatted HTTP
/// `Authorization` header value expected by `JmapClientStd`.
#[cfg(feature = "jmap")]
pub fn jmap_http_auth(config: JmapAuthConfig) -> Result<SecretString> {
    Ok(match config {
        JmapAuthConfig::Header(token) => token.get()?,
        JmapAuthConfig::Bearer { token } => {
            let token = token.get()?;
            format!("Bearer {}", token.expose_secret()).into()
        }
        JmapAuthConfig::Basic { username, password } => {
            let creds = format!("{}:{}", username, password.get()?.expose_secret());
            let encoded = BASE64_STANDARD.encode(creds.into_bytes());
            format!("Basic {encoded}").into()
        }
    })
}

#[cfg(feature = "maildir")]
fn open_maildir(config: crate::config::MaildirConfig) -> Result<EmailClientStd> {
    Ok(EmailClientStd::new().with_maildir(MaildirClient::new(config.root)))
}

/// Parses an IMAP server string into a URL.
///
/// Accepts a bare authority (`imap.example.org[:port]`), treated as
/// `imaps://<authority>` (secure by default), or a full URL with an
/// `imap://` / `imaps://` scheme used verbatim.
#[cfg(feature = "imap")]
pub fn parse_imap_server(server: &str) -> Result<Url> {
    match Url::parse(server) {
        Ok(url) => Ok(url),
        Err(url::ParseError::RelativeUrlWithoutBase) => {
            Ok(Url::parse(&format!("imaps://{server}"))?)
        }
        Err(err) => Err(err.into()),
    }
}

/// Parses a JMAP server string into a URL.
///
/// Accepts a bare authority (auto-discovered via
/// `GET /.well-known/jmap`), or a full URL pointing directly at the
/// session endpoint.
#[cfg(feature = "jmap")]
pub fn parse_jmap_server(server: &str) -> Result<Url> {
    match Url::parse(server) {
        Ok(url) => Ok(url),
        Err(url::ParseError::RelativeUrlWithoutBase) => {
            Ok(Url::parse(&format!("https://{server}"))?)
        }
        Err(err) => Err(err.into()),
    }
}
