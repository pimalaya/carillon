//! # Pimdir
//!
//! The pimdir backend: a calendar, an addressbook or both of a local
//! pimdir store, read as a reader (pimdir STORAGE §8, §14.1) and never
//! written. Its committed rows are read, the pending queue left out, and
//! the store's change feed (§4.5) is followed to read again only what
//! moved. The store's owner keeps it in sync; this backend only fires the
//! alarms of the calendar and the birthdays and anniversaries of the
//! addressbook.

use std::{
    collections::{HashMap, HashSet},
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
    config::pimdir::PimdirConfig,
    error::{self, ErrorCode},
    event::WatchDomain,
    firing::{FiringBook, FiringShared},
    poll,
};

/// How long the watch waits between two reads of the change cursor,
/// unless the config names another interval.
const POLL_INTERVAL: Duration = Duration::from_secs(30);

/// How many rows one page of a listing or of the change feed holds.
const PAGE: usize = 500;

/// The program a refused store's message names.
const PROGRAM: &str = "carillon";

/// What a member of a collection became, read before the firings are
/// changed so the lock is held only to apply it.
enum Moved {
    Read {
        key: String,
        etag: Option<String>,
        body: Vec<u8>,
    },
    Gone(String),
}

/// The object hash of each item the firings were read from, by its public
/// id: a collection listed again reads only the bodies it lacks.
type Known = HashMap<String, String>;

/// What a watched collection holds, which is what its items fire.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PimdirKind {
    /// A calendar, whose items fire their alarms.
    Calendar,
    /// An addressbook, whose cards fire their birthdays and anniversaries.
    Addressbook,
}

impl PimdirKind {
    /// The media type of a collection of this kind, its parameters aside.
    const fn media_type(self) -> &'static str {
        match self {
            Self::Calendar => "text/calendar",
            Self::Addressbook => "text/vcard",
        }
    }

    /// The word the configuration names a collection of this kind with.
    const fn noun(self) -> &'static str {
        match self {
            Self::Calendar => PimdirConfig::CALENDAR,
            Self::Addressbook => PimdirConfig::ADDRESSBOOK,
        }
    }

    /// What a collection of this kind holds, for a refusal.
    const fn items(self) -> &'static str {
        match self {
            Self::Calendar => "calendars",
            Self::Addressbook => "contacts",
        }
    }

    /// The domain a missing collection of this kind is coded by.
    const fn domain(self) -> WatchDomain {
        match self {
            Self::Calendar => WatchDomain::Event,
            Self::Addressbook => WatchDomain::Card,
        }
    }

    /// Folds what an item of a collection of this kind became into
    /// `book`.
    fn fold(self, book: &mut FiringBook, moved: Moved) {
        match (self, moved) {
            (Self::Calendar, Moved::Read { key, etag, body }) => {
                book.alarms.read(&key, &key, etag.as_deref(), &body)
            }
            (Self::Calendar, Moved::Gone(key)) => book.alarms.forget(&key),
            (Self::Addressbook, Moved::Read { key, body, .. }) => {
                book.birthdays.read(&key, &key, &body)
            }
            (Self::Addressbook, Moved::Gone(key)) => book.birthdays.forget(&key),
        }
    }
}

/// One collection the watch reads, and what it knows of it.
struct Watched {
    /// The collection id.
    id: String,
    kind: PimdirKind,
    known: Known,
}

impl Watched {
    /// The collections `config` names, the calendar first.
    fn of(config: &PimdirConfig) -> Vec<Self> {
        let calendar = config.calendar.iter().map(|id| (id, PimdirKind::Calendar));
        let addressbook = config
            .addressbook
            .iter()
            .map(|id| (id, PimdirKind::Addressbook));

        calendar
            .chain(addressbook)
            .map(|(id, kind)| Self {
                id: id.clone(),
                kind,
                known: Known::new(),
            })
            .collect()
    }

