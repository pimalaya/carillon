//! # Alarm
//!
//! The alarms of a watched calendar: the `VALARM`s of its events and
//! tasks (RFC 5545 3.6.6), resolved by ical-rs into firings over a
//! rolling horizon, and the thread firing `on-alarm-trigger` when the
//! wall clock reaches one.
//!
//! A calendar backend reads the bodies of its members into an
//! [`AlarmBook`], keyed by its own item id, and shares it with the alarm
//! thread through [`AlarmShared`]. [`AlarmQueue`] decides what is due;
//! [`run`] waits for it. Nothing is stored between runs: a watch starting
//! fires nothing whose trigger is already past.

use std::{
    collections::BTreeMap,
    str,
    sync::{
        Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use ical::{
    alarm::IcalAlarms,
    component::{IcalComponent, IcalComponentKind, IcalComponentName},
    prop::{IcalProp, IcalPropKind, IcalPropName},
    recur::{IcalRecurDateTime, instant::IcalRecurTimeline, set::IcalRecurOverride},
    tree::cst::IcalCst,
    tz::IcalTz,
    value::IcalValue,
};
use jiff::{Timestamp, tz::TimeZone};
use log::{debug, info, trace, warn};

/// How far ahead firings are resolved.
const HORIZON: i64 = 48 * 3_600;
/// How late a firing may be reached and still fire.
const GRACE: i64 = 15 * 60;
/// How long a wait sleeps at a time, so a shutdown, a moved firing and a
/// changed wall clock are all noticed promptly.
const STEP: Duration = Duration::from_millis(200);
/// A day, in seconds.
const DAY: i64 = 86_400;
/// How many years past the current one the bundled zones are exact to.
const ZONE_YEARS: i16 = 10;

/// One firing of one alarm, with what a hook says of it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlarmFiring {
    /// The backend's id of the item holding the alarm.
    pub id: String,
    /// The entity tag of the item, as it was read.
    pub etag: Option<String>,
    /// The `UID` of the event or task.
    pub uid: String,
    /// Whether the alarm belongs to an event or a task.
    pub kind: AlarmKind,
    /// The `SUMMARY` of the component holding the alarm.
    pub summary: Option<String>,
    /// The index of the alarm among the `VALARM`s of its component.
    pub alarm: usize,
    /// The alarm's own `UID` (RFC 9074 4), when it has one.
    pub alarm_uid: Option<String>,
    /// The identity of the override holding the alarm, `None` for the
    /// series' own.
    pub over: Option<String>,
    /// The identity of the occurrence the firing belongs to, `None` for
    /// an item that does not recur and for an absolute trigger.
    pub occurrence: Option<String>,
    /// The rank of the firing in its alarm's repetition, zero for the
    /// trigger itself.
    pub repeat: u32,
    /// What the alarm asks for.
    pub action: AlarmAction,
    /// When it fires, in seconds since the Unix epoch.
    pub trigger: i64,
    /// When the occurrence starts, for an event, or is due, for a task.
    pub at: Option<i64>,
}

impl AlarmFiring {
    /// What tells this firing apart from every other: an edit moving the
    /// alarm makes a new one, the trigger being part of it.
    fn key(&self) -> String {
        let over = self.over.as_deref().unwrap_or_default();
        let occurrence = self.occurrence.as_deref().unwrap_or_default();

        format!(
            "{}/{}/{over}/{}/{occurrence}/{}@{}",
            self.id, self.uid, self.alarm, self.repeat, self.trigger
        )
    }
}

/// The component an alarm belongs to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AlarmKind {
    /// A `VEVENT`.
    Event,
    /// A `VTODO`.
    Task,
}

impl AlarmKind {
    /// The word a hook reads it as.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Event => "event",
            Self::Task => "task",
        }
    }
}

/// The `ACTION` of an alarm that fires (RFC 5545 3.8.6.1).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AlarmAction {
    /// A notification.
    Display,
    /// A notification with a sound.
    Audio,
}

impl AlarmAction {
    /// The action an `ACTION` value names, `None` for one that never
    /// fires here: `EMAIL` is the server's job.
    fn parse(action: &str) -> Option<Self> {
        if action.eq_ignore_ascii_case("DISPLAY") {
            Some(Self::Display)
        } else if action.eq_ignore_ascii_case("AUDIO") {
            Some(Self::Audio)
        } else {
            None
        }
    }

