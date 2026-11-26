//! # Firing
//!
//! What a watch fires on the wall clock rather than on a change: the
//! alarms of a calendar ([`crate::alarm`]) and the birthdays and
//! anniversaries of an addressbook ([`crate::birthday`]), resolved into
//! firings over a rolling horizon, and the thread firing them when the
//! wall clock reaches one.
//!
//! A backend reads the bodies of its members into the [`FiringBook`] it
//! shares with that thread through [`FiringShared`]. [`FiringQueue`]
//! decides what is due, each kind of firing with its own late rule;
//! [`run`] waits for it. Nothing is stored between runs: a watch starting
//! fires nothing whose firing is already past.

use std::{
    collections::BTreeMap,
    sync::{
        Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use ceichas::{
    timer::{CeichasTimer, CeichasWait},
    waker::CeichasWaker,
};
#[cfg(birthday)]
use jiff::tz::TimeZone;
use log::{debug, info, warn};

#[cfg(alarm)]
use crate::alarm::{self, AlarmBook, AlarmFiring};
#[cfg(birthday)]
use crate::birthday::{BirthdayBook, BirthdayFiring, BirthdayRule};

/// How far ahead firings are resolved.
const HORIZON: i64 = 48 * 3_600;
/// How long the firing thread waits before trying again a timer that
/// failed, rather than spinning on it.
const RETRY: Duration = Duration::from_secs(1);

/// One firing, of whichever kind a backend reads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Firing {
    /// An alarm of a calendar.
    #[cfg(alarm)]
    Alarm(AlarmFiring),
    /// A birthday or an anniversary of an addressbook.
    #[cfg(birthday)]
    Birthday(BirthdayFiring),
}

impl Firing {
    /// What tells this firing apart from every other of its kind.
    fn key(&self) -> String {
        match self {
            #[cfg(alarm)]
            Self::Alarm(firing) => format!("alarm/{}", firing.key()),
            #[cfg(birthday)]
            Self::Birthday(firing) => format!("birthday/{}", firing.key()),
        }
    }

    /// The backend's id of the item it fires for.
    pub fn id(&self) -> &str {
        match self {
            #[cfg(alarm)]
            Self::Alarm(firing) => &firing.id,
            #[cfg(birthday)]
            Self::Birthday(firing) => &firing.id,
        }
    }

    /// When it fires, in seconds since the Unix epoch.
    pub fn trigger(&self) -> i64 {
        match self {
            #[cfg(alarm)]
            Self::Alarm(firing) => firing.trigger,
            #[cfg(birthday)]
            Self::Birthday(firing) => firing.trigger,
        }
    }

    /// Whether, reached at `now`, it still fires: an alarm within its
    /// grace window, a date on its day or its late days.
    fn fires_at(&self, now: i64) -> bool {
        match self {
            #[cfg(alarm)]
            Self::Alarm(firing) => firing.fires_at(now),
            #[cfg(birthday)]
            Self::Birthday(firing) => firing.fires_at(now),
        }
    }

    /// What it is about, for a log line: the alarm and its trigger, or the
    /// date and its day.
    fn what(&self) -> String {
        match self {
            #[cfg(alarm)]
            Self::Alarm(firing) => format!("alarm of {} at {}", firing.id, firing.trigger),
            #[cfg(birthday)]
            Self::Birthday(firing) => format!(
                "{} of {} on {}",
                firing.kind.as_str(),
                firing.id,
                firing.day
            ),
        }
    }

    /// The alarm it is, `None` for another kind.
    #[cfg(all(test, alarm))]
    pub fn alarm(&self) -> Option<&AlarmFiring> {
        match self {
            Self::Alarm(firing) => Some(firing),
            #[allow(unreachable_patterns)]
            _ => None,
        }
    }

    /// The birthday it is, `None` for another kind.
    #[cfg(all(test, birthday))]
    pub fn birthday(&self) -> Option<&BirthdayFiring> {
        match self {
            Self::Birthday(firing) => Some(firing),
            #[allow(unreachable_patterns)]
            _ => None,
        }
    }
}

/// What an account fires on the wall clock: the alarms of its calendar,
/// the dates of its addressbook, each read by its backend.
pub struct FiringBook {
    /// The alarms of the calendar.
    #[cfg(alarm)]
    pub alarms: AlarmBook,
    /// The dates of the addressbook.
    #[cfg(birthday)]
    pub birthdays: BirthdayBook,
    /// When the dates fire, `None` firing none.
    #[cfg(birthday)]
    pub rule: Option<BirthdayRule>,
}

impl FiringBook {
    /// An empty book, read in the system's zone, firing no date.
    pub fn new() -> Self {
        Self {
            #[cfg(alarm)]
            alarms: AlarmBook::new(),
            #[cfg(birthday)]
            birthdays: BirthdayBook::new(TimeZone::system()),
            #[cfg(birthday)]
            rule: None,
        }
    }

    /// How far before now a firing may lie and still fire, whichever its
    /// kind.
    fn reach(&self) -> i64 {
        #[cfg(alarm)]
        let reach = alarm::GRACE;
        #[cfg(not(alarm))]
        let reach = 0;
        #[cfg(birthday)]
        let reach = self.rule.map_or(reach, |rule| reach.max(rule.reach()));

        reach
    }

    /// Every firing over `[from, until)` still firing at `now`, in time
    /// order, those that never fire left out.
    fn firings(&self, from: i64, now: i64, until: i64) -> Vec<Firing> {
        let mut firings: Vec<Firing> = Vec::new();

        #[cfg(alarm)]
        firings.extend(
            self.alarms
                .firings(from.max(now - alarm::GRACE), until)
                .into_iter()
                .map(Firing::Alarm),
        );

        #[cfg(birthday)]
        if let Some(rule) = &self.rule {
            firings.extend(
                self.birthdays
                    .firings(rule, from, now, until)
                    .into_iter()
                    .map(Firing::Birthday),
            );
        }

        firings.sort_by(|a, b| (a.trigger(), a.id()).cmp(&(b.trigger(), b.id())));
        firings
    }
}

/// What a pass found due.
#[derive(Debug, Default)]
pub struct FiringPass {
    /// The firings to fire now.
    pub due: Vec<Firing>,
    /// The firings reached past what their late rule allows, dropped.
    pub late: Vec<Firing>,
    /// When the next pass is wanted, in seconds since the Unix epoch.
    pub wake: i64,
}

/// The firings still to come over the horizon, and the ones already
/// fired, so that a rebuild never fires one twice.
pub struct FiringQueue {
    /// When the watch started: nothing earlier ever fires.
    started: i64,
    queue: Vec<Firing>,
    /// The firings fired, by key, with their trigger.
    fired: BTreeMap<String, i64>,
    built_at: Option<i64>,
}

impl FiringQueue {
    /// A queue for a watch starting at `now`.
    pub fn new(now: i64) -> Self {
        Self {
            started: now,
            queue: Vec::new(),
            fired: BTreeMap::new(),
            built_at: None,
        }
    }

    /// Sorts what is due at `now`, building the queue again from `book`
    /// when its firings `moved` or half the horizon has passed.
    ///
    /// What the previous queue held is sorted first, so a firing a
    /// suspend made late is dropped and logged rather than vanishing in
    /// the rebuild. The window starts at the later of the watch's start
    /// and the longest late rule before now, so a late change, or a
    /// suspend longer than the horizon, still brings a firing its rule
    /// lets fire.
    pub fn pass(&mut self, book: &FiringBook, moved: bool, now: i64) -> FiringPass {
        let mut pass = FiringPass::default();
        self.sort_due(now, &mut pass);

        let stale = self.built_at.is_none_or(|at| now - at >= HORIZON / 2);

        if moved || stale {
            let from = self.started.max(now - book.reach());
            let until = now + HORIZON;

            self.fired.retain(|_, trigger| *trigger >= from);
            self.queue = book
                .firings(from, now, until)
                .into_iter()
                .filter(|firing| !self.fired.contains_key(&firing.key()))
                .collect();
            self.built_at = Some(now);

            debug!("queued {} firings until {until}", self.queue.len());
            self.sort_due(now, &mut pass);
        }

        let rebuild = self.built_at.unwrap_or(now) + HORIZON / 2;
        pass.wake = self
            .queue
            .first()
            .map_or(rebuild, |firing| firing.trigger().min(rebuild));

        pass
    }

    /// Moves the firings due at `now` out of the queue, into what fires
    /// and what is too late to.
    fn sort_due(&mut self, now: i64, pass: &mut FiringPass) {
        let due = self.queue.partition_point(|firing| firing.trigger() <= now);

        for firing in self.queue.drain(..due) {
            self.fired.insert(firing.key(), firing.trigger());

            if firing.fires_at(now) {
                pass.due.push(firing);
            } else {
                pass.late.push(firing);
            }
        }
    }

    /// How many firings are queued.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.queue.len()
    }
}

