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

//! Mirador configuration.
//!
//! The `[accounts.<name>]` block mirrors the schema used by
//! [himalaya CLI v2] and [himalaya TUI]: each backend lives under its
//! own protocol key (`imap`, `jmap`, `maildir`); declaring more than
//! one is allowed and the runtime picks the active one via
//! `-b/--backend`. Mirador-only fields (`mailbox`, the four `on-*`
//! hook blocks) coexist with the shared keys and are silently ignored
//! by the other binaries.
//!
//! [himalaya CLI v2]: https://github.com/pimalaya/himalaya
//! [himalaya TUI]: https://github.com/pimalaya/himalaya-tui

use std::{collections::HashMap, path::PathBuf};

use anyhow::Result;
#[cfg(feature = "imap")]
use pimalaya_config::secret::Secret;
use pimalaya_config::toml::TomlConfig;
#[cfg(feature = "imap")]
use pimalaya_stream::sasl::{
    Sasl, SaslAnonymous, SaslLogin, SaslOauthbearer, SaslPlain, SaslScramSha256, SaslXoauth2,
};
#[cfg(any(feature = "imap", feature = "jmap"))]
use pimalaya_stream::tls::{Rustls, RustlsCrypto, Tls, TlsProvider};
use serde::{Deserialize, Serialize};

use crate::hook::config::HooksConfig;

/// Root configuration: a map of named accounts.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub accounts: HashMap<String, AccountConfig>,
}

impl TomlConfig for Config {
    type Account = AccountConfig;

    fn project_name() -> &'static str {
        env!("CARGO_PKG_NAME")
    }

    fn take_named_account(&mut self, name: &str) -> Option<(String, Self::Account)> {
        self.accounts.remove_entry(name)
    }

    fn take_default_account(&mut self) -> Option<(String, Self::Account)> {
        let name = self
            .accounts
            .iter()
            .find_map(|(name, account)| account.default.then(|| name.clone()))?;
        self.take_named_account(&name)
    }
}

impl Config {
    /// Loads the config from `config_paths`, bailing when no file
    /// resolves. Mirador has no interactive wizard; point the user at
    /// the sample so they can hand-edit one.
    pub fn load(config_paths: &[PathBuf]) -> Result<Config> {
        match Config::from_paths_or_default(config_paths)? {
            Some(config) => Ok(config),
            None => anyhow::bail!(
                "No configuration found. Copy `config.sample.toml` to \
                 `$XDG_CONFIG_HOME/mirador/config.toml`, edit it, then \
                 re-run mirador"
            ),
        }
    }
}

/// Per-account configuration.
///
/// `deny_unknown_fields` is intentionally omitted so the same TOML file
/// can be shared with `himalaya` CLI v2 and `himalaya-tui`. Their
/// extra fields (`smtp`, `m2dir`, `display-name`, `signature`, …)
/// coexist silently with the mirador-only ones (`mailbox`, the four
/// `on-*` hook tables).
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct AccountConfig {
    #[serde(default)]
    pub default: bool,

    /// Mailbox watched by `mirador watch` when `-m/--mailbox` is
    /// omitted. Defaults to `"INBOX"` for IMAP/JMAP and to the mailbox
    /// name for Maildir. For Maildir, the value resolves relative to
    /// the backend `root` (use `"."` to watch the root mailbox).
    #[serde(default)]
    pub mailbox: Option<String>,

    #[cfg(feature = "imap")]
    #[serde(default)]
    pub imap: Option<ImapConfig>,
    #[cfg(feature = "jmap")]
    #[serde(default)]
    pub jmap: Option<JmapConfig>,
    #[cfg(feature = "maildir")]
    #[serde(default)]
    pub maildir: Option<MaildirConfig>,

    #[serde(default)]
    pub hooks: HooksConfig,
}

// ---- IMAP ---------------------------------------------------------

#[cfg(feature = "imap")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ImapConfig {
    /// IMAP server address. Either a bare authority
    /// (`imap.example.org[:port]`, treated as `imaps://<authority>`
    /// by default) or a full URL with `imap://` (cleartext, with
    /// optional STARTTLS upgrade) or `imaps://` (implicit TLS) used
    /// verbatim.
    pub server: String,

    #[serde(default)]
    pub tls: TlsConfig,
    #[serde(default)]
    pub starttls: bool,

    /// Optional SASL credentials. When omitted, the connection skips
    /// authentication entirely (no `AUTHENTICATE` is sent).
    pub sasl: Option<SaslConfig>,
}

// ---- JMAP ---------------------------------------------------------

#[cfg(feature = "jmap")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct JmapConfig {
    /// JMAP server address. Either a bare authority (`fastmail.com`,
    /// `mail.example.org:8080`) for automatic discovery via
    /// `GET /.well-known/jmap`, or a full URL pointing directly at
    /// the session endpoint.
    pub server: String,

    #[serde(default)]
    pub tls: TlsConfig,

    /// Authentication. Exactly one of `header`, `bearer`, `basic`.
    pub auth: JmapAuthConfig,
}