    /// The word a hook reads it as.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Display => "display",
            Self::Audio => "audio",
        }
    }
}

/// The items of one calendar holding an alarm, by the backend's key.
pub struct AlarmBook {
    items: BTreeMap<String, AlarmItem>,
    /// The zone a floating time resolves in, `None` reading it as UTC.
    floating: Option<IcalTz>,
    /// The year the bundled zones are exact to.
    through: i32,
}

impl AlarmBook {
    /// An empty book, floating times resolving in the system's zone.
    pub fn new() -> Self {
        let through = Timestamp::now()
            .to_zoned(TimeZone::UTC)
            .year()
            .saturating_add(ZONE_YEARS);
        let through = i32::from(through);

        let floating = TimeZone::system()
            .iana_name()
            .and_then(|name| ical::tzdb::zone(name, through));

        Self {
            items: BTreeMap::new(),
            floating,
            through,
        }
    }

    /// Reads the body of the item at `key` again, keeping it only when it
    /// holds an alarm. `id` is what a hook names the item by.
    ///
    /// A body that is not UTF-8 or not iCalendar is logged and forgotten,
    /// never an error: one broken member must not stop the others.
    pub fn read(&mut self, key: &str, id: &str, etag: Option<&str>, body: &[u8]) {
        self.items.remove(key);

        let Ok(body) = str::from_utf8(body) else {
            warn!("body of calendar item {id} is not UTF-8");
            return;
        };

        if !body.to_ascii_uppercase().contains("BEGIN:VALARM") {
            return;
        }

        let cst = match IcalCst::parse(body) {
            Ok(cst) => cst,
            Err(err) => {
                warn!("cannot parse calendar item {id}: {err:?}");
                return;
            }
        };

        let ical = cst.decode();
        let components = &ical.components;
        let zones = ical::tzdb::zones(components, self.through);

        let mut uids: Vec<&str> = Vec::new();

        for component in components.iter().filter(|c| kind_of(c).is_some()) {
            if let Some(uid) = text(component, IcalPropKind::Uid)
                && !uids.contains(&uid)
            {
                uids.push(uid);
            }
        }

        let series: Vec<AlarmSeries> = uids
            .into_iter()
            .filter_map(|uid| AlarmSeries::of_uid(components, uid, &zones))
            .collect();

        if series.is_empty() {
            return;
        }

        trace!(
            "calendar item {id} holds {} series with alarms",
            series.len()
        );

        let item = AlarmItem {
            id: id.to_owned(),
            etag: etag.map(ToOwned::to_owned),
            series,
        };

        self.items.insert(key.to_owned(), item);
    }

    /// Forgets the item at `key`, as a removal does.
    pub fn forget(&mut self, key: &str) {
        self.items.remove(key);
    }

    /// How many items holding an alarm the book has.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Every firing over `[from, until)`, in time order, those that never
    /// fire left out: an alarm of a cancelled component or a completed
    /// task, one acknowledged at or after its trigger, and one neither
    /// `DISPLAY` nor `AUDIO`.
    fn firings(&self, from: i64, until: i64) -> Vec<AlarmFiring> {
        let mut firings = Vec::new();

        for item in self.items.values() {
            for series in &item.series {
                series.firings(item, from, until, self.floating.as_ref(), &mut firings);
            }
        }

        firings.sort_by(|a, b| (a.trigger, &a.id).cmp(&(b.trigger, &b.id)));
        firings
    }
}

/// One calendar item holding alarms.
struct AlarmItem {
    id: String,
    etag: Option<String>,
    /// Every `UID` its body holds alarms for.
    series: Vec<AlarmSeries>,
}

/// The alarms of one `UID`, with what a hook says of them.
struct AlarmSeries {
    uid: String,
    kind: AlarmKind,
    alarms: IcalAlarms,
    timeline: IcalRecurTimeline,
    /// The series, then every override, by identity.
    holders: Vec<AlarmHolder>,
}

