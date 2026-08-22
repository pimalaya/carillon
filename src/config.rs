//! Mirador configuration.
//!
//! The `[accounts.<name>]` block mirrors the schema used by
//! [himalaya CLI v2] and [himalaya TUI]: each backend lives under its
//! own protocol key (`imap`, `jmap`, `maildir`); declaring more than
//! one is allowed and the runtime picks the active one via
//! `-b/--backend`. Mirador-only fields (`mailbox`, the `hooks.on-*`
//! tables) coexist with the shared keys and are silently ignored by
//! the other binaries.
//!
//! [himalaya CLI v2]: https://github.com/pimalaya/himalaya
//! [himalaya TUI]: https://github.com/pimalaya/himalaya-tui

use std::{
    collections::{BTreeSet, HashMap},
    path::PathBuf,
    process::Command,
};

use anyhow::Result;
#[cfg(feature = "imap")]
use anyhow::anyhow;
#[cfg(feature = "imap")]
use io_imap::types::{
    IntoStatic,
    core::{IString, NString},
};
#[cfg(feature = "imap")]
use io_sasl::{
    login::SaslLoginCreds, mechanism::Sasl, rfc4505::anonymous::SaslAnonymousCreds,
    rfc4616::plain::SaslPlainCreds, rfc5801::SaslGs2ChannelBinding, rfc5802::SaslScramCreds,
    rfc7628::oauthbearer::SaslOauthbearerCreds, xoauth2::SaslXoauth2Creds,
};
use pimalaya_config::command;
#[cfg(feature = "imap")]
use pimalaya_config::secret::Secret;
use pimalaya_config::toml::TomlConfig;
#[cfg(any(feature = "imap", feature = "jmap"))]
use pimalaya_stream::tls::{Rustls, RustlsCrypto, Tls, TlsProvider};
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
/// coexist silently with the mirador-only ones (`mailbox`, the
/// `hooks.on-*` tables).
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

    /// Forces the RFC 4959 SASL-IR initial response on or off. Unset
    /// follows the advertised `SASL-IR` capability, which Coremail
    /// (126.com, 163.com) advertises falsely; set it to `false` there.
    #[serde(default)]
    pub sasl_ir: Option<bool>,

    /// RFC 2971 `ID` extension quirks. Some providers (notably
    /// mail.qq.com, fastmail) require an `ID` exchange straight after
    /// authentication; set `id.auto = true` to opt in.
    #[serde(default)]
    pub id: ImapIdConfig,
}

/// Per-account `imap.id.*` quirks.
#[cfg(feature = "imap")]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ImapIdConfig {
    /// When `true`, the auth coroutine chains an `ID` round-trip
    /// after the tagged auth response. Default `false` skips ID
    /// entirely.
    #[serde(default)]
    pub auto: bool,

    /// Parameters sent with the auto-ID command. Empty (default)
    /// sends `ID NIL`. For each entry: `true` substitutes mirador's
    /// canned value for the well-known keys (`name`, `version`,
    /// `vendor`, `support-url`) or `NIL` for unknown keys; `false`
    /// always sends `NIL`. Keys absent from this map are not
    /// transmitted.
    #[serde(default)]
    pub fields: HashMap<String, bool>,
}

/// Resolves an [`ImapIdConfig`] into the wire-level parameter list
/// passed to the io-imap auth coroutines.
///
/// [`None`] when `auto = false`; otherwise a vec where each entry
/// maps the user-supplied key to either mirador's canned value
/// (when the user set `true` and the key is well-known) or `NIL`.
/// Unknown keys with `true` log a warning and fall back to `NIL`.
#[cfg(feature = "imap")]
pub fn resolve_auto_id_params(
    config: &ImapIdConfig,
) -> Result<Option<Vec<(IString<'static>, NString<'static>)>>> {
    if !config.auto {
        return Ok(None);
    }

    let mut params = Vec::with_capacity(config.fields.len());
    for (key, &use_canned) in &config.fields {
        let ikey = IString::try_from(key.clone())
            .map_err(|err| anyhow!("Invalid IMAP ID parameter key `{key}`: {err}"))?
            .into_static();

        let nval = if use_canned {
            match canned_imap_id_value(key) {
                Some(value) => NString::try_from(value)
                    .map_err(|err| {
                        anyhow!("Invalid canned IMAP ID value `{value}` for `{key}`: {err}")
                    })?
                    .into_static(),
                None => {
                    log::warn!("imap.id.fields.{key} = true: no canned value defined, sending NIL");
                    NString::NIL
                }
            }
        } else {
            NString::NIL
        };

        params.push((ikey, nval));
    }
    Ok(Some(params))
}

