//! Carillon configuration.
//!
//! The `[accounts.<name>]` block keeps the shape used by
//! [himalaya CLI v2] and [himalaya TUI]: each backend lives under its
//! own protocol key, declaring more than one is allowed, and the
//! runtime picks the active one via `-b/--backend`. A whole file is
//! not portable between the three binaries, every backend block being
//! `deny_unknown_fields` on each side and carrying keys the others do
//! not know.
//!
//! Everything a backend needs lives under that backend: the collection
//! it watches, under the name its own domain uses, how it watches, and
//! the hooks it fires. What it reports and what a hook may template
//! against are both the backend's, so each table declares only its own
//! events and its own variables, and anything else is refused when the
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

#[cfg(feature = "imap")]
use anyhow::anyhow;
use anyhow::{Context, Result};
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
#[cfg(feature = "imap")]
use log::warn;
#[cfg(any(feature = "imap", feature = "jmap", feature = "dav"))]
use pimalaya_config::secret::Secret;
use pimalaya_config::{command, toml::TomlConfig};
#[cfg(any(feature = "imap", feature = "jmap", feature = "dav"))]
use pimalaya_stream::tls::{Rustls, RustlsCrypto, Tls, TlsProvider};
use serde::{Deserialize, Serialize};

#[cfg(feature = "dav")]
use crate::event::WatchDomain;
use crate::{
    event::WatchEvent,
    hook::{self, HookCollection, Vocabulary},
};

/// The documented sample configuration, pointed at wherever a
/// configuration is missing and wherever the wizard stops short.
pub const CONFIG_SAMPLE_URL: &str =
    "https://github.com/pimalaya/carillon/blob/master/config.sample.toml";

/// The order a rendered account groups its keys in, most defining
/// first: whether the account is the default, then the backend it
/// watches.
///
/// A key outside this list still renders, after the ones listed, so a
/// field added to [`AccountConfig`] can never go missing from a
/// generated document just because nobody updated this table.
const RENDER_ORDER: [&str; 6] = ["default", "imap", "jmap", "maildir", "caldav", "carddav"];

/// The keys a backend group leads with, in reading order: the
/// collection it watches (under whichever name its domain uses), the
/// server it watches it on, then the credential it authenticates with.
/// Everything else follows alphabetically, since it only adjusts what
/// those three state.
const BACKEND_ORDER: [&str; 6] = [
    "mailbox",
    "calendar",
    "addressbook",
    "root",
    "server",
    "auth",
];

/// Whether a value is what its type defaults to, which is what keeps a
/// generated document down to what was actually configured.
fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

/// Ranks one dotted line inside its backend group, `imap.server = …`
/// being ranked on `server`. The SASL table is the IMAP spelling of
/// `auth`, so it ranks with it.
fn backend_rank(group: &str, line: &str) -> usize {
    let Some(key) = line
        .split_once(" = ")
        .map(|(key, _)| key)
        .and_then(|key| key.strip_prefix(group))
        .and_then(|key| key.strip_prefix('.'))
    else {
        return BACKEND_ORDER.len();
    };

    let key = key.split('.').next().unwrap_or(key);
    let key = if key == "sasl" { "auth" } else { key };

    BACKEND_ORDER
        .iter()
        .position(|known| *known == key)
        .unwrap_or(BACKEND_ORDER.len())
}

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
    /// Loads the config from `config_paths`, or [`None`] when no file
    /// resolves.
    ///
    /// A missing file is not an error here: what to do about it is the
    /// caller's, and for an interactive one that is to offer the
    /// wizard (see [`crate::cli::load_config`]).
    pub fn load(config_paths: &[PathBuf]) -> Result<Option<Config>> {
        let Some(config) = Config::from_paths_or_default(config_paths)? else {
            return Ok(None);
        };

        // NOTE: what a hook's notification may name is as fixed as
        // which hooks a backend has, and serde cannot check it: a
        // template is a string until something expands it. Doing it
        // here keeps both refusals at load time.
        for (name, account) in &config.accounts {
            account
                .validate()
                .with_context(|| format!("account `{name}` is misconfigured"))?;
        }

        Ok(Some(config))
    }
}

/// Per-account configuration.
///
/// `deny_unknown_fields` is intentionally omitted here so an account
/// written for `himalaya` CLI v2 or `himalaya-tui` is still recognised:
/// their account-level fields (`smtp`, `m2dir`, `display-name`,
/// `signature`, …) coexist silently. The backend blocks are strict, so
/// the tolerance stops at this level.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct AccountConfig {
    #[serde(default, skip_serializing_if = "is_default")]
    pub default: bool,

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
}

