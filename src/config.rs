//! # Configuration
//!
//! The TOML schema: one `[accounts.<name>]` block per account, in the
//! shape [himalaya CLI v2] and [himalaya TUI] use, each backend under its
//! own protocol key.
//!
//! Declaring more than one backend is allowed, `-b/--backend` picking the
//! active one. Every table refuses a key it does not read, the root and
//! the account included, so a file is not shared with the himalaya
//! binaries, and a key an earlier release read is refused as any unknown
//! one.
//! A backend the build leaves out is no unknown key: its block loads
//! unread, and a command reaching it fails naming the cargo feature.
//!
//! Everything a backend needs lives under it: the collection it watches,
//! under its own domain's name, how it watches, and the hooks it fires.
//! What it reports and what a hook may template against are the backend's
//! too, so anything else is refused when the file is read.
//!
//! [himalaya CLI v2]: https://github.com/pimalaya/himalaya
//! [himalaya TUI]: https://github.com/pimalaya/himalaya-tui

#[cfg(api)]
pub mod api;
#[cfg(feature = "caldav")]
pub mod caldav;
#[cfg(feature = "carddav")]
pub mod carddav;
#[cfg(dav)]
pub mod dav;
#[cfg(any(feature = "jmap", feature = "msgraph"))]
pub mod domains;
#[cfg(feature = "gcal")]
pub mod gcal;
#[cfg(feature = "gmail")]
pub mod gmail;
#[cfg(feature = "gpeople")]
pub mod gpeople;
#[cfg(feature = "imap")]
pub mod imap;
#[cfg(feature = "jmap")]
pub mod jmap;
#[cfg(feature = "maildir")]
pub mod maildir;
#[cfg(feature = "msgraph")]
pub mod msgraph;
#[cfg(feature = "pimdir")]
pub mod pimdir;
#[cfg(network)]
pub mod proxy;
#[cfg(network)]
pub mod tls;

use std::{
    collections::{BTreeSet, HashMap},
    path::PathBuf,
    time::Duration,
};

#[cfg(any(dav, feature = "jmap"))]
use anyhow::bail;
use anyhow::{Context, Result};
use pimalaya_config::{
    command::{CommandConfig, Timeout},
    notify::NotificationTemplate,
    toml::TomlConfig,
};
use serde::Deserialize;
#[cfg(not(all(
    feature = "imap",
    feature = "jmap",
    feature = "maildir",
    feature = "caldav",
    feature = "carddav",
    feature = "msgraph",
    feature = "gmail",
    feature = "gcal",
    feature = "gpeople",
    feature = "pimdir"
)))]
use serde::de;
#[cfg(any(dav, feature = "jmap"))]
use url::{ParseError, Url};

/// Root configuration: a map of named accounts.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Config {
    pub accounts: HashMap<String, AccountConfig>,
}

impl TomlConfig for Config {
    type Account = AccountConfig;

    fn project_name() -> &'static str {
        env!("CARGO_PKG_NAME")
    }

    fn sample_url() -> &'static str {
        "https://github.com/pimalaya/carillon/blob/master/config.sample.toml"
    }

    fn accounts(&self) -> impl Iterator<Item = (&str, bool)> {
        self.accounts
            .iter()
            .map(|(name, account)| (name.as_str(), account.default))
    }

    fn take_named_account(&mut self, name: &str) -> Option<(String, Self::Account)> {
        self.accounts.remove_entry(name)
    }
}

impl Config {
    /// Loads the config from `config_paths`.
    ///
    /// No configuration fails with pimalaya-config's `ConfigMissingError`,
    /// naming the path looked at, the first `-c` path or else the default
    /// one, and the documented sample: carillon writes none, an account
    /// being written by hand.
    pub fn load(config_paths: &[PathBuf]) -> Result<Config> {
        let config = <Config as TomlConfig>::load(config_paths)?;

        #[cfg(network)]
        let config = config.with_inherited_network();

        // NOTE: what a notification may name is as fixed as which hooks a
        // backend has, and serde cannot check it, a template being a
        // string until something expands it. Here, both are refused at
        // load time.
        for (name, account) in &config.accounts {
            account
                .validate()
                .with_context(|| format!("Account `{name}` is misconfigured"))?;
        }

        Ok(config)
    }

    /// Hands each account proxy and TLS table down to its backends, so
    /// every connection reads its own backend's keys.
    #[cfg(network)]
    fn with_inherited_network(mut self) -> Self {
        for account in self.accounts.values_mut() {
            account.inherit_proxy();
            account.inherit_tls();
        }

        self
    }
}