/// The firings an account's watch and its firing thread share.
pub struct FiringShared {
    book: Mutex<FiringBook>,
    /// Ends the firing thread's wait, its firings having moved.
    waker: CeichasWaker,
}

impl FiringShared {
    /// Holds `book`, `waker` ending the wait of the firing thread's
    /// timer.
    pub fn new(waker: CeichasWaker, book: FiringBook) -> Self {
        Self {
            book: Mutex::new(book),
            waker,
        }
    }

    /// Changes the firings held, and wakes the firing thread.
    pub fn update(&self, change: impl FnOnce(&mut FiringBook)) {
        change(&mut self.book.lock().unwrap_or_else(PoisonError::into_inner));
        self.wake();
    }

    /// Ends the firing thread's wait, so it reads its firings again.
    fn wake(&self) {
        if let Err(err) = self.waker.wake() {
            warn!("cannot wake the firing thread: {err}");
        }
    }

    /// The item and the trigger of every firing over `[from, until)`.
    #[cfg(test)]
    pub fn triggers(&self, from: i64, until: i64) -> Vec<(String, i64)> {
        let book = self.book.lock().unwrap_or_else(PoisonError::into_inner);

        book.firings(from, from, until)
            .into_iter()
            .map(|firing| (firing.id().to_owned(), firing.trigger()))
            .collect()
    }
}