/// A component holding alarms: the series, or one override.
struct AlarmHolder {
    /// The override's identity, `None` for the series.
    over: Option<IcalRecurDateTime>,
    summary: Option<String>,
    /// Whether it is cancelled, or a completed task.
    dropped: bool,
    /// Whether it is a task with a `DUE`, told by its due time.
    due: bool,
}

impl AlarmSeries {
    /// Reads the alarms of one `UID`, `None` when none of its components
    /// holds one.
    fn of_uid(components: &[IcalComponent<'_>], uid: &str, zones: &[IcalTz]) -> Option<Self> {
        let ours: Vec<&IcalComponent<'_>> = components
            .iter()
            .filter(|component| kind_of(component).is_some())
            .filter(|component| text(component, IcalPropKind::Uid) == Some(uid))
            .collect();

        let holds_alarm = ours.iter().any(|component| {
            component.components.iter().any(|child| {
                matches!(
                    child.name,
                    IcalComponentName::Kind(IcalComponentKind::VAlarm)
                )
            })
        });

        if !holds_alarm {
            return None;
        }

        let kind = ours.iter().find_map(|component| kind_of(component))?;
        let alarms = IcalAlarms::of_uid(components, uid, zones);

        for problem in alarms.problems() {
            debug!(
                "alarm {} of {uid} fires nothing as written: {:?}",
                problem.alarm, problem.kind
            );
        }

        let timeline = IcalRecurTimeline::of_uid(components, uid, zones);
        let zone = timeline.set().zone.clone();

        let holders = ours
            .iter()
            .filter_map(|component| {
                let over = match prop(component, IcalPropKind::RecurrenceId) {
                    None => None,
                    Some(_) => Some(IcalRecurOverride::of_component(component, &zone, zones)?.id),
                };

                let status = text(component, IcalPropKind::Status).unwrap_or_default();
                let cancelled = status.eq_ignore_ascii_case("CANCELLED");
                let completed = kind == AlarmKind::Task
                    && (status.eq_ignore_ascii_case("COMPLETED")
                        || prop(component, IcalPropKind::Completed).is_some());

                Some(AlarmHolder {
                    over,
                    summary: text(component, IcalPropKind::Summary).map(ToOwned::to_owned),
                    dropped: cancelled || completed,
                    due: kind == AlarmKind::Task && prop(component, IcalPropKind::Due).is_some(),
                })
            })
            .collect();

        Some(Self {
            uid: uid.to_owned(),
            kind,
            alarms,
            timeline,
            holders,
        })
    }

    /// Pushes the firings of this series over `[from, until)` that fire.
    fn firings(
        &self,
        item: &AlarmItem,
        from: i64,
        until: i64,
        floating: Option<&IcalTz>,
        firings: &mut Vec<AlarmFiring>,
    ) {
        let series = self.holders.iter().find(|holder| holder.over.is_none());

        for firing in self.alarms.firings(from, until, floating) {
            let Some(action) = firing.action.as_deref().and_then(AlarmAction::parse) else {
                trace!(
                    "skip firing of {} with action {:?}",
                    self.uid, firing.action
                );
                continue;
            };

            if firing.acknowledged.is_some_and(|at| at >= firing.instant) {
                trace!("skip firing of {} acknowledged", self.uid);
                continue;
            }

            let holder = self
                .holders
                .iter()
                .find(|holder| holder.over == firing.over)
                .or(series);

            if holder.is_some_and(|holder| holder.dropped) || series.is_some_and(|s| s.dropped) {
                trace!("skip firing of {} cancelled or completed", self.uid);
                continue;
            }

            let due = holder.is_some_and(|holder| holder.due);
            let at = self.at(firing.occurrence, firing.instant, due, floating);

            firings.push(AlarmFiring {
                id: item.id.clone(),
                etag: item.etag.clone(),
                uid: self.uid.clone(),
                kind: self.kind,
                summary: holder.and_then(|holder| holder.summary.clone()),
                alarm: firing.alarm,
                alarm_uid: firing.uid,
                over: firing.over.map(civil),
                occurrence: firing.occurrence.map(civil),
                repeat: firing.repeat,
                action,
                trigger: firing.instant,
                at,
            });
        }
    }

    /// When the occurrence a firing belongs to starts, or is due, looked
    /// up from a week before the firing to a little over a year after it.
    ///
    /// A firing of no occurrence, a single item or an absolute trigger,
    /// takes the first occurrence of that span.
    fn at(
        &self,
        occurrence: Option<IcalRecurDateTime>,
        instant: i64,
        due: bool,
        floating: Option<&IcalTz>,
    ) -> Option<i64> {
        let mut instances = self.timeline.expand(instant - 7 * DAY, instant + 400 * DAY);

        let instance = match occurrence {
            Some(id) => instances.find(|instance| instance.id == id)?,
            None => instances.next()?,
        };

        let at = match due {
            true => instance.end,
            false => instance.start,
        };

        // NOTE: the timeline reads a floating time as if on UTC.
        match (instance.floating, floating) {
            (true, Some(zone)) => {
                let local = IcalRecurDateTime::from_seconds(at);
                Some(zone.resolve(local).literal_instant(local))
            }
            _ => Some(at),
        }
    }
}

/// What a pass found due.
#[derive(Debug, Default)]
pub struct AlarmPass {
    /// The firings to fire now.
    pub due: Vec<AlarmFiring>,
    /// The firings reached beyond the grace window, dropped.
    pub late: Vec<AlarmFiring>,
    /// When the next pass is wanted, in seconds since the Unix epoch.
    pub wake: i64,
}

/// The firings still to come over the horizon, and the ones already
/// fired, so that a rebuild never fires one twice.
pub struct AlarmQueue {
    /// When the watch started: nothing earlier ever fires.
    started: i64,
    queue: Vec<AlarmFiring>,
    /// The firings fired, by key, with their trigger.
    fired: BTreeMap<String, i64>,
    built_at: Option<i64>,
}

impl AlarmQueue {
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
    /// and the grace window before now, so a late change still brings a
    /// firing within the grace window.
    pub fn pass(&mut self, book: &AlarmBook, moved: bool, now: i64) -> AlarmPass {
        let mut pass = AlarmPass::default();
        self.sort_due(now, &mut pass);

        let stale = self.built_at.is_none_or(|at| now - at >= HORIZON / 2);

        if moved || stale {
            let from = self.started.max(now - GRACE);
            let until = now + HORIZON;

            self.fired.retain(|_, trigger| *trigger >= from);
            self.queue = book
                .firings(from, until)
                .into_iter()
                .filter(|firing| !self.fired.contains_key(&firing.key()))
                .collect();
            self.built_at = Some(now);

            debug!("queued {} alarm firings until {until}", self.queue.len());
            self.sort_due(now, &mut pass);
        }

        let rebuild = self.built_at.unwrap_or(now) + HORIZON / 2;
        pass.wake = self
            .queue
            .first()
            .map_or(rebuild, |firing| firing.trigger.min(rebuild));

        pass
    }

