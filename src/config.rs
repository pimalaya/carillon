//! # Configuration
//!
//! The TOML schema: one `[accounts.<name>]` block per account, in the
//! shape [himalaya CLI v2] and [himalaya TUI] use, each backend under its
//! own protocol key.
//!
//! Declaring more than one backend is allowed, `-b/--backend` picking the
//! active one. A whole file is not portable between the three binaries:
//! every backend block is `deny_unknown_fields` on each side and carries
//! keys the others do not know.
//!
//! Everything a backend needs lives under it: the collection it watches,
//! under its own domain's name, how it watches, and the hooks it fires.
//! What it reports and what a hook may template against are the backend's
//! too, so anything else is refused when the file is read.
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
#[cfg(network)]
use anyhow::bail;
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
#[cfg(network)]
use pimalaya_config::secret::{Secret, SecretResolver};
#[cfg(network)]
use pimalaya_config::toml::opt_shell_expanded_path;
use pimalaya_config::{command, toml::TomlConfig};
#[cfg(network)]
use pimalaya_stream::{
    proxy::{Proxy, ProxyAuth},
    tls::{Rustls, RustlsCrypto, Tls, TlsProvider},
};
#[cfg(network)]
use secrecy::SecretString;
use serde::{Deserialize, Serialize};

#[cfg(any(feature = "jmap", feature = "dav", api))]
use crate::event::WatchDomain;
use crate::{
    event::WatchEvent,
    hook::{self, HookCollection, Vocabulary},
};

/// The documented sample, pointed at wherever a configuration is missing
/// and wherever the wizard stops short.
pub const CONFIG_SAMPLE_URL: &str =
    "https://github.com/pimalaya/carillon/blob/master/config.sample.toml";

/// The order a rendered account groups its keys in, most defining first.
///
/// A key outside this list still renders, after the listed ones, so a
/// field added to [`AccountConfig`] can never go missing from a generated
/// document because nobody updated this table.
const RENDER_ORDER: [&str; 11] = [
    "default", "proxy", "imap", "jmap", "msgraph", "gmail", "maildir", "caldav", "gcal", "carddav",
    "gpeople",
];

/// The keys a backend group leads with, in reading order: the collection
/// it watches, the server, then the credential.
///
/// Everything else follows alphabetically, only adjusting what those
/// three state.
const BACKEND_ORDER: [&str; 6] = [
    "mailbox",
    "calendar",
    "addressbook",
    "root",
    "server",
    "auth",
];

/// Whether a value is what its type defaults to, which keeps a generated
/// document down to what was actually configured.
fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

/// Ranks one dotted line inside its backend group, `imap.server = …`
/// ranking on `server`.
///
/// The SASL table is the IMAP spelling of `auth`, so it ranks with it.
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
    /// Loads the config from `config_paths`, [`None`] when none resolves.
    ///
    /// A missing file is not an error here: what to do about it is the
    /// caller's, and for an interactive one that is to offer the wizard
    /// (see [`crate::cli::load_config`]).
    pub fn load(config_paths: &[PathBuf]) -> Result<Option<Config>> {
        let Some(config) = Config::from_paths_or_default(config_paths)? else {
            return Ok(None);
        };

        #[cfg(network)]
        let config = config.with_inherited_proxies();

        // NOTE: what a notification may name is as fixed as which hooks a
        // backend has, and serde cannot check it, a template being a
        // string until something expands it. Here, both are refused at
        // load time.
        for (name, account) in &config.accounts {
            account
                .validate()
                .with_context(|| format!("Account `{name}` is misconfigured"))?;
        }

        Ok(Some(config))
    }

    /// Hands each account proxy down to its backends, so every connection
    /// reads its own backend's key.
    #[cfg(network)]
    fn with_inherited_proxies(mut self) -> Self {
        for account in self.accounts.values_mut() {
            account.inherit_proxy();
        }

        self
    }
}

/// Per-account configuration.
///
/// `deny_unknown_fields` is deliberately omitted, so the account-level
/// fields of himalaya CLI v2 and himalaya-tui coexist silently. The
/// backend blocks are strict, so the tolerance stops at this level.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct AccountConfig {
    /// Use this account when `-a/--account` names none.
    #[serde(default, skip_serializing_if = "is_default")]
    pub default: bool,
    /// Proxy every network backend of this account goes through, unless
    /// its own block names one.
    #[cfg(network)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy: Option<ProxyConfig>,
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
    #[cfg(feature = "msgraph")]
    #[serde(default)]
    pub msgraph: Option<MsgraphConfig>,
    #[cfg(feature = "gmail")]
    #[serde(default)]
    pub gmail: Option<GmailConfig>,
    #[cfg(feature = "gcal")]
    #[serde(default)]
    pub gcal: Option<GcalConfig>,
    #[cfg(feature = "gpeople")]
    #[serde(default)]
    pub gpeople: Option<GpeopleConfig>,
}

impl AccountConfig {
    /// Renders this account as an `[accounts.<name>]` block, to be written
    /// to a configuration file or appended to one.
    ///
    /// The serializer decides what is written, so a defaulted field is
    /// omitted. What this adds is reading order: alphabetical dotted keys
    /// bury `imap.server` under the credentials authenticating against it.
    pub fn render(&self, name: &str) -> Result<String> {
        // NOTE: borrowed rather than built into a `Config`, which would
        // mean cloning the account to render it. The emitter only looks
        // for an `accounts` table, so any shape carrying one will do.
        #[derive(Serialize)]
        struct AccountDocument<'a> {
            accounts: HashMap<&'a str, &'a AccountConfig>,
        }

        let document = AccountDocument {
            accounts: HashMap::from([(name, self)]),
        };
        let rendered = pimalaya_config::toml::to_string(&document)?;

        // NOTE: the emitter writes the header itself, everything below it
        // being one dotted key per line.
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

            // NOTE: a backend reads the way it is explained: what it
            // watches, where, who it authenticates as, then the rest.
            lines.sort_by_key(|line| backend_rank(&key, line));