/// Per-account configuration.
///
/// A block of a backend the build leaves out is read as anything and kept
/// as present, so one file serves every build: a command reaching it
/// fails coded `backend-not-built`, naming the cargo feature.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct AccountConfig {
    /// Use this account when `-a/--account` names none.
    #[serde(default)]
    pub default: bool,
    /// Proxy every network backend of this account goes through, unless
    /// its own block names one.
    #[cfg(network)]
    #[serde(default)]
    pub proxy: Option<proxy::ProxyConfig>,
    /// Read and unused: this build has no network backend.
    #[cfg(not(network))]
    #[serde(default)]
    #[allow(dead_code)]
    pub proxy: Option<de::IgnoredAny>,
    /// TLS every network backend of this account takes, field by field,
    /// where its own `tls` table leaves one unset.
    #[cfg(network)]
    #[serde(default)]
    pub tls: Option<tls::TlsConfig>,
    /// Read and unused: this build has no network backend.
    #[cfg(not(network))]
    #[serde(default)]
    #[allow(dead_code)]
    pub tls: Option<de::IgnoredAny>,
    #[cfg(feature = "imap")]
    #[serde(default)]
    pub imap: Option<imap::ImapConfig>,
    #[cfg(not(feature = "imap"))]
    #[serde(default)]
    pub imap: Option<de::IgnoredAny>,
    #[cfg(feature = "jmap")]
    #[serde(default)]
    pub jmap: Option<jmap::JmapConfig>,
    #[cfg(not(feature = "jmap"))]
    #[serde(default)]
    pub jmap: Option<de::IgnoredAny>,
    #[cfg(feature = "maildir")]
    #[serde(default)]
    pub maildir: Option<maildir::MaildirConfig>,
    #[cfg(not(feature = "maildir"))]
    #[serde(default)]
    pub maildir: Option<de::IgnoredAny>,
    #[cfg(feature = "caldav")]
    #[serde(default)]
    pub caldav: Option<caldav::CaldavConfig>,
    #[cfg(not(feature = "caldav"))]
    #[serde(default)]
    pub caldav: Option<de::IgnoredAny>,
    #[cfg(feature = "carddav")]
    #[serde(default)]
    pub carddav: Option<carddav::CarddavConfig>,
    #[cfg(not(feature = "carddav"))]
    #[serde(default)]
    pub carddav: Option<de::IgnoredAny>,
    #[cfg(feature = "msgraph")]
    #[serde(default)]
    pub msgraph: Option<msgraph::MsgraphConfig>,
    #[cfg(not(feature = "msgraph"))]
    #[serde(default)]
    pub msgraph: Option<de::IgnoredAny>,
    #[cfg(feature = "gmail")]
    #[serde(default)]
    pub gmail: Option<gmail::GmailConfig>,
    #[cfg(not(feature = "gmail"))]
    #[serde(default)]
    pub gmail: Option<de::IgnoredAny>,
    #[cfg(feature = "gcal")]
    #[serde(default)]
    pub gcal: Option<gcal::GcalConfig>,
    #[cfg(not(feature = "gcal"))]
    #[serde(default)]
    pub gcal: Option<de::IgnoredAny>,
    #[cfg(feature = "gpeople")]
    #[serde(default)]
    pub gpeople: Option<gpeople::GpeopleConfig>,
    #[cfg(not(feature = "gpeople"))]
    #[serde(default)]
    pub gpeople: Option<de::IgnoredAny>,
    #[cfg(feature = "pimdir")]
    #[serde(default)]
    pub pimdir: Option<pimdir::PimdirConfig>,
    #[cfg(not(feature = "pimdir"))]
    #[serde(default)]
    pub pimdir: Option<de::IgnoredAny>,
}

impl AccountConfig {
    /// The backends the account carries a block for, as `--backend`
    /// spells them, in the order `auto` tries them, each with whether this
    /// build serves it.
    pub fn blocks(&self) -> Vec<(&'static str, bool)> {
        let blocks = [
            ("imap", self.imap.is_some(), cfg!(feature = "imap")),
            ("jmap", self.jmap.is_some(), cfg!(feature = "jmap")),
            ("maildir", self.maildir.is_some(), cfg!(feature = "maildir")),
            ("caldav", self.caldav.is_some(), cfg!(feature = "caldav")),
            ("carddav", self.carddav.is_some(), cfg!(feature = "carddav")),
            ("msgraph", self.msgraph.is_some(), cfg!(feature = "msgraph")),
            ("gmail", self.gmail.is_some(), cfg!(feature = "gmail")),
            ("gcal", self.gcal.is_some(), cfg!(feature = "gcal")),
            ("gpeople", self.gpeople.is_some(), cfg!(feature = "gpeople")),
            ("pimdir", self.pimdir.is_some(), cfg!(feature = "pimdir")),
        ];

        let carried = blocks.into_iter().filter(|(_, carried, _)| *carried);
        carried.map(|(name, _, built)| (name, built)).collect()
    }

