//! # CalDAV
//!
//! The `caldav` block: the calendar, how it is polled and the hooks it
//! fires, alarms included. The transport half is [`crate::config::dav`]'s.

use anyhow::Result;
use serde::Deserialize;
use url::Url;

use crate::{
    config::{
        Hook, HookTableRemoved, ItemHook, OnEventAddedRenamed, OnEventChangedRenamed,
        OnEventRemovedRenamed, OnTaskAddedRenamed, OnTaskChangedRenamed, OnTaskRemovedRenamed,
        dav::{DavAuthConfig, DavServer, DavWatchConfig, ascii_url},
        proxy::ProxyConfig,
        tls::TlsConfig,
    },
    event::{WatchDomain, WatchEvent},
    hook::{self, HookCollection, Vocabulary},
};

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

/// Hooks a CalDAV watch fires, one set per component a calendar holds.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CaldavHookConfig {
    /// Fires when a VEVENT appears in the watched calendar.
    pub on_event_add: Option<ItemHook>,
    /// Fires when a VEVENT leaves it.
    pub on_event_remove: Option<ItemHook>,
    /// Fires when a VEVENT is edited where it stands.
    pub on_event_change: Option<ItemHook>,
    /// Fires when a VTODO appears in the watched calendar.
    pub on_task_add: Option<ItemHook>,
    /// Fires when a VTODO leaves it.
    pub on_task_remove: Option<ItemHook>,
    /// Fires when a VTODO is edited where it stands.
    pub on_task_change: Option<ItemHook>,
    /// Fires when an alarm of a VEVENT or a VTODO reaches its trigger.
    pub on_alarm_trigger: Option<ItemHook>,
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
    #[serde(default)]
    #[allow(dead_code)]
    pub on_task_added: Option<OnTaskAddedRenamed>,
    #[serde(default)]
    #[allow(dead_code)]
    pub on_task_removed: Option<OnTaskRemovedRenamed>,
    #[serde(default)]
    #[allow(dead_code)]
    pub on_task_changed: Option<OnTaskChangedRenamed>,
}

impl CaldavHookConfig {
    /// The hook `event` calls for, which on a calendar depends on the
    /// component its member turned out to be.
    pub fn get(&self, event: &WatchEvent) -> Option<Hook<'_>> {
        let hook = match event {
            WatchEvent::ItemAdded {
                domain: WatchDomain::Event,
                ..
            } => &self.on_event_add,
            WatchEvent::ItemRemoved {
                domain: WatchDomain::Event,
                ..
            } => &self.on_event_remove,
            WatchEvent::ItemChanged {
                domain: WatchDomain::Event,
                ..
            } => &self.on_event_change,
            WatchEvent::ItemAdded {
                domain: WatchDomain::Task,
                ..
            } => &self.on_task_add,
            WatchEvent::ItemRemoved {
                domain: WatchDomain::Task,
                ..
            } => &self.on_task_remove,
            WatchEvent::ItemChanged {
                domain: WatchDomain::Task,
                ..
            } => &self.on_task_change,
            _ => return None,
        };

        hook.as_ref().map(Hook::Item)
    }

    /// The components this table has hooks for, which a calendar
    /// advertising only some of them is checked against.
    pub fn domains(&self) -> Vec<WatchDomain> {
        let mut domains = Vec::new();

        if self.on_event_add.is_some()
            || self.on_event_remove.is_some()
            || self.on_event_change.is_some()
        {
            domains.push(WatchDomain::Event);
        }

        if self.on_task_add.is_some()
            || self.on_task_remove.is_some()
            || self.on_task_change.is_some()
        {
            domains.push(WatchDomain::Task);
        }

        domains
    }
}

impl CaldavHookConfig {
    /// Refuses a notification naming what its event cannot fill.
    pub fn validate(&self) -> Result<()> {
        for (hook, name) in [
            (&self.on_event_add, "on-event-add"),
            (&self.on_event_remove, "on-event-remove"),
            (&self.on_event_change, "on-event-change"),
            (&self.on_task_add, "on-task-add"),
            (&self.on_task_remove, "on-task-remove"),
            (&self.on_task_change, "on-task-change"),
        ] {
            let notify = hook.as_ref().and_then(|hook| hook.notify.as_ref());
            hook::validate(
                notify,
                Vocabulary::item(CaldavConfig::COLLECTION),
                &format!("caldav.hooks.{name}"),
            )?;
        }

        let notify = self
            .on_alarm_trigger
            .as_ref()
            .and_then(|hook| hook.notify.as_ref());
        hook::validate(
            notify,
            Vocabulary::alarm(CaldavConfig::COLLECTION),
            "caldav.hooks.on-alarm-trigger",
        )?;

        Ok(())
    }
}

/// CalDAV configuration: one watched calendar, polled through RFC 6578
/// `sync-collection`.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CaldavConfig {
    /// The calendar this account watches: an id looked up under the
    /// calendar home set, an absolute path taken as it stands, or any other
    /// path read under the URL the connection opens on.
    pub calendar: String,
    /// The DAV server URL, `http://` or `https://`, where the principal and
    /// its calendar home set are discovered from. Required without `home`.
    pub server: Option<String>,
    /// The calendar home set, pinned: discovery is skipped and the
    /// connection opens on it, `server` left unread.
    #[serde(default, deserialize_with = "ascii_url")]
    pub home: Option<Url>,
    #[serde(default)]
    pub tls: TlsConfig,
    /// The ALPN identifiers offered during the TLS handshake.
    ///
    /// Unset takes io-http's own default, `["http/1.1"]`, WebDAV riding on
    /// HTTP/1.1; `[]` skips ALPN negotiation, and a non-empty list
    /// replaces the default. Only rustls reads it, native-tls having no
    /// ALPN.
    #[serde(default)]
    pub alpn: Option<Vec<String>>,
    /// Proxy the connection goes through, falling back to the account one
    /// and then to the `all_proxy`/`https_proxy` environment variables.
    #[serde(default)]
    pub proxy: Option<ProxyConfig>,
    /// Authentication, none by default, for a calendar readable without.
    #[serde(default)]
    pub auth: DavAuthConfig,
    /// How this account learns about a change. Unset polls.
    #[serde(default)]
    pub watch: Option<DavWatchConfig>,
    /// The hooks this backend fires, one per component it holds.
    #[serde(default)]
    pub hooks: CaldavHookConfig,
    /// Refused: the table before 1.0, now `hooks`.
    #[serde(default)]
    #[allow(dead_code)]
    pub hook: Option<HookTableRemoved>,
}

impl CaldavConfig {
    /// What it takes to open a connection to this server.
    pub fn server(&self) -> DavServer<'_> {
        DavServer {
            server: self.server.as_deref(),
            home: self.home.as_ref(),
            tls: &self.tls,
            alpn: self.alpn.as_deref(),
            proxy: self.proxy.as_ref(),
            auth: &self.auth,
        }
    }
}