            for line in lines {
                document.push_str(line);
                document.push('\n');
            }
        }

        Ok(document)
    }

    /// Hands the account proxy to every network backend naming none of
    /// its own.
    #[cfg(network)]
    fn inherit_proxy(&mut self) {
        let Some(proxy) = &self.proxy else {
            return;
        };

        let slots = [
            #[cfg(feature = "imap")]
            self.imap.as_mut().map(|c| &mut c.proxy),
            #[cfg(feature = "jmap")]
            self.jmap.as_mut().map(|c| &mut c.proxy),
            #[cfg(feature = "dav")]
            self.caldav.as_mut().map(|c| &mut c.proxy),
            #[cfg(feature = "dav")]
            self.carddav.as_mut().map(|c| &mut c.proxy),
            #[cfg(feature = "msgraph")]
            self.msgraph.as_mut().map(|c| &mut c.proxy),
            #[cfg(feature = "gmail")]
            self.gmail.as_mut().map(|c| &mut c.proxy),
            #[cfg(feature = "gcal")]
            self.gcal.as_mut().map(|c| &mut c.proxy),
            #[cfg(feature = "gpeople")]
            self.gpeople.as_mut().map(|c| &mut c.proxy),
        ];

        for slot in slots.into_iter().flatten() {
            slot.get_or_insert_with(|| proxy.clone());
        }
    }

    /// Refuses a hook whose notification names a variable its event cannot
    /// fill, which serde cannot see: a template is a string until expanded.
    pub fn validate(&self) -> Result<()> {
        #[cfg(feature = "imap")]
        if let Some(imap) = &self.imap {
            imap.hook.validate()?;
        }

        #[cfg(feature = "jmap")]
        if let Some(jmap) = &self.jmap {
            jmap.validate()?;
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

        #[cfg(feature = "msgraph")]
        if let Some(msgraph) = &self.msgraph {
            msgraph.validate()?;
        }

        #[cfg(feature = "gmail")]
        if let Some(gmail) = &self.gmail {
            gmail.hook.validate()?;
        }

        #[cfg(feature = "gcal")]
        if let Some(gcal) = &self.gcal {
            gcal.hook.validate()?;
        }

        #[cfg(feature = "gpeople")]
        if let Some(gpeople) = &self.gpeople {
            gpeople.hook.validate()?;
        }

        Ok(())
    }
}

#[cfg(feature = "imap")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ImapConfig {
    /// The one mailbox this account watches.
    ///
    /// Watching a second is a second account, which is also how it gets
    /// its own hooks.
    pub mailbox: String,
    /// IMAP server address.
    ///
    /// A bare authority (`imap.example.org[:port]`) is read as
    /// `imaps://<authority>`; an `imap://` (cleartext, optionally upgraded
    /// with STARTTLS) or `imaps://` URL is used verbatim.
    pub server: String,
    #[serde(default)]
    pub tls: TlsConfig,
    #[serde(default, skip_serializing_if = "is_default")]
    pub starttls: bool,
    /// The ALPN identifiers offered during the TLS handshake.
    ///
    /// Unset takes io-imap's own default, the `["imap"]` RFC 7595
    /// registers; `[]` skips ALPN negotiation, and a non-empty list
    /// replaces the default. Only rustls reads it, native-tls having no
    /// ALPN.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpn: Option<Vec<String>>,
    /// Proxy the connection goes through, falling back to the account one
    /// and then to the `all_proxy`/`https_proxy` environment variables.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy: Option<ProxyConfig>,
    /// The SASL credentials, omitted to skip authentication entirely.
    pub sasl: Option<SaslConfig>,
    /// Forces the RFC 4959 SASL-IR initial response on or off.
    ///
    /// Unset follows the advertised `SASL-IR` capability, which Coremail
    /// (126.com, 163.com) advertises falsely: set it to `false` there.
    #[serde(default)]
    pub sasl_ir: Option<bool>,
    /// RFC 2971 `ID` quirks, `id.auto = true` opting in to the exchange
    /// mail.qq.com and Fastmail require straight after authentication.
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
    /// Chains an `ID` round-trip after the tagged auth response.
    #[serde(default, skip_serializing_if = "is_default")]
    pub auto: bool,
    /// Parameters sent with the auto-ID command, empty sending `ID NIL`.
    ///
    /// `true` sends carillon's canned value for the well-known keys
    /// (`name`, `version`, `vendor`, `support-url`) and `NIL` for the
    /// others, `false` always `NIL`. An absent key is not transmitted.
    #[serde(default)]
    pub fields: HashMap<String, bool>,
}

/// Resolves an [`ImapIdConfig`] into the wire parameters the io-imap auth
/// coroutines take.
///
/// [`None`] when `auto = false`. Each entry maps its key to carillon's
/// canned value or to `NIL`, an unknown key asked for a canned value
/// warning and falling back to `NIL`.
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

#[cfg(feature = "jmap")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct JmapConfig {
    /// The mailbox this account watches, matched by name and
    /// case-insensitively, `INBOX` falling back to the special-use role.
    ///
    /// The three collections are each optional, at least one required:
    /// they share the session, the connection and the event stream.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mailbox: Option<String>,
    /// The addressbook this account watches (RFC 9610), matched by name
    /// case-insensitively or by id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub addressbook: Option<String>,
    /// The calendar this account watches (JMAP for Calendars), matched by
    /// name case-insensitively or by id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calendar: Option<String>,
    /// JMAP server address.
    ///
    /// A bare authority (`fastmail.com`, `mail.example.org:8080`) is
    /// discovered through `GET /.well-known/jmap`, a full URL points
    /// straight at the session endpoint.
    pub server: String,
    #[serde(default)]
    pub tls: TlsConfig,
    /// The ALPN identifiers offered during the TLS handshake.
    ///
    /// Unset takes io-jmap's own default, `["http/1.1"]`, JMAP riding on
    /// HTTP/1.1; `[]` skips ALPN negotiation, and a non-empty list
    /// replaces the default. Only rustls reads it, native-tls having no
    /// ALPN.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpn: Option<Vec<String>>,
    /// Proxy the connection goes through, falling back to the account one
    /// and then to the `all_proxy`/`https_proxy` environment variables.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy: Option<ProxyConfig>,
    /// Authentication: exactly one of `header`, `bearer`, `basic`.
    pub auth: JmapAuthConfig,
    /// How this account learns about a change. Unset holds the stream.
    #[serde(default)]
    pub watch: Option<JmapWatchConfig>,
    /// The hooks this backend fires.
    #[serde(default, alias = "hooks")]
    pub hook: DomainsHookConfig,
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

#[cfg(feature = "maildir")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct MaildirConfig {
    /// The mailbox this account watches, resolved under `root` through
    /// io-maildir's store; `.` and `INBOX` both name the root itself.
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

/// Proxy configuration.
///
/// `url` is a `socks5://`, `socks5h://` or `http://` proxy URL. Its user
/// info authenticates too, but `username` and `password` keep the secret
/// out of the URL.
#[cfg(network)]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ProxyConfig {
    /// The proxy URL.
    pub url: String,
    /// The proxy username, required by `password`.
    pub username: Option<String>,
    /// The proxy password.
    pub password: Option<Secret>,
}

#[cfg(network)]
impl ProxyConfig {
    /// Resolves an optional configuration, an absent one reading the
    /// environment at connect time.
    pub fn resolve(config: Option<Self>, resolver: &mut SecretResolver) -> Result<Proxy> {
        match config {
            Some(config) => config.try_into_proxy(resolver),
            None => Ok(Proxy::System),
        }
    }

