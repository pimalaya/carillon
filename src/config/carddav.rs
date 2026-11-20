//! # CardDAV
//!
//! The `carddav` block: the addressbook, how it is polled and the hooks it
//! fires. The transport half is [`crate::config::dav`]'s.

use anyhow::Result;
use serde::Deserialize;
use url::Url;

use crate::{
    config::{
        Hook, ItemHook,
        dav::{DavAuthConfig, DavServer, DavWatchConfig, ascii_url},
        proxy::ProxyConfig,
        tls::TlsConfig,
    },
    event::WatchEvent,
    hook::{self, HookCollection, Vocabulary},
};

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

/// Hooks a CardDAV watch fires.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CarddavHookConfig {
    /// Fires when a vCard appears in the watched addressbook.
    pub on_card_add: Option<ItemHook>,
    /// Fires when a vCard leaves it.
    pub on_card_remove: Option<ItemHook>,
    /// Fires when a vCard is edited where it stands.
    pub on_card_change: Option<ItemHook>,
}

impl CarddavHookConfig {
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
}

impl CarddavHookConfig {
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
                Vocabulary::item(CarddavConfig::COLLECTION),
                &format!("carddav.hooks.{name}"),
            )?;
        }

        Ok(())
    }
}

/// CardDAV configuration: one watched addressbook, polled the same way.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CarddavConfig {
    /// The addressbook this account watches: an id looked up under the
    /// addressbook home set, an absolute path taken as it stands, or any
    /// other path read under the URL the connection opens on.
    pub addressbook: String,
    /// The DAV server URL, `http://` or `https://`, where the principal and
    /// its addressbook home set are discovered from. Required without
    /// `home`.
    pub server: Option<String>,
    /// The addressbook home set, pinned: discovery is skipped and the
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
    /// Authentication, none by default, for a book readable without.
    #[serde(default)]
    pub auth: DavAuthConfig,
    /// How this account learns about a change. Unset polls.
    #[serde(default)]
    pub watch: Option<DavWatchConfig>,
    /// The hooks this backend fires.
    #[serde(default)]
    pub hooks: CarddavHookConfig,
}

impl CarddavConfig {
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
