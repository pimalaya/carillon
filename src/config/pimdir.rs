//! # pimdir
//!
//! The `pimdir` block: the store, the calendar read from it, the poll and
//! the alarm hook.

use std::{path::PathBuf, time::Duration};

use anyhow::{Result, bail};
use serde::Deserialize;

use crate::{
    config::{ItemHook, PollWatchConfig},
    hook::{self, HookCollection, Vocabulary},
};

/// pimdir configuration: one calendar collection of a local pimdir store,
/// read and never written, its alarms fired.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct PimdirConfig {
    /// The calendar this account watches, by the collection id a
    /// listing of the store gives.
    pub calendar: String,
    /// The directory holding the store, which its owner keeps in sync.
    #[serde(deserialize_with = "pimalaya_config::toml::shell_expanded_path")]
    pub root: PathBuf,
    /// The account of the store the calendar is grouped under (pimdir
    /// STORAGE §9.2), for a store several accounts share.
    ///
    /// Unset reads the calendar whichever account it is grouped under, its
    /// id naming one collection of the store; set, a calendar grouped
    /// under another account is not found.
    #[serde(default)]
    pub account: Option<String>,
    /// How this account learns about a change. Unset polls.
    #[serde(default)]
    pub watch: Option<PimdirWatchConfig>,
    /// The hooks this backend fires.
    #[serde(default)]
    pub hooks: PimdirHookConfig,
}

impl PimdirConfig {
    /// What pimdir calls the collection it watches, a calendar.
    pub const COLLECTION: &'static str = "calendar";

    /// The collection this backend watches, under its own name.
    pub fn collection(&self) -> HookCollection<'_> {
        HookCollection {
            name: Self::COLLECTION,
            value: &self.calendar,
        }
    }

    /// The interval between two reads of the change cursor.
    pub fn interval(&self) -> Option<Duration> {
        let Some(PimdirWatchConfig::Poll(poll)) = &self.watch else {
            return None;
        };

        poll.interval()
    }
}

/// Hooks a pimdir watch fires.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct PimdirHookConfig {
    /// Fires when an alarm of a VEVENT or a VTODO reaches its trigger.
    pub on_alarm_trigger: Option<ItemHook>,
}

impl PimdirHookConfig {
    /// Refuses a table without its one hook, which would watch for
    /// nothing, and a notification naming what an alarm cannot fill.
    pub fn validate(&self) -> Result<()> {
        let Some(hook) = &self.on_alarm_trigger else {
            bail!("pimdir.hooks needs `on-alarm-trigger`, the one hook a pimdir watch fires");
        };

        hook::validate(
            hook.notify.as_ref(),
            Vocabulary::alarm(PimdirConfig::COLLECTION),
            "pimdir.hooks.on-alarm-trigger",
        )
    }
}

/// How a pimdir account learns about a change.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum PimdirWatchConfig {
    /// Read the store's change cursor on an interval.
    Poll(PollWatchConfig),
}
