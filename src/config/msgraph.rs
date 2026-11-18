//! # Microsoft Graph
//!
//! The `msgraph` block: its token, the collection of each domain, the poll
//! and the hooks.

use anyhow::Result;
use serde::Deserialize;

use crate::{
    config::{
        HookTableRemoved,
        api::{ApiWatchConfig, BearerAuthConfig},
        domains::{DOMAINS, DomainsHookConfig, domain_collection, refuse_unwatched},
        proxy::ProxyConfig,
        tls::TlsConfig,
    },
    event::WatchDomain,
    hook::HookCollection,
};

/// Microsoft Graph configuration: a collection per domain, at least one,
/// read through one token and one connection.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct MsgraphConfig {
    /// The mail folder this account watches, matched by display name
    /// case-insensitively, by id or by well-known name (`inbox`).
    #[serde(default)]
    pub mailbox: Option<String>,
    /// The contact folder this account watches, matched by display name
    /// case-insensitively or by id, `Contacts` naming the default one.
    #[serde(default)]
    pub addressbook: Option<String>,
    /// The calendar this account watches, matched by name
    /// case-insensitively or by id.
    #[serde(default)]
    pub calendar: Option<String>,
    /// The mailbox owner, a user id or a principal name, unset meaning
    /// the token's own user.
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub tls: TlsConfig,
    /// The ALPN identifiers offered during the TLS handshake, unset
    /// taking `["http/1.1"]`.
    #[serde(default)]
    pub alpn: Option<Vec<String>>,
    /// Proxy the connection goes through, falling back to the account one
    /// and then to the `all_proxy`/`https_proxy` environment variables.
    #[serde(default)]
    pub proxy: Option<ProxyConfig>,
    /// Authentication: the bearer token.
    pub auth: BearerAuthConfig,
    /// How this account learns about a change. Unset polls.
    #[serde(default)]
    pub watch: Option<ApiWatchConfig>,
    /// The hooks this backend fires.
    #[serde(default)]
    pub hooks: DomainsHookConfig,
    /// Refused: the table before 1.0, now `hooks`.
    #[serde(default)]
    #[allow(dead_code)]
    pub hook: Option<HookTableRemoved>,
}

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
        refuse_unwatched("msgraph", collection, self.hooks.configured())?;
        self.hooks.validate("msgraph")
    }
}
