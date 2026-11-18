//! # Maildir
//!
//! The `maildir` block: the root, the mailbox, the poll and the hooks.

use std::path::PathBuf;

use anyhow::Result;
use serde::Deserialize;

use crate::{
    config::{
        FlagHook, Hook, HookTableRemoved, ItemHook, OnFlagAddedRenamed, OnFlagRemovedRenamed,
        OnMessageAddedRenamed, OnMessageRemovedRenamed, PollWatchConfig,
    },
    event::WatchEvent,
    hook::{self, HookCollection, Vocabulary},
};

#[derive(Clone, Debug, Deserialize)]
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
    #[serde(default)]
    pub hooks: MaildirHookConfig,
    /// Refused: the table before 1.0, now `hooks`.
    #[serde(default)]
    #[allow(dead_code)]
    pub hook: Option<HookTableRemoved>,
}

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

/// Hooks a Maildir watch fires.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct MaildirHookConfig {
    /// Fires when a message file appears in the watched maildir.
    pub on_message_add: Option<ItemHook>,
    /// Fires when one disappears from it.
    pub on_message_remove: Option<ItemHook>,
    /// Fires once for each flag letter added to a message.
    pub on_flag_add: Option<FlagHook>,
    /// Fires once for each flag letter removed from one.
    pub on_flag_remove: Option<FlagHook>,
    /// Refused: the hook names before 1.0, each pointing at its successor.
    #[serde(default)]
    #[allow(dead_code)]
    pub on_message_added: Option<OnMessageAddedRenamed>,
    #[serde(default)]
    #[allow(dead_code)]
    pub on_message_removed: Option<OnMessageRemovedRenamed>,
    #[serde(default)]
    #[allow(dead_code)]
    pub on_flag_added: Option<OnFlagAddedRenamed>,
    #[serde(default)]
    #[allow(dead_code)]
    pub on_flag_removed: Option<OnFlagRemovedRenamed>,
}

impl MaildirHookConfig {
    /// What the backend this table hangs on calls its collection.
    const COLLECTION: &'static str = MaildirConfig::COLLECTION;

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
}

impl MaildirHookConfig {
    /// Refuses a notification naming what its event cannot fill.
    pub fn validate(&self) -> Result<()> {
        hook::validate(
            self.on_message_add.as_ref().and_then(|h| h.notify.as_ref()),
            Vocabulary::item(Self::COLLECTION),
            "maildir.hooks.on-message-add",
        )?;
        hook::validate(
            self.on_message_remove
                .as_ref()
                .and_then(|h| h.notify.as_ref()),
            Vocabulary::item(Self::COLLECTION),
            "maildir.hooks.on-message-remove",
        )?;
        hook::validate(
            self.on_flag_add.as_ref().and_then(|h| h.notify.as_ref()),
            Vocabulary::flag(Self::COLLECTION),
            "maildir.hooks.on-flag-add",
        )?;
        hook::validate(
            self.on_flag_remove.as_ref().and_then(|h| h.notify.as_ref()),
            Vocabulary::flag(Self::COLLECTION),
            "maildir.hooks.on-flag-remove",
        )
    }
}

/// How a Maildir account learns about a change.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum MaildirWatchConfig {
    /// Re-list the directory on an interval, the only way a filesystem
    /// with no notification channel leaves.
    Poll(PollWatchConfig),
}
