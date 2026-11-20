//! # Google People
//!
//! The `gpeople` block: its token, the contact group, the poll and the
//! hooks.

use anyhow::Result;
use serde::Deserialize;

use crate::{
    config::{
        Hook, ItemHook,
        api::{ApiWatchConfig, BearerAuthConfig},
        proxy::ProxyConfig,
        tls::TlsConfig,
    },
    event::{WatchDomain, WatchEvent},
    hook::{self, HookCollection, Vocabulary},
};

/// Google People configuration: one watched contact group.
#[derive(Clone, Debug, Deserialize)]
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
    pub hooks: GpeopleHookConfig,
}

impl GpeopleConfig {
    /// The collection this backend watches, under its own name.
    pub fn collection(&self) -> HookCollection<'_> {
        HookCollection {
            name: WatchDomain::Card.collection_name(),
            value: &self.addressbook,
        }
    }
}

/// Hooks a Google People watch fires.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct GpeopleHookConfig {
    /// Fires when a contact joins the watched group.
    pub on_card_add: Option<ItemHook>,
    /// Fires when one leaves it, deleted or ungrouped.
    pub on_card_remove: Option<ItemHook>,
    /// Fires when one is edited where it stands.
    pub on_card_change: Option<ItemHook>,
}

impl GpeopleHookConfig {
    /// The hook `event` calls for, when one is configured.
    pub fn get(&self, event: &WatchEvent) -> Option<Hook<'_>> {
        let hook = match event {
            WatchEvent::ItemAdded { .. } => &self.on_card_add,
            WatchEvent::ItemRemoved { .. } => &self.on_card_remove,
            WatchEvent::ItemChanged { .. } => &self.on_card_change,
            _ => return None,
        };

        hook.as_ref().map(Hook::Item)
    }

    /// Refuses a notification naming what its event cannot fill.
    pub fn validate(&self) -> Result<()> {
        for (hook, name) in [
            (&self.on_card_add, "on-card-add"),
            (&self.on_card_remove, "on-card-remove"),
            (&self.on_card_change, "on-card-change"),
        ] {
            let notify = hook.as_ref().and_then(|hook| hook.notify.as_ref());
            hook::validate(
                notify,
                Vocabulary::item(WatchDomain::Card.collection_name()),
                &format!("gpeople.hooks.{name}"),
            )?;
        }

        Ok(())
    }
}