    /// Resolves the configuration into a runtime [`Proxy`], the password
    /// going through `resolver`.
    pub fn try_into_proxy(self, resolver: &mut SecretResolver) -> Result<Proxy> {
        let mut proxy = Proxy::from_url(&self.url)?;

        let auth = match (self.username, self.password) {
            (None, None) => return Ok(proxy),
            (None, Some(_)) => bail!("Proxy password requires a username"),
            (Some(user), pass) => ProxyAuth {
                user,
                pass: match pass {
                    Some(pass) => resolver.resolve(pass)?,
                    None => SecretString::default(),
                },
            },
        };

        match &mut proxy {
            Proxy::Socks5 { auth: slot, .. } | Proxy::Http { auth: slot, .. } => {
                *slot = Some(auth);
            }
            Proxy::None | Proxy::System => {}
        }

        Ok(proxy)
    }
}

#[cfg(network)]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct TlsConfig {
    /// The TLS implementation the connection is opened with, unset taking
    /// the first one compiled in.
    pub provider: Option<TlsProviderConfig>,
    /// The rustls options, read by the rustls provider alone.
    #[serde(default)]
    pub rustls: RustlsConfig,
    /// An additional certificate to trust, in PEM format.
    #[serde(default, deserialize_with = "opt_shell_expanded_path")]
    pub cert: Option<PathBuf>,
}

#[cfg(network)]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum TlsProviderConfig {
    Rustls,
    NativeTls,
}

#[cfg(network)]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct RustlsConfig {
    pub crypto: Option<RustlsCryptoConfig>,
}

#[cfg(network)]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum RustlsCryptoConfig {
    Aws,
    Ring,
}

#[cfg(network)]
impl TlsConfig {
    /// Builds the runtime [`Tls`] handle the connect helpers take, folding
    /// in the ALPN list its backend resolved.
    ///
    /// The schema never exposes `tls.rustls.alpn`, the per-backend `alpn`
    /// key standing for it, so a new call site has to say what it
    /// negotiates rather than silently negotiate nothing.
    pub fn into_tls(self, alpn: Vec<String>) -> Tls {
        Tls {
            provider: self.provider.map(|p| match p {
                TlsProviderConfig::Rustls => TlsProvider::Rustls,
                TlsProviderConfig::NativeTls => TlsProvider::NativeTls,
            }),
            rustls: Rustls {
                crypto: self.rustls.crypto.map(|c| match c {
                    RustlsCryptoConfig::Aws => RustlsCrypto::Aws,
                    RustlsCryptoConfig::Ring => RustlsCrypto::Ring,
                }),
                alpn,
            },
            cert: self.cert,
        }
    }
}

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
    /// Resolves the SASL config into a runtime [`Sasl`].
    ///
    /// `host` and `port` come from the live server URL, and only
    /// OAUTHBEARER uses them, echoed in its GS2 header. `resolver` spawns
    /// each distinct credential command once.
    pub fn try_into_sasl(
        self,
        host: impl ToString,
        port: u16,
        resolver: &mut SecretResolver,
    ) -> Result<Sasl> {
        Ok(match self {
            SaslConfig::Anonymous(c) => Sasl::Anonymous(SaslAnonymousCreds { message: c.message }),
            SaslConfig::Login(c) => Sasl::Login(SaslLoginCreds {
                username: c.username,
                password: resolver.resolve(c.password)?,
            }),
            SaslConfig::Plain(c) => Sasl::Plain(SaslPlainCreds {
                authzid: c.authzid,
                authcid: c.authcid,
                passwd: resolver.resolve(c.passwd)?,
            }),
            SaslConfig::Oauthbearer(c) => Sasl::Oauthbearer(SaslOauthbearerCreds {
                username: c.username,
                host: host.to_string(),
                port,
                token: resolver.resolve(c.token)?,
            }),
            SaslConfig::Xoauth2(c) => Sasl::Xoauth2(SaslXoauth2Creds {
                username: c.username,
                token: resolver.resolve(c.token)?,
            }),
            // NOTE: an empty nonce means "draw one for me": the client
            // fills it before the exchange, an I/O-free coroutine having
            // no way to generate randomness.
            SaslConfig::ScramSha256(c) => Sasl::ScramSha256(SaslScramCreds {
                username: c.username,
                password: resolver.resolve(c.password)?,
                nonce: Vec::new(),
                channel_binding: SaslGs2ChannelBinding::Unsupported,
            }),
        })
    }
}

// NOTE: each backend names the collection it watches in its own domain's
// word, and a hook templates against that same word, so the name is
// declared once here and read by both.

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

// NOTE: JMAP and Graph each serve mail, contacts and calendars, so one
// block takes a collection per domain, and both read and check them the
// same way.

/// The domains a backend serving several watches, in the order its rounds
/// run.
#[cfg(any(feature = "jmap", feature = "msgraph"))]
const DOMAINS: [WatchDomain; 3] = [WatchDomain::Message, WatchDomain::Card, WatchDomain::Event];

/// The collection an event of `domain` is about, under its own name, out
/// of the three keys a multi-domain block carries.
#[cfg(any(feature = "jmap", feature = "msgraph"))]
fn domain_collection<'a>(
    domain: WatchDomain,
    mailbox: &'a Option<String>,
    addressbook: &'a Option<String>,
    calendar: &'a Option<String>,
) -> Option<HookCollection<'a>> {
    let value = match domain {
        WatchDomain::Message => mailbox,
        WatchDomain::Card => addressbook,
        WatchDomain::Event => calendar,
        WatchDomain::Task => return None,
    };

    Some(HookCollection {
        name: domain.collection_name(),
        value: value.as_deref()?,
    })
}