    /// Moves the firings due at `now` out of the queue, into what fires
    /// and what is too late to.
    fn sort_due(&mut self, now: i64, pass: &mut AlarmPass) {
        let due = self.queue.partition_point(|firing| firing.trigger <= now);

        for firing in self.queue.drain(..due) {
            self.fired.insert(firing.key(), firing.trigger);

            if now - firing.trigger <= GRACE {
                pass.due.push(firing);
            } else {
                pass.late.push(firing);
            }
        }
    }
}

/// The alarms an account's watch and its alarm thread share.
pub struct AlarmShared {
    book: Mutex<AlarmBook>,
    /// Set when the firings may have moved, taken by the alarm thread.
    moved: AtomicBool,
}

impl AlarmShared {
    /// Nothing held yet.
    pub fn new() -> Self {
        Self {
            book: Mutex::new(AlarmBook::new()),
            moved: AtomicBool::new(false),
        }
    }

    /// Replaces every alarm held, as a session that read the calendar
    /// again does, and wakes the alarm thread.
    pub fn replace(&self, book: AlarmBook) {
        *self.book.lock().unwrap_or_else(PoisonError::into_inner) = book;
        self.moved.store(true, Ordering::SeqCst);
    }

    /// Changes the alarms held, and wakes the alarm thread.
    pub fn update(&self, change: impl FnOnce(&mut AlarmBook)) {
        change(&mut self.book.lock().unwrap_or_else(PoisonError::into_inner));
        self.moved.store(true, Ordering::SeqCst);
    }

