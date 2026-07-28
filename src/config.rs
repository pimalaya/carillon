//! carillon configuration.
//!
//! An `[accounts.<name>]` block per account, each with an `imap` connection
//! block, an optional `mailbox`, and an optional `hook` fired on every
//! change. The account name is carried on every event and identifies the
//! watch. The schema stays close to the other Pimalaya tools so a config
//! file can be shared; unknown keys on the account are ignored.

use std::{collections::HashMap, path::PathBuf, process::Command};

use anyhow::Result;
use pimalaya_config::{command, secret::Secret, toml::TomlConfig};
use serde::{Deserialize, Serialize};

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
    /// Loads the config from `config_paths`, bailing when no file resolves.
    /// carillon has no interactive wizard; point the user at the sample.
    pub fn load(config_paths: &[PathBuf]) -> Result<Config> {
        match Config::from_paths_or_default(config_paths)? {
            Some(config) => Ok(config),
            None => anyhow::bail!(
                "No configuration found. Copy `config.sample.toml` to \
                 `$XDG_CONFIG_HOME/carillon/config.toml`, edit it, then \
                 re-run carillon"
            ),
        }
    }
}

/// Per-account configuration.
///
/// `deny_unknown_fields` is omitted so the same TOML file can be shared
/// with other Pimalaya tools whose extra keys coexist silently.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct AccountConfig {
    /// Whether this account is the default when none is named.
    #[serde(default)]
    pub default: bool,
    /// Mailbox watched when `-m/--mailbox` is omitted. Defaults to INBOX.
    #[serde(default)]
    pub mailbox: Option<String>,
    /// The IMAP connection to watch.
    pub imap: ImapConfig,
    /// The hook fired on every change.
    #[serde(default)]
    pub hook: HookConfig,
}

/// IMAP connection block.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ImapConfig {
    /// Server address, `host` or `host:port` (implicit TLS; default port
    /// 993).
    pub server: String,
    /// Login, the authentication identity.
    pub login: String,
    /// Cleartext or keyring-resolved password, used with `LOGIN`.
    pub password: Option<Secret>,
    /// A pre-minted OAuth 2.0 access token, used with SASL `OAUTHBEARER`.
    pub token: Option<Secret>,
}

/// The hook fired on a change. Both parts are optional and content-free.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct HookConfig {
    /// A desktop notification.
    pub notify: Option<NotifyConfig>,
    /// A shell command.
    pub cmd: Option<HookCmd>,
}

/// Desktop notification payload. Summary and body accept shell-style
/// `$name` / `${name}` placeholders over `account`, `mailbox`, `source`,
/// and `state`.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct NotifyConfig {
    pub summary: String,
    #[serde(default)]
    pub body: String,
}

/// Shell-command payload. A TOML string is wrapped through the platform
/// shell; a TOML list `[program, args…]` is spawned directly. The change
/// fields are exported as `CARILLON_*` environment variables.
#[derive(Debug, Deserialize, Serialize)]
pub struct HookCmd(#[serde(with = "command")] pub Command);

impl Clone for HookCmd {
    fn clone(&self) -> Self {
        let mut new = Command::new(self.0.get_program());
        new.args(self.0.get_args());
        Self(new)
    }
}
