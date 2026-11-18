//! # Domains
//!
//! What the `jmap` and `msgraph` blocks share: a collection per domain,
//! and the hooks of each.

use anyhow::{Result, bail};
use serde::Deserialize;

use crate::{
    config::{
        FlagHook, Hook, ItemHook, OnCardAddedRenamed, OnCardChangedRenamed, OnCardRemovedRenamed,
        OnEventAddedRenamed, OnEventChangedRenamed, OnEventRemovedRenamed, OnFlagAddedRenamed,
        OnFlagRemovedRenamed, OnMessageAddedRenamed, OnMessageChangedRenamed,
        OnMessageRemovedRenamed,
    },
    event::{WatchDomain, WatchEvent},
    hook::{self, HookCollection, Vocabulary},
};

// NOTE: JMAP and Graph each serve mail, contacts and calendars, so one
// block takes a collection per domain, and both read and check them the
// same way.

/// The domains a backend serving several watches, in the order its rounds
/// run.
pub(crate) const DOMAINS: [WatchDomain; 3] =
    [WatchDomain::Message, WatchDomain::Card, WatchDomain::Event];

/// The collection an event of `domain` is about, under its own name, out
/// of the three keys a multi-domain block carries.
pub(crate) fn domain_collection<'a>(
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
pub(crate) fn refuse_unwatched(
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
                "Hook `{backend}.hooks.{name}` needs `{backend}.{key}`, which this account does not configure"
            );
        }
    }

    Ok(())
}

/// Hooks a JMAP or a Microsoft Graph watch fires, one set per domain.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct DomainsHookConfig {
    /// Fires when a message arrives in the watched mailbox.
    pub on_message_add: Option<ItemHook>,
    /// Fires when a message leaves it.
    pub on_message_remove: Option<ItemHook>,
    /// Fires when a message is edited where it stands, which only a
    /// Microsoft Graph draft is.
    pub on_message_change: Option<ItemHook>,
    /// Fires once for each keyword set on a message.
    pub on_flag_add: Option<FlagHook>,
    /// Fires once for each keyword cleared on a message.
    pub on_flag_remove: Option<FlagHook>,
    /// Fires when a contact appears in the watched addressbook.
    pub on_card_add: Option<ItemHook>,
    /// Fires when a contact leaves it.
    pub on_card_remove: Option<ItemHook>,
    /// Fires when a contact is edited where it stands.
    pub on_card_change: Option<ItemHook>,
    /// Fires when an event appears in the watched calendar.
    pub on_event_add: Option<ItemHook>,
    /// Fires when an event leaves it.
    pub on_event_remove: Option<ItemHook>,
    /// Fires when an event is edited where it stands.
    pub on_event_change: Option<ItemHook>,
    /// Refused: the hook names before 1.0, each pointing at its successor.
    #[serde(default)]
    #[allow(dead_code)]
    pub on_message_added: Option<OnMessageAddedRenamed>,
    #[serde(default)]
    #[allow(dead_code)]
    pub on_message_removed: Option<OnMessageRemovedRenamed>,
    #[serde(default)]
    #[allow(dead_code)]
    pub on_message_changed: Option<OnMessageChangedRenamed>,
    #[serde(default)]
    #[allow(dead_code)]
    pub on_flag_added: Option<OnFlagAddedRenamed>,
    #[serde(default)]
    #[allow(dead_code)]
    pub on_flag_removed: Option<OnFlagRemovedRenamed>,
    #[serde(default)]
    #[allow(dead_code)]
    pub on_card_added: Option<OnCardAddedRenamed>,
    #[serde(default)]
    #[allow(dead_code)]
    pub on_card_removed: Option<OnCardRemovedRenamed>,
    #[serde(default)]
    #[allow(dead_code)]
    pub on_card_changed: Option<OnCardChangedRenamed>,
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