    /// The item and the trigger of every firing over `[from, until)`.
    #[cfg(all(test, feature = "pimdir"))]
    pub fn triggers(&self, from: i64, until: i64) -> Vec<(String, i64)> {
        let book = self.book.lock().unwrap_or_else(PoisonError::into_inner);

        book.firings(from, until)
            .into_iter()
            .map(|firing| (firing.id, firing.trigger))
            .collect()
    }
}

/// How a wait ended.
#[derive(Debug, Eq, PartialEq)]
enum AlarmWait {
    /// The wall clock reached the time waited for.
    Reached,
    /// The firings moved, or a shutdown was requested.
    Woken,
}

/// Fires the alarms `shared` holds until `shutdown` is set, calling
/// `fire` for every firing due.
pub fn run(shared: &AlarmShared, shutdown: &AtomicBool, mut fire: impl FnMut(&AlarmFiring)) {
    let mut queue = AlarmQueue::new(now());
    let mut moved = true;

    while !shutdown.load(Ordering::SeqCst) {
        let at = now();
        let pass = {
            let book = shared.book.lock().unwrap_or_else(PoisonError::into_inner);
            queue.pass(&book, moved, at)
        };

        for firing in &pass.late {
            info!(
                "drop alarm of {} at {}, reached {}s late",
                firing.id,
                firing.trigger,
                at - firing.trigger
            );
        }

        for firing in &pass.due {
            info!("fire alarm of {} at {}", firing.id, firing.trigger);
            fire(firing);
        }

        let deadline = UNIX_EPOCH + Duration::from_secs(pass.wake.max(0) as u64);
        moved = wait_until(deadline, &shared.moved, shutdown) == AlarmWait::Woken;
    }
}

/// Waits until the wall clock reaches `deadline`, or until `woken` or
/// `shutdown` is set, `woken` being taken.
///
/// The wall clock is read at every step, a duration slept being counted
/// on a clock that stops while the machine sleeps: a suspend or a clock
/// change delays the end of the wait by one step at most.
// NOTE: shaped as ceichas's `CeichasTimer::wait_until`, which replaces it
// once released (carillon change alarm-hooks).
fn wait_until(deadline: SystemTime, woken: &AtomicBool, shutdown: &AtomicBool) -> AlarmWait {
    loop {
        if shutdown.load(Ordering::SeqCst) || woken.swap(false, Ordering::SeqCst) {
            return AlarmWait::Woken;
        }

        match deadline.duration_since(SystemTime::now()) {
            Ok(left) if !left.is_zero() => thread::sleep(left.min(STEP)),
            _ => return AlarmWait::Reached,
        }
    }
}

/// Now, in seconds since the Unix epoch.
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |now| now.as_secs() as i64)
}

/// The kind of item a component is, `None` for one holding no alarm
/// that fires.
fn kind_of(component: &IcalComponent<'_>) -> Option<AlarmKind> {
    match component.name {
        IcalComponentName::Kind(IcalComponentKind::VEvent) => Some(AlarmKind::Event),
        IcalComponentName::Kind(IcalComponentKind::VTodo) => Some(AlarmKind::Task),
        _ => None,
    }
}

/// The first property of a kind a component carries.
fn prop<'a, 'v>(component: &'a IcalComponent<'v>, kind: IcalPropKind) -> Option<&'a IcalProp<'v>> {
    component
        .props
        .iter()
        .find(|prop| matches!(prop.name, IcalPropName::Kind(found) if found == kind))
}

/// The text of the first property of a kind, empty ones counting as none.
fn text<'a>(component: &'a IcalComponent<'_>, kind: IcalPropKind) -> Option<&'a str> {
    match &prop(component, kind)?.value {
        IcalValue::Text(text) if !text.0.is_empty() => Some(&text.0),
        _ => None,
    }
}

