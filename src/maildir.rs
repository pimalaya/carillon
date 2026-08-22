//! Maildir backend: a poll that diffs the mailbox against what it last
//! saw.
//!
//! A Maildir has no notification channel, so the watch re-lists the
//! mailbox on an interval and reports what moved: a file that appeared,
//! one that vanished, and the flag letters that changed in a file name.
//! The listing is names only, never message bodies, so a poll costs one
//! directory read.
//!
//! Flags are reported under their IMAP-ish names rather than their
//! Maildir letters, so one hook filter (`flags = ["Seen"]`) works the
//! same against every backend.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use anyhow::{Context, Result};
use io_maildir::{
    client::MaildirClient,
    flag::{MaildirFlag, MaildirFlags},
    maildir::Maildir,
};
use log::{debug, trace};

use crate::{config::MaildirConfig, event::WatchEvent};

/// How long the watch sleeps between two listings.
const POLL_INTERVAL: Duration = Duration::from_secs(2);
/// How long it sleeps at a time, so a shutdown is noticed promptly.
const POLL_STEP: Duration = Duration::from_millis(200);

/// Watches `mailbox` under the configured root until `shutdown` is set,
/// calling `on_event` for every change.
///
/// The first listing is a baseline: it reports nothing, since every
/// message already there is not news.
pub fn watch(
    config: &MaildirConfig,
    mailbox: &str,
    shutdown: &Arc<AtomicBool>,
    mut on_event: impl FnMut(WatchEvent),
) -> Result<()> {
    let client = MaildirClient::new(config.root.clone());
    let maildir = resolve(config, mailbox);

    let mut seen = list(&client, &maildir)?;
    debug!("watching maildir with {} entries", seen.len());

    while !shutdown.load(Ordering::SeqCst) {
        if !sleep(POLL_INTERVAL, shutdown) {
            break;
        }

        let current = match list(&client, &maildir) {
            Ok(current) => current,
            // NOTE: a transient read failure (a rename mid-listing, a
            // mount hiccup) must not end the watch; the next poll
            // re-reads the whole mailbox anyway.
            Err(err) => {
                debug!("maildir listing failed, retrying next poll: {err:#}");
                continue;
            }
        };

        for event in diff(&seen, &current) {
            on_event(event);
        }

        seen = current;
    }

    Ok(())
}

/// Lists the mailbox as message id to flag names.
fn list(client: &MaildirClient, maildir: &Maildir) -> Result<BTreeMap<String, BTreeSet<String>>> {
    let entries = client
        .list_entries(maildir.clone())
        .context("cannot list maildir entries")?;

    let listed = entries
        .into_iter()
        .filter_map(|entry| {
            let id = entry.id()?.to_string();
            Some((id, render_flags(&entry.flags())))
        })
        .collect();

    trace!("listed maildir entries: {listed:?}");

    Ok(listed)
}

/// Reports what changed between two listings.
fn diff(
    before: &BTreeMap<String, BTreeSet<String>>,
    after: &BTreeMap<String, BTreeSet<String>>,
) -> Vec<WatchEvent> {
    let mut events = Vec::new();

    for id in before.keys() {
        if !after.contains_key(id) {
            events.push(WatchEvent::MessageRemoved { id: id.clone() });
        }
    }

    for (id, flags) in after {
        let Some(before) = before.get(id) else {
            events.push(WatchEvent::MessageAdded { id: id.clone() });
            continue;
        };

        let added: BTreeSet<String> = flags.difference(before).cloned().collect();
        if !added.is_empty() {
            events.push(WatchEvent::FlagsAdded {
                id: id.clone(),
                flags: added,
            });
        }

        let removed: BTreeSet<String> = before.difference(flags).cloned().collect();
        if !removed.is_empty() {
            events.push(WatchEvent::FlagsRemoved {
                id: id.clone(),
                flags: removed,
            });
        }
    }

    events
}

/// Resolves the watched mailbox: the root itself for `.`, otherwise the
/// named Maildir under it.
fn resolve(config: &MaildirConfig, mailbox: &str) -> Maildir {
    if mailbox == "." || mailbox.is_empty() {
        return Maildir::from_path(config.root.clone());
    }

    Maildir::from_path(config.root.join(mailbox))
}

/// Renders Maildir flags under the names a hook filter matches, so a
/// filter written for IMAP fires here too.
fn render_flags(flags: &MaildirFlags) -> BTreeSet<String> {
    flags
        .iter()
        .map(|flag| match flag {
            MaildirFlag::Passed => String::from("Passed"),
            MaildirFlag::Replied => String::from("Answered"),
            MaildirFlag::Seen => String::from("Seen"),
            MaildirFlag::Trashed => String::from("Deleted"),
            MaildirFlag::Draft => String::from("Draft"),
            MaildirFlag::Flagged => String::from("Flagged"),
            MaildirFlag::Keyword(keyword) => keyword.clone(),
        })
        .collect()
}

/// Sleeps `total` in small steps, returning false as soon as a
/// shutdown is requested.
fn sleep(total: Duration, shutdown: &Arc<AtomicBool>) -> bool {
    let mut left = total;

    while left > Duration::ZERO {
        if shutdown.load(Ordering::SeqCst) {
            return false;
        }

        let step = left.min(POLL_STEP);
        thread::sleep(step);
        left -= step;
    }

    !shutdown.load(Ordering::SeqCst)
}
