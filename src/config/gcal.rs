//! # Google Calendar
//!
//! The `gcal` block: its token, the calendar, the poll and the hooks.

use anyhow::Result;
use serde::Deserialize;

use crate::{
    config::{
        Hook, HookTableRemoved, ItemHook, OnEventAddedRenamed, OnEventChangedRenamed,
        OnEventRemovedRenamed,
        api::{ApiWatchConfig, BearerAuthConfig},
        proxy::ProxyConfig,
        tls::TlsConfig,
    },
    event::{WatchDomain, WatchEvent},
    hook::{self, HookCollection, Vocabulary},
};

/// Google Calendar configuration: one watched calendar.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct GcalConfig {
    /// The calendar this account watches, by name case-insensitively or
    /// by id, `primary` naming the account's own.
    pub calendar: String,
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
    pub hooks: GcalHookConfig,
    /// Refused: the table before 1.0, now `hooks`.
    #[serde(default)]
    #[allow(dead_code)]
    pub hook: Option<HookTableRemoved>,
}

impl GcalConfig {
    /// The collection this backend watches, under its own name.
    pub fn collection(&self) -> HookCollection<'_> {
        HookCollection {
            name: WatchDomain::Event.collection_name(),
            value: &self.calendar,
        }
    }
}

/// Hooks a Google Calendar watch fires, events alone: the API has no
/// task type.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct GcalHookConfig {
    /// Fires when an event appears in the watched calendar.
    pub on_event_add: Option<ItemHook>,
    /// Fires when an event leaves it.
    pub on_event_remove: Option<ItemHook>,
    /// Fires when an event is edited where it stands.
    pub on_event_change: Option<ItemHook>,
    /// Refused: the hook names before 1.0, each pointing at its successor.
    #[serde(default)]
    #[allow(dead_code)]
    pub on_event_added: Option<OnEventAddedRenamed>,
    #[serde(default)]
    #[allow(dead_code)]
    pub on_event_removed: Option<OnEventRemovedRenamed>,
    #[serde(default)]
    #[allow(dead_code)]
    pub on_event_changed: Option<OnEventChangedRenamed>,
}

impl GcalHookConfig {
    /// The hook `event` calls for, when one is configured.
    pub fn get(&self, event: &WatchEvent) -> Option<Hook<'_>> {
        let hook = match event {
            WatchEvent::ItemAdded { .. } => &self.on_event_add,
            WatchEvent::ItemRemoved { .. } => &self.on_event_remove,
            WatchEvent::ItemChanged { .. } => &self.on_event_change,
            _ => return None,
        };

        hook.as_ref().map(Hook::Item)
    }

    /// Refuses a notification naming what its event cannot fill.
    pub fn validate(&self) -> Result<()> {
        for (hook, name) in [
            (&self.on_event_add, "on-event-add"),
            (&self.on_event_remove, "on-event-remove"),
            (&self.on_event_change, "on-event-change"),
        ] {
            let notify = hook.as_ref().and_then(|hook| hook.notify.as_ref());
            hook::validate(
                notify,
                Vocabulary::item(WatchDomain::Event.collection_name()),
                &format!("gcal.hooks.{name}"),
            )?;
        }

        Ok(())
    }
}