/// Refuses a multi-domain block watching nothing, and a hook whose domain
/// has no collection.
///
/// The second is not serde's to refuse: what a hook may be depends on a
/// sibling key rather than on the table's own shape.
#[cfg(any(feature = "jmap", feature = "msgraph"))]
fn refuse_unwatched(
    backend: &str,
    collection: impl Fn(WatchDomain) -> bool,
    hooks: impl Iterator<Item = (&'static str, WatchDomain)>,
) -> Result<()> {
    if !DOMAINS.into_iter().any(&collection) {
        bail!(
            "Backend `{backend}` needs at least one of `{backend}.mailbox`, \
             `{backend}.addressbook` and `{backend}.calendar`"
        );
    }

    for (name, domain) in hooks {
        if !collection(domain) {
            let key = domain.collection_name();
            bail!(
                "Hook `{backend}.hook.{name}` needs `{backend}.{key}`, which this account does not configure"
            );
        }
    }

    Ok(())
}

#[cfg(feature = "jmap")]
impl JmapConfig {
    /// The collection an event of `domain` is about, under its own name,
    /// when the account configures one.
    pub fn collection(&self, domain: WatchDomain) -> Option<HookCollection<'_>> {
        domain_collection(domain, &self.mailbox, &self.addressbook, &self.calendar)
    }

    /// Every collection this backend watches, with its domain.
    pub fn collections(&self) -> Vec<(WatchDomain, HookCollection<'_>)> {
        DOMAINS
            .into_iter()
            .filter_map(|domain| Some((domain, self.collection(domain)?)))
            .collect()
    }

    /// Refuses a block watching nothing, a hook whose domain has no
    /// collection, and a notification naming what its event cannot fill.
    pub fn validate(&self) -> Result<()> {
        let collection = |domain| self.collection(domain).is_some();
        refuse_unwatched("jmap", collection, self.hook.configured())?;
        self.hook.validate("jmap")
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

// NOTE: each backend declares only the events it reports, so a hook it
// cannot fire is refused when the file is read rather than staying quiet
// forever. The events are named after their domain, which is why the
// tables below do not share a shape: mail has no edit, WebDAV no flags.

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

/// Hooks a JMAP or a Microsoft Graph watch fires, one set per domain.
#[cfg(any(feature = "jmap", feature = "msgraph"))]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct DomainsHookConfig {
    /// Fires when a message arrives in the watched mailbox.
    pub on_message_added: Option<ItemHook>,
    /// Fires when a message leaves it.
    pub on_message_removed: Option<ItemHook>,
    /// Fires once for each keyword set on a message.
    pub on_flag_added: Option<FlagHook>,
    /// Fires once for each keyword cleared on a message.
    pub on_flag_removed: Option<FlagHook>,
    /// Fires when a contact appears in the watched addressbook.
    pub on_card_added: Option<ItemHook>,
    /// Fires when a contact leaves it.
    pub on_card_removed: Option<ItemHook>,
    /// Fires when a contact is edited where it stands.
    pub on_card_changed: Option<ItemHook>,
    /// Fires when an event appears in the watched calendar.
    pub on_event_added: Option<ItemHook>,
    /// Fires when an event leaves it.
    pub on_event_removed: Option<ItemHook>,
    /// Fires when an event is edited where it stands.
    pub on_event_changed: Option<ItemHook>,
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

/// The hook one event resolved to, in either of the two shapes a hook is
/// written in.
// NOTE: which shapes can be constructed depends on the backends compiled
// in, so a build with no flag-carrying backend leaves one unused.
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

#[cfg(any(feature = "jmap", feature = "msgraph"))]
impl DomainsHookConfig {
    /// The hook `event` calls for, which depends on the domain the
    /// method that answered was about.
    pub fn get(&self, event: &WatchEvent) -> Option<Hook<'_>> {
        let hook = match (event.domain(), event) {
            (WatchDomain::Message, WatchEvent::FlagAdded { .. }) => {
                return self.on_flag_added.as_ref().map(Hook::Flag);
            }
            (WatchDomain::Message, WatchEvent::FlagRemoved { .. }) => {
                return self.on_flag_removed.as_ref().map(Hook::Flag);
            }
            (WatchDomain::Message, WatchEvent::ItemAdded { .. }) => &self.on_message_added,
            (WatchDomain::Message, WatchEvent::ItemRemoved { .. }) => &self.on_message_removed,
            (WatchDomain::Card, WatchEvent::ItemAdded { .. }) => &self.on_card_added,
            (WatchDomain::Card, WatchEvent::ItemRemoved { .. }) => &self.on_card_removed,
            (WatchDomain::Card, WatchEvent::ItemChanged { .. }) => &self.on_card_changed,
            (WatchDomain::Event, WatchEvent::ItemAdded { .. }) => &self.on_event_added,
            (WatchDomain::Event, WatchEvent::ItemRemoved { .. }) => &self.on_event_removed,
            (WatchDomain::Event, WatchEvent::ItemChanged { .. }) => &self.on_event_changed,
            _ => return None,
        };

        hook.as_ref().map(Hook::Item)
    }

    /// The hooks this table configures, by name, with the domain each
    /// fires for.
    fn configured(&self) -> impl Iterator<Item = (&'static str, WatchDomain)> {
        [
            (
                "on-message-added",
                WatchDomain::Message,
                self.on_message_added.is_some(),
            ),
            (
                "on-message-removed",
                WatchDomain::Message,
                self.on_message_removed.is_some(),
            ),
            (
                "on-flag-added",
                WatchDomain::Message,
                self.on_flag_added.is_some(),
            ),
            (
                "on-flag-removed",
                WatchDomain::Message,
                self.on_flag_removed.is_some(),
            ),
            (
                "on-card-added",
                WatchDomain::Card,
                self.on_card_added.is_some(),
            ),
            (
                "on-card-removed",
                WatchDomain::Card,
                self.on_card_removed.is_some(),
            ),
            (
                "on-card-changed",
                WatchDomain::Card,
                self.on_card_changed.is_some(),
            ),
            (
                "on-event-added",
                WatchDomain::Event,
                self.on_event_added.is_some(),
            ),
            (
                "on-event-removed",
                WatchDomain::Event,
                self.on_event_removed.is_some(),
            ),
            (
                "on-event-changed",
                WatchDomain::Event,
                self.on_event_changed.is_some(),
            ),
        ]
        .into_iter()
        .filter_map(|(name, domain, set)| set.then_some((name, domain)))
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
    /// component its member turned out to be.
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

    /// The components this table has hooks for, which a calendar
    /// advertising only some of them is checked against.
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
    /// IMAP resolves an arrival's envelope, on a second connection, so
    /// its arrival hook may name one.
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

#[cfg(any(feature = "jmap", feature = "msgraph"))]
impl DomainsHookConfig {
    /// Refuses a notification naming what its event cannot fill.
    ///
    /// Both backends read an envelope from the request their round already
    /// makes, so a message arrival hook may name one. `backend` prefixes
    /// the names an error reports.
    pub fn validate(&self, backend: &str) -> Result<()> {
        let mailbox = WatchDomain::Message.collection_name();

        hook::validate(
            self.on_message_added
                .as_ref()
                .and_then(|h| h.notify.as_ref()),
            Vocabulary::resolved(mailbox),
            &format!("{backend}.hook.on-message-added"),
        )?;
        hook::validate(
            self.on_message_removed
                .as_ref()
                .and_then(|h| h.notify.as_ref()),
            Vocabulary::item(mailbox),
            &format!("{backend}.hook.on-message-removed"),
        )?;
        hook::validate(
            self.on_flag_added.as_ref().and_then(|h| h.notify.as_ref()),
            Vocabulary::flag(mailbox),
            &format!("{backend}.hook.on-flag-added"),
        )?;
        hook::validate(
            self.on_flag_removed
                .as_ref()
                .and_then(|h| h.notify.as_ref()),
            Vocabulary::flag(mailbox),
            &format!("{backend}.hook.on-flag-removed"),
        )?;

        for (hook, domain, name) in [
            (&self.on_card_added, WatchDomain::Card, "on-card-added"),
            (&self.on_card_removed, WatchDomain::Card, "on-card-removed"),
            (&self.on_card_changed, WatchDomain::Card, "on-card-changed"),
            (&self.on_event_added, WatchDomain::Event, "on-event-added"),
            (
                &self.on_event_removed,
                WatchDomain::Event,
                "on-event-removed",
            ),
            (
                &self.on_event_changed,
                WatchDomain::Event,
                "on-event-changed",
            ),
        ] {
            let notify = hook.as_ref().and_then(|hook| hook.notify.as_ref());
            hook::validate(
                notify,
                Vocabulary::item(domain.collection_name()),
                &format!("{backend}.hook.{name}"),
            )?;
        }

        Ok(())
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
/// Summary and body template on `$name` / `${name}`, with `$id` and the
/// collection always available. The envelope names (`$subject`, `$sender`,
/// …) belong to an arrival on IMAP or JMAP, which read one.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ItemHook {
    pub notify: Option<NotifyConfig>,
    pub cmd: Option<HookCmd>,
}

/// Hook that fires for flag-level events, once per flag that moved.
///
/// `flags` narrows it to the flags it names, matched case-insensitively
/// with or without an IMAP backslash or a keyword dollar. The flag a
/// firing is about reaches the templates as `$flag`.
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

/// Shell-command payload, deserialized by [`pimalaya_config::command`].
///
/// A TOML string goes through the platform shell (`/bin/sh -c <line>` on
/// Unix, `cmd /C <line>` on Windows), a TOML list `[program, args…]` is
/// spawned directly. Both shapes take the variables in their environment.
#[derive(Debug, Deserialize, Serialize)]
pub struct HookCmd(#[serde(with = "command")] pub Command);

impl Clone for HookCmd {
    fn clone(&self) -> Self {
        // NOTE: `Command` is not `Clone`, so a fresh one is rebuilt from
        // the same program and args, as `Secret` does.
        let mut new = Command::new(self.0.get_program());
        new.args(self.0.get_args());
        Self(new)
    }
}

// NOTE: CalDAV and CardDAV are WebDAV, so the transport half is one
// shape; what differs is the domain the collection holds, which is what
// names the events. Two blocks rather than one is what lets a card hook
// on a calendar be refused when the file is read, and they are written
// out rather than flattened because serde cannot deny unknown fields
// across a flatten.

/// CalDAV configuration: one watched calendar, polled through RFC 6578
/// `sync-collection`.
#[cfg(feature = "dav")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CaldavConfig {
    /// The calendar this account watches, read as a path under `server`,
    /// or as an absolute path when it starts with a slash.
    pub calendar: String,
    /// The DAV server URL, `http://` or `https://`, naming the root the
    /// calendar hangs under.
    pub server: String,
    #[serde(default)]
    pub tls: TlsConfig,
    /// The ALPN identifiers offered during the TLS handshake.
    ///
    /// Unset takes io-http's own default, `["http/1.1"]`, WebDAV riding on
    /// HTTP/1.1; `[]` skips ALPN negotiation, and a non-empty list
    /// replaces the default. Only rustls reads it, native-tls having no
    /// ALPN.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpn: Option<Vec<String>>,
    /// Proxy the connection goes through, falling back to the account one
    /// and then to the `all_proxy`/`https_proxy` environment variables.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy: Option<ProxyConfig>,
    /// Authentication, none by default, for a calendar readable without.
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
    /// The DAV server URL, `http://` or `https://`, naming the root the
    /// addressbook hangs under.
    pub server: String,
    #[serde(default)]
    pub tls: TlsConfig,
    /// The ALPN identifiers offered during the TLS handshake.
    ///
    /// Unset takes io-http's own default, `["http/1.1"]`, WebDAV riding on
    /// HTTP/1.1; `[]` skips ALPN negotiation, and a non-empty list
    /// replaces the default. Only rustls reads it, native-tls having no
    /// ALPN.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpn: Option<Vec<String>>,
    /// Proxy the connection goes through, falling back to the account one
    /// and then to the `all_proxy`/`https_proxy` environment variables.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy: Option<ProxyConfig>,
    /// Authentication, none by default, for a book readable without.
    #[serde(default, skip_serializing_if = "DavAuthConfig::is_none")]
    pub auth: DavAuthConfig,
    /// How this account learns about a change. Unset polls.
    #[serde(default)]
    pub watch: Option<DavWatchConfig>,
    /// The hooks this backend fires.
    #[serde(default, alias = "hooks")]
    pub hook: CarddavHookConfig,
}

/// The transport half of a DAV backend, shared by a calendar and an
/// addressbook.
#[cfg(feature = "dav")]
pub struct DavServer<'a> {
    pub server: &'a str,
    pub tls: &'a TlsConfig,
    pub alpn: Option<&'a [String]>,
    pub proxy: Option<&'a ProxyConfig>,
    pub auth: &'a DavAuthConfig,
}

#[cfg(feature = "dav")]
impl CaldavConfig {
    /// What it takes to open a connection to this server.
    pub fn server(&self) -> DavServer<'_> {
        DavServer {
            server: &self.server,
            tls: &self.tls,
            alpn: self.alpn.as_deref(),
            proxy: self.proxy.as_ref(),
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
            alpn: self.alpn.as_deref(),
            proxy: self.proxy.as_ref(),
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
    /// HTTP Basic (RFC 7617), what most DAV servers ask for.
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
    /// Whether the server is reached with no `Authorization` header, which
    /// is what a generated document leaves out.
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

// NOTE: each backend declares only the methods it has, so asking IMAP to
// push or Maildir to idle is refused when the config is read rather than
// when the watch runs.

/// How an IMAP account learns about a change.
#[cfg(feature = "imap")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum ImapWatchConfig {
    /// Hold an IDLE connection and let the server speak first.
    Idle(IdleWatchConfig),
    /// Re-read the mailbox on an interval, for an IDLE that cannot be
    /// trusted.
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
    /// Re-list the directory on an interval, the only way a filesystem
    /// with no notification channel leaves.
    Poll(PollWatchConfig),
}

/// How a WebDAV account learns about a change.
#[cfg(feature = "dav")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum DavWatchConfig {
    /// Ask `sync-collection` on an interval, what WebDAV offers a client
    /// with no public endpoint.
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
    /// Short survives a NAT middle-box dropping a quiet connection, at a
    /// round trip per interval; a server known to hold one open is asked
    /// less often, up to the 29 minutes RFC 2177 allows.
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
    /// Seconds between the server's keep-alive pings, which are also what
    /// proves the stream is still there.
    #[serde(default = "default_push_ping")]
    pub ping: u64,
}

/// Options of the poll method.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct PollWatchConfig {
    /// Seconds between two rounds, unset taking what suits the backend: a
    /// couple of seconds for a directory read, longer for a remote one.
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

// NOTE: the vendor REST APIs authenticate with a bearer token alone and
// push only to a public endpoint or a Pub/Sub topic, so each block takes
// one token and the poll is its one method.

/// Microsoft Graph configuration: a collection per domain, at least one,
/// read through one token and one connection.
#[cfg(feature = "msgraph")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct MsgraphConfig {
    /// The mail folder this account watches, matched by display name
    /// case-insensitively, by id or by well-known name (`inbox`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mailbox: Option<String>,
    /// The contact folder this account watches, matched by display name
    /// case-insensitively or by id, `Contacts` naming the default one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub addressbook: Option<String>,
    /// The calendar this account watches, matched by name
    /// case-insensitively or by id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calendar: Option<String>,
    /// The mailbox owner, a user id or a principal name, unset meaning
    /// the token's own user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(default)]
    pub tls: TlsConfig,
    /// The ALPN identifiers offered during the TLS handshake, unset
    /// taking `["http/1.1"]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpn: Option<Vec<String>>,
    /// Proxy the connection goes through, falling back to the account one
    /// and then to the `all_proxy`/`https_proxy` environment variables.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy: Option<ProxyConfig>,
    /// Authentication: the bearer token.
    pub auth: BearerAuthConfig,
    /// How this account learns about a change. Unset polls.
    #[serde(default)]
    pub watch: Option<ApiWatchConfig>,
    /// The hooks this backend fires.
    #[serde(default, alias = "hooks")]
    pub hook: DomainsHookConfig,
}

#[cfg(feature = "msgraph")]
impl MsgraphConfig {
    /// The collection an event of `domain` is about, under its own name,
    /// when the account configures one.
    pub fn collection(&self, domain: WatchDomain) -> Option<HookCollection<'_>> {
        domain_collection(domain, &self.mailbox, &self.addressbook, &self.calendar)
    }

    /// Every collection this backend watches, with its domain.
    pub fn collections(&self) -> Vec<(WatchDomain, HookCollection<'_>)> {
        DOMAINS
            .into_iter()
            .filter_map(|domain| Some((domain, self.collection(domain)?)))
            .collect()
    }

    /// Refuses a block watching nothing, a hook whose domain has no
    /// collection, and a notification naming what its event cannot fill.
    pub fn validate(&self) -> Result<()> {
        let collection = |domain| self.collection(domain).is_some();
        refuse_unwatched("msgraph", collection, self.hook.configured())?;
        self.hook.validate("msgraph")
    }
}

/// Gmail configuration: one watched label.
#[cfg(feature = "gmail")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct GmailConfig {
    /// The label this account watches, by name (`INBOX`, `Work/Clients`)
    /// or by id.
    pub mailbox: String,
    /// The mailbox owner, unset meaning the token's own user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(default)]
    pub tls: TlsConfig,
    /// The ALPN identifiers offered during the TLS handshake, unset
    /// taking `["http/1.1"]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpn: Option<Vec<String>>,
    /// Proxy the connection goes through, falling back to the account one
    /// and then to the `all_proxy`/`https_proxy` environment variables.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy: Option<ProxyConfig>,
    /// Authentication: the bearer token.
    pub auth: BearerAuthConfig,
    /// How this account learns about a change. Unset polls.
    #[serde(default)]
    pub watch: Option<ApiWatchConfig>,
    /// The hooks this backend fires.
    #[serde(default, alias = "hooks")]
    pub hook: GmailHookConfig,
}

