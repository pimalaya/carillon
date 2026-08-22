//! Carillon configuration.
//!
//! The `[accounts.<name>]` block mirrors the schema used by
//! [himalaya CLI v2] and [himalaya TUI]: each backend lives under its
//! own protocol key (`imap`, `jmap`, `maildir`); declaring more than
//! one is allowed and the runtime picks the active one via
//! `-b/--backend`. Carillon-only fields (`collection`, and under each
//! backend its `watch` and `hook` tables) coexist with the shared keys
//! and are silently ignored by the other binaries.
//!
//! The hooks belong to their backend rather than to the account,
//! since what a backend reports and what a hook can template against
//! are both the backend's. Each table declares only the events its
//! backend has, so a hook it could never fire is refused when the
//! file is read.
//!
//! [himalaya CLI v2]: https://github.com/pimalaya/himalaya
//! [himalaya TUI]: https://github.com/pimalaya/himalaya-tui

use std::{
    collections::{BTreeSet, HashMap},
    path::PathBuf,
    process::Command,
    time::Duration,
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
#[cfg(any(feature = "imap", feature = "dav"))]
use pimalaya_config::secret::Secret;
use pimalaya_config::toml::TomlConfig;
#[cfg(any(feature = "imap", feature = "jmap", feature = "dav"))]
use pimalaya_stream::tls::{Rustls, RustlsCrypto, Tls, TlsProvider};
use serde::{Deserialize, Serialize};

#[cfg(feature = "dav")]
use crate::event::WatchDomain;
use crate::event::WatchEvent;

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
    /// resolves. Carillon has no interactive wizard; point the user at
    /// the sample so they can hand-edit one.
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
/// `deny_unknown_fields` is intentionally omitted so the same TOML file
/// can be shared with `himalaya` CLI v2 and `himalaya-tui`. Their
/// extra fields (`smtp`, `m2dir`, `display-name`, `signature`, …)
/// coexist silently with the carillon-only ones (`collection`, and the
/// `watch` and `hook` tables under each backend).
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct AccountConfig {
    #[serde(default)]
    pub default: bool,

    /// What this account watches: an IMAP mailbox, a JMAP mailbox
    /// name, a Maildir under the backend `root` (`.` naming the root
    /// itself), a WebDAV collection path under the backend `server`.
    ///
    /// One account watches one collection. Watching a second one is a
    /// second account, which is also how it gets its own hooks.
    #[serde(alias = "mailbox")]
    pub collection: String,

    #[cfg(feature = "imap")]
    #[serde(default)]
    pub imap: Option<ImapConfig>,
    #[cfg(feature = "jmap")]
    #[serde(default)]
    pub jmap: Option<JmapConfig>,
    #[cfg(feature = "maildir")]
    #[serde(default)]
    pub maildir: Option<MaildirConfig>,
    #[cfg(feature = "dav")]
    #[serde(default)]
    pub caldav: Option<CaldavConfig>,
    #[cfg(feature = "dav")]
    #[serde(default)]
    pub carddav: Option<CarddavConfig>,
    #[cfg(feature = "dav")]
    #[serde(default)]
    pub dav: Option<DavConfig>,
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

    /// How this account learns about a change. Unset holds IDLE.
    #[serde(default)]
    pub watch: Option<ImapWatchConfig>,

    /// The hooks this backend fires.
    #[serde(default, alias = "hooks")]
    pub hook: ImapHookConfig,
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
    /// sends `ID NIL`. For each entry: `true` substitutes carillon's
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
/// maps the user-supplied key to either carillon's canned value
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
        "support-url" => Some("https://github.com/pimalaya/carillon"),
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

    /// How this account learns about a change. Unset holds an
    /// EventSource stream.
    #[serde(default)]
    pub watch: Option<JmapWatchConfig>,

    /// The hooks this backend fires.
    #[serde(default, alias = "hooks")]
    pub hook: JmapHookConfig,
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

    /// How this account learns about a change. Unset polls.
    #[serde(default)]
    pub watch: Option<MaildirWatchConfig>,

    /// The hooks this backend fires.
    #[serde(default, alias = "hooks")]
    pub hook: MaildirHookConfig,
}

// ---- TLS ----------------------------------------------------------

#[cfg(any(feature = "imap", feature = "jmap", feature = "dav"))]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct TlsConfig {
    pub provider: Option<TlsProviderConfig>,
    #[serde(default)]
    pub rustls: RustlsConfig,
    pub cert: Option<PathBuf>,
}

