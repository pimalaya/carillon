//! # pimdir
//!
//! The `pimdir` block: the store, the calendar and the addressbook read
//! from it, the poll and the hooks firing their alarms and their dates.

use std::{path::PathBuf, time::Duration};

use anyhow::{Result, bail};
use serde::Deserialize;

use crate::{
    config::{BirthdayHook, ItemHook, PollWatchConfig},
    hook::{self, HookCollection, Vocabulary},
};

/// pimdir configuration: a calendar, an addressbook or both of a local
/// pimdir store, read and never written, the alarms of the first and the
/// birthdays and anniversaries of the second fired.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct PimdirConfig {
    /// The calendar whose alarms this account fires, by the collection id
    /// a listing of the store gives.
    #[serde(default)]
    pub calendar: Option<String>,
    /// The addressbook whose birthdays and anniversaries this account
    /// fires, by the collection id a listing of the store gives.
    #[serde(default)]
    pub addressbook: Option<String>,
    /// The directory holding the store, which its owner keeps in sync.
    #[serde(deserialize_with = "pimalaya_config::toml::shell_expanded_path")]
    pub root: PathBuf,
    /// The account of the store the collections are grouped under (pimdir
    /// STORAGE §9.2), for a store several accounts share.
    ///
    /// Unset reads a collection whichever account it is grouped under, its
    /// id naming one collection of the store; set, a collection grouped
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
    /// What pimdir calls the calendar it watches.
    pub const CALENDAR: &'static str = "calendar";
    /// What pimdir calls the addressbook it watches.
    pub const ADDRESSBOOK: &'static str = "addressbook";

    /// The calendar this backend watches, under its own name.
    pub fn calendar(&self) -> Option<HookCollection<'_>> {
        let value = self.calendar.as_deref()?;

        Some(HookCollection {
            name: Self::CALENDAR,
            value,
        })
    }

    /// The addressbook this backend watches, under its own name.
    pub fn addressbook(&self) -> Option<HookCollection<'_>> {
        let value = self.addressbook.as_deref()?;

        Some(HookCollection {
            name: Self::ADDRESSBOOK,
            value,
        })
    }

    /// The collections this backend watches, the calendar first.
    pub fn collections(&self) -> Vec<HookCollection<'_>> {
        self.calendar()
            .into_iter()
            .chain(self.addressbook())
            .collect()
    }

    /// The interval between two reads of the change cursor.
    pub fn interval(&self) -> Option<Duration> {
        let Some(PimdirWatchConfig::Poll(poll)) = &self.watch else {
            return None;
        };

        poll.interval()
    }

    /// Refuses a block watching nothing, a collection without the hook it
    /// fires, a hook without its collection, and a notification naming
    /// what its firing cannot fill.
    pub fn validate(&self) -> Result<()> {
        let alarm = &self.hooks.on_alarm_trigger;
        let birthday = &self.hooks.on_birthday_trigger;

        match (&self.calendar, &self.addressbook) {
            (None, None) => {
                bail!(
                    "`pimdir` needs `calendar`, `addressbook` or both, the collections it watches"
                )
            }
            (Some(_), _) if alarm.is_none() => {
                bail!("`pimdir.calendar` needs `pimdir.hooks.on-alarm-trigger`, the hook it fires")
            }
            (_, Some(_)) if birthday.is_none() => bail!(
                "`pimdir.addressbook` needs `pimdir.hooks.on-birthday-trigger`, the hook it fires"
            ),
            (None, _) if alarm.is_some() => {
                bail!(
                    "`pimdir.hooks.on-alarm-trigger` needs `pimdir.calendar`, the alarms it fires"
                )
            }
            (_, None) if birthday.is_some() => bail!(
                "`pimdir.hooks.on-birthday-trigger` needs `pimdir.addressbook`, the dates it fires"
            ),
            _ => (),
        }

        hook::validate(
            alarm.as_ref().and_then(|hook| hook.notify.as_ref()),
            Vocabulary::alarm(Self::CALENDAR),
            "pimdir.hooks.on-alarm-trigger",
        )?;

        hook::validate(
            birthday.as_ref().and_then(|hook| hook.notify.as_ref()),
            Vocabulary::birthday(Self::ADDRESSBOOK),
            "pimdir.hooks.on-birthday-trigger",
        )
    }
}

/// Hooks a pimdir watch fires.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct PimdirHookConfig {
    /// Fires when an alarm of a VEVENT or a VTODO reaches its trigger.
    pub on_alarm_trigger: Option<ItemHook>,
    /// Fires once a year for each `BDAY` and `ANNIVERSARY` of a vCard.
    pub on_birthday_trigger: Option<BirthdayHook>,
}

/// How a pimdir account learns about a change.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum PimdirWatchConfig {
    /// Read the store's change cursor on an interval.
    Poll(PollWatchConfig),
}
