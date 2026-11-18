//! # JMAP
//!
//! The `jmap` block: the server, its credential, the collection of each
//! domain, how they are watched and the hooks they fire.

use std::time::Duration;

use anyhow::{Result, bail};
use pimalaya_config::secret::Secret;
use serde::Deserialize;

use crate::{
    config::{
        HookTableRemoved, PollWatchConfig,
        domains::{DOMAINS, DomainsHookConfig, domain_collection, refuse_unwatched},
        proxy::ProxyConfig,
        tls::TlsConfig,
    },
    event::WatchDomain,
    hook::HookCollection,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct JmapConfig {
    /// The mailbox this account watches, matched by name and
    /// case-insensitively, `INBOX` falling back to the special-use role.
    ///
    /// The three collections are each optional, at least one required:
    /// they share the session, the connection and the event stream.
    #[serde(default)]
    pub mailbox: Option<String>,
    /// The addressbook this account watches (RFC 9610), matched by name
    /// case-insensitively or by id.
    #[serde(default)]
    pub addressbook: Option<String>,
    /// The calendar this account watches (JMAP for Calendars), matched by
    /// name case-insensitively or by id.
    #[serde(default)]
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
    #[serde(default)]
    pub alpn: Option<Vec<String>>,
    /// Proxy the connection goes through, falling back to the account one
    /// and then to the `all_proxy`/`https_proxy` environment variables.
    #[serde(default)]
    pub proxy: Option<ProxyConfig>,
    /// Authentication: exactly one of `header`, `bearer`, `basic`.
    pub auth: JmapAuthConfig,
    /// How this account learns about a change. Unset holds the stream.
    #[serde(default)]
    pub watch: Option<JmapWatchConfig>,
    /// The hooks this backend fires.
    #[serde(default)]
    pub hooks: DomainsHookConfig,
    /// Refused: the table before 1.0, now `hooks`.
    #[serde(default)]
    #[allow(dead_code)]
    pub hook: Option<HookTableRemoved>,
}

#[derive(Clone, Debug, Deserialize)]
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
        if self.hooks.on_message_change.is_some() {
            bail!("Hook `jmap.hooks.on-message-change` never fires: a JMAP email is immutable");
        }

        let collection = |domain| self.collection(domain).is_some();
        refuse_unwatched("jmap", collection, self.hooks.configured())?;
        self.hooks.validate("jmap")
    }
}

/// How a JMAP account learns about a change.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum JmapWatchConfig {
    /// Hold an EventSource stream and let the server push.
    Push(PushWatchConfig),
    /// Ask `Email/changes` on an interval instead.
    Poll(PollWatchConfig),
}

/// Options of the push method.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct PushWatchConfig {
    /// How long between the server's keep-alive pings, which are also
    /// what proves the stream is still there.
    #[serde(default = "default_push_ping", with = "pimalaya_config::duration")]
    pub ping: Duration,
}

impl Default for PushWatchConfig {
    fn default() -> Self {
        Self {
            ping: default_push_ping(),
        }
    }
}

/// Half a minute between pings, short enough to notice a dead stream
/// and long enough to be quiet.
fn default_push_ping() -> Duration {
    Duration::from_secs(30)
}