#[cfg(feature = "gmail")]
impl GmailConfig {
    /// The collection this backend watches, under its own name.
    pub fn collection(&self) -> HookCollection<'_> {
        HookCollection {
            name: WatchDomain::Message.collection_name(),
            value: &self.mailbox,
        }
    }
}

/// Google Calendar configuration: one watched calendar.
#[cfg(feature = "gcal")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct GcalConfig {
    /// The calendar this account watches, by name case-insensitively or
    /// by id, `primary` naming the account's own.
    pub calendar: String,
    #[serde(default)]
    pub tls: TlsConfig,
    /// The ALPN identifiers offered during the TLS handshake, unset
    /// taking `["http/1.1"]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpn: Option<Vec<String>>,
    /// Proxy the connection goes through, falling back to the account one
    /// and then to the `all_proxy`/`https_proxy` environment variables.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy: Option<ProxyConfig>,
    /// Authentication: the bearer token.
    pub auth: BearerAuthConfig,
    /// How this account learns about a change. Unset polls.
    #[serde(default)]
    pub watch: Option<ApiWatchConfig>,
    /// The hooks this backend fires.
    #[serde(default, alias = "hooks")]
    pub hook: GcalHookConfig,
}

#[cfg(feature = "gcal")]
impl GcalConfig {
    /// The collection this backend watches, under its own name.
    pub fn collection(&self) -> HookCollection<'_> {
        HookCollection {
            name: WatchDomain::Event.collection_name(),
            value: &self.calendar,
        }
    }
}