#[cfg(any(feature = "imap", feature = "jmap", feature = "dav"))]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum TlsProviderConfig {
    Rustls,
    NativeTls,
}

#[cfg(any(feature = "imap", feature = "jmap", feature = "dav"))]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct RustlsConfig {
    pub crypto: Option<RustlsCryptoConfig>,
}

#[cfg(any(feature = "imap", feature = "jmap", feature = "dav"))]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum RustlsCryptoConfig {
    Aws,
    Ring,
}

#[cfg(any(feature = "imap", feature = "jmap", feature = "dav"))]
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

// NOTE: the hooks live under their backend, and each backend declares
// only the events it reports, so a hook it cannot fire is refused when
// the file is read rather than staying quiet forever. The events are
// named after their domain, which is why the tables below do not share
// a shape: mail has no edit and WebDAV has no flags.

/// Hooks an IMAP watch fires.
#[cfg(feature = "imap")]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ImapHookConfig {
    /// Fires when a message arrives in the watched mailbox.
    pub on_message_added: Option<ItemHook>,
    /// Fires when a message leaves it, expunged or moved away.
    pub on_message_removed: Option<ItemHook>,
    /// Fires once for each flag set on a message.
    pub on_flag_added: Option<FlagHook>,
    /// Fires once for each flag cleared on a message.
    pub on_flag_removed: Option<FlagHook>,
}

/// Hooks a JMAP watch fires.
#[cfg(feature = "jmap")]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct JmapHookConfig {
    /// Fires when a message arrives in the watched mailbox.
    pub on_message_added: Option<ItemHook>,
    /// Fires when a message leaves it.
    pub on_message_removed: Option<ItemHook>,
    /// Fires once for each keyword set on a message.
    pub on_flag_added: Option<FlagHook>,
    /// Fires once for each keyword cleared on a message.
    pub on_flag_removed: Option<FlagHook>,
}

/// Hooks a Maildir watch fires.
#[cfg(feature = "maildir")]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct MaildirHookConfig {
    /// Fires when a message file appears in the watched maildir.
    pub on_message_added: Option<ItemHook>,
    /// Fires when one disappears from it.
    pub on_message_removed: Option<ItemHook>,
    /// Fires once for each flag letter added to a message.
    pub on_flag_added: Option<FlagHook>,
    /// Fires once for each flag letter removed from one.
    pub on_flag_removed: Option<FlagHook>,
}

/// Hooks a CalDAV watch fires, one set per component a calendar holds.
#[cfg(feature = "dav")]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CaldavHookConfig {
    /// Fires when a VEVENT appears in the watched calendar.
    pub on_event_added: Option<ItemHook>,
    /// Fires when a VEVENT leaves it.
    pub on_event_removed: Option<ItemHook>,
    /// Fires when a VEVENT is edited where it stands.
    pub on_event_changed: Option<ItemHook>,
    /// Fires when a VTODO appears in the watched calendar.
    pub on_task_added: Option<ItemHook>,
    /// Fires when a VTODO leaves it.
    pub on_task_removed: Option<ItemHook>,
    /// Fires when a VTODO is edited where it stands.
    pub on_task_changed: Option<ItemHook>,
}

/// Hooks a CardDAV watch fires.
#[cfg(feature = "dav")]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CarddavHookConfig {
    /// Fires when a vCard appears in the watched addressbook.
    pub on_card_added: Option<ItemHook>,
    /// Fires when a vCard leaves it.
    pub on_card_removed: Option<ItemHook>,
    /// Fires when a vCard is edited where it stands.
    pub on_card_changed: Option<ItemHook>,
}

/// Hooks a plain DAV watch fires, over a collection naming no domain.
#[cfg(feature = "dav")]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct DavHookConfig {
    /// Fires when a member appears in the watched collection.
    pub on_item_added: Option<ItemHook>,
    /// Fires when a member leaves it.
    pub on_item_removed: Option<ItemHook>,
    /// Fires when a member is edited where it stands.
    pub on_item_changed: Option<ItemHook>,
}

