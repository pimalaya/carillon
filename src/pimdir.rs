//! # Pimdir
//!
//! The pimdir backend: one calendar collection of a local pimdir store,
//! read as a reader (pimdir STORAGE §8, §14.1) and never written. Its
//! committed rows are read, the pending queue left out, and the store's
//! change feed (§4.5) is followed to read again only what moved. The
//! store's owner keeps it in sync; this backend only fires its alarms.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use anyhow::{Context, Result, bail};
use io_pimdir::client::{
    blobs::PimdirBlobs,
    reader::{PimdirChangeCursor, PimdirItem, PimdirReader},
};
use log::{debug, trace, warn};

use crate::{
    alarm::{AlarmBook, AlarmShared},
    config::PimdirConfig,
    poll,
};

/// How long the watch waits between two reads of the change cursor,
/// unless the config names another interval.
const POLL_INTERVAL: Duration = Duration::from_secs(30);

/// How many rows one page of a listing or of the change feed holds.
const PAGE: usize = 500;

/// The media type of a calendar collection, its parameters aside.
const CALENDAR_KIND: &str = "text/calendar";

/// What a member of the calendar became, read before the alarms are
/// changed so the lock is held only to apply it.
enum Moved {
    Read {
        key: String,
        etag: Option<String>,
        body: Vec<u8>,
    },
    Gone(String),
}

/// Watches the configured calendar until `shutdown` is set, keeping
/// `alarms` current.
pub fn watch(
    config: &PimdirConfig,
    shutdown: &Arc<AtomicBool>,
    alarms: &AlarmShared,
) -> Result<()> {
    let reader = open(config)?;
    let interval = config.interval().unwrap_or(POLL_INTERVAL);
    let mut cursor = load(&reader, &config.calendar, alarms)?;

    while !shutdown.load(Ordering::SeqCst) {
        if !poll::sleep(interval, shutdown) {
            break;
        }

        cursor = follow(&reader, &config.calendar, cursor, alarms)?;
    }

    Ok(())
}

/// Checks that the store opens and holds the calendar, without reading
/// any item.
pub fn probe(config: &PimdirConfig) -> Result<()> {
    let reader = open(config)?;
    check_calendar(&reader, &config.calendar)
}

/// Opens the store as a reader, which creates nothing: a store its owner
/// has not created is refused, naming the root.
fn open(config: &PimdirConfig) -> Result<PimdirReader> {
    let root = &config.root;

    match PimdirReader::open(root) {
        Ok(reader) => Ok(reader),
        Err(err) => bail!("Cannot open pimdir store at {}: {err}", root.display()),
    }
}

/// Refuses a calendar the store does not hold, or holding no calendars.
fn check_calendar(reader: &PimdirReader, calendar: &str) -> Result<()> {
    let Some(kind) = reader.collection_kind(calendar)? else {
        bail!("Cannot find pimdir collection `{calendar}`");
    };

    if !is_calendar(&kind) {
        bail!("Pimdir collection `{calendar}` holds {kind}, not calendars");
    }

    Ok(())
}

/// Reads every item of the calendar, and answers the cursor it was read
/// at: read before the items, so a change landing meanwhile is folded
/// again rather than skipped.
fn load(reader: &PimdirReader, calendar: &str, alarms: &AlarmShared) -> Result<PimdirChangeCursor> {
    let cursor = reader.change_cursor()?;
    check_calendar(reader, calendar)?;

    let blobs = reader.blobs();
    let mut book = AlarmBook::new();
    let mut after: Option<String> = None;

    loop {
        let page = reader
            .list_items(calendar, after.as_deref(), PAGE)
            .with_context(|| format!("Cannot list pimdir collection `{calendar}`"))?;

        let Some(last) = page.last() else {
            break;
        };

        after = Some(last.link_id.0.clone());
        let full = page.len() == PAGE;

        for item in &page {
            match body(&blobs, item) {
                Moved::Read { key, etag, body } => book.read(&key, &key, etag.as_deref(), &body),
                Moved::Gone(key) => book.forget(&key),
            }
        }

        if !full {
            break;
        }
    }

    debug!("read {} pimdir items holding an alarm", book.len());
    alarms.replace(book);

    Ok(cursor)
}