/// Fires the firings `shared` holds until `shutdown` is set, calling
/// `fire` for every firing due.
///
/// `timer` waits on the wall clock (ceichas), so a suspend counts and a
/// clock change is followed; its waker, the one `shared` holds, ends the
/// wait when the firings move, and the caller wakes it once `shutdown` is
/// set.
pub fn run(
    shared: &FiringShared,
    mut timer: CeichasTimer,
    shutdown: &AtomicBool,
    mut fire: impl FnMut(&Firing),
) {
    let mut queue = FiringQueue::new(now());
    let mut moved = true;

    while !shutdown.load(Ordering::SeqCst) {
        let at = now();
        let pass = {
            let book = shared.book.lock().unwrap_or_else(PoisonError::into_inner);
            queue.pass(&book, moved, at)
        };

        for firing in &pass.late {
            info!(
                "drop {}, reached {}s late",
                firing.what(),
                at - firing.trigger()
            );
        }

        for firing in &pass.due {
            info!("fire {}", firing.what());
            fire(firing);
        }

        let deadline = UNIX_EPOCH + Duration::from_secs(pass.wake.max(0) as u64);
        moved = match timer.wait_until(deadline) {
            Ok(CeichasWait::Reached) => false,
            Ok(CeichasWait::Woken) => true,
            Err(err) => {
                warn!("cannot wait for the next firing, retrying: {err}");
                thread::sleep(RETRY);
                true
            }
        };
    }
}