/// The hook one event resolved to, in whichever of the two shapes a
/// hook is written.
// NOTE: which shapes exist is the vocabulary's business; which of them
// can be constructed depends on the backends compiled in, so a build
// with no flag-carrying backend leaves one unused by construction.
#[allow(dead_code)]
pub enum Hook<'a> {
    /// An item-level hook: added, removed or changed.
    Item(&'a ItemHook),
    /// A flag-level hook, which carries its own filter.
    Flag(&'a FlagHook),
}

#[cfg(feature = "imap")]
impl ImapHookConfig {
    /// The hook `event` calls for, when one is configured.
    pub fn get(&self, event: &WatchEvent) -> Option<Hook<'_>> {
        match event {
            WatchEvent::ItemAdded { .. } => self.on_message_added.as_ref().map(Hook::Item),
            WatchEvent::ItemRemoved { .. } => self.on_message_removed.as_ref().map(Hook::Item),
            WatchEvent::ItemChanged { .. } => None,
            WatchEvent::FlagAdded { .. } => self.on_flag_added.as_ref().map(Hook::Flag),
            WatchEvent::FlagRemoved { .. } => self.on_flag_removed.as_ref().map(Hook::Flag),
        }
    }
}

#[cfg(feature = "jmap")]
impl JmapHookConfig {
    /// The hook `event` calls for, when one is configured.
    pub fn get(&self, event: &WatchEvent) -> Option<Hook<'_>> {
        match event {
            WatchEvent::ItemAdded { .. } => self.on_message_added.as_ref().map(Hook::Item),
            WatchEvent::ItemRemoved { .. } => self.on_message_removed.as_ref().map(Hook::Item),
            WatchEvent::ItemChanged { .. } => None,
            WatchEvent::FlagAdded { .. } => self.on_flag_added.as_ref().map(Hook::Flag),
            WatchEvent::FlagRemoved { .. } => self.on_flag_removed.as_ref().map(Hook::Flag),
        }
    }
}

#[cfg(feature = "maildir")]
impl MaildirHookConfig {
    /// The hook `event` calls for, when one is configured.
    pub fn get(&self, event: &WatchEvent) -> Option<Hook<'_>> {
        match event {
            WatchEvent::ItemAdded { .. } => self.on_message_added.as_ref().map(Hook::Item),
            WatchEvent::ItemRemoved { .. } => self.on_message_removed.as_ref().map(Hook::Item),
            WatchEvent::ItemChanged { .. } => None,
            WatchEvent::FlagAdded { .. } => self.on_flag_added.as_ref().map(Hook::Flag),
            WatchEvent::FlagRemoved { .. } => self.on_flag_removed.as_ref().map(Hook::Flag),
        }
    }
}

#[cfg(feature = "dav")]
impl CaldavHookConfig {
    /// The hook `event` calls for, which on a calendar depends on the
    /// component the member turned out to be.
    pub fn get(&self, event: &WatchEvent) -> Option<Hook<'_>> {
        let hook = match event {
            WatchEvent::ItemAdded {
                domain: WatchDomain::Event,
                ..
            } => &self.on_event_added,
            WatchEvent::ItemRemoved {
                domain: WatchDomain::Event,
                ..
            } => &self.on_event_removed,
            WatchEvent::ItemChanged {
                domain: WatchDomain::Event,
                ..
            } => &self.on_event_changed,
            WatchEvent::ItemAdded {
                domain: WatchDomain::Task,
                ..
            } => &self.on_task_added,
            WatchEvent::ItemRemoved {
                domain: WatchDomain::Task,
                ..
            } => &self.on_task_removed,
            WatchEvent::ItemChanged {
                domain: WatchDomain::Task,
                ..
            } => &self.on_task_changed,
            _ => return None,
        };

        hook.as_ref().map(Hook::Item)
    }

    /// The components this table has hooks for, which is what a
    /// calendar advertising only some of them is checked against.
    pub fn domains(&self) -> Vec<WatchDomain> {
        let mut domains = Vec::new();

        if self.on_event_added.is_some()
            || self.on_event_removed.is_some()
            || self.on_event_changed.is_some()
        {
            domains.push(WatchDomain::Event);
        }

        if self.on_task_added.is_some()
            || self.on_task_removed.is_some()
            || self.on_task_changed.is_some()
        {
            domains.push(WatchDomain::Task);
        }

        domains
    }
}

#[cfg(feature = "dav")]
impl CarddavHookConfig {
    /// The hook `event` calls for, when one is configured.
    pub fn get(&self, event: &WatchEvent) -> Option<Hook<'_>> {
        let hook = match event {
            WatchEvent::ItemAdded { .. } => &self.on_card_added,
            WatchEvent::ItemRemoved { .. } => &self.on_card_removed,
            WatchEvent::ItemChanged { .. } => &self.on_card_changed,
            _ => return None,
        };

        hook.as_ref().map(Hook::Item)
    }
}