impl DomainsHookConfig {
    /// The hook `event` calls for, which depends on the domain the
    /// method that answered was about.
    pub fn get(&self, event: &WatchEvent) -> Option<Hook<'_>> {
        let hook = match (event.domain(), event) {
            (WatchDomain::Message, WatchEvent::FlagAdded { .. }) => {
                return self.on_flag_add.as_ref().map(Hook::Flag);
            }
            (WatchDomain::Message, WatchEvent::FlagRemoved { .. }) => {
                return self.on_flag_remove.as_ref().map(Hook::Flag);
            }
            (WatchDomain::Message, WatchEvent::ItemAdded { .. }) => &self.on_message_add,
            (WatchDomain::Message, WatchEvent::ItemRemoved { .. }) => &self.on_message_remove,
            (WatchDomain::Message, WatchEvent::ItemChanged { .. }) => &self.on_message_change,
            (WatchDomain::Card, WatchEvent::ItemAdded { .. }) => &self.on_card_add,
            (WatchDomain::Card, WatchEvent::ItemRemoved { .. }) => &self.on_card_remove,
            (WatchDomain::Card, WatchEvent::ItemChanged { .. }) => &self.on_card_change,
            (WatchDomain::Event, WatchEvent::ItemAdded { .. }) => &self.on_event_add,
            (WatchDomain::Event, WatchEvent::ItemRemoved { .. }) => &self.on_event_remove,
            (WatchDomain::Event, WatchEvent::ItemChanged { .. }) => &self.on_event_change,
            _ => return None,
        };

        hook.as_ref().map(Hook::Item)
    }

    /// The hooks this table configures, by name, with the domain each
    /// fires for.
    pub(crate) fn configured(&self) -> impl Iterator<Item = (&'static str, WatchDomain)> {
        [
            (
                "on-message-add",
                WatchDomain::Message,
                self.on_message_add.is_some(),
            ),
            (
                "on-message-remove",
                WatchDomain::Message,
                self.on_message_remove.is_some(),
            ),
            (
                "on-message-change",
                WatchDomain::Message,
                self.on_message_change.is_some(),
            ),
            (
                "on-flag-add",
                WatchDomain::Message,
                self.on_flag_add.is_some(),
            ),
            (
                "on-flag-remove",
                WatchDomain::Message,
                self.on_flag_remove.is_some(),
            ),
            ("on-card-add", WatchDomain::Card, self.on_card_add.is_some()),
            (
                "on-card-remove",
                WatchDomain::Card,
                self.on_card_remove.is_some(),
            ),
            (
                "on-card-change",
                WatchDomain::Card,
                self.on_card_change.is_some(),
            ),
            (
                "on-event-add",
                WatchDomain::Event,
                self.on_event_add.is_some(),
            ),
            (
                "on-event-remove",
                WatchDomain::Event,
                self.on_event_remove.is_some(),
            ),
            (
                "on-event-change",
                WatchDomain::Event,
                self.on_event_change.is_some(),
            ),
        ]
        .into_iter()
        .filter_map(|(name, domain, set)| set.then_some((name, domain)))
    }
}

impl DomainsHookConfig {
    /// Refuses a notification naming what its event cannot fill.
    ///
    /// Both backends read an envelope from the request their round already
    /// makes, so a message arrival hook may name one. `backend` prefixes
    /// the names an error reports.
    pub fn validate(&self, backend: &str) -> Result<()> {
        let mailbox = WatchDomain::Message.collection_name();

        hook::validate(
            self.on_message_add.as_ref().and_then(|h| h.notify.as_ref()),
            Vocabulary::resolved(mailbox),
            &format!("{backend}.hooks.on-message-add"),
        )?;
        hook::validate(
            self.on_message_remove
                .as_ref()
                .and_then(|h| h.notify.as_ref()),
            Vocabulary::item(mailbox),
            &format!("{backend}.hooks.on-message-remove"),
        )?;
        hook::validate(
            self.on_message_change
                .as_ref()
                .and_then(|h| h.notify.as_ref()),
            Vocabulary::item(mailbox),
            &format!("{backend}.hooks.on-message-change"),
        )?;
        hook::validate(
            self.on_flag_add.as_ref().and_then(|h| h.notify.as_ref()),
            Vocabulary::flag(mailbox),
            &format!("{backend}.hooks.on-flag-add"),
        )?;
        hook::validate(
            self.on_flag_remove.as_ref().and_then(|h| h.notify.as_ref()),
            Vocabulary::flag(mailbox),
            &format!("{backend}.hooks.on-flag-remove"),
        )?;

        for (hook, domain, name) in [
            (&self.on_card_add, WatchDomain::Card, "on-card-add"),
            (&self.on_card_remove, WatchDomain::Card, "on-card-remove"),
            (&self.on_card_change, WatchDomain::Card, "on-card-change"),
            (&self.on_event_add, WatchDomain::Event, "on-event-add"),
            (&self.on_event_remove, WatchDomain::Event, "on-event-remove"),
            (&self.on_event_change, WatchDomain::Event, "on-event-change"),
        ] {
            let notify = hook.as_ref().and_then(|hook| hook.notify.as_ref());
            hook::validate(
                notify,
                Vocabulary::item(domain.collection_name()),
                &format!("{backend}.hooks.{name}"),
            )?;
        }

        Ok(())
    }
}
