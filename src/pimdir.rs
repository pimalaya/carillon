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

use anyhow::{Context, Error, Result, bail};
use io_pimdir::{
    client::{
        PimdirError,
        blobs::PimdirBlobs,
        reader::{PimdirChangeCursor, PimdirItem, PimdirReader},
    },
    sql::FORMAT_VERSION,
    version::PimdirFormatRefusal,
};
use log::{debug, trace, warn};
use pimalaya_cli::error::CodedError;

use crate::{
    alarm::{AlarmBook, AlarmShared},
    config::pimdir::PimdirConfig,
    error::{self, ErrorCode},
    event::WatchDomain,
    poll,
};

/// How long the watch waits between two reads of the change cursor,
/// unless the config names another interval.
const POLL_INTERVAL: Duration = Duration::from_secs(30);

/// How many rows one page of a listing or of the change feed holds.
const PAGE: usize = 500;

/// The media type of a calendar collection, its parameters aside.
const CALENDAR_KIND: &str = "text/calendar";

/// The program a refused store's message names.
const PROGRAM: &str = "carillon";

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
///
/// `on_open` is called once the session is up and its collection read,
/// right before the first wait.
pub fn watch(
    config: &PimdirConfig,
    shutdown: &Arc<AtomicBool>,
    alarms: &AlarmShared,
    on_open: impl FnOnce(),
) -> Result<()> {
    let reader = open(config)?;
    let interval = config.interval().unwrap_or(POLL_INTERVAL);
    let account = config.account.as_deref();
    let mut cursor = load(&reader, &config.calendar, account, alarms)?;
    on_open();

    while !shutdown.load(Ordering::SeqCst) {
        if !poll::sleep(interval, shutdown) {
            break;
        }

        cursor = follow(&reader, &config.calendar, account, cursor, alarms)?;
    }

    Ok(())
}

/// Checks that the store opens and holds the calendar, without reading
/// any item.
pub fn probe(config: &PimdirConfig) -> Result<()> {
    let reader = open(config)?;
    check_calendar(&reader, &config.calendar, config.account.as_deref())
}

/// Opens the store as a reader, which creates nothing: a store its owner
/// has not created is refused, naming the root.
///
/// A store of another format version is refused naming carillon and the
/// version it reads (see [`PimdirFormatRefusal`]), coded as io-pimdir
/// codes it.
fn open(config: &PimdirConfig) -> Result<PimdirReader> {
    let root = &config.root;

    PimdirReader::open(root)
        .map_err(refusal)
        .with_context(|| format!("Cannot open pimdir store at {}", root.display()))
}

/// A failure to open the store, a store of another format version coded
/// with the message naming carillon, any other kept as io-pimdir's.
fn refusal(err: PimdirError) -> Error {
    let PimdirError::Version { found } = &err else {
        return Error::new(err);
    };

    let message = PimdirFormatRefusal {
        found: found.as_deref(),
        own: FORMAT_VERSION,
        program: PROGRAM,
    }
    .to_string();

    match ErrorCode::of_pimdir(&err) {
        Some(code) => CodedError::new(code, message).into(),
        None => Error::new(err),
    }
}