    /// Refuses a collection the store does not hold, one grouped under
    /// another account than `account` when given, and one holding
    /// another kind of item.
    fn check(&self, reader: &PimdirReader, account: Option<&str>) -> Result<()> {
        let (id, noun) = (&self.id, self.kind.noun());

        let Some(kind) = reader.collection_kind(id)? else {
            let message = format!("Cannot find pimdir {noun} `{id}`");
            return Err(error::not_found(self.kind.domain(), message));
        };

        if let Some(account) = account
            && reader.collection_account(id)?.flatten().as_deref() != Some(account)
        {
            let message = format!("Cannot find pimdir {noun} `{id}` under account `{account}`");
            return Err(error::not_found(self.kind.domain(), message));
        }

        let media_type = self.kind.media_type();
        let holds = kind
            .split(';')
            .next()
            .is_some_and(|kind| kind.trim().eq_ignore_ascii_case(media_type));

        if !holds {
            bail!(
                "Pimdir collection `{id}` holds {kind}, not {}",
                self.kind.items()
            );
        }

        Ok(())
    }

    /// Applies what moved to `shared`, and records it in what is known.
    fn apply(&mut self, shared: &FiringShared, moved: Vec<Moved>) {
        if moved.is_empty() {
            return;
        }

        for moved in &moved {
            match moved {
                Moved::Read { key, etag, .. } => match etag {
                    Some(etag) => self.known.insert(key.clone(), etag.clone()),
                    None => self.known.remove(key),
                },
                Moved::Gone(key) => self.known.remove(key),
            };
        }

        let kind = self.kind;

        shared.update(|book| {
            for moved in moved {
                kind.fold(book, moved);
            }
        });
    }
}

/// Watches the configured collections until `shutdown` is set, keeping
/// `firings` current.
///
/// `on_open` is called once the session is up and its collections read,
/// right before the first wait.
///
/// The store is opened again at each poll: a reader keeps the file it
/// opened, so a store moved aside and synced anew under the root is seen
/// only by a new reader, its other `store_id` reading as a reset.
pub fn watch(
    config: &PimdirConfig,
    shutdown: &Arc<AtomicBool>,
    firings: &FiringShared,
    on_open: impl FnOnce(),
) -> Result<()> {
    let interval = config.interval().unwrap_or(POLL_INTERVAL);
    let account = config.account.as_deref();
    let mut watched = Watched::of(config);
    let mut cursor = load(&open(config)?, &mut watched, account, firings)?;
    on_open();

    while !shutdown.load(Ordering::SeqCst) {
        if !poll::sleep(interval, shutdown) {
            break;
        }

        let reader = open(config)?;
        cursor = follow(&reader, &mut watched, account, cursor, firings)?;
    }

    Ok(())
}