#[cfg(feature = "jmap")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum JmapAuthConfig {
    Header(pimalaya_config::secret::Secret),
    Bearer {
        token: pimalaya_config::secret::Secret,
    },
    Basic {
        #[serde(deserialize_with = "pimalaya_config::toml::shell_expanded_string")]
        username: String,
        password: pimalaya_config::secret::Secret,
    },
}

// ---- Maildir ------------------------------------------------------

#[cfg(feature = "maildir")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct MaildirConfig {
    #[serde(deserialize_with = "pimalaya_config::toml::shell_expanded_path")]
    pub root: PathBuf,
}

// ---- TLS ----------------------------------------------------------

#[cfg(any(feature = "imap", feature = "jmap"))]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct TlsConfig {
    pub provider: Option<TlsProviderConfig>,
    #[serde(default)]
    pub rustls: RustlsConfig,
    pub cert: Option<PathBuf>,
}

#[cfg(any(feature = "imap", feature = "jmap"))]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum TlsProviderConfig {
    Rustls,
    NativeTls,
}

#[cfg(any(feature = "imap", feature = "jmap"))]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct RustlsConfig {
    pub crypto: Option<RustlsCryptoConfig>,
}

#[cfg(any(feature = "imap", feature = "jmap"))]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum RustlsCryptoConfig {
    Aws,
    Ring,
}

#[cfg(any(feature = "imap", feature = "jmap"))]
impl From<TlsConfig> for Tls {
    fn from(config: TlsConfig) -> Self {
        Tls {
            provider: config.provider.map(|p| match p {
                TlsProviderConfig::Rustls => TlsProvider::Rustls,
                TlsProviderConfig::NativeTls => TlsProvider::NativeTls,
            }),
            rustls: Rustls {
                crypto: config.rustls.crypto.map(|c| match c {
                    RustlsCryptoConfig::Aws => RustlsCrypto::Aws,
                    RustlsCryptoConfig::Ring => RustlsCrypto::Ring,
                }),
                alpn: Vec::new(),
            },
            cert: config.cert,
        }
    }
}

// ---- SASL (IMAP) --------------------------------------------------

#[cfg(feature = "imap")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum SaslConfig {
    Anonymous(SaslAnonymousConfig),
    Login(SaslLoginConfig),
    Plain(SaslPlainConfig),
    Oauthbearer(SaslOauthbearerConfig),
    Xoauth2(SaslXoauth2Config),
    #[serde(rename = "scram-sha-256")]
    ScramSha256(SaslScramSha256Config),
}

#[cfg(feature = "imap")]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct SaslAnonymousConfig {
    pub message: Option<String>,
}

#[cfg(feature = "imap")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct SaslLoginConfig {
    #[serde(deserialize_with = "pimalaya_config::toml::shell_expanded_string")]
    pub username: String,
    pub password: Secret,
}

#[cfg(feature = "imap")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct SaslPlainConfig {
    pub authzid: Option<String>,
    #[serde(deserialize_with = "pimalaya_config::toml::shell_expanded_string")]
    pub authcid: String,
    pub passwd: Secret,
}

#[cfg(feature = "imap")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct SaslOauthbearerConfig {
    #[serde(deserialize_with = "pimalaya_config::toml::shell_expanded_string")]
    pub username: String,
    pub host: String,
    pub port: u16,
    pub token: Secret,
}

#[cfg(feature = "imap")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct SaslXoauth2Config {
    #[serde(deserialize_with = "pimalaya_config::toml::shell_expanded_string")]
    pub username: String,
    pub token: Secret,
}

#[cfg(feature = "imap")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct SaslScramSha256Config {
    #[serde(deserialize_with = "pimalaya_config::toml::shell_expanded_string")]
    pub username: String,
    pub password: Secret,
}

#[cfg(feature = "imap")]
impl TryFrom<SaslConfig> for Sasl {
    type Error = anyhow::Error;

    fn try_from(config: SaslConfig) -> Result<Self, Self::Error> {
        Ok(match config {
            SaslConfig::Anonymous(c) => Sasl::Anonymous(SaslAnonymous { message: c.message }),
            SaslConfig::Login(c) => Sasl::Login(SaslLogin {
                username: c.username,
                password: c.password.get()?,
            }),
            SaslConfig::Plain(c) => Sasl::Plain(SaslPlain {
                authzid: c.authzid,
                authcid: c.authcid,
                passwd: c.passwd.get()?,
            }),
            SaslConfig::Oauthbearer(c) => Sasl::Oauthbearer(SaslOauthbearer {
                username: c.username,
                host: c.host,
                port: c.port,
                token: c.token.get()?,
            }),
            SaslConfig::Xoauth2(c) => Sasl::Xoauth2(SaslXoauth2 {
                username: c.username,
                token: c.token.get()?,
            }),
            SaslConfig::ScramSha256(c) => Sasl::ScramSha256(SaslScramSha256 {
                username: c.username,
                password: c.password.get()?,
            }),
        })
    }
}