/// The code of a store failure in an error chain, as io-pimdir names it
/// ([`ErrorCode::of_pimdir`]).
pub(crate) fn store_code(cause: &(dyn std::error::Error + 'static)) -> Option<ErrorCode> {
    ErrorCode::of_pimdir(cause.downcast_ref::<PimdirError>()?)
}

/// Refuses a calendar the store does not hold, one grouped under another
/// account than `account` when given, and one holding no calendars.
fn check_calendar(reader: &PimdirReader, calendar: &str, account: Option<&str>) -> Result<()> {
    let Some(kind) = reader.collection_kind(calendar)? else {
        let message = format!("Cannot find pimdir calendar `{calendar}`");
        return Err(error::not_found(WatchDomain::Event, message));
    };

    if let Some(account) = account
        && reader.collection_account(calendar)?.flatten().as_deref() != Some(account)
    {
        let message = format!("Cannot find pimdir calendar `{calendar}` under account `{account}`");
        return Err(error::not_found(WatchDomain::Event, message));
    }

    if !is_calendar(&kind) {
        bail!("Pimdir collection `{calendar}` holds {kind}, not calendars");
    }

    Ok(())
}

/// Reads every item of the calendar, and answers the cursor it was read
/// at: read before the items, so a change landing meanwhile is folded
/// again rather than skipped.
fn load(
    reader: &PimdirReader,
    calendar: &str,
    account: Option<&str>,
    alarms: &AlarmShared,
) -> Result<PimdirChangeCursor> {
    let cursor = reader.change_cursor()?;
    check_calendar(reader, calendar, account)?;

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
    account: Option<&str>,
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
        return load(reader, calendar, account, alarms);
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
    use std::{
        path::Path,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        thread,
        time::Duration,
    };

    use ceichas::timer::CeichasTimer;
    use io_pimdir::{
        client::{PimdirStore, producer::PimdirProducer, reader::PimdirReader},
        codec::PimdirAction,
        object::PimdirHash,
        placement::PimdirFlags,
    };
    use jiff::Timestamp;
    use tempfile::TempDir;

    use crate::{
        alarm::{self, AlarmShared},
        config::pimdir::PimdirConfig,
        pimdir::*,
        poll,
    };

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
            account: None,
            watch: None,
            hooks: Default::default(),
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
    fn a_watch_of_a_store_not_created_is_never_reported_open() {
        let dir = tempfile::tempdir().unwrap();
        let timer = CeichasTimer::new(Default::default()).unwrap();
        let alarms = AlarmShared::new(timer.waker());
        let shutdown = Arc::new(AtomicBool::new(false));
        let mut opened = false;

        let result = watch(&config(dir.path(), CAL), &shutdown, &alarms, || {
            opened = true
        });
        assert!(result.is_err());
        assert!(!opened);
    }

    #[test]
    fn a_collection_holding_no_calendars_is_refused() {
        let dir = scratch();

        probe(&config(dir.path(), CAL)).expect("a calendar");
        probe(&config(dir.path(), "mail")).expect_err("a mailbox");
        probe(&config(dir.path(), "nowhere")).expect_err("no such collection");
    }

    /// A calendar the store does not hold, or holds under another account
    /// than `pimdir.account` names, is `calendar-not-found`.
    #[test]
    fn a_calendar_not_found_is_coded_so() {
        use crate::error::{ErrorCode, code};

        let dir = scratch();
        let store = PimdirStore::open(dir.path()).unwrap();
        store.set_collection_account(CAL, Some("work")).unwrap();
        drop(store);

        let err = probe(&config(dir.path(), "nowhere")).expect_err("no such calendar");
        assert_eq!(Some(ErrorCode::CalendarNotFound), code(&err), "{err:#}");

        let mut work = config(dir.path(), CAL);
        work.account = Some(String::from("work"));
        probe(&work).expect("the calendar of work");
        probe(&config(dir.path(), CAL)).expect("the calendar, whichever account");

        let mut perso = config(dir.path(), CAL);
        perso.account = Some(String::from("perso"));
        let err = probe(&perso).expect_err("a calendar of work");
        assert_eq!(Some(ErrorCode::CalendarNotFound), code(&err), "{err:#}");
        assert!(format!("{err:#}").contains("`perso`"), "{err:#}");
    }

    #[test]
    fn a_calendar_is_read_then_followed() {
        let dir = scratch();
        add(dir.path(), CAL, &event("standup", "090000"));
        add(dir.path(), "mail", &event("quoted", "090000"));

        let reader = PimdirReader::open(dir.path()).unwrap();
        let timer = CeichasTimer::new(Default::default()).unwrap();
        let alarms = AlarmShared::new(timer.waker());
        let cursor = load(&reader, CAL, None, &alarms).unwrap();
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
        let cursor = follow(&reader, CAL, None, cursor, &alarms).unwrap();

        assert_eq!(
            vec![(id, at("09:45:00"))],
            alarms.triggers(at("00:00:00"), at("23:00:00"))
        );

        let mut producer = PimdirProducer::open(dir.path(), "test").unwrap();
        producer
            .enqueue(
                CAL,
                &PimdirAction::Remove {
                    seq,
                    if_match: None,
                    resolves: None,
                },
                None,
            )
            .unwrap();
        drop(producer);
        drain(dir.path());
        follow(&reader, CAL, None, cursor, &alarms).unwrap();

        assert!(alarms.triggers(at("00:00:00"), at("23:00:00")).is_empty());
    }

    /// An event of `uid` whose one `-PT15M` display alarm triggers at
    /// `trigger`, in seconds since the Unix epoch.
    fn event_alarming_at(uid: &str, trigger: i64) -> String {
        let start = Timestamp::from_second(trigger + 900).unwrap();
        let start = start.strftime("%Y%m%dT%H%M%SZ");

        format!(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Test//EN\r\nBEGIN:VEVENT\r\nUID:{uid}\r\nDTSTAMP:20261101T000000Z\r\nDTSTART:{start}\r\nDURATION:PT30M\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:x\r\nTRIGGER:-PT15M\r\nEND:VALARM\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
        )
    }

    #[test]
    fn a_store_of_the_current_draft_is_watched_and_its_alarm_fires() {
        let dir = scratch();
        let trigger = Timestamp::now().as_second() + 2;
        add(dir.path(), CAL, &event_alarming_at("standup", trigger));

        let config = config(dir.path(), CAL);
        let timer = CeichasTimer::new(Default::default()).unwrap();
        let alarms = AlarmShared::new(timer.waker());
        let shutdown = Arc::new(AtomicBool::new(false));
        let mut fired = Vec::new();

        thread::scope(|scope| {
            let watch = scope.spawn(|| watch(&config, &shutdown, &alarms, || ()));

            // Ends the test should the alarm never fire.
            scope.spawn(|| {
                if poll::sleep(Duration::from_secs(30), &shutdown) {
                    shutdown.store(true, Ordering::SeqCst);
                    alarms.update(|_| ());
                }
            });

            alarm::run(&alarms, timer, &shutdown, |firing| {
                fired.push((firing.uid.clone(), firing.trigger));
                shutdown.store(true, Ordering::SeqCst);
                alarms.update(|_| ());
            });

            watch.join().unwrap().unwrap();
        });

        assert_eq!(vec![(String::from("standup"), trigger)], fired);
    }
}