/// Folds what moved in the calendar since `cursor`, and answers the
/// cursor to look from next.
///
/// A reset of the store, a purge or the calendar itself moving reads the
/// calendar whole again; otherwise each item stamped above the cursor is
/// read again, or forgotten when deleted or retained.
fn follow(
    reader: &PimdirReader,
    calendar: &str,
    cursor: PimdirChangeCursor,
    alarms: &AlarmShared,
) -> Result<PimdirChangeCursor> {
    let now = reader.change_cursor()?;

    if now == cursor {
        return Ok(cursor);
    }

    let reset = now.store_id != cursor.store_id
        || now.incarnation != cursor.incarnation
        || now.purges != cursor.purges
        || now.changed < cursor.changed;

    let renamed = || -> Result<bool> {
        let changed = reader.collections_changed_since(cursor.changed, PAGE)?;
        Ok(changed.iter().any(|collection| collection.id == calendar))
    };

    if reset || renamed()? {
        debug!("pimdir store reset or calendar moved, reading it again");
        return load(reader, calendar, alarms);
    }

    let blobs = reader.blobs();
    let mut moved = Vec::new();
    let mut since = cursor.changed;

    loop {
        let page = reader.items_changed_since(since, PAGE)?;

        let Some(last) = page.last() else {
            break;
        };

        since = last.changed;
        let full = page.len() == PAGE;

        for change in page.iter().filter(|change| change.collection == calendar) {
            trace!("pimdir item {} moved", change.seq);

            if change.deleted || change.retained_at.is_some() {
                moved.push(Moved::Gone(change.seq.to_string()));
                continue;
            }

            match reader.get_item(calendar, change.seq)? {
                Some(item) => moved.push(body(&blobs, &item)),
                None => moved.push(Moved::Gone(change.seq.to_string())),
            }
        }

        if !full {
            break;
        }
    }

    if !moved.is_empty() {
        alarms.update(|book| {
            for moved in moved {
                match moved {
                    Moved::Read { key, etag, body } => {
                        book.read(&key, &key, etag.as_deref(), &body)
                    }
                    Moved::Gone(key) => book.forget(&key),
                }
            }
        });
    }

    Ok(now)
}

/// The body of an item, keyed by its public id (pimdir STORAGE §9.1),
/// its object hash standing as its entity tag. An item whose body is not
/// local yet is gone until a change brings it.
fn body(blobs: &PimdirBlobs, item: &PimdirItem) -> Moved {
    let key = item.seq.to_string();

    let Some(hash) = &item.object else {
        trace!("pimdir item {key} has no body yet");
        return Moved::Gone(key);
    };

    match blobs.get(hash) {
        Ok(Some(body)) => Moved::Read {
            key,
            etag: Some(hash.0.clone()),
            body,
        },
        Ok(None) => {
            warn!("body of pimdir item {key} is missing");
            Moved::Gone(key)
        }
        Err(err) => {
            warn!("cannot read the body of pimdir item {key}: {err}");
            Moved::Gone(key)
        }
    }
}