/// Checks that the store opens and holds the collections, without
/// reading any item.
pub fn probe(config: &PimdirConfig) -> Result<()> {
    let reader = open(config)?;
    let account = config.account.as_deref();

    for watched in Watched::of(config) {
        watched.check(&reader, account)?;
    }

    Ok(())
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

/// Reads every item of the collections into new firings, and answers the
/// cursor they were read at: read before the items, so a change landing
/// meanwhile is folded again rather than skipped.
fn load(
    reader: &PimdirReader,
    watched: &mut [Watched],
    account: Option<&str>,
    shared: &FiringShared,
) -> Result<PimdirChangeCursor> {
    let cursor = reader.change_cursor()?;

    for watched in watched.iter() {
        watched.check(reader, account)?;
    }

    let blobs = reader.blobs();
    let mut fresh = FiringBook::new();

    for watched in watched.iter_mut() {
        let (kind, known) = (watched.kind, &mut watched.known);
        known.clear();

        each_item(reader, &watched.id, |item| {
            let moved = body(&blobs, item);

            if let Moved::Read {
                key,
                etag: Some(etag),
                ..
            } = &moved
            {
                known.insert(key.clone(), etag.clone());
            }

            kind.fold(&mut fresh, moved);
        })?;
    }

    let calendar = watched.iter().any(|w| w.kind == PimdirKind::Calendar);
    let addressbook = watched.iter().any(|w| w.kind == PimdirKind::Addressbook);

    debug!(
        "read {} pimdir items holding an alarm, {} stating a date",
        fresh.alarms.len(),
        fresh.birthdays.len()
    );

    shared.update(|book| {
        if calendar {
            book.alarms = fresh.alarms;
        }

        if addressbook {
            book.birthdays = fresh.birthdays;
        }
    });

    Ok(cursor)
}

/// Lists the collection again, reads the items new or of another body
/// than it knows, and forgets the ones gone.
fn reread(reader: &PimdirReader, watched: &mut Watched, shared: &FiringShared) -> Result<()> {
    let blobs = reader.blobs();
    let mut listed = HashSet::new();
    let mut moved = Vec::new();

    each_item(reader, &watched.id, |item| {
        let key = item.seq.to_string();

        if !is_known(&watched.known, &key, item) {
            moved.push(body(&blobs, item));
        }

        listed.insert(key);
    })?;

    for key in watched.known.keys().filter(|key| !listed.contains(*key)) {
        moved.push(Moved::Gone(key.clone()));
    }

    debug!(
        "{} items of pimdir collection `{}` moved since the last read",
        moved.len(),
        watched.id
    );
    watched.apply(shared, moved);

    Ok(())
}

/// Calls `read` on every item of the collection, page by page.
fn each_item(
    reader: &PimdirReader,
    collection: &str,
    mut read: impl FnMut(&PimdirItem),
) -> Result<()> {
    let mut after: Option<String> = None;

    loop {
        let page = reader
            .list_items(collection, after.as_deref(), PAGE)
            .with_context(|| format!("Cannot list pimdir collection `{collection}`"))?;

        let Some(last) = page.last() else {
            break;
        };

        after = Some(last.link_id.0.clone());
        page.iter().for_each(&mut read);

        if page.len() < PAGE {
            break;
        }
    }

    Ok(())
}

/// Whether `item` holds the body the firings were read from.
fn is_known(known: &Known, key: &str, item: &PimdirItem) -> bool {
    item.object
        .as_ref()
        .is_some_and(|hash| known.get(key) == Some(&hash.0))
}

/// Folds what moved in the collections since `cursor`, and answers the
/// cursor to look from next.
///
/// A reset of the store, a purge or a watched collection itself moving
/// lists every collection again, reading only the bodies not known;
/// otherwise each item stamped above the cursor is read again unless its
/// body is known, or forgotten when deleted or retained.
fn follow(
    reader: &PimdirReader,
    watched: &mut [Watched],
    account: Option<&str>,
    cursor: PimdirChangeCursor,
    shared: &FiringShared,
) -> Result<PimdirChangeCursor> {
    let now = reader.change_cursor()?;

    if now == cursor {
        return Ok(cursor);
    }

    let reset = now.store_id != cursor.store_id
        || now.incarnation != cursor.incarnation
        || now.purges != cursor.purges
        || now.changed < cursor.changed;

    let moved_collection = || -> Result<bool> {
        let changed = reader.collections_changed_since(cursor.changed, PAGE)?;
        let watched = |id: &str| watched.iter().any(|w| w.id == id);
        Ok(changed.iter().any(|collection| watched(&collection.id)))
    };

    if reset || moved_collection()? {
        debug!("pimdir store reset or a collection moved, listing them again");
        let cursor = reader.change_cursor()?;

        for watched in watched.iter_mut() {
            watched.check(reader, account)?;
            reread(reader, watched, shared)?;
        }

        return Ok(cursor);
    }

    let blobs = reader.blobs();
    let mut moved: Vec<Vec<Moved>> = watched.iter().map(|_| Vec::new()).collect();
    let mut since = cursor.changed;

    loop {
        let page = reader.items_changed_since(since, PAGE)?;

        let Some(last) = page.last() else {
            break;
        };

        since = last.changed;
        let full = page.len() == PAGE;

        for change in &page {
            let Some(at) = watched.iter().position(|w| w.id == change.collection) else {
                continue;
            };

            trace!("pimdir item {} moved", change.seq);
            let (collection, key) = (&watched[at], change.seq.to_string());

            if change.deleted || change.retained_at.is_some() {
                moved[at].push(Moved::Gone(key));
                continue;
            }

            match reader.get_item(&collection.id, change.seq)? {
                Some(item) if is_known(&collection.known, &key, &item) => (),
                Some(item) => moved[at].push(body(&blobs, &item)),
                None => moved[at].push(Moved::Gone(key)),
            }
        }

        if !full {
            break;
        }
    }

    for (watched, moved) in watched.iter_mut().zip(moved) {
        watched.apply(shared, moved);
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
    use jiff::{
        Timestamp,
        civil::{Date, Time},
        tz::TimeZone,
    };
    use tempfile::TempDir;

    use crate::{
        birthday::BirthdayRule,
        config::pimdir::PimdirConfig,
        firing::{self, FiringBook, FiringShared},
        pimdir::*,
        poll,
    };

    /// The calendar collection of every case.
    const CAL: &str = "cal";
    /// The addressbook collection of every case.
    const BOOK: &str = "contacts";

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
        create(dir.path());
        dir
    }

    /// Creates a store at `root` holding a calendar and a mail collection.
    fn create(root: &Path) {
        std::fs::create_dir_all(root).unwrap();
        let store = PimdirStore::open(root).unwrap();
        store.ensure_collection(CAL, "text/calendar").unwrap();
        store.ensure_collection(BOOK, "text/vcard").unwrap();
        store.ensure_collection("mail", "message/rfc822").unwrap();
    }

    fn config(dir: &Path, calendar: &str) -> PimdirConfig {
        PimdirConfig {
            calendar: Some(calendar.to_owned()),
            addressbook: None,
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
        let mut owner = PimdirStore::open(dir).unwrap().for_source("test").unwrap();
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

    /// A card of `uid` born on `bday`.
    fn card(uid: &str, bday: &str) -> String {
        format!(
            "BEGIN:VCARD\r\nVERSION:4.0\r\nUID:{uid}\r\nFN:Jane Doe\r\nBDAY:{bday}\r\nEND:VCARD\r\n"
        )
    }

    /// When `date` reaches 09:00 in the system's zone, in seconds.
    fn nine(date: &str) -> i64 {
        let date: Date = date.parse().unwrap();
        let nine = date.to_datetime(Time::constant(9, 0, 0, 0));

        TimeZone::system()
            .to_ambiguous_timestamp(nine)
            .compatible()
            .unwrap()
            .as_second()
    }

    /// A watch of the calendar and the addressbook of the store at `dir`.
    fn both(dir: &Path) -> PimdirConfig {
        PimdirConfig {
            addressbook: Some(BOOK.to_owned()),
            ..config(dir, CAL)
        }
    }

    /// The firings an account shares, its dates firing at 09:00.
    fn dated(timer: &CeichasTimer) -> FiringShared {
        let mut book = FiringBook::new();
        book.rule = Some(BirthdayRule {
            at: Time::constant(9, 0, 0, 0),
            days_before: 0,
            late_days: 0,
        });

        FiringShared::new(timer.waker(), book)
    }

    #[test]
    fn an_addressbook_is_read_then_followed_beside_a_calendar() {
        let dir = scratch();
        add(dir.path(), CAL, &event("standup", "090000"));
        add(dir.path(), BOOK, &card("jane", "1990-03-10"));
        add(dir.path(), "mail", &card("quoted", "1990-03-10"));

        let reader = PimdirReader::open(dir.path()).unwrap();
        let timer = CeichasTimer::new(Default::default()).unwrap();
        let firings = dated(&timer);
        let mut watched = Watched::of(&both(dir.path()));
        let cursor = load(&reader, &mut watched, None, &firings).unwrap();

        let jane = PimdirReader::open(dir.path())
            .unwrap()
            .seq_for_link(BOOK, "jane")
            .unwrap()
            .unwrap();
        let march = || firings.triggers(nine("2027-03-01"), nine("2027-04-01"));
        let standup = seq(dir.path(), "standup").to_string();

        assert_eq!(vec![(jane.to_string(), nine("2027-03-10"))], march());
        assert_eq!(
            vec![(standup, at("08:45:00"))],
            firings.triggers(at("00:00:00"), at("23:00:00"))
        );

        apply(dir.path(), BOOK, &card("jane", "1990-03-11"), |hash| {
            PimdirAction::Update {
                seq: jane,
                object: hash,
                if_match: None,
                resolves: None,
            }
        });
        let cursor = follow(&reader, &mut watched, None, cursor, &firings).unwrap();

        assert_eq!(vec![(jane.to_string(), nine("2027-03-11"))], march());

        let mut producer = PimdirProducer::open(dir.path(), "test").unwrap();
        let remove = PimdirAction::Remove {
            seq: jane,
            if_match: None,
            resolves: None,
        };
        producer.enqueue(BOOK, &remove, None).unwrap();
        drop(producer);
        drain(dir.path());
        follow(&reader, &mut watched, None, cursor, &firings).unwrap();

        assert!(march().is_empty(), "{:?}", march());
    }

    /// An addressbook the store does not hold is `addressbook-not-found`,
    /// and a collection holding no cards is refused.
    #[test]
    fn an_addressbook_not_found_or_holding_no_cards_is_refused() {
        use crate::error::{ErrorCode, code};

        let dir = scratch();
        let book = |id: &str| PimdirConfig {
            calendar: None,
            addressbook: Some(id.to_owned()),
            ..config(dir.path(), CAL)
        };

        probe(&book(BOOK)).expect("an addressbook");
        probe(&both(dir.path())).expect("a calendar and an addressbook");

        let err = probe(&book("nowhere")).expect_err("no such addressbook");
        assert_eq!(Some(ErrorCode::AddressbookNotFound), code(&err), "{err:#}");

        let err = probe(&book(CAL)).expect_err("a calendar");
        assert!(format!("{err:#}").contains("not contacts"), "{err:#}");
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
        let alarms = FiringShared::new(timer.waker(), FiringBook::new());
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
        let alarms = FiringShared::new(timer.waker(), FiringBook::new());
        let mut watched = Watched::of(&config(dir.path(), CAL));
        let cursor = load(&reader, &mut watched, None, &alarms).unwrap();
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
        let cursor = follow(&reader, &mut watched, None, cursor, &alarms).unwrap();

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
        follow(&reader, &mut watched, None, cursor, &alarms).unwrap();

        assert!(alarms.triggers(at("00:00:00"), at("23:00:00")).is_empty());
    }

    /// A purge in another collection reads the calendar again, but only
    /// what moved in it: a body whose hash the watch knows is not read, so
    /// its file gone, its alarm stays.
    #[test]
    fn a_purge_elsewhere_reads_no_body_of_an_item_unchanged() {
        let dir = scratch();
        add(dir.path(), CAL, &event("standup", "090000"));
        add(dir.path(), "mail", &event("quoted", "090000"));

        let timer = CeichasTimer::new(Default::default()).unwrap();
        let alarms = FiringShared::new(timer.waker(), FiringBook::new());
        let mut watched = Watched::of(&config(dir.path(), CAL));
        let reader = PimdirReader::open(dir.path()).unwrap();
        let cursor = load(&reader, &mut watched, None, &alarms).unwrap();
        let id = seq(dir.path(), "standup").to_string();
        let alarmed = vec![(id, at("08:45:00"))];
        assert_eq!(alarmed, alarms.triggers(at("00:00:00"), at("23:00:00")));

        let item = reader.get_item(CAL, seq(dir.path(), "standup")).unwrap();
        let hash = item.unwrap().object.unwrap();
        std::fs::remove_file(reader.blobs().path(&hash).unwrap()).unwrap();

        let mail = reader.list_items("mail", None, 1).unwrap()[0].seq;
        let mut producer = PimdirProducer::open(dir.path(), "test").unwrap();
        let remove = PimdirAction::Remove {
            seq: mail,
            if_match: None,
            resolves: None,
        };
        producer.enqueue("mail", &remove, None).unwrap();
        drop(producer);
        drain(dir.path());
        assert!(
            PimdirStore::open(dir.path())
                .unwrap()
                .purge("mail", mail)
                .unwrap()
        );

        let reader = PimdirReader::open(dir.path()).unwrap();
        assert_ne!(cursor.purges, reader.change_cursor().unwrap().purges);
        follow(&reader, &mut watched, None, cursor, &alarms).unwrap();

        assert_eq!(alarmed, alarms.triggers(at("00:00:00"), at("23:00:00")));
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

    /// A store moved aside and synced anew under the same root, while the
    /// watch runs, is read on the next poll.
    #[cfg(unix)]
    #[test]
    fn a_store_synced_anew_under_the_watch_is_read() {
        use crate::config::{PollWatchConfig, pimdir::PimdirWatchConfig};

        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("store");
        let now = Timestamp::now().as_second();
        let (old, new) = (now + 3600, now + 7200);

        create(&root);
        add(&root, CAL, &event_alarming_at("standup", old));

        let mut config = config(&root, CAL);
        config.watch = Some(PimdirWatchConfig::Poll(PollWatchConfig {
            interval: Some(Duration::from_millis(100)),
        }));

        let timer = CeichasTimer::new(Default::default()).unwrap();
        let alarms = FiringShared::new(timer.waker(), FiringBook::new());
        let shutdown = Arc::new(AtomicBool::new(false));
        let opened = AtomicBool::new(false);
        let has = |trigger: i64| {
            alarms
                .triggers(now, now + 10_000)
                .iter()
                .any(|(_, at)| *at == trigger)
        };

        let seen = thread::scope(|scope| {
            let watch = scope.spawn(|| {
                watch(&config, &shutdown, &alarms, || {
                    opened.store(true, Ordering::SeqCst)
                })
            });

            while !opened.load(Ordering::SeqCst) {
                assert!(!watch.is_finished(), "the watch ended before it opened");
                thread::sleep(Duration::from_millis(10));
            }
            assert!(has(old));

            let anew = dir.path().join("anew");
            create(&anew);
            add(&anew, CAL, &event_alarming_at("standup", new));
            std::fs::rename(&root, dir.path().join("aside")).unwrap();
            std::fs::rename(&anew, &root).unwrap();

            let mut seen = false;

            for _ in 0..20 {
                thread::sleep(Duration::from_millis(100));

                if has(new) {
                    seen = true;
                    break;
                }
            }

            poll::stop(&shutdown);
            watch.join().unwrap().unwrap();
            seen
        });

        assert!(seen, "{:?}", alarms.triggers(now, now + 10_000));
        assert!(!has(old));
    }

    #[test]
    fn a_store_of_the_current_draft_is_watched_and_its_alarm_fires() {
        let dir = scratch();
        let trigger = Timestamp::now().as_second() + 2;
        add(dir.path(), CAL, &event_alarming_at("standup", trigger));

        let config = config(dir.path(), CAL);
        let timer = CeichasTimer::new(Default::default()).unwrap();
        let alarms = FiringShared::new(timer.waker(), FiringBook::new());
        let shutdown = Arc::new(AtomicBool::new(false));
        let mut fired = Vec::new();

        thread::scope(|scope| {
            let watch = scope.spawn(|| watch(&config, &shutdown, &alarms, || ()));

            // Ends the test should the alarm never fire.
            scope.spawn(|| {
                if poll::sleep(Duration::from_secs(30), &shutdown) {
                    poll::stop(&shutdown);
                    alarms.update(|_| ());
                }
            });

            firing::run(&alarms, timer, &shutdown, |firing| {
                let firing = firing.alarm().unwrap();
                fired.push((firing.uid.clone(), firing.trigger));
                poll::stop(&shutdown);
                alarms.update(|_| ());
            });

            watch.join().unwrap().unwrap();
        });

        assert_eq!(vec![(String::from("standup"), trigger)], fired);
    }

    /// A store synced anew right after a firing numbers the event again:
    /// read under its new seq, the firing does not fire twice.
    #[cfg(unix)]
    #[test]
    fn a_firing_never_fires_again_once_its_store_is_synced_anew() {
        use crate::config::{PollWatchConfig, pimdir::PimdirWatchConfig};

        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("store");
        let trigger = Timestamp::now().as_second() + 2;
        let body = event_alarming_at("standup", trigger);

        create(&root);
        add(&root, CAL, &body);

        let mut config = config(&root, CAL);
        config.watch = Some(PimdirWatchConfig::Poll(PollWatchConfig {
            interval: Some(Duration::from_millis(100)),
        }));

        let timer = CeichasTimer::new(Default::default()).unwrap();
        let alarms = FiringShared::new(timer.waker(), FiringBook::new());
        let shutdown = Arc::new(AtomicBool::new(false));
        let mut fired = Vec::new();

        thread::scope(|scope| {
            let watch = scope.spawn(|| watch(&config, &shutdown, &alarms, || ()));

            scope.spawn(|| {
                if poll::sleep(Duration::from_secs(5), &shutdown) {
                    poll::stop(&shutdown);
                    alarms.update(|_| ());
                }
            });

            firing::run(&alarms, timer, &shutdown, |firing| {
                let firing = firing.alarm().unwrap();
                fired.push((firing.uid.clone(), firing.trigger));

                if fired.len() == 1 {
                    let anew = dir.path().join("anew");
                    create(&anew);
                    add(&anew, CAL, &event_alarming_at("other", trigger + 3600));
                    add(&anew, CAL, &body);
                    std::fs::rename(&root, dir.path().join("aside")).unwrap();
                    std::fs::rename(&anew, &root).unwrap();
                }
            });

            watch.join().unwrap().unwrap();
        });

        assert_eq!(seq(&root, "standup"), 2);
        assert_eq!(vec![(String::from("standup"), trigger)], fired);
    }
}