#[cfg(feature = "imap")]
fn canned_imap_id_value(key: &str) -> Option<&'static str> {
    match key {
        "name" => Some(env!("CARGO_PKG_NAME")),
        "version" => Some(env!("CARGO_PKG_VERSION")),
        "vendor" => Some("Pimalaya"),
        "support-url" => Some("https://github.com/pimalaya/mirador"),
        _ => None,
    }
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
    #[serde(alias = "username")]
    pub authcid: String,
    #[serde(alias = "password")]
    pub passwd: Secret,
}

#[cfg(feature = "imap")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct SaslOauthbearerConfig {
    #[serde(deserialize_with = "pimalaya_config::toml::shell_expanded_string")]
    pub username: String,
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
impl SaslConfig {
    /// Resolves the SASL config into a runtime [`Sasl`]. `host` and
    /// `port` come from the live server URL; they are only used by
    /// OAUTHBEARER (echoed in the GS2 header) and ignored by every
    /// other mechanism.
    pub fn try_into_sasl(self, host: impl ToString, port: u16) -> Result<Sasl> {
        Ok(match self {
            SaslConfig::Anonymous(c) => Sasl::Anonymous(SaslAnonymousCreds { message: c.message }),
            SaslConfig::Login(c) => Sasl::Login(SaslLoginCreds {
                username: c.username,
                password: c.password.get()?,
            }),
            SaslConfig::Plain(c) => Sasl::Plain(SaslPlainCreds {
                authzid: c.authzid,
                authcid: c.authcid,
                passwd: c.passwd.get()?,
            }),
            SaslConfig::Oauthbearer(c) => Sasl::Oauthbearer(SaslOauthbearerCreds {
                username: c.username,
                host: host.to_string(),
                port,
                token: c.token.get()?,
            }),
            SaslConfig::Xoauth2(c) => Sasl::Xoauth2(SaslXoauth2Creds {
                username: c.username,
                token: c.token.get()?,
            }),
            // NOTE: an empty nonce means "draw one for me": the client
            // fills it before the exchange, an I/O-free coroutine having
            // no way to generate randomness itself.
            SaslConfig::ScramSha256(c) => Sasl::ScramSha256(SaslScramCreds {
                username: c.username,
                password: c.password.get()?,
                nonce: Vec::new(),
                channel_binding: SaslGs2ChannelBinding::Unsupported,
            }),
        })
    }
}

// ---- Hooks --------------------------------------------------------

/// Per-account hook configuration: one optional hook per watch
/// event kind.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct HooksConfig {
    pub on_message_added: Option<MessageHook>,
    pub on_message_removed: Option<MessageHook>,
    pub on_flags_added: Option<FlagsHook>,
    pub on_flags_removed: Option<FlagsHook>,
}

/// Hook that fires for envelope-level events (added or removed).
/// Placeholders use shell-style `$name` / `${name}` syntax in the
/// notification summary/body. Available names: `id`, `mailbox`, and
/// (for `on-message-added` only) `subject`, `sender`, `sender_name`,
/// `sender_address`, `recipient`, `recipient_name`,
/// `recipient_address`.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct MessageHook {
    pub notify: Option<NotifyConfig>,
    pub cmd: Option<HookCmd>,
}

/// Hook that fires for flag-level events (added or removed). `flags`
/// optionally restricts firing to deltas whose IANA-classified flag
/// raw name matches one of the listed names (case-insensitive; both
/// `Seen` and `\Seen` work).
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct FlagsHook {
    pub notify: Option<NotifyConfig>,
    pub cmd: Option<HookCmd>,
    #[serde(default)]
    pub flags: BTreeSet<String>,
}

/// Desktop notification payload: a one-line summary and an optional
/// multi-line body.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct NotifyConfig {
    pub summary: String,
    #[serde(default)]
    pub body: String,
}

/// Shell-command payload. Deserialization delegates to
/// [`pimalaya_config::command`]: a TOML string is wrapped through the
/// platform shell (`/bin/sh -c <line>` on Unix, `cmd /C <line>` on
/// Windows), a TOML list `[program, args…]` is spawned directly with
/// no shell. Template vars are exported as environment variables on
/// the spawned process in both shapes.
#[derive(Debug, Deserialize, Serialize)]
pub struct HookCmd(#[serde(with = "command")] pub Command);

impl Clone for HookCmd {
    fn clone(&self) -> Self {
        // `Command` itself is not `Clone`; rebuild a fresh one with
        // the same program + args (mirrors `Secret`'s manual impl).
        let mut new = Command::new(self.0.get_program());
        new.args(self.0.get_args());
        Self(new)
    }
}