/// Google People configuration: one watched contact group.
#[cfg(feature = "gpeople")]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct GpeopleConfig {
    /// The contact group this account watches, by name case-insensitively
    /// or by resource name, `myContacts` holding every contact the
    /// account owns.
    pub addressbook: String,
    #[serde(default)]
    pub tls: TlsConfig,
    /// The ALPN identifiers offered during the TLS handshake, unset
    /// taking `["http/1.1"]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpn: Option<Vec<String>>,
    /// Proxy the connection goes through, falling back to the account one
    /// and then to the `all_proxy`/`https_proxy` environment variables.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy: Option<ProxyConfig>,
    /// Authentication: the bearer token.
    pub auth: BearerAuthConfig,
    /// How this account learns about a change. Unset polls.
    #[serde(default)]
    pub watch: Option<ApiWatchConfig>,
    /// The hooks this backend fires.
    #[serde(default, alias = "hooks")]
    pub hook: GpeopleHookConfig,
}

#[cfg(feature = "gpeople")]
impl GpeopleConfig {
    /// The collection this backend watches, under its own name.
    pub fn collection(&self) -> HookCollection<'_> {
        HookCollection {
            name: WatchDomain::Card.collection_name(),
            value: &self.addressbook,
        }
    }
}

/// The credential a vendor API takes, a bearer token alone.
#[cfg(api)]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct BearerAuthConfig {
    /// The OAuth 2.0 access token, usually a broker command since it
    /// expires.
    pub token: Secret,
}

/// How a vendor API account learns about a change.
#[cfg(api)]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum ApiWatchConfig {
    /// Read the change feed on an interval, what these APIs offer a
    /// client with no public endpoint.
    Poll(PollWatchConfig),
}