    /// The backends the account carries a block for and this build serves,
    /// in the order `auto` tries them.
    pub fn backends(&self) -> Vec<&'static str> {
        let blocks = self.blocks().into_iter();
        blocks
            .filter_map(|(name, built)| built.then_some(name))
            .collect()
    }

    /// The backends the account carries a block for and this build leaves
    /// out, in the order `auto` tries them.
    pub fn not_built(&self) -> Vec<&'static str> {
        let blocks = self.blocks().into_iter();
        blocks
            .filter_map(|(name, built)| (!built).then_some(name))
            .collect()
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
            #[cfg(feature = "caldav")]
            self.caldav.as_mut().map(|c| &mut c.proxy),
            #[cfg(feature = "carddav")]
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

    /// Hands the account `tls` table to every network backend, each field
    /// its own table leaves unset.
    #[cfg(network)]
    fn inherit_tls(&mut self) {
        let Some(tls) = &self.tls else {
            return;
        };

        let slots = [
            #[cfg(feature = "imap")]
            self.imap.as_mut().map(|c| &mut c.tls),
            #[cfg(feature = "jmap")]
            self.jmap.as_mut().map(|c| &mut c.tls),
            #[cfg(feature = "caldav")]
            self.caldav.as_mut().map(|c| &mut c.tls),
            #[cfg(feature = "carddav")]
            self.carddav.as_mut().map(|c| &mut c.tls),
            #[cfg(feature = "msgraph")]
            self.msgraph.as_mut().map(|c| &mut c.tls),
            #[cfg(feature = "gmail")]
            self.gmail.as_mut().map(|c| &mut c.tls),
            #[cfg(feature = "gcal")]
            self.gcal.as_mut().map(|c| &mut c.tls),
            #[cfg(feature = "gpeople")]
            self.gpeople.as_mut().map(|c| &mut c.tls),
        ];

        for slot in slots.into_iter().flatten() {
            slot.inherit(tls);
        }
    }

    /// Refuses a hook whose notification names a variable its event cannot
    /// fill, which serde cannot see: a template is a string until expanded.
    pub fn validate(&self) -> Result<()> {
        #[cfg(feature = "imap")]
        if let Some(imap) = &self.imap {
            imap.hooks.validate()?;
        }

        #[cfg(feature = "jmap")]
        if let Some(jmap) = &self.jmap {
            jmap.validate()?;
        }

        #[cfg(feature = "maildir")]
        if let Some(maildir) = &self.maildir {
            maildir.hooks.validate()?;
        }

        #[cfg(feature = "caldav")]
        if let Some(caldav) = &self.caldav {
            caldav.server().validate("caldav")?;
            caldav.hooks.validate()?;
        }

        #[cfg(feature = "carddav")]
        if let Some(carddav) = &self.carddav {
            carddav.server().validate("carddav")?;
            carddav.hooks.validate()?;
        }

        #[cfg(feature = "msgraph")]
        if let Some(msgraph) = &self.msgraph {
            msgraph.validate()?;
        }

        #[cfg(feature = "gmail")]
        if let Some(gmail) = &self.gmail {
            gmail.hooks.validate()?;
        }

        #[cfg(feature = "gcal")]
        if let Some(gcal) = &self.gcal {
            gcal.hooks.validate()?;
        }

        #[cfg(feature = "gpeople")]
        if let Some(gpeople) = &self.gpeople {
            gpeople.hooks.validate()?;
        }

        #[cfg(feature = "pimdir")]
        if let Some(pimdir) = &self.pimdir {
            pimdir.hooks.validate()?;
        }

        Ok(())
    }
}

// NOTE: each backend names the collection it watches in its own domain's
// word, and a hook templates against that same word, so the name is
// declared once here and read by both.

// NOTE: each backend declares only the events it reports, so a hook it
// cannot fire is refused when the file is read rather than staying quiet
// forever. The events are named after their domain, which is why the
// tables below do not share a shape: mail has no edit but a Graph draft,
// WebDAV no flags.

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

/// Hook that fires for item-level events: added, removed, changed.
///
/// Summary and body template on `$name`, `${name}` and `${name:-default}`,
/// with `$id` and the collection always available. The envelope names
/// (`$subject`, `$sender`, …) belong to an arrival on a backend reading
/// one.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ItemHook {
    pub notify: Option<NotificationTemplate>,
    /// A shell line, or a program and its arguments, run with the
    /// variables in its environment.
    ///
    /// `cmd` is read as its alias, as everywhere a command is configured.
    #[serde(alias = "cmd")]
    pub command: Option<CommandConfig>,
    /// How long the command may run before it is sent SIGTERM, and
    /// SIGKILL once [`Timeout::GRACE`] has passed too.
    pub timeout: Option<Timeout>,
}

/// Hook that fires for flag-level events, once per flag that moved.
///
/// `flags` narrows it to the flags it names, matched case-insensitively
/// with or without an IMAP backslash or a keyword dollar. The flag a
/// firing is about reaches the templates as `$flag`.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct FlagHook {
    pub notify: Option<NotificationTemplate>,
    /// A shell line, or a program and its arguments, run with the
    /// variables in its environment.
    ///
    /// `cmd` is read as its alias, as everywhere a command is configured.
    #[serde(alias = "cmd")]
    pub command: Option<CommandConfig>,
    /// How long the command may run before it is sent SIGTERM, and
    /// SIGKILL once [`Timeout::GRACE`] has passed too.
    pub timeout: Option<Timeout>,
    #[serde(default)]
    pub flags: BTreeSet<String>,
}

/// Parses `input` into a URL, a host that is not ASCII refused with the
/// way out: its Punycode form, url linking no Unicode IDNA back end.
#[cfg(any(dav, feature = "jmap"))]
pub fn parse_url(input: &str) -> Result<Url> {
    match Url::parse(input) {
        Err(ParseError::IdnaError) => bail!(
            "Host `{}` is not ASCII: write it in its Punycode form (xn--...)",
            host_of(input)
        ),
        result => Ok(result?),
    }
}

/// The host `input` names, as written: what follows the scheme, short of
/// the path, the user info and the port.
#[cfg(any(dav, feature = "jmap"))]
fn host_of(input: &str) -> &str {
    let rest = input.split_once("://").map_or(input, |(_, rest)| rest);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);

    match host.rsplit_once(':') {
        Some((name, port)) if port.bytes().all(|b| b.is_ascii_digit()) => name,
        _ => host,
    }
}

// NOTE: each backend declares only the methods it has, so asking IMAP to
// push or Maildir to idle is refused when the config is read rather than
// when the watch runs.

/// Options of the poll method.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct PollWatchConfig {
    /// How long between two rounds, unset taking what suits the backend:
    /// a couple of seconds for a directory read, longer for a remote one.
    #[serde(default, with = "pimalaya_config::duration::option")]
    pub interval: Option<Duration>,
}

impl PollWatchConfig {
    /// The interval this config overrides the backend default with.
    pub fn interval(&self) -> Option<Duration> {
        self.interval
    }
}

#[cfg(all(test, feature = "imap"))]
mod tests {
    use super::*;
    use pimalaya_config::secret::{Secret, SecretResolver};
    use secrecy::SecretString;