impl AccountConfig {
    /// Renders this account as an `[accounts.<name>]` block, ready to
    /// be written to a configuration file or appended to one.
    ///
    /// The serializer decides what is written, so a field left at its
    /// default is omitted and nothing has to be listed here twice.
    /// What this adds is reading order: the flattened dotted keys come
    /// out alphabetically, which buries `imap.server` under the
    /// credentials that authenticate against it and runs every group
    /// together. The groups are reordered, the endpoint is lifted to
    /// the top of its own, and a blank line separates them.
    pub fn render(&self, name: &str) -> Result<String> {
        // NOTE: borrowed rather than built into a `Config`, which
        // would mean cloning the account to render it. The emitter
        // only looks for an `accounts` table, so any shape carrying
        // one will do.
        #[derive(Serialize)]
        struct AccountDocument<'a> {
            accounts: HashMap<&'a str, &'a AccountConfig>,
        }

        let document = AccountDocument {
            accounts: HashMap::from([(name, self)]),
        };
        let rendered = pimalaya_config::toml::to_string(&document)?;

        // The emitter writes the header itself, and everything below
        // it is one dotted key per line.
        let (header, body) = match rendered.split_once('\n') {
            Some((header, body)) => (header, body),
            None => return Ok(rendered),
        };

        let mut groups: Vec<(String, Vec<&str>)> = Vec::new();

        for line in body.lines().filter(|line| !line.trim().is_empty()) {
            let key = line.split(['.', ' ']).next().unwrap_or(line).to_string();

            match groups.iter_mut().find(|(name, _)| *name == key) {
                Some((_, lines)) => lines.push(line),
                None => groups.push((key, vec![line])),
            }
        }

        groups.sort_by_key(|(key, _)| {
            RENDER_ORDER
                .iter()
                .position(|known| known == key)
                .unwrap_or(RENDER_ORDER.len())
        });

        let mut document = format!("{header}\n");

        for (index, (key, mut lines)) in groups.into_iter().enumerate() {
            if index > 0 {
                document.push('\n');
            }

            // A backend reads the way it is explained: what it watches,
            // where, who it authenticates as, then everything the
            // account only adjusts.
            lines.sort_by_key(|line| backend_rank(&key, line));

            for line in lines {
                document.push_str(line);
                document.push('\n');
            }
        }

        Ok(document)
    }

    /// Refuses a hook whose notification names a variable its event
    /// cannot fill, which serde cannot see because a template is only
    /// a string until it is expanded.
    pub fn validate(&self) -> Result<()> {
        #[cfg(feature = "imap")]
        if let Some(imap) = &self.imap {
            imap.hook.validate()?;
        }

        #[cfg(feature = "jmap")]
        if let Some(jmap) = &self.jmap {
            jmap.hook.validate()?;
        }

        #[cfg(feature = "maildir")]
        if let Some(maildir) = &self.maildir {
            maildir.hook.validate()?;
        }

        #[cfg(feature = "dav")]
        if let Some(caldav) = &self.caldav {
            caldav.hook.validate()?;
        }

        #[cfg(feature = "dav")]
        if let Some(carddav) = &self.carddav {
            carddav.hook.validate()?;
        }

        Ok(())
    }
}

// ---- IMAP ---------------------------------------------------------

#[cfg(feature = "imap")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ImapConfig {
    /// The mailbox this account watches, and the only one it
    /// watches: watching a second is a second account, which is also
    /// how it gets its own hooks.
    pub mailbox: String,

    /// IMAP server address. Either a bare authority
    /// (`imap.example.org[:port]`, treated as `imaps://<authority>`
    /// by default) or a full URL with `imap://` (cleartext, with
    /// optional STARTTLS upgrade) or `imaps://` (implicit TLS) used
    /// verbatim.
    pub server: String,

    #[serde(default)]
    pub tls: TlsConfig,
    #[serde(default, skip_serializing_if = "is_default")]
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
    #[serde(default, skip_serializing_if = "is_default")]
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
                    warn!("imap.id.fields.{key} = true: no canned value defined, sending NIL");
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
    /// The mailbox this account watches, matched by name and
    /// case-insensitively, falling back to the special-use role for
    /// "INBOX".
    pub mailbox: String,

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
    Header(Secret),
    Bearer {
        token: Secret,
    },
    Basic {
        #[serde(deserialize_with = "pimalaya_config::toml::shell_expanded_string")]
        username: String,
        password: Secret,
    },
}