#[cfg(api)]
impl ApiWatchConfig {
    /// The poll interval, unset taking the backend default.
    pub fn interval(watch: &Option<Self>) -> Option<Duration> {
        let Some(Self::Poll(poll)) = watch else {
            return None;
        };

        poll.interval()
    }
}

/// Hooks a Gmail watch fires.
#[cfg(feature = "gmail")]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct GmailHookConfig {
    /// Fires when a message arrives in the watched label.
    pub on_message_added: Option<ItemHook>,
    /// Fires when a message leaves it, deleted or relabelled.
    pub on_message_removed: Option<ItemHook>,
    /// Fires once for each flag set on a message: `Seen` when `UNREAD`
    /// is cleared, `Flagged` when `STARRED` is set.
    pub on_flag_added: Option<FlagHook>,
    /// Fires once for each flag cleared on a message.
    pub on_flag_removed: Option<FlagHook>,
}

#[cfg(feature = "gmail")]
impl GmailHookConfig {
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

    /// Refuses a notification naming what its event cannot fill.
    ///
    /// The arrival's envelope rides the metadata the poll already reads,
    /// so its hook may name one.
    pub fn validate(&self) -> Result<()> {
        let mailbox = WatchDomain::Message.collection_name();
        hook::validate(
            self.on_message_added
                .as_ref()
                .and_then(|h| h.notify.as_ref()),
            Vocabulary::resolved(mailbox),
            "gmail.hook.on-message-added",
        )?;
        hook::validate(
            self.on_message_removed
                .as_ref()
                .and_then(|h| h.notify.as_ref()),
            Vocabulary::item(mailbox),
            "gmail.hook.on-message-removed",
        )?;
        hook::validate(
            self.on_flag_added.as_ref().and_then(|h| h.notify.as_ref()),
            Vocabulary::flag(mailbox),
            "gmail.hook.on-flag-added",
        )?;
        hook::validate(
            self.on_flag_removed
                .as_ref()
                .and_then(|h| h.notify.as_ref()),
            Vocabulary::flag(mailbox),
            "gmail.hook.on-flag-removed",
        )
    }
}

/// Hooks a Google Calendar watch fires, events alone: the API has no
/// task type.
#[cfg(feature = "gcal")]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct GcalHookConfig {
    /// Fires when an event appears in the watched calendar.
    pub on_event_added: Option<ItemHook>,
    /// Fires when an event leaves it.
    pub on_event_removed: Option<ItemHook>,
    /// Fires when an event is edited where it stands.
    pub on_event_changed: Option<ItemHook>,
}

#[cfg(feature = "gcal")]
impl GcalHookConfig {
    /// The hook `event` calls for, when one is configured.
    pub fn get(&self, event: &WatchEvent) -> Option<Hook<'_>> {
        let hook = match event {
            WatchEvent::ItemAdded { .. } => &self.on_event_added,
            WatchEvent::ItemRemoved { .. } => &self.on_event_removed,
            WatchEvent::ItemChanged { .. } => &self.on_event_changed,
            _ => return None,
        };

        hook.as_ref().map(Hook::Item)
    }

    /// Refuses a notification naming what its event cannot fill.
    pub fn validate(&self) -> Result<()> {
        for (hook, name) in [
            (&self.on_event_added, "on-event-added"),
            (&self.on_event_removed, "on-event-removed"),
            (&self.on_event_changed, "on-event-changed"),
        ] {
            let notify = hook.as_ref().and_then(|hook| hook.notify.as_ref());
            hook::validate(
                notify,
                Vocabulary::item(WatchDomain::Event.collection_name()),
                &format!("gcal.hook.{name}"),
            )?;
        }

        Ok(())
    }
}

/// Hooks a Google People watch fires.
#[cfg(feature = "gpeople")]
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct GpeopleHookConfig {
    /// Fires when a contact joins the watched group.
    pub on_card_added: Option<ItemHook>,
    /// Fires when one leaves it, deleted or ungrouped.
    pub on_card_removed: Option<ItemHook>,
    /// Fires when one is edited where it stands.
    pub on_card_changed: Option<ItemHook>,
}

#[cfg(feature = "gpeople")]
impl GpeopleHookConfig {
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
                Vocabulary::item(WatchDomain::Card.collection_name()),
                &format!("gpeople.hook.{name}"),
            )?;
        }

        Ok(())
    }
}

#[cfg(all(test, feature = "imap"))]
mod tests {
    use super::*;

    /// The minimal IMAP account the ALPN and TLS cases hang off.
    fn document(extra: &str) -> String {
        format!(
            "[accounts.perso]\n\
             imap.mailbox = \"INBOX\"\n\
             imap.server = \"imaps://imap.example.org\"\n\
             {extra}"
        )
    }

    #[test]
    fn a_tilde_certificate_path_is_expanded_at_deserialize() {
        let document = document("imap.tls.cert = \"~/certs/example.pem\"\n");
        let config: Config = toml::from_str(&document).expect("parse the config");
        let cert = config.accounts["perso"]
            .imap
            .as_ref()
            .and_then(|imap| imap.tls.cert.as_ref())
            .expect("a certificate path");

        assert!(cert.is_absolute(), "{} is not absolute", cert.display());
        assert!(cert.ends_with("certs/example.pem"));
    }

    #[test]
    fn an_unset_alpn_is_told_apart_from_an_empty_one() {
        let unset: Config = toml::from_str(&document("")).expect("parse the config");
        let empty: Config =
            toml::from_str(&document("imap.alpn = []\n")).expect("parse the config");
        let listed: Config =
            toml::from_str(&document("imap.alpn = [\"imap\"]\n")).expect("parse the config");

        let alpn = |config: &Config| config.accounts["perso"].imap.as_ref().unwrap().alpn.clone();

        assert_eq!(alpn(&unset), None);
        assert_eq!(alpn(&empty), Some(Vec::new()));
        assert_eq!(alpn(&listed), Some(vec![String::from("imap")]));
    }

    #[test]
    fn an_unset_alpn_is_not_rendered_back() {
        let config: Config =
            toml::from_str(&document("imap.alpn = []\n")).expect("parse the config");
        let rendered = config.accounts["perso"]
            .render("perso")
            .expect("render the account");
        let parsed: Config = toml::from_str(&rendered).expect("parse the rendered account");

        assert!(rendered.contains("imap.alpn = []"));
        assert_eq!(
            parsed.accounts["perso"].imap.as_ref().unwrap().alpn,
            Some(Vec::new())
        );

        let bare: Config = toml::from_str(&document("")).expect("parse the config");
        let rendered = bare.accounts["perso"]
            .render("perso")
            .expect("render the account");

        assert!(!rendered.contains("alpn"));
    }