    #[cfg(api)]
    use crate::config::api::ApiWatchConfig;
    #[cfg(dav)]
    use crate::config::dav::DavAuthConfig;
    use crate::config::{
        imap::{ImapWatchConfig, SaslConfig},
        proxy::ProxyConfig,
        tls::TlsProviderConfig,
    };
    #[cfg(feature = "jmap")]
    use crate::{
        config::jmap::{JmapWatchConfig, PushWatchConfig},
        event::{WatchDomain, WatchEvent},
    };

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
    fn the_account_proxy_is_inherited_unless_the_backend_names_one() {
        let proxy = |extra: &str| {
            let config: Config = toml::from_str(&document(extra)).expect("parse the config");
            let config = config.with_inherited_network();
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

    /// A backend's `tls` takes from the account's each field it leaves
    /// unset, the client certificate as one so it keeps its own key.
    #[test]
    fn the_account_tls_is_inherited_field_by_field() {
        let tls = |extra: &str| {
            let config: Config = toml::from_str(&document(extra)).expect("parse the config");
            let config = config.with_inherited_network();
            config.accounts["perso"].imap.clone().unwrap().tls
        };

        let account = "tls.provider = \"native-tls\"\n\
                       tls.cert = \"/ca.pem\"\n\
                       tls.client = { cert = \"/me.pem\", key = \"/me.key\" }\n";
        let backend = "imap.tls.cert = \"/imap.pem\"\n\
                       imap.tls.client = { cert = \"/imap-me.pem\", key = \"/imap-me.key\" }\n";

        let inherited = tls(account);
        assert!(matches!(
            inherited.provider,
            Some(TlsProviderConfig::NativeTls)
        ));
        assert_eq!(inherited.cert, Some(PathBuf::from("/ca.pem")));
        let client = inherited.client.unwrap();
        assert_eq!(client.key, PathBuf::from("/me.key"));

        let own = tls(&format!("{account}{backend}"));
        assert!(matches!(own.provider, Some(TlsProviderConfig::NativeTls)));
        assert_eq!(own.cert, Some(PathBuf::from("/imap.pem")));
        let client = own.client.unwrap();
        assert_eq!(client.cert, PathBuf::from("/imap-me.pem"));
        assert_eq!(client.key, PathBuf::from("/imap-me.key"));

        let err = toml::from_str::<Config>(&document("tls.client.cert = \"/me.pem\"\n"));
        let err = err.expect_err("a certificate without its key").to_string();
        assert!(err.contains("missing field `key`"), "{err}");
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
        let hook = "jmap.hooks.on-card-add.command = \"true\"\n";

        let err = format!(
            "{:#}",
            jmap(&format!("jmap.mailbox = \"INBOX\"\n{hook}")).expect_err("no addressbook")
        );
        assert!(err.contains("jmap.hooks.on-card-add"), "got {err}");
        assert!(err.contains("jmap.addressbook"), "got {err}");

        jmap(&format!("jmap.addressbook = \"Personal\"\n{hook}")).expect("an addressbook");
    }

    #[cfg(feature = "jmap")]
    #[test]
    fn a_jmap_message_edit_hook_is_refused() {
        let hook = "jmap.hooks.on-message-change.command = \"true\"\n";

        let err = format!(
            "{:#}",
            jmap(&format!("jmap.mailbox = \"INBOX\"\n{hook}")).expect_err("immutable")
        );
        assert!(err.contains("jmap.hooks.on-message-change"), "got {err}");
    }

    #[cfg(feature = "jmap")]
    #[test]
    fn a_jmap_card_hook_templates_against_its_addressbook() {
        let notify = "jmap.hooks.on-card-add.notify.summary = \"$mailbox\"\n";
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
        assert!(jmap.hooks.get(&card).is_none());
    }

    #[cfg(all(feature = "imap", not(feature = "notify")))]
    #[test]
    fn a_notification_is_refused_by_a_build_that_cannot_show_it() {
        let document = document("imap.hooks.on-message-add.notify.summary = \"$id\"\n");
        let err = toml::from_str::<Config>(&document).expect_err("no notify feature");

        assert!(err.to_string().contains("`notify` cargo feature"), "{err}");
    }

    #[cfg(all(feature = "caldav", feature = "carddav", feature = "pimdir"))]
    #[test]
    fn an_alarm_hook_is_a_calendar_s() {
        let read = |block: &str| -> Result<Config> {
            let config: Config = toml::from_str(&format!("[accounts.perso]\n{block}"))?;
            config.accounts["perso"].validate()?;
            Ok(config)
        };
        let caldav = "caldav.server = \"https://dav.example.org\"\n\
                      caldav.calendar = \"work\"\n";
        let pimdir = "pimdir.root = \"/tmp/pimdir\"\n\
                      pimdir.calendar = \"cal\"\n";

        read(&format!(
            "{caldav}caldav.hooks.on-alarm-trigger.notify.summary = \"$summary at $at\"\n"
        ))
        .expect("a calendar fires its alarms");

        let err = read(&format!(
            "{caldav}caldav.hooks.on-alarm-trigger.notify.summary = \"$subject\"\n"
        ))
        .expect_err("an alarm has no subject");
        assert!(
            format!("{err:#}").contains("caldav.hooks.on-alarm-trigger"),
            "{err:#}"
        );

        read(
            "carddav.server = \"https://dav.example.org\"\n\
             carddav.addressbook = \"default\"\n\
             carddav.hooks.on-alarm-trigger.command = \"true\"\n",
        )
        .expect_err("an addressbook holds no alarm");

        read(&format!(
            "{pimdir}pimdir.hooks.on-alarm-trigger.command = \"true\"\n"
        ))
        .expect("a pimdir calendar fires its alarms");

        let err = read(pimdir).expect_err("a pimdir account with nothing to fire");
        assert!(format!("{err:#}").contains("on-alarm-trigger"), "{err:#}");
    }

    /// A template from before the camelCase variables and the shell's
    /// default syntax is refused when read, naming what to rewrite.
    #[cfg(all(feature = "jmap", feature = "notify"))]
    #[test]
    fn a_template_in_a_former_spelling_is_refused() {
        for (template, named) in [
            ("$sender_name", "`sender_name`"),
            ("${subject:none}", "`${subject:none}`"),
        ] {
            let notify = format!("jmap.hooks.on-message-add.notify.summary = \"{template}\"\n");
            let err = format!(
                "{:#}",
                jmap(&format!("jmap.mailbox = \"INBOX\"\n{notify}")).expect_err(template)
            );
            assert!(err.contains(named), "{template}: {err}");
        }

        let notify =
            "jmap.hooks.on-message-add.notify.summary = \"$senderName ${subject:-none}\"\n";
        jmap(&format!("jmap.mailbox = \"INBOX\"\n{notify}")).expect("the current spelling");
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

        let hook = "msgraph.hooks.on-event-change.command = \"true\"\n";
        let err = format!(
            "{:#}",
            api(&format!("{token}msgraph.mailbox = \"inbox\"\n{hook}")).expect_err("no calendar")
        );
        assert!(err.contains("msgraph.hooks.on-event-change"), "got {err}");
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
            "{block}gpeople.hooks.on-card-change.notify.summary = \"$addressbook\"\n"
        ))
        .expect("an addressbook hook");
        let gpeople = config.accounts["perso"].gpeople.as_ref().unwrap();
        assert_eq!("addressbook", gpeople.collection().name);

        let err = format!(
            "{:#}",
            api(&format!(
                "{block}gpeople.hooks.on-card-add.notify.summary = \"$calendar\"\n"
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
        assert!(
            api(&format!(
                "{block}gcal.hooks.on-task-add.command = \"true\"\n"
            ))
            .is_err()
        );
    }

    #[test]
    fn an_unknown_key_is_refused_at_every_level() {
        let root = format!("colour = true\n{}", document(""));
        let mut documents = vec![("root", root)];

        for (level, line) in [
            ("account", "colour = true\n"),
            ("backend", "imap.colour = true\n"),
            ("tls", "imap.tls.colour = true\n"),
            ("sasl", "imap.sasl.plain.colour = true\n"),
            ("watch", "imap.watch.idle.colour = 30\n"),
            ("hook table", "imap.hooks.colour.command = \"true\"\n"),
            ("hook", "imap.hooks.on-message-add.colour = true\n"),
        ] {
            documents.push((level, document(line)));
        }

        for (level, document) in documents {
            let err = toml::from_str::<Config>(&document).expect_err(level);
            let err = err.to_string();
            assert!(err.contains("unknown field `colour`"), "{level}: {err}");
        }
    }

    /// A duration reads integer seconds as it did, or a number and its
    /// unit, pimalaya-config's shape; a value of neither is refused.
    #[test]
    fn a_duration_takes_seconds_or_a_unit() {
        let read = |line: &str| {
            let config: Config = toml::from_str(&document(line)).expect(line);
            config.accounts["perso"].imap.clone().unwrap().watch
        };

        for (value, secs) in [("90", 90), ("\"90s\"", 90), ("\"5m\"", 300)] {
            let line = format!("imap.watch.poll.interval = {value}\n");
            let Some(ImapWatchConfig::Poll(poll)) = read(&line) else {
                panic!("{line}: a poll");
            };
            assert_eq!(poll.interval(), Some(Duration::from_secs(secs)), "{line}");
        }

        let Some(ImapWatchConfig::Idle(idle)) = read("imap.watch.idle.timeout = \"2h\"\n") else {
            panic!("an IDLE");
        };
        assert_eq!(idle.timeout(), Some(Duration::from_secs(7200)));

        let line = "imap.watch.poll.interval = \"5 minutes\"\n";
        let err = toml::from_str::<Config>(&document(line)).expect_err(line);
        assert!(err.to_string().contains("Invalid duration"), "{err}");
    }

    #[cfg(feature = "jmap")]
    #[test]
    fn a_push_ping_takes_seconds_or_a_unit() {
        for (value, secs) in [("45", 45), ("\"1m\"", 60)] {
            let line = format!(
                "jmap.server = \"fastmail.com\"\n\
                 jmap.mailbox = \"Inbox\"\n\
                 jmap.auth.bearer.token.raw = \"token\"\n\
                 jmap.watch.push.ping = {value}\n"
            );
            let config: Config = toml::from_str(&document(&line)).expect(&line);
            let jmap = config.accounts["perso"].jmap.clone().unwrap();
            let Some(JmapWatchConfig::Push(push)) = jmap.watch else {
                panic!("{line}: a push");
            };
            assert_eq!(push.ping, Duration::from_secs(secs), "{line}");
        }

        assert_eq!(PushWatchConfig::default().ping, Duration::from_secs(30));
    }

    /// A key carillon read before 1.0 is an unknown key like any other,
    /// refused by serde naming it, never by name with a pointer to a
    /// migration guide.
    #[test]
    fn a_key_carillon_no_longer_reads_is_an_unknown_field() {
        #[cfg_attr(not(any(feature = "caldav", feature = "jmap")), allow(unused_mut))]
        let mut cases = vec![
            ("folder = \"INBOX\"\n", "folder"),
            ("mailbox = \"INBOX\"\n", "mailbox"),
            ("collection = \"INBOX\"\n", "collection"),
            ("watch.poll.interval = 60\n", "watch"),
            ("hooks.on-message-add.command = \"true\"\n", "hooks"),
            ("dav.server = \"https://dav.example.org\"\n", "dav"),
            ("imap.hook.on-message-add.command = \"true\"\n", "hook"),
            (
                "imap.hooks.on-message-added.command = \"true\"\n",
                "on-message-added",
            ),
            (
                "imap.hooks.on-flag-removed.flags = [\"Seen\"]\n",
                "on-flag-removed",
            ),
        ];

        #[cfg(feature = "caldav")]
        cases.push((
            "caldav.server = \"https://dav.example.org\"\n\
             caldav.calendar = \"work\"\n\
             caldav.hooks.on-task-changed.command = \"true\"\n",
            "on-task-changed",
        ));

        #[cfg(feature = "jmap")]
        cases.push((
            "jmap.server = \"fastmail.com\"\n\
             jmap.addressbook = \"Personal\"\n\
             jmap.hooks.on-card-removed.command = \"true\"\n",
            "on-card-removed",
        ));

        for (line, key) in cases {
            let err = toml::from_str::<Config>(&document(line)).expect_err(line);
            let err = err.to_string();
            let unknown = format!("unknown field `{key}`");

            assert!(err.contains(&unknown), "{line}: {err}");
            assert!(!err.contains("MIGRATION.md"), "{line}: {err}");
        }
    }

    /// The SASL spellings the sample writes and a secret's `cmd` load, the
    /// hooks being under `hooks` with a `command`.
    #[test]
    fn the_sasl_aliases_still_load() {
        let document = document(
            "imap.sasl.plain.username = \"alice\"\n\
             imap.sasl.plain.password.cmd = \"pass show imap\"\n\
             imap.hooks.on-message-add.command = \"true\"\n",
        );
        let config: Config = toml::from_str(&document).expect("the aliases");
        let imap = config.accounts["perso"].imap.as_ref().unwrap();

        assert!(imap.hooks.on_message_add.is_some());
        let Some(SaslConfig::Plain(plain)) = &imap.sasl else {
            panic!("a plain SASL table");
        };
        assert_eq!("alice", plain.authcid);
    }

    /// A hook's `cmd` is read as its `command`, as every command of the
    /// family is (pimalaya-config's spec), where 1.0 first refused it.
    #[test]
    fn a_hook_reads_cmd_as_its_command() {
        let line = "imap.hooks.on-message-add.cmd = \"true\"\n\
                    imap.hooks.on-flag-add.cmd = [\"notify-send\", \"$flag\"]\n";
        let config: Config = toml::from_str(&document(line)).expect("the alias");
        let hooks = &config.accounts["perso"].imap.as_ref().unwrap().hooks;

        let add = hooks.on_message_add.as_ref().unwrap().command.as_ref();
        assert_eq!(Some(&CommandConfig::Shell(String::from("true"))), add);

        let flag = hooks.on_flag_add.as_ref().unwrap().command.as_ref();
        let argv = CommandConfig::Argv {
            program: String::from("notify-send"),
            args: vec![String::from("$flag")],
        };
        assert_eq!(Some(&argv), flag);

        let both = "imap.hooks.on-message-add.cmd = \"true\"\n\
                    imap.hooks.on-message-add.command = \"true\"\n";
        let err = toml::from_str::<Config>(&document(both)).expect_err("two commands");
        assert!(err.to_string().contains("command"), "{err}");
    }

    /// A hook's `timeout` is read beside its command, zero refused.
    #[test]
    fn a_hook_reads_its_timeout() {
        let line = "imap.hooks.on-message-add.command = \"true\"\n\
                    imap.hooks.on-message-add.timeout = \"90s\"\n\
                    imap.hooks.on-flag-add.command = \"true\"\n\
                    imap.hooks.on-flag-add.timeout = 5\n";
        let config: Config = toml::from_str(&document(line)).expect("the timeouts");
        let hooks = &config.accounts["perso"].imap.as_ref().unwrap().hooks;

        let add = hooks.on_message_add.as_ref().unwrap().timeout;
        assert_eq!(Some(Timeout(Duration::from_secs(90))), add);
        let flag = hooks.on_flag_add.as_ref().unwrap().timeout;
        assert_eq!(Some(Timeout(Duration::from_secs(5))), flag);

        let zero = "imap.hooks.on-message-add.command = \"true\"\n\
                    imap.hooks.on-message-add.timeout = 0\n";
        let err = toml::from_str::<Config>(&document(zero)).expect_err("zero");
        assert!(err.to_string().contains("timeout of zero"), "{err}");
    }

    /// A DAV block with no `auth` loads, sending no credential, as
    /// `auth = "none"` does.
    #[cfg(all(feature = "caldav", feature = "carddav"))]
    #[test]
    fn a_dav_block_without_auth_loads() {
        let blocks = "caldav.calendar = \"work\"\n\
                      caldav.server = \"http://localhost:5232\"\n\
                      carddav.addressbook = \"book\"\n\
                      carddav.home = \"http://localhost:5232/alice/\"\n";
        let config: Config = toml::from_str(&document(blocks)).expect("no auth");
        let account = &config.accounts["perso"];

        let caldav = account.caldav.as_ref().unwrap();
        assert!(matches!(caldav.auth, DavAuthConfig::None));
        let carddav = account.carddav.as_ref().unwrap();
        assert!(matches!(carddav.auth, DavAuthConfig::None));
    }

    /// A DAV `home` and a JMAP `server` whose host is not ASCII are
    /// refused, naming the host and its Punycode form as the way out.
    #[cfg(all(feature = "caldav", feature = "jmap"))]
    #[test]
    fn a_non_ascii_host_names_its_punycode_form() {
        let hint = "Host `b\u{fc}cher.example` is not ASCII: write it in its Punycode form";

        for server in ["https://b\u{fc}cher.example/jmap", "b\u{fc}cher.example"] {
            let err = crate::jmap::parse_server(server).unwrap_err();
            assert!(err.to_string().contains(hint), "{server}: {err}");
        }

        let home = "caldav.calendar = \"work\"\n\
                    caldav.home = \"https://alice@b\u{fc}cher.example:8443/cal/\"\n";
        let err = toml::from_str::<Config>(&document(home)).unwrap_err();
        assert!(err.to_string().contains(hint), "{err}");

        let url = crate::jmap::parse_server("https://xn--bcher-kva.example/jmap").unwrap();
        assert_eq!(Some("xn--bcher-kva.example"), url.host_str());
    }

    /// A DAV block takes `server`, `home` or both, and is refused naming
    /// both when it takes neither.
    #[cfg(all(feature = "caldav", feature = "carddav"))]
    #[test]
    fn a_dav_block_takes_a_server_or_a_pinned_home() {
        let home = "caldav.calendar = \"work\"\n\
                    caldav.home = \"https://dav.example.org/cal/alice/\"\n";
        let config: Config = toml::from_str(&document(home)).expect("a home");
        config.accounts["perso"].validate().expect("a pinned home");
        let caldav = config.accounts["perso"].caldav.as_ref().unwrap();
        assert_eq!(None, caldav.server);
        assert_eq!("/cal/alice/", caldav.home.as_ref().unwrap().path());

        let neither = "carddav.addressbook = \"default\"\n";
        let config: Config = toml::from_str(&document(neither)).expect("a block");
        let err = config.accounts["perso"].validate().expect_err("no route");
        let err = format!("{err:#}");
        assert!(err.contains("`carddav` needs `server`, or `home`"), "{err}");
    }

    /// `pimdir.account` names the account of the store the calendar is
    /// grouped under.
    #[cfg(feature = "pimdir")]
    #[test]
    fn a_pimdir_block_takes_the_account_of_its_store() {
        let line = "pimdir.root = \"/nonexistent\"\n\
                    pimdir.calendar = \"cal\"\n\
                    pimdir.account = \"work\"\n\
                    pimdir.hooks.on-alarm-trigger.command = \"true\"\n";
        let config: Config = toml::from_str(&document(line)).expect("an account");
        let pimdir = config.accounts["perso"].pimdir.as_ref().unwrap();

        assert_eq!(Some("work"), pimdir.account.as_deref());
    }

    /// An argv hook gets its variables in its environment only, so one the
    /// sample names a variable in hands it to a shell.
    #[test]
    fn a_sample_argv_hook_naming_a_variable_runs_through_a_shell() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config.sample.toml");
        let sample = std::fs::read_to_string(&path).unwrap();
        let mut seen = 0;

        for line in sample.lines() {
            let line = line.trim_start_matches('#');
            let Some((key, value)) = line.split_once(" = [") else {
                continue;
            };
            if !key.contains(".hooks.") {
                continue;
            }

            let value: toml::Value = toml::from_str(&format!("argv = [{value}")).unwrap();
            let argv: Vec<&str> = value["argv"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(toml::Value::as_str)
                .collect();
            seen += 1;

            if argv[1..].iter().any(|arg| arg.contains('$')) {
                assert_eq!(["sh", "-c"], argv[..2], "{line}");
            }
        }

        assert!(seen > 0);
    }

    /// Every hook a backend fires is shown in config.sample.toml at least
    /// once.
    #[test]
    fn the_sample_names_every_hook() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config.sample.toml");
        let sample = std::fs::read_to_string(&path).unwrap();

        for hook in [
            "on-message-add",
            "on-message-remove",
            "on-message-change",
            "on-flag-add",
            "on-flag-remove",
            "on-card-add",
            "on-card-remove",
            "on-card-change",
            "on-event-add",
            "on-event-remove",
            "on-event-change",
            "on-task-add",
            "on-task-remove",
            "on-task-change",
            "on-alarm-trigger",
        ] {
            assert!(sample.contains(&format!(".hooks.{hook}.")), "{hook}");
        }
    }

    /// The sample loaded as written and once per rank, a notification
    /// being refused by a build without the `notify` feature.
    #[cfg(feature = "notify")]
    mod sample {
        use std::{fs, io::Write, ops::Range};

        use tempfile::NamedTempFile;

        use super::*;

        /// Checks config.sample.toml parses as written and once per rank
        /// fully uncommented, `example` its only account and the default
        /// each time.
        #[test]
        fn the_sample_configuration_parses_with_each_variant() {
            let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config.sample.toml");
            let sample = fs::read_to_string(&path).unwrap();
            let headers: Vec<_> = sample.lines().filter(|l| l.starts_with('[')).collect();
            assert_eq!(headers, ["[accounts.example]"]);

            load_sample(&sample, "as written");

            let paragraphs = sample_paragraphs(&sample);

            for rank in 1..=sample_ranks(&paragraphs) {
                let text = sample_variant(&paragraphs, rank);
                load_sample(&text, &format!("variant {rank}"));
            }
        }

        /// One line of config.sample.toml.
        struct SampleLine<'a> {
            /// The line as written.
            line: &'a str,
            /// The code before the comment, one `#` off when commented.
            code: &'a str,
            /// Whether the code is commented.
            commented: bool,
            /// The field the code sets, a secret's `raw` and `command`
            /// being one.
            field: Option<&'a str>,
            /// The other values the comment lists in brackets.
            values: Vec<&'a str>,
        }

        impl<'a> SampleLine<'a> {
            fn parse(line: &'a str) -> Self {
                let (code, comment) = line.split_once('\t').unwrap_or((line, ""));
                let commented = code.starts_with('#');
                let code = code.strip_prefix('#').unwrap_or(code);

                let field = code
                    .split_once(" = ")
                    .map(|(key, _)| key)
                    .filter(|key| {
                        key.chars()
                            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
                    })
                    .map(|key| {
                        let secret = key.strip_suffix(".raw").or(key.strip_suffix(".command"));
                        secret.unwrap_or(key)
                    });

                let values = comment
                    .trim_end()
                    .strip_suffix(']')
                    .and_then(|comment| comment.rsplit_once('['))
                    .map(|(_, values)| values.split(", ").collect())
                    .unwrap_or_default();

                Self {
                    line,
                    code,
                    commented,
                    field,
                    values,
                }
            }

            /// The first two segments of the field, the table a replacing
            /// paragraph shares with the one above it (`imap.sasl`).
            fn table(&self) -> Option<&'a str> {
                let field = self.field?;
                let end = field
                    .match_indices('.')
                    .nth(1)
                    .map_or(field.len(), |(i, _)| i);
                Some(&field[..end])
            }

            /// The values the line offers: its code as written, then each
            /// bracketed value in its place, typed like the written one.
            fn options(&self) -> Vec<String> {
                let (key, value) = self.code.split_once(" = ").unwrap();
                let mut options = vec![self.code.to_owned()];

                for other in &self.values {
                    if value.starts_with('"') {
                        options.push(format!("{key} = \"{other}\""));
                    } else {
                        options.push(format!("{key} = {other}"));
                    }
                }

                options
            }
        }

        fn load_sample(text: &str, name: &str) {
            let context = format!("config.sample.toml, {name}");

            let mut file = NamedTempFile::new().unwrap();
            file.write_all(text.as_bytes()).unwrap();

            let mut config = Config::load(&[file.path().to_path_buf()])
                .unwrap_or_else(|err| panic!("{context}: {err:?}\n{text}"));
            let (account, _) = config
                .take_account(None)
                .unwrap_or_else(|err| panic!("{context}: {err:?}"));

            assert_eq!(account, "example", "{context}");
        }

        fn sample_paragraphs(sample: &str) -> Vec<Vec<SampleLine<'_>>> {
            let paragraphs = sample.split("\n\n");
            let paragraphs = paragraphs.map(|p| p.lines().map(SampleLine::parse).collect());
            paragraphs.collect()
        }