#[cfg(feature = "dav")]
impl DavHookConfig {
    /// The hook `event` calls for, when one is configured.
    pub fn get(&self, event: &WatchEvent) -> Option<Hook<'_>> {
        let hook = match event {
            WatchEvent::ItemAdded { .. } => &self.on_item_added,
            WatchEvent::ItemRemoved { .. } => &self.on_item_removed,
            WatchEvent::ItemChanged { .. } => &self.on_item_changed,
            _ => return None,
        };

        hook.as_ref().map(Hook::Item)
    }
}

/// Hook that fires for item-level events: added, removed, changed.
///
/// Placeholders use shell-style `$name` / `${name}` syntax in the
/// notification summary and body. Always available: `id`, `collection`.
/// The envelope names (`subject`, `date`, `sender`, `sender_name`,
/// `sender_address`, `recipient`, `recipient_name`,
/// `recipient_address`) are resolved only for an arrival, and only by
/// a backend that can read one, which today is IMAP alone.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ItemHook {
    pub notify: Option<NotifyConfig>,
    pub cmd: Option<HookCmd>,
}

/// Hook that fires for flag-level events, once per flag that moved.
///
/// `flags` optionally narrows it to the flags it names, matched
/// case-insensitively with or without an IMAP backslash or a keyword
/// dollar. The flag a firing is about reaches the templates as
/// `$flag`.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct FlagHook {
    pub notify: Option<NotifyConfig>,
    pub cmd: Option<HookCmd>,
    #[serde(default)]
    pub flags: BTreeSet<String>,
}

/// Desktop notification payload: a one-line summary and an optional
/// multi-line body.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
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
        // NOTE: `Command` itself is not `Clone`; rebuild a fresh one
        // with the same program + args (mirrors `Secret`'s manual
        // impl).
        let mut new = Command::new(self.0.get_program());
        new.args(self.0.get_args());
        Self(new)
    }
}

// ---- WebDAV -------------------------------------------------------

// NOTE: CalDAV and CardDAV are WebDAV, so the transport half is one
// shape; what differs is the domain the collection holds, and that is
// what names the events. Three blocks rather than one is what lets a
// card hook on a calendar be refused when the file is read. The shape
// is written out three times rather than shared through a flattened
// struct, since serde cannot deny unknown fields across a flatten.

/// CalDAV configuration: one watched calendar, polled through RFC 6578
/// `sync-collection`.
#[cfg(feature = "dav")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CaldavConfig {
    /// The DAV server URL, `http://` or `https://`. What to watch
    /// under it is the account's `collection`, read as a path.
    pub server: String,
    #[serde(default)]
    pub tls: TlsConfig,
    /// Authentication. Defaults to none, for a calendar that is
    /// readable without it.
    #[serde(default)]
    pub auth: DavAuthConfig,
    /// How this account learns about a change. Unset polls.
    #[serde(default)]
    pub watch: Option<DavWatchConfig>,
    /// The hooks this backend fires, one per component it holds.
    #[serde(default, alias = "hooks")]
    pub hook: CaldavHookConfig,
}

/// CardDAV configuration: one watched addressbook, polled the same way.
#[cfg(feature = "dav")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CarddavConfig {
    /// The DAV server URL, `http://` or `https://`. What to watch
    /// under it is the account's `collection`, read as a path.
    pub server: String,
    #[serde(default)]
    pub tls: TlsConfig,
    /// Authentication. Defaults to none, for an addressbook that is
    /// readable without it.
    #[serde(default)]
    pub auth: DavAuthConfig,
    /// How this account learns about a change. Unset polls.
    #[serde(default)]
    pub watch: Option<DavWatchConfig>,
    /// The hooks this backend fires.
    #[serde(default, alias = "hooks")]
    pub hook: CarddavHookConfig,
}

/// Plain WebDAV configuration: a collection that names no domain, whose
/// members are items.
#[cfg(feature = "dav")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct DavConfig {
    /// The DAV server URL, `http://` or `https://`. What to watch
    /// under it is the account's `collection`, read as a path.
    pub server: String,
    #[serde(default)]
    pub tls: TlsConfig,
    /// Authentication. Defaults to none, for a collection that is
    /// readable without it.
    #[serde(default)]
    pub auth: DavAuthConfig,
    /// How this account learns about a change. Unset polls.
    #[serde(default)]
    pub watch: Option<DavWatchConfig>,
    /// The hooks this backend fires.
    #[serde(default, alias = "hooks")]
    pub hook: DavHookConfig,
}

/// The transport half of a DAV backend, which a calendar, an
/// addressbook and a plain collection share.
#[cfg(feature = "dav")]
pub struct DavServer<'a> {
    pub server: &'a str,
    pub tls: &'a TlsConfig,
    pub auth: &'a DavAuthConfig,
}