/// The civil text of an identity, as `2026-10-30T09:00:00`.
fn civil(id: IcalRecurDateTime) -> String {
    match Timestamp::from_second(id.seconds()) {
        Ok(at) => at.to_zoned(TimeZone::UTC).datetime().to_string(),
        Err(_) => id.seconds().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::atomic::AtomicBool, time::SystemTime};

    use jiff::Timestamp;

    use crate::alarm::*;

    /// An instant written in UTC, in seconds.
    fn at(time: &str) -> i64 {
        format!("2026-11-05T{time}Z")
            .parse::<Timestamp>()
            .unwrap()
            .as_second()
    }

    /// A calendar holding one component.
    fn calendar(component: &str) -> String {
        format!(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Test//EN\r\n{component}END:VCALENDAR\r\n"
        )
    }

    /// An event of `uid` starting at `start` (UTC, `HHMMSS`), with extra
    /// properties and alarms, each alarm an `ACTION` and a `TRIGGER`.
    fn event(uid: &str, start: &str, props: &str, alarms: &[(&str, &str)]) -> String {
        let alarms: String = alarms
            .iter()
            .map(|(action, trigger)| {
                format!("BEGIN:VALARM\r\nACTION:{action}\r\nDESCRIPTION:x\r\nTRIGGER:{trigger}\r\nEND:VALARM\r\n")
            })
            .collect();

        calendar(&format!(
            "BEGIN:VEVENT\r\nUID:{uid}\r\nDTSTAMP:20261101T000000Z\r\nDTSTART:20261105T{start}Z\r\nDURATION:PT30M\r\nSUMMARY:Standup\r\n{props}{alarms}END:VEVENT\r\n"
        ))
    }

    /// A book holding `bodies`, each under its own key.
    fn book(bodies: &[(&str, &str)]) -> AlarmBook {
        let mut book = AlarmBook::new();

        for (key, body) in bodies {
            book.read(key, key, Some("etag"), body.as_bytes());
        }

        book
    }

    #[test]
    fn a_firing_fires_at_its_trigger() {
        let book = book(&[(
            "a",
            &event("standup", "090000", "", &[("DISPLAY", "-PT15M")]),
        )]);
        let mut queue = AlarmQueue::new(at("08:00:00"));

        let pass = queue.pass(&book, true, at("08:00:00"));
        assert!(pass.due.is_empty());
        assert_eq!(at("08:45:00"), pass.wake);

        let pass = queue.pass(&book, false, at("08:45:00"));
        let [firing] = pass.due.as_slice() else {
            panic!("one firing expected, got {:?}", pass.due);
        };

        assert_eq!("standup", firing.uid);
        assert_eq!(Some("Standup"), firing.summary.as_deref());
        assert_eq!(Some("etag"), firing.etag.as_deref());
        assert_eq!(AlarmKind::Event, firing.kind);
        assert_eq!(AlarmAction::Display, firing.action);
        assert_eq!(at("08:45:00"), firing.trigger);
        assert_eq!(Some(at("09:00:00")), firing.at);
    }

    #[test]
    fn a_firing_that_never_fires_is_left_out() {
        let todo = calendar(
            "BEGIN:VTODO\r\nUID:t\r\nDTSTAMP:20261101T000000Z\r\nDUE:20261105T090000Z\r\nSTATUS:COMPLETED\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:x\r\nTRIGGER:-PT1H\r\nEND:VALARM\r\nEND:VTODO\r\n",
        );

        for body in [
            event(
                "c",
                "090000",
                "STATUS:CANCELLED\r\n",
                &[("DISPLAY", "-PT15M")],
            ),
            event("e", "090000", "", &[("EMAIL", "-PT15M")]),
            event(
                "k",
                "090000",
                "",
                &[("DISPLAY", "-PT15M\r\nACKNOWLEDGED:20261105T084600Z")],
            ),
            todo,
        ] {
            let book = book(&[("x", &body)]);
            assert!(
                book.firings(at("00:00:00"), at("23:00:00")).is_empty(),
                "{body}"
            );
        }
    }

    #[test]
    fn a_restart_fires_nothing_before_its_start() {
        let book = book(&[(
            "a",
            &event("standup", "090000", "", &[("DISPLAY", "-PT15M")]),
        )]);
        let mut queue = AlarmQueue::new(at("08:50:00"));

        let pass = queue.pass(&book, true, at("08:50:00"));

        assert!(pass.due.is_empty());
        assert!(pass.late.is_empty());
    }

    #[test]
    fn a_firing_beyond_the_horizon_is_queued_once_reached() {
        let body = calendar(
            "BEGIN:VEVENT\r\nUID:far\r\nDTSTAMP:20261101T000000Z\r\nDTSTART:20261108T090000Z\r\nDURATION:PT30M\r\nBEGIN:VALARM\r\nACTION:DISPLAY\r\nDESCRIPTION:x\r\nTRIGGER:-PT15M\r\nEND:VALARM\r\nEND:VEVENT\r\n",
        );
        let book = book(&[("far", &body)]);
        let mut queue = AlarmQueue::new(at("08:00:00"));

        queue.pass(&book, true, at("08:00:00"));
        assert!(queue.queue.is_empty());

        // NOTE: two days on, the half-horizon rebuild reaches it.
        queue.pass(&book, false, at("08:00:00") + 2 * DAY);
        assert_eq!(1, queue.queue.len());
    }

    #[test]
    fn a_late_firing_fires_within_the_grace_window_only() {
        let book = book(&[
            ("a", &event("near", "090000", "", &[("DISPLAY", "-PT15M")])),
            ("b", &event("early", "021000", "", &[("DISPLAY", "-PT10M")])),
        ]);

        let mut queue = AlarmQueue::new(at("00:00:00"));
        queue.pass(&book, true, at("00:00:00"));

        // NOTE: asleep from 01:00 to 08:55, the queue built before.
        let pass = queue.pass(&book, false, at("08:55:00"));

        assert_eq!(
            vec!["near"],
            pass.due.iter().map(|f| f.uid.as_str()).collect::<Vec<_>>()
        );
        assert_eq!(
            vec!["early"],
            pass.late.iter().map(|f| f.uid.as_str()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_firing_never_fires_twice() {
        let book = book(&[(
            "a",
            &event("standup", "090000", "", &[("DISPLAY", "-PT15M")]),
        )]);
        let mut queue = AlarmQueue::new(at("08:00:00"));

        queue.pass(&book, true, at("08:00:00"));
        assert_eq!(1, queue.pass(&book, false, at("08:45:00")).due.len());
        assert!(queue.pass(&book, true, at("08:46:00")).due.is_empty());
    }

    #[test]
    fn an_edit_moves_its_alarm_and_a_removal_drops_it() {
        let mut book = book(&[(
            "a",
            &event("standup", "090000", "", &[("DISPLAY", "-PT15M")]),
        )]);
        let mut queue = AlarmQueue::new(at("08:00:00"));
        queue.pass(&book, true, at("08:00:00"));

        let moved = event("standup", "100000", "", &[("DISPLAY", "-PT15M")]);
        book.read("a", "a", None, moved.as_bytes());
        queue.pass(&book, true, at("08:01:00"));

        assert!(queue.pass(&book, false, at("08:45:00")).due.is_empty());
        assert_eq!(1, queue.pass(&book, false, at("09:45:00")).due.len());

        let added = event("other", "110000", "", &[("AUDIO", "-PT15M")]);
        book.read("b", "b", None, added.as_bytes());
        book.forget("b");
        queue.pass(&book, true, at("09:46:00"));

        assert!(queue.pass(&book, false, at("10:45:00")).due.is_empty());
    }

    #[test]
    fn an_event_gaining_an_alarm_is_read_again() {
        let mut book = book(&[("a", &event("standup", "090000", "", &[]))]);
        assert_eq!(0, book.len());

        let alarmed = event("standup", "090000", "", &[("DISPLAY", "-PT10M")]);
        book.read("a", "a", None, alarmed.as_bytes());

        let firings = book.firings(at("00:00:00"), at("23:00:00"));
        assert_eq!(
            vec![at("08:50:00")],
            firings.iter().map(|f| f.trigger).collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_wait_ends_on_its_time_or_on_a_wake_sent_before() {
        let woken = AtomicBool::new(true);
        let shutdown = AtomicBool::new(false);
        let later = SystemTime::now() + Duration::from_secs(3_600);

        assert_eq!(AlarmWait::Woken, wait_until(later, &woken, &shutdown));
        assert_eq!(
            AlarmWait::Reached,
            wait_until(SystemTime::now(), &woken, &shutdown)
        );
    }
}
