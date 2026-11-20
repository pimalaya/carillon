//! # Gmail
//!
//! The `gmail` block: its token, the label, the poll and the hooks.

use anyhow::Result;
use serde::Deserialize;

use crate::{
    config::{
        FlagHook, Hook, ItemHook,
        api::{ApiWatchConfig, BearerAuthConfig},
        proxy::ProxyConfig,
        tls::TlsConfig,
    },
    event::{WatchDomain, WatchEvent},
    hook::{self, HookCollection, Vocabulary},
};

/// Gmail configuration: one watched label.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct GmailConfig {
    /// The label this account watches, by name (`INBOX`, `Work/Clients`)
    /// or by id.
    pub mailbox: String,
    /// The mailbox owner, unset meaning the token's own user.
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
    pub hooks: GmailHookConfig,
}

impl GmailConfig {
    /// The collection this backend watches, under its own name.
    pub fn collection(&self) -> HookCollection<'_> {
        HookCollection {
            name: WatchDomain::Message.collection_name(),
            value: &self.mailbox,
        }
    }
}

/// Hooks a Gmail watch fires.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct GmailHookConfig {
    /// Fires when a message arrives in the watched label.
    pub on_message_add: Option<ItemHook>,
    /// Fires when a message leaves it, deleted or relabelled.
    pub on_message_remove: Option<ItemHook>,
    /// Fires once for each flag set on a message: `Seen` when `UNREAD`
    /// is cleared, `Flagged` when `STARRED` is set.
    pub on_flag_add: Option<FlagHook>,
    /// Fires once for each flag cleared on a message.
    pub on_flag_remove: Option<FlagHook>,
}

impl GmailHookConfig {
    /// The hook `event` calls for, when one is configured.
    pub fn get(&self, event: &WatchEvent) -> Option<Hook<'_>> {
        match event {
            WatchEvent::ItemAdded { .. } => self.on_message_add.as_ref().map(Hook::Item),
            WatchEvent::ItemRemoved { .. } => self.on_message_remove.as_ref().map(Hook::Item),
            WatchEvent::ItemChanged { .. } => None,
            WatchEvent::FlagAdded { .. } => self.on_flag_add.as_ref().map(Hook::Flag),
            WatchEvent::FlagRemoved { .. } => self.on_flag_remove.as_ref().map(Hook::Flag),
        }
    }

    /// Refuses a notification naming what its event cannot fill.
    ///
    /// The arrival's envelope rides the metadata the poll already reads,
    /// so its hook may name one.
    pub fn validate(&self) -> Result<()> {
        let mailbox = WatchDomain::Message.collection_name();
        hook::validate(
            self.on_message_add.as_ref().and_then(|h| h.notify.as_ref()),
            Vocabulary::resolved(mailbox),
            "gmail.hooks.on-message-add",
        )?;
        hook::validate(
            self.on_message_remove
                .as_ref()
                .and_then(|h| h.notify.as_ref()),
            Vocabulary::item(mailbox),
            "gmail.hooks.on-message-remove",
        )?;
        hook::validate(
            self.on_flag_add.as_ref().and_then(|h| h.notify.as_ref()),
            Vocabulary::flag(mailbox),
            "gmail.hooks.on-flag-add",
        )?;
        hook::validate(
            self.on_flag_remove.as_ref().and_then(|h| h.notify.as_ref()),
            Vocabulary::flag(mailbox),
            "gmail.hooks.on-flag-remove",
        )
    }
}