        /// Whether `below`, all commented, replaces `above`, setting keys
        /// under a table one of its lines sets.
        fn sample_replaces(above: &[SampleLine], below: &[SampleLine]) -> bool {
            let mut code = below.iter().filter(|line| line.field.is_some());
            let Some(first) = code.next() else {
                return false;
            };

            if !first.commented || code.any(|line| !line.commented) {
                return false;
            }

            above
                .iter()
                .filter_map(SampleLine::table)
                .any(|t| Some(t) == first.table())
        }

        /// The runs of paragraphs each replacing the one above it, as
        /// index ranges, a paragraph replacing none being a run of one.
        fn sample_runs(paragraphs: &[Vec<SampleLine>]) -> Vec<Range<usize>> {
            let mut runs: Vec<Range<usize>> = Vec::new();

            for i in 0..paragraphs.len() {
                match runs.last_mut() {
                    Some(run) if sample_replaces(&paragraphs[i - 1], &paragraphs[i]) => {
                        run.end = i + 1;
                    }
                    _ => runs.push(i..i + 1),
                }
            }

            runs
        }

        /// The values each field takes in turn, the lines setting it in
        /// their order, each followed by its bracketed values.
        fn sample_options<'a>(lines: &[&'a SampleLine<'a>]) -> HashMap<&'a str, Vec<String>> {
            let mut options: HashMap<&str, Vec<String>> = HashMap::new();

            for line in lines {
                if let Some(field) = line.field {
                    options.entry(field).or_default().extend(line.options());
                }
            }

            options
        }