// ---- Maildir ------------------------------------------------------

#[cfg(feature = "maildir")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct MaildirConfig {
    /// The mailbox this account watches, resolved under `root`
    /// through io-maildir's store; `.` and "INBOX" both name the
    /// root itself.
    pub mailbox: String,

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

// NOTE: each backend names the collection it watches in its own
// domain's word, and a hook templates against that same word, so the
// name is declared once here and read by both.

#[cfg(feature = "imap")]
impl ImapConfig {
    /// What IMAP calls the collection it watches.
    pub const COLLECTION: &'static str = "mailbox";

    /// The collection this backend watches, under its own name.
    pub fn collection(&self) -> HookCollection<'_> {
        HookCollection {
            name: Self::COLLECTION,
            value: &self.mailbox,
        }
    }
}

#[cfg(feature = "jmap")]
impl JmapConfig {
    /// What JMAP calls the collection it watches.
    pub const COLLECTION: &'static str = "mailbox";

    /// The collection this backend watches, under its own name.
    pub fn collection(&self) -> HookCollection<'_> {
        HookCollection {
            name: Self::COLLECTION,
            value: &self.mailbox,
        }
    }
}

#[cfg(feature = "maildir")]
impl MaildirConfig {
    /// What Maildir calls the collection it watches.
    pub const COLLECTION: &'static str = "mailbox";

    /// The collection this backend watches, under its own name.
    pub fn collection(&self) -> HookCollection<'_> {
        HookCollection {
            name: Self::COLLECTION,
            value: &self.mailbox,
        }
    }
}

#[cfg(feature = "dav")]
impl CaldavConfig {
    /// What CalDAV calls the collection it watches.
    pub const COLLECTION: &'static str = "calendar";

    /// The collection this backend watches, under its own name.
    pub fn collection(&self) -> HookCollection<'_> {
        HookCollection {
            name: Self::COLLECTION,
            value: &self.calendar,
        }
    }
}

#[cfg(feature = "dav")]
impl CarddavConfig {
    /// What CardDAV calls the collection it watches.
    pub const COLLECTION: &'static str = "addressbook";