    #[test]
    fn the_account_proxy_is_inherited_unless_the_backend_names_one() {
        let proxy = |extra: &str| {
            let config: Config = toml::from_str(&document(extra)).expect("parse the config");
            let config = config.with_inherited_proxies();
            let imap = config.accounts["perso"].imap.clone().unwrap();
            imap.proxy.map(|proxy| proxy.url)
        };

        let account = "proxy.url = \"socks5h://127.0.0.1:9050\"\n";
        let backend = "imap.proxy.url = \"http://proxy.example.org:3128\"\n";

        assert_eq!(proxy(""), None);
        assert_eq!(proxy(account).as_deref(), Some("socks5h://127.0.0.1:9050"));
        assert_eq!(
            proxy(&format!("{account}{backend}")).as_deref(),
            Some("http://proxy.example.org:3128")
        );
    }

    #[test]
    fn a_proxy_password_without_a_username_is_refused() {
        let config = ProxyConfig {
            url: String::from("socks5://127.0.0.1:1080"),
            username: None,
            password: Some(Secret::Raw(SecretString::from("secret"))),
        };

        assert!(config.try_into_proxy(&mut SecretResolver::new()).is_err());
    }

    /// A JMAP account in the shape the tests below vary.
    #[cfg(feature = "jmap")]
    fn jmap(extra: &str) -> Result<Config> {
        let document = format!(
            "[accounts.perso]\n\
             jmap.server = \"fastmail.com\"\n\
             jmap.auth.bearer.token.raw = \"token\"\n\
             {extra}"
        );
        let config: Config = toml::from_str(&document)?;
        config.accounts["perso"].validate()?;

        Ok(config)
    }

    #[cfg(feature = "jmap")]
    #[test]
    fn a_jmap_account_watches_at_least_one_collection() {
        let err = format!("{:#}", jmap("").expect_err("nothing to watch"));
        assert!(err.contains("at least one"), "got {err}");

        let config = jmap("jmap.addressbook = \"Personal\"\njmap.calendar = \"Work\"\n")
            .expect("contacts and a calendar without mail");
        let jmap = config.accounts["perso"].jmap.as_ref().unwrap();
        let names: Vec<_> = jmap.collections().iter().map(|(_, c)| c.name).collect();

        assert_eq!(vec!["addressbook", "calendar"], names);
    }

    #[cfg(feature = "jmap")]
    #[test]
    fn a_jmap_hook_needs_its_domain_s_collection() {
        let hook = "jmap.hook.on-card-added.cmd = \"true\"\n";

        let err = format!(
            "{:#}",
            jmap(&format!("jmap.mailbox = \"INBOX\"\n{hook}")).expect_err("no addressbook")
        );
        assert!(err.contains("jmap.hook.on-card-added"), "got {err}");
        assert!(err.contains("jmap.addressbook"), "got {err}");

        jmap(&format!("jmap.addressbook = \"Personal\"\n{hook}")).expect("an addressbook");
    }

    #[cfg(feature = "jmap")]
    #[test]
    fn a_jmap_card_hook_templates_against_its_addressbook() {
        let notify = "jmap.hook.on-card-added.notify.summary = \"$mailbox\"\n";
        let err = format!(
            "{:#}",
            jmap(&format!("jmap.addressbook = \"Personal\"\n{notify}")).expect_err("$mailbox")
        );
        assert!(err.contains("$addressbook"), "got {err}");

        let config = jmap("jmap.mailbox = \"INBOX\"\njmap.addressbook = \"Personal\"\n").unwrap();
        let jmap = config.accounts["perso"].jmap.as_ref().unwrap();
        let card = WatchEvent::ItemChanged {
            domain: WatchDomain::Card,
            id: String::from("A"),
        };

        assert_eq!(
            Some("Personal"),
            jmap.collection(card.domain()).map(|c| c.value)
        );
        assert!(jmap.hook.get(&card).is_none());
    }

    /// An account in the shape the vendor API tests vary, `block` being
    /// the backend's own lines.
    #[cfg(api)]
    fn api(block: &str) -> Result<Config> {
        let document = format!("[accounts.perso]\n{block}");
        let config: Config = toml::from_str(&document)?;
        config.accounts["perso"].validate()?;

        Ok(config)
    }

    #[cfg(feature = "msgraph")]
    #[test]
    fn a_graph_account_is_checked_like_a_jmap_one() {
        let token = "msgraph.auth.token.raw = \"token\"\n";

        let err = format!("{:#}", api(token).expect_err("nothing to watch"));
        assert!(err.contains("`msgraph.mailbox`"), "got {err}");

        let hook = "msgraph.hook.on-event-changed.cmd = \"true\"\n";
        let err = format!(
            "{:#}",
            api(&format!("{token}msgraph.mailbox = \"inbox\"\n{hook}")).expect_err("no calendar")
        );
        assert!(err.contains("msgraph.hook.on-event-changed"), "got {err}");
        assert!(err.contains("`msgraph.calendar`"), "got {err}");

        let config = api(&format!("{token}msgraph.calendar = \"Calendar\"\n{hook}"))
            .expect("a calendar and its hook");
        let msgraph = config.accounts["perso"].msgraph.as_ref().unwrap();
        let names: Vec<_> = msgraph.collections().iter().map(|(_, c)| c.name).collect();

        assert_eq!(vec!["calendar"], names);
    }

    #[cfg(feature = "gpeople")]
    #[test]
    fn a_google_contact_group_templates_as_an_addressbook() {
        let block = "gpeople.addressbook = \"myContacts\"\n\
                     gpeople.auth.token.raw = \"token\"\n";

        let config = api(&format!(
            "{block}gpeople.hook.on-card-changed.notify.summary = \"$addressbook\"\n"
        ))
        .expect("an addressbook hook");
        let gpeople = config.accounts["perso"].gpeople.as_ref().unwrap();
        assert_eq!("addressbook", gpeople.collection().name);

        let err = format!(
            "{:#}",
            api(&format!(
                "{block}gpeople.hook.on-card-added.notify.summary = \"$calendar\"\n"
            ))
            .expect_err("$calendar")
        );
        assert!(err.contains("$addressbook"), "got {err}");
    }

    /// The poll is the one method a vendor API has, and Google Calendar
    /// has no task type to hook.
    #[cfg(feature = "gcal")]
    #[test]
    fn a_google_calendar_takes_a_poll_and_no_task_hook() {
        let block = "gcal.calendar = \"primary\"\n\
                     gcal.auth.token.raw = \"token\"\n";

        let config = api(&format!("{block}gcal.watch.poll.interval = 120\n")).expect("a poll");
        let gcal = config.accounts["perso"].gcal.as_ref().unwrap();
        assert_eq!(
            Some(Duration::from_secs(120)),
            ApiWatchConfig::interval(&gcal.watch)
        );

        assert!(api(&format!("{block}gcal.watch.push.ping = 30\n")).is_err());
        assert!(api(&format!("{block}gcal.hook.on-task-added.cmd = \"true\"\n")).is_err());
    }
}