        /// How many variants give every value its turn: the most values
        /// of one field, or the longest run of replacing paragraphs.
        fn sample_ranks(paragraphs: &[Vec<SampleLine>]) -> usize {
            let lines: Vec<&SampleLine> = paragraphs.iter().flatten().collect();
            let options = sample_options(&lines);
            let values = options.values().map(Vec::len).max().unwrap_or(0);

            let runs = sample_runs(paragraphs);
            let longest = runs.iter().map(Range::len).max().unwrap_or(0);

            values.max(longest)
        }

        /// The sample at `rank`, every line uncommented, each field set to
        /// its value of that rank and each run of replacing paragraphs to
        /// its paragraph of that rank (the last when either has fewer).
        fn sample_variant(paragraphs: &[Vec<SampleLine>], rank: usize) -> String {
            let runs = sample_runs(paragraphs).into_iter();
            let kept = runs.map(|run| &paragraphs[run.start + rank.min(run.len()) - 1]);
            let lines: Vec<&SampleLine> = kept.flatten().collect();

            let mut options = sample_options(&lines);
            let mut text = Vec::with_capacity(lines.len());

            for line in lines {
                let Some(field) = line.field else {
                    text.push(line.line.to_owned());
                    continue;
                };

                if let Some(values) = options.remove(field) {
                    text.push(values[rank.min(values.len()) - 1].clone());
                }
            }

            text.join("\n")
        }
    }
}