    /// The collection this backend watches, under its own name.
    pub fn collection(&self) -> HookCollection<'_> {
        HookCollection {
            name: Self::COLLECTION,
            value: &self.addressbook,
        }
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
    /// What the backend this table hangs on calls its collection.
    const COLLECTION: &'static str = ImapConfig::COLLECTION;

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
    /// What the backend this table hangs on calls its collection.
    const COLLECTION: &'static str = JmapConfig::COLLECTION;

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
    /// What the backend this table hangs on calls its collection.
    const COLLECTION: &'static str = MaildirConfig::COLLECTION;

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

#[cfg(feature = "imap")]
impl ImapHookConfig {
    /// Refuses a notification naming what its event cannot fill.
    ///
    /// IMAP is the one backend that resolves an arrival's envelope, so
    /// it is the one whose arrival hook may name it.
    pub fn validate(&self) -> Result<()> {
        hook::validate(
            self.on_message_added
                .as_ref()
                .and_then(|h| h.notify.as_ref()),
            Vocabulary::resolved(Self::COLLECTION),
            "imap.hook.on-message-added",
        )?;
        hook::validate(
            self.on_message_removed
                .as_ref()
                .and_then(|h| h.notify.as_ref()),
            Vocabulary::item(Self::COLLECTION),
            "imap.hook.on-message-removed",
        )?;
        hook::validate(
            self.on_flag_added.as_ref().and_then(|h| h.notify.as_ref()),
            Vocabulary::flag(Self::COLLECTION),
            "imap.hook.on-flag-added",
        )?;
        hook::validate(
            self.on_flag_removed
                .as_ref()
                .and_then(|h| h.notify.as_ref()),
            Vocabulary::flag(Self::COLLECTION),
            "imap.hook.on-flag-removed",
        )
    }
}

#[cfg(feature = "jmap")]
impl JmapHookConfig {
    /// Refuses a notification naming what its event cannot fill.
    ///
    /// JMAP reads an envelope, from the request its round already
    /// makes, so its arrival hook may name one.
    pub fn validate(&self) -> Result<()> {
        hook::validate(
            self.on_message_added
                .as_ref()
                .and_then(|h| h.notify.as_ref()),
            Vocabulary::resolved(Self::COLLECTION),
            "jmap.hook.on-message-added",
        )?;
        hook::validate(
            self.on_message_removed
                .as_ref()
                .and_then(|h| h.notify.as_ref()),
            Vocabulary::item(Self::COLLECTION),
            "jmap.hook.on-message-removed",
        )?;
        hook::validate(
            self.on_flag_added.as_ref().and_then(|h| h.notify.as_ref()),
            Vocabulary::flag(Self::COLLECTION),
            "jmap.hook.on-flag-added",
        )?;
        hook::validate(
            self.on_flag_removed
                .as_ref()
                .and_then(|h| h.notify.as_ref()),
            Vocabulary::flag(Self::COLLECTION),
            "jmap.hook.on-flag-removed",
        )
    }
}

#[cfg(feature = "maildir")]
impl MaildirHookConfig {
    /// Refuses a notification naming what its event cannot fill.
    pub fn validate(&self) -> Result<()> {
        hook::validate(
            self.on_message_added
                .as_ref()
                .and_then(|h| h.notify.as_ref()),
            Vocabulary::item(Self::COLLECTION),
            "maildir.hook.on-message-added",
        )?;
        hook::validate(
            self.on_message_removed
                .as_ref()
                .and_then(|h| h.notify.as_ref()),
            Vocabulary::item(Self::COLLECTION),
            "maildir.hook.on-message-removed",
        )?;
        hook::validate(
            self.on_flag_added.as_ref().and_then(|h| h.notify.as_ref()),
            Vocabulary::flag(Self::COLLECTION),
            "maildir.hook.on-flag-added",
        )?;
        hook::validate(
            self.on_flag_removed
                .as_ref()
                .and_then(|h| h.notify.as_ref()),
            Vocabulary::flag(Self::COLLECTION),
            "maildir.hook.on-flag-removed",
        )
    }
}

#[cfg(feature = "dav")]
impl CaldavHookConfig {
    /// Refuses a notification naming what its event cannot fill.
    pub fn validate(&self) -> Result<()> {
        for (hook, name) in [
            (&self.on_event_added, "on-event-added"),
            (&self.on_event_removed, "on-event-removed"),
            (&self.on_event_changed, "on-event-changed"),
            (&self.on_task_added, "on-task-added"),
            (&self.on_task_removed, "on-task-removed"),
            (&self.on_task_changed, "on-task-changed"),
        ] {
            let notify = hook.as_ref().and_then(|hook| hook.notify.as_ref());
            hook::validate(
                notify,
                Vocabulary::item(CaldavConfig::COLLECTION),
                &format!("caldav.hook.{name}"),
            )?;
        }

        Ok(())
    }
}

#[cfg(feature = "dav")]
impl CarddavHookConfig {
    /// Refuses a notification naming what its event cannot fill.
    pub fn validate(&self) -> Result<()> {
        for (hook, name) in [
            (&self.on_card_added, "on-card-added"),
            (&self.on_card_removed, "on-card-removed"),
            (&self.on_card_changed, "on-card-changed"),
        ] {
            let notify = hook.as_ref().and_then(|hook| hook.notify.as_ref());
            hook::validate(
                notify,
                Vocabulary::item(CarddavConfig::COLLECTION),
                &format!("carddav.hook.{name}"),
            )?;
        }

        Ok(())
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
    /// The calendar this account watches, read as a path under
    /// `server`, or as an absolute path when it starts with a slash.
    pub calendar: String,

    /// The DAV server URL, `http://` or `https://`, naming the DAV
    /// root the calendar hangs under.
    pub server: String,
    #[serde(default)]
    pub tls: TlsConfig,
    /// Authentication. Defaults to none, for a calendar that is
    /// readable without it.
    #[serde(default, skip_serializing_if = "DavAuthConfig::is_none")]
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
    /// The addressbook this account watches, read as a path under
    /// `server`, or as an absolute path when it starts with a slash.
    pub addressbook: String,

    /// The DAV server URL, `http://` or `https://`, naming the DAV
    /// root the addressbook hangs under.
    pub server: String,
    #[serde(default)]
    pub tls: TlsConfig,
    /// Authentication. Defaults to none, for an addressbook that is
    /// readable without it.
    #[serde(default, skip_serializing_if = "DavAuthConfig::is_none")]
    pub auth: DavAuthConfig,
    /// How this account learns about a change. Unset polls.
    #[serde(default)]
    pub watch: Option<DavWatchConfig>,
    /// The hooks this backend fires.
    #[serde(default, alias = "hooks")]
    pub hook: CarddavHookConfig,
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

#[cfg(feature = "dav")]
impl DavAuthConfig {
    /// Whether the server is reached with no `Authorization` header,
    /// which is what a generated document leaves out.
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
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