#[cfg(feature = "dav")]
impl CaldavConfig {
    /// What it takes to open a connection to this server.
    pub fn server(&self) -> DavServer<'_> {
        DavServer {
            server: &self.server,
            tls: &self.tls,
            auth: &self.auth,
        }
    }
}

#[cfg(feature = "dav")]
impl CarddavConfig {
    /// What it takes to open a connection to this server.
    pub fn server(&self) -> DavServer<'_> {
        DavServer {
            server: &self.server,
            tls: &self.tls,
            auth: &self.auth,
        }
    }
}

#[cfg(feature = "dav")]
impl DavConfig {
    /// What it takes to open a connection to this server.
    pub fn server(&self) -> DavServer<'_> {
        DavServer {
            server: &self.server,
            tls: &self.tls,
            auth: &self.auth,
        }
    }
}

/// The credential presented to the DAV server.
#[cfg(feature = "dav")]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum DavAuthConfig {
    /// No `Authorization` header at all.
    #[default]
    None,
    /// HTTP Basic (RFC 7617), what most CalDAV and CardDAV servers ask
    /// for.
    Basic {
        #[serde(deserialize_with = "pimalaya_config::toml::shell_expanded_string")]
        username: String,
        password: Secret,
    },
    /// HTTP Bearer (RFC 6750), for a server behind OAuth.
    Bearer { token: Secret },
}

// ---- Watch method -------------------------------------------------

// NOTE: the method lives under its backend, and each backend declares
// only the methods it has, so asking IMAP to push or Maildir to idle
// is refused when the config is read rather than when the watch runs.

/// How an IMAP account learns about a change.
#[cfg(feature = "imap")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum ImapWatchConfig {
    /// Hold an IDLE connection and let the server speak first.
    Idle(IdleWatchConfig),
    /// Re-read the mailbox on an interval, for a server whose IDLE
    /// cannot be trusted.
    Poll(PollWatchConfig),
}

/// How a JMAP account learns about a change.
#[cfg(feature = "jmap")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum JmapWatchConfig {
    /// Hold an EventSource stream and let the server push.
    Push(PushWatchConfig),
    /// Ask `Email/changes` on an interval instead.
    Poll(PollWatchConfig),
}

/// How a Maildir account learns about a change.
#[cfg(feature = "maildir")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum MaildirWatchConfig {
    /// Re-list the directory on an interval, which a filesystem with
    /// no notification channel leaves as the only way.
    Poll(PollWatchConfig),
}

/// How a WebDAV account learns about a change.
#[cfg(feature = "dav")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum DavWatchConfig {
    /// Ask `sync-collection` on an interval, which is what WebDAV
    /// offers a client with no public endpoint.
    Poll(PollWatchConfig),
}

/// Options of the IDLE method.
#[cfg(feature = "imap")]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct IdleWatchConfig {
    /// Seconds an IDLE is held before it is re-issued, unset taking
    /// io-imap's own default of 29.
    ///
    /// Short enough survives a NAT middle-box that drops a quiet
    /// connection, at a round trip per interval; a server known to
    /// hold one open is asked less often, up to the 29 minutes RFC
    /// 2177 allows.
    #[serde(default)]
    pub timeout: Option<u64>,
}

#[cfg(feature = "imap")]
impl IdleWatchConfig {
    /// The interval this config overrides the io-imap default with.
    pub fn timeout(&self) -> Option<Duration> {
        self.timeout.map(Duration::from_secs)
    }
}

/// Options of the push method.
#[cfg(feature = "jmap")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct PushWatchConfig {
    /// Seconds between the server's keep-alive pings on the stream. A
    /// ping is also what proves the connection is still there.
    #[serde(default = "default_push_ping")]
    pub ping: u64,
}

/// Options of the poll method.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct PollWatchConfig {
    /// Seconds between two rounds. Unset takes what suits the backend:
    /// a couple of seconds for a local directory read, longer for a
    /// remote collection.
    #[serde(default)]
    pub interval: Option<u64>,
}

impl PollWatchConfig {
    /// The interval this config overrides the backend default with.
    pub fn interval(&self) -> Option<Duration> {
        self.interval.map(Duration::from_secs)
    }
}

#[cfg(feature = "jmap")]
impl Default for PushWatchConfig {
    fn default() -> Self {
        Self {
            ping: default_push_ping(),
        }
    }
}

/// Half a minute between pings, short enough to notice a dead stream
/// and long enough to be quiet.
#[cfg(feature = "jmap")]
fn default_push_ping() -> u64 {
    30
}
