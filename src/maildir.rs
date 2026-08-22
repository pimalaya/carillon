//! Maildir backend: a poll that diffs the mailbox against what it last
//! saw.
//!
//! A Maildir has no notification channel, so the watch re-lists it on
//! an interval. The listing is file names only, never bodies, so a
//! poll costs one directory read, and the flag letters live in those
//! names. They are reported under their shared names, so one filter
//! (`flags = ["Seen"]`) fires on every backend.

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
    path::MaildirPath,
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
    let maildir = resolve(&client, mailbox)?;

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
            events.push(WatchEvent::ItemRemoved { id: id.clone() });
        }
    }

    for (id, flags) in after {
        let Some(before) = before.get(id) else {
            events.push(WatchEvent::ItemAdded { id: id.clone() });
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

/// Resolves the watched mailbox through the store, so the layout and
/// the validation are io-maildir's rather than a hand-joined path.
fn resolve(client: &MaildirClient, mailbox: &str) -> Result<Maildir> {
    // NOTE: the empty path is the store root, which is the mailbox a
    // flat Maildir holds; `.` is how a config says so readably.
    let name = match mailbox {
        "." | "INBOX" => MaildirPath::default(),
        mailbox => MaildirPath::from(mailbox),
    };

    // NOTE: resolving through the client rather than joining the root
    // by hand is what applies the store's layout (dot-prefixed flat
    // names under Maildir++, real nested directories otherwise) and
    // what turns a wrong name into an error instead of a listing that
    // stays empty forever.
    client
        .load_maildir(name)
        .with_context(|| format!("cannot open maildir `{mailbox}`"))
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

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    /// Builds a Maildir at `path`, with the three subdirectories a
    /// listing needs.
    fn maildir(path: &std::path::Path) {
        for sub in ["cur", "new", "tmp"] {
            fs::create_dir_all(path.join(sub)).expect("maildir subdirectory");
        }
    }

    /// Writes an entry into one of the Maildir subdirectories, named
    /// the way a delivering agent would.
    fn entry(path: &std::path::Path, sub: &str, name: &str) {
        fs::write(path.join(sub).join(name), b"body").expect("maildir entry");
    }

    fn listing(pairs: &[(&str, &[&str])]) -> BTreeMap<String, BTreeSet<String>> {
        pairs
            .iter()
            .map(|(id, flags)| {
                let flags = flags.iter().map(|flag| flag.to_string()).collect();
                (id.to_string(), flags)
            })
            .collect()
    }

    #[test]
    fn an_arrival_and_a_departure_are_reported() {
        let before = listing(&[("kept", &[]), ("gone", &[])]);
        let after = listing(&[("kept", &[]), ("new", &[])]);

        assert_eq!(
            vec![
                WatchEvent::ItemRemoved {
                    id: String::from("gone")
                },
                WatchEvent::ItemAdded {
                    id: String::from("new")
                },
            ],
            diff(&before, &after),
        );
    }

    #[test]
    fn a_flag_moving_either_way_is_reported() {
        let before = listing(&[("one", &["Flagged"])]);
        let after = listing(&[("one", &["Seen"])]);

        let events = diff(&before, &after);
        assert_eq!(2, events.len(), "got {events:?}");

        let WatchEvent::FlagsAdded { flags, .. } = &events[0] else {
            panic!("expected FlagsAdded, got {:?}", events[0]);
        };
        assert_eq!(&BTreeSet::from([String::from("Seen")]), flags);

        let WatchEvent::FlagsRemoved { flags, .. } = &events[1] else {
            panic!("expected FlagsRemoved, got {:?}", events[1]);
        };
        assert_eq!(&BTreeSet::from([String::from("Flagged")]), flags);
    }

    #[test]
    fn an_unchanged_mailbox_reports_nothing() {
        let listed = listing(&[("one", &["Seen"]), ("two", &[])]);

        assert!(diff(&listed, &listed).is_empty());
    }

    /// The regression this file exists for: a subfolder is resolved
    /// through the store, so it is found, and a wrong name fails
    /// instead of listing a directory that is not there.
    #[test]
    fn a_subfolder_resolves_through_the_store() {
        let root = TempDir::new().expect("temp dir");
        maildir(root.path());
        maildir(&root.path().join("Archive"));
        entry(root.path(), "new", "1700000000.a.host");
        entry(&root.path().join("Archive"), "cur", "1700000001.b.host:2,S");

        let client = MaildirClient::new(root.path().to_path_buf());

        let inbox = resolve(&client, "INBOX").expect("inbox resolves");
        let listed = list(&client, &inbox).expect("inbox lists");
        assert_eq!(vec!["1700000000.a.host"], listed.keys().collect::<Vec<_>>());

        let archive = resolve(&client, "Archive").expect("subfolder resolves");
        let listed = list(&client, &archive).expect("subfolder lists");
        assert_eq!(
            vec![&BTreeSet::from([String::from("Seen")])],
            listed.values().collect::<Vec<_>>(),
        );

        let err = resolve(&client, "Nope").expect_err("unknown mailbox fails");
        assert!(format!("{err:#}").contains("Nope"), "got {err:#}");
    }

    /// Reading a message moves its file from `new` to `cur` and appends
    /// the `S` letter, keeping the name before `:2,`. The watch must
    /// see one flag change, not a message leaving and another arriving.
    #[test]
    fn reading_a_message_is_a_flag_change_not_a_move() {
        let root = TempDir::new().expect("temp dir");
        maildir(root.path());
        entry(root.path(), "new", "1700000000.a.host");

        let client = MaildirClient::new(root.path().to_path_buf());
        let maildir = resolve(&client, ".").expect("root resolves");
        let before = list(&client, &maildir).expect("first listing");

        fs::rename(
            root.path().join("new").join("1700000000.a.host"),
            root.path().join("cur").join("1700000000.a.host:2,S"),
        )
        .expect("mark as read");

        let after = list(&client, &maildir).expect("second listing");
        let events = diff(&before, &after);

        assert_eq!(
            vec![WatchEvent::FlagsAdded {
                id: String::from("1700000000.a.host"),
                flags: BTreeSet::from([String::from("Seen")]),
            }],
            events,
        );
    }
}