/// Now, in seconds since the Unix epoch.
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |now| now.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use ceichas::timer::{CeichasTimer, CeichasWait};
    #[cfg(birthday)]
    use jiff::{
        Timestamp,
        civil::{Date, Time},
        tz::TimeZone,
    };

    use crate::firing::*;

    /// An instant written with its offset, in seconds.
    #[cfg(birthday)]
    fn at(instant: &str) -> i64 {
        instant.parse::<Timestamp>().unwrap().as_second()
    }

    /// What an account fires whose addressbook holds one card born on 27
    /// November, read in Paris, its dates firing at 09:00 with
    /// `late_days`.
    #[cfg(birthday)]
    fn birthday(late_days: u16) -> FiringBook {
        let mut book = FiringBook::new();
        let card = "BEGIN:VCARD\r\nVERSION:4.0\r\nUID:jane\r\nFN:Jane Doe\r\nBDAY:1990-11-27\r\nEND:VCARD\r\n";

        book.birthdays = BirthdayBook::new(TimeZone::get("Europe/Paris").unwrap());
        book.birthdays.read("a", "a", card.as_bytes());
        book.rule = Some(BirthdayRule {
            at: Time::constant(9, 0, 0, 0),
            days_before: 0,
            late_days,
        });

        book
    }

    /// The days late of each date firing, on the local day `today`.
    #[cfg(birthday)]
    fn late(firings: &[Firing], today: &str) -> Vec<i32> {
        let today: Date = today.parse().unwrap();

        firings
            .iter()
            .filter_map(|firing| Some(firing.birthday()?.late(today)))
            .collect()
    }

    #[cfg(birthday)]
    #[test]
    fn a_date_asleep_over_its_time_fires_on_waking_the_same_day() {
        let book = birthday(0);
        let mut queue = FiringQueue::new(at("2026-11-27T08:00:00+01:00"));
        queue.pass(&book, true, at("2026-11-27T08:00:00+01:00"));

        let pass = queue.pass(&book, false, at("2026-11-27T11:00:00+01:00"));

        assert_eq!(vec![0], late(&pass.due, "2026-11-27"));
        assert!(pass.late.is_empty());
        assert_eq!(Some(36), pass.due[0].birthday().unwrap().age);
    }

    #[cfg(birthday)]
    #[test]
    fn a_date_asleep_over_its_day_fires_within_its_late_days_only() {
        for (late_days, fired) in [(2, vec![1]), (0, vec![])] {
            let book = birthday(late_days);
            let mut queue = FiringQueue::new(at("2026-11-26T21:00:00+01:00"));
            queue.pass(&book, true, at("2026-11-26T21:00:00+01:00"));

            let pass = queue.pass(&book, false, at("2026-11-28T08:00:00+01:00"));

            assert_eq!(fired, late(&pass.due, "2026-11-28"), "{late_days}");
            assert_eq!(fired.is_empty(), pass.late.len() == 1, "{late_days}");
        }
    }

    /// A suspend longer than the horizon leaves the queue built before it
    /// short of the date: the rebuild on waking still reaches it.
    #[cfg(birthday)]
    #[test]
    fn a_date_past_the_queue_built_before_a_long_suspend_fires_on_waking() {
        let book = birthday(5);
        let mut queue = FiringQueue::new(at("2026-11-23T08:00:00+01:00"));
        queue.pass(&book, true, at("2026-11-23T08:00:00+01:00"));
        assert_eq!(0, queue.len());

        let pass = queue.pass(&book, false, at("2026-11-29T08:00:00+01:00"));

        assert_eq!(vec![2], late(&pass.due, "2026-11-29"));
    }

    #[cfg(birthday)]
    #[test]
    fn a_restart_fires_no_date_before_its_start_and_none_twice() {
        let book = birthday(2);
        let mut queue = FiringQueue::new(at("2026-11-27T10:00:00+01:00"));
        let pass = queue.pass(&book, true, at("2026-11-27T10:00:00+01:00"));

        assert!(pass.due.is_empty(), "{:?}", pass.due);
        assert!(pass.late.is_empty(), "{:?}", pass.late);

        let mut queue = FiringQueue::new(at("2026-11-27T08:00:00+01:00"));
        queue.pass(&book, true, at("2026-11-27T08:00:00+01:00"));
        let fired = queue.pass(&book, false, at("2026-11-27T09:00:00+01:00"));
        assert_eq!(1, fired.due.len());

        let again = queue.pass(&book, true, at("2026-11-27T09:30:00+01:00"));
        assert!(again.due.is_empty(), "{:?}", again.due);
    }

    #[test]
    fn a_change_ends_the_firing_thread_s_wait() {
        let mut timer = CeichasTimer::new(Default::default()).expect("a timer");
        let shared = FiringShared::new(timer.waker(), FiringBook::new());
        let later = SystemTime::now() + Duration::from_secs(3_600);

        shared.update(|_| ());
        assert_eq!(CeichasWait::Woken, timer.wait_until(later).expect("a wait"));
    }
}