/// Whether a collection kind is a calendar's, its parameters aside.
fn is_calendar(kind: &str) -> bool {
    kind.split(';')
        .next()
        .is_some_and(|kind| kind.trim().eq_ignore_ascii_case(CALENDAR_KIND))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use io_pimdir::{
        client::{PimdirStore, producer::PimdirProducer, reader::PimdirReader},
        codec::PimdirAction,
        object::PimdirHash,
        placement::PimdirFlags,
    };
    use jiff::Timestamp;
    use tempfile::TempDir;

    use crate::{alarm::AlarmShared, config::PimdirConfig, pimdir::*};

    /// The calendar collection of every case.
    const CAL: &str = "cal";

    /// An instant written in UTC, in seconds.
    fn at(time: &str) -> i64 {
        format!("2026-11-05T{time}Z")
            .parse::<Timestamp>()
            .unwrap()
            .as_second()
    }

    /// An event of `uid` starting at `start` (UTC, `HHMMSS`) with one
    /// `-PT15M` display alarm.
    fn event(uid: &str, start: &str) -> String {
        format!(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Test//EN\r\nBEGIN:VEVENT\r\nUID:{uid}\r\nDTSTAMP:20261101T000000Z\r\nDTSTART:20261105T{start}Z\r\nDURATION:PT30M\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:x\r\nTRIGGER:-PT15M\r\nEND:VALARM\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
        )
    }

    /// A scratch store holding a calendar and a mail collection.
    fn scratch() -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        let store = PimdirStore::open(dir.path()).unwrap();
        store.ensure_collection(CAL, "text/calendar").unwrap();
        store.ensure_collection("mail", "message/rfc822").unwrap();
        dir
    }

    fn config(dir: &Path, calendar: &str) -> PimdirConfig {
        PimdirConfig {
            calendar: calendar.to_owned(),
            root: dir.to_path_buf(),
            watch: None,
            hook: Default::default(),
        }
    }

    /// Queues one action with its body as a producer does, then applies
    /// it as the store's owner does.
    fn apply(
        dir: &Path,
        collection: &str,
        body: &str,
        action: impl FnOnce(PimdirHash) -> PimdirAction,
    ) {
        let mut producer = PimdirProducer::open(dir, "test").unwrap();
        producer
            .enqueue_with_body(collection, body.as_bytes(), action)
            .unwrap();
        drop(producer);

        drain(dir);
    }

    fn drain(dir: &Path) {
        let mut owner = PimdirStore::open(dir).unwrap().for_source("test");
        owner.drain().unwrap();
    }

    fn add(dir: &Path, collection: &str, body: &str) {
        apply(dir, collection, body, |hash| PimdirAction::Add {
            link_id: None,
            flags: PimdirFlags::default(),
            object: Some(hash),
            supersedes: None,
        });
    }

    fn seq(dir: &Path, uid: &str) -> i64 {
        PimdirReader::open(dir)
            .unwrap()
            .seq_for_link(CAL, uid)
            .unwrap()
            .unwrap()
    }

    #[test]
    fn a_store_its_owner_has_not_created_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let err = probe(&config(dir.path(), CAL)).expect_err("no store");

        assert!(
            format!("{err:#}").contains(&dir.path().display().to_string()),
            "{err:#}"
        );
        assert!(!dir.path().join("pimdir.db").exists());
    }

    #[test]
    fn a_collection_holding_no_calendars_is_refused() {
        let dir = scratch();

        probe(&config(dir.path(), CAL)).expect("a calendar");
        probe(&config(dir.path(), "mail")).expect_err("a mailbox");
        probe(&config(dir.path(), "nowhere")).expect_err("no such collection");
    }

    #[test]
    fn a_calendar_is_read_then_followed() {
        let dir = scratch();
        add(dir.path(), CAL, &event("standup", "090000"));
        add(dir.path(), "mail", &event("quoted", "090000"));

        let reader = PimdirReader::open(dir.path()).unwrap();
        let alarms = AlarmShared::new();
        let cursor = load(&reader, CAL, &alarms).unwrap();
        let id = seq(dir.path(), "standup").to_string();

        assert_eq!(
            vec![(id.clone(), at("08:45:00"))],
            alarms.triggers(at("00:00:00"), at("23:00:00"))
        );

        let seq = seq(dir.path(), "standup");
        apply(dir.path(), CAL, &event("standup", "100000"), |hash| {
            PimdirAction::Update {
                seq,
                object: hash,
                if_match: None,
                resolves: None,
            }
        });
        let cursor = follow(&reader, CAL, cursor, &alarms).unwrap();

        assert_eq!(
            vec![(id, at("09:45:00"))],
            alarms.triggers(at("00:00:00"), at("23:00:00"))
        );

        let mut producer = PimdirProducer::open(dir.path(), "test").unwrap();
        producer
            .enqueue(CAL, &PimdirAction::Remove { seq }, None)
            .unwrap();
        drop(producer);
        drain(dir.path());
        follow(&reader, CAL, cursor, &alarms).unwrap();

        assert!(alarms.triggers(at("00:00:00"), at("23:00:00")).is_empty());
    }
}
