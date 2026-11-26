//! # Birthday
//!
//! The dates of a watched addressbook: the `BDAY` and `ANNIVERSARY` of its
//! cards (RFC 6350 6.2.5, 6.2.6), resolved into firings over the horizon,
//! which the firing thread fires as `on-birthday-trigger`.
//!
//! A vCard date is floating: a firing is the date's local day, moved
//! `days-before` days earlier, at the hook's `at` local time, in the zone
//! the book was built with. A year-less date (`--MMDD`) fires every year,
//! and a 29 February on 28 February in a common year. A date given as text
//! (`VALUE=text`) is no date and is ignored.

use std::collections::BTreeMap;

use jiff::{
    Span, Timestamp,
    civil::{Date, Time},
    tz::TimeZone,
};
use log::{trace, warn};
use vcard::{
    prop::{VcardPropKind, VcardPropName},
    tree::cst::VcardCst,
    value::VcardValue,
};

/// When the dates of an addressbook fire, as its hook configures it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BirthdayRule {
    /// The local time of day a date fires at.
    pub at: Time,
    /// How many days before the date it fires.
    pub days_before: u16,
    /// How many days after its own a firing reached late still fires.
    pub late_days: u16,
}

impl BirthdayRule {
    /// How far before now a firing may lie and still fire: its own day
    /// and the late days after it, plus a day for the time of day.
    pub fn reach(&self) -> i64 {
        (i64::from(self.late_days) + 2) * 86_400
    }
}

/// Which date of a card a firing is about.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BirthdayKind {
    /// A `BDAY`.
    Birthday,
    /// An `ANNIVERSARY`.
    Anniversary,
}

impl BirthdayKind {
    /// The word a hook reads it as.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Birthday => "birthday",
            Self::Anniversary => "anniversary",
        }
    }
}

/// One yearly firing of one date of a card, with what a hook says of it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BirthdayFiring {
    /// The backend's id of the card.
    pub id: String,
    /// The `UID` of the card, empty when it has none.
    pub uid: String,
    /// The `FN` of the card.
    pub name: Option<String>,
    /// Whether the date is a birthday or an anniversary.
    pub kind: BirthdayKind,
    /// The date as the card states it, `1990-11-27`, or `--11-27` with no
    /// year.
    pub date: String,
    /// The years the date completes on this firing, `None` with no year.
    pub age: Option<i16>,
    /// The local day it fires on.
    pub day: Date,
    /// When it fires, in seconds since the Unix epoch.
    pub trigger: i64,
    /// From when it no longer fires, its late days spent.
    pub deadline: i64,
}

impl BirthdayFiring {
    /// What tells this firing apart from every other: the card's `UID`
    /// (its id when it has none), the date's kind and the instant.
    pub fn key(&self) -> String {
        let card = match self.uid.is_empty() {
            true => &self.id,
            false => &self.uid,
        };

        format!("{card}/{}@{}", self.kind.as_str(), self.trigger)
    }

    /// Whether it still fires at `now`.
    pub fn fires_at(&self, now: i64) -> bool {
        now < self.deadline
    }

    /// How many days late it fires on `today`, 0 on its own day.
    pub fn late(&self, today: Date) -> i32 {
        self.day
            .until(today)
            .map_or(0, |span| span.get_days().max(0))
    }
}

/// The cards of one addressbook holding a date, by the backend's key.
pub struct BirthdayBook {
    cards: BTreeMap<String, BirthdayCard>,
    /// The zone a date is read in.
    zone: TimeZone,
}

impl BirthdayBook {
    /// An empty book, its dates read in `zone`.
    pub fn new(zone: TimeZone) -> Self {
        Self {
            cards: BTreeMap::new(),
            zone,
        }
    }

    /// Reads the body of the card at `key` again, keeping it only when it
    /// states a date. `id` is what a hook names the card by.
    ///
    /// A body that is not a vCard is logged and forgotten, never an error:
    /// one broken card must not stop the others.
    pub fn read(&mut self, key: &str, id: &str, body: &[u8]) {
        self.cards.remove(key);

        let cst = match VcardCst::parse(body) {
            Ok(cst) => cst,
            Err(err) => {
                warn!("cannot parse card {id}: {err:?}");
                return;
            }
        };

        let vcard = cst.decode();
        let mut card = BirthdayCard {
            id: id.to_owned(),
            uid: String::new(),
            name: None,
            dates: Vec::new(),
        };

        for prop in &vcard.properties {
            let VcardPropName::Kind(kind) = prop.name else {
                continue;
            };

            match (kind, &prop.value) {
                (VcardPropKind::Fn, VcardValue::Text(text)) if !text.0.is_empty() => {
                    card.name = Some(text.0.to_string());
                }
                (VcardPropKind::Uid, VcardValue::Text(text)) => card.uid = text.0.to_string(),
                (VcardPropKind::Uid, VcardValue::Uri(uri)) => card.uid = uri.0.to_string(),
                (VcardPropKind::Bday, VcardValue::DateAndOrTime(date)) => {
                    card.dates
                        .extend(BirthdayDate::parse(BirthdayKind::Birthday, &date.0));
                }
                (VcardPropKind::Anniversary, VcardValue::DateAndOrTime(date)) => {
                    card.dates
                        .extend(BirthdayDate::parse(BirthdayKind::Anniversary, &date.0));
                }
                _ => (),
            }
        }

        if card.dates.is_empty() {
            return;
        }

        trace!("card {id} states {} dates", card.dates.len());
        self.cards.insert(key.to_owned(), card);
    }

    /// Forgets the card at `key`, as a removal does.
    pub fn forget(&mut self, key: &str) {
        self.cards.remove(key);
    }

    /// How many cards stating a date the book holds.
    pub fn len(&self) -> usize {
        self.cards.len()
    }

    /// Every firing of `rule` over `[from, until)` still firing at `now`,
    /// in time order.
    pub fn firings(
        &self,
        rule: &BirthdayRule,
        from: i64,
        now: i64,
        until: i64,
    ) -> Vec<BirthdayFiring> {
        let mut firings = Vec::new();

        let year = |instant: i64| match Timestamp::from_second(instant) {
            Ok(at) => at.to_zoned_datetime(self.zone.clone()).year(),
            Err(_) => 0,
        };

        let first = year(from).saturating_sub(1);
        let last = year(until).saturating_add(1 + (rule.days_before / 365) as i16);

        for card in self.cards.values() {
            for date in &card.dates {
                for year in first..=last {
                    let Some(firing) = self.firing(card, date, rule, year) else {
                        continue;
                    };

                    if firing.trigger >= from && firing.trigger < until && firing.fires_at(now) {
                        firings.push(firing);
                    }
                }
            }
        }

        firings.sort_by(|a, b| (a.trigger, &a.id).cmp(&(b.trigger, &b.id)));
        firings
    }

    /// The firing of `date` on its occurrence in `year`, `None` before the
    /// year it states or out of range.
    fn firing(
        &self,
        card: &BirthdayCard,
        date: &BirthdayDate,
        rule: &BirthdayRule,
        year: i16,
    ) -> Option<BirthdayFiring> {
        let age = match date.year {
            Some(born) if year < born => return None,
            Some(born) => Some(year - born),
            None => None,
        };

        let occurrence = Date::new(year, date.month, date.day)
            .or_else(|_| Date::new(year, date.month, date.day - 1))
            .ok()?;
        let day = occurrence
            .checked_sub(Span::new().days(rule.days_before))
            .ok()?;
        let last = day
            .checked_add(Span::new().days(i64::from(rule.late_days) + 1))
            .ok()?;

        let instant = |date: Date, time: Time| {
            self.zone
                .to_ambiguous_timestamp(date.to_datetime(time))
                .compatible()
                .map(|at| at.as_second())
        };

        Some(BirthdayFiring {
            id: card.id.clone(),
            uid: card.uid.clone(),
            name: card.name.clone(),
            kind: date.kind,
            date: date.text(),
            age,
            day,
            trigger: instant(day, rule.at).ok()?,
            deadline: instant(last, Time::midnight()).ok()?,
        })
    }
}

/// One card stating a date.
struct BirthdayCard {
    id: String,
    uid: String,
    name: Option<String>,
    dates: Vec<BirthdayDate>,
}

/// A date a card states, its year when it gives one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BirthdayDate {
    kind: BirthdayKind,
    year: Option<i16>,
    month: i8,
    day: i8,
}

impl BirthdayDate {
    /// Reads a date-and-or-time value (RFC 6350 4.3.4): a full date in
    /// the basic or extended form, or a year-less `--MMDD` or `--MM-DD`, a
    /// time after it dropped. A date naming no day, or no valid one, is
    /// `None`, a year-less one checked against a leap year.
    fn parse(kind: BirthdayKind, value: &str) -> Option<Self> {
        let date = value.trim().split(['T', 't']).next()?;

        if !date.is_ascii() {
            return None;
        }

        let (year, rest) = match date.strip_prefix("--") {
            Some(rest) => (None, rest),
            None if date.len() == 10 && date.as_bytes()[4] == b'-' => {
                (Some(&date[..4]), &date[5..])
            }
            None if date.len() == 8 => (Some(&date[..4]), &date[4..]),
            None => return None,
        };

        let (month, day) = match rest.len() {
            4 => (&rest[..2], &rest[2..]),
            5 if rest.as_bytes()[2] == b'-' => (&rest[..2], &rest[3..]),
            _ => return None,
        };

        let digits = |text: &str| text.bytes().all(|b| b.is_ascii_digit());

        if !year.into_iter().chain([month, day]).all(digits) {
            return None;
        }

        let year: Option<i16> = year.map(str::parse).transpose().ok()?;
        let month: i8 = month.parse().ok()?;
        let day: i8 = day.parse().ok()?;
        Date::new(year.unwrap_or(2000), month, day).ok()?;

        Some(Self {
            kind,
            year,
            month,
            day,
        })
    }

    /// The date as a hook reads it: `1990-11-27`, or `--11-27` with no
    /// year.
    fn text(&self) -> String {
        match self.year {
            Some(year) => format!("{year:04}-{:02}-{:02}", self.month, self.day),
            None => format!("--{:02}-{:02}", self.month, self.day),
        }
    }
}

#[cfg(test)]
mod tests {
    use jiff::{
        Timestamp,
        civil::{Date, Time},
        tz::TimeZone,
    };

    use crate::birthday::*;

    /// The zone every case reads its dates in.
    fn paris() -> TimeZone {
        TimeZone::get("Europe/Paris").unwrap()
    }

    /// An instant written in UTC, in seconds.
    fn at(instant: &str) -> i64 {
        instant.parse::<Timestamp>().unwrap().as_second()
    }

    /// A vCard 4.0 holding `props`.
    fn card(props: &str) -> String {
        format!("BEGIN:VCARD\r\nVERSION:4.0\r\nUID:jane\r\nFN:Jane Doe\r\n{props}END:VCARD\r\n")
    }

    /// The default rule: 09:00 on the day, no late day.
    fn nine() -> BirthdayRule {
        BirthdayRule {
            at: Time::constant(9, 0, 0, 0),
            days_before: 0,
            late_days: 0,
        }
    }

    /// The firings of a book holding `body`, over 2026 and 2027.
    fn firings(body: &str, rule: &BirthdayRule) -> Vec<BirthdayFiring> {
        let mut book = BirthdayBook::new(paris());
        book.read("a", "/books/a.vcf", body.as_bytes());

        let from = at("2026-01-01T00:00:00Z");
        book.firings(rule, from, from, at("2028-01-01T00:00:00Z"))
    }

    #[test]
    fn a_birthday_fires_at_nine_local_with_its_age() {
        for bday in ["BDAY:1990-11-27\r\n", "BDAY:19901127\r\n"] {
            let fired = firings(&card(bday), &nine());
            let first = &fired[0];

            assert_eq!(2, fired.len(), "{bday}");
            assert_eq!(at("2026-11-27T08:00:00Z"), first.trigger);
            assert_eq!(at("2026-11-28T00:00:00+01:00"), first.deadline);
            assert_eq!("jane", first.uid);
            assert_eq!("/books/a.vcf", first.id);
            assert_eq!(Some("Jane Doe"), first.name.as_deref());
            assert_eq!(BirthdayKind::Birthday, first.kind);
            assert_eq!("1990-11-27", first.date);
            assert_eq!(Some(36), first.age);
            assert_eq!(Some(37), fired[1].age);
        }
    }

    #[test]
    fn a_year_less_birthday_fires_with_no_age() {
        for bday in ["BDAY:--1127\r\n", "BDAY:--11-27\r\n"] {
            let fired = firings(&card(bday), &nine());

            assert_eq!(2, fired.len(), "{bday}");
            assert_eq!(at("2026-11-27T08:00:00Z"), fired[0].trigger);
            assert_eq!("--11-27", fired[0].date);
            assert_eq!(None, fired[0].age);
        }
    }

    #[test]
    fn a_leap_day_fires_on_28_february_in_a_common_year() {
        let fired = firings(&card("BDAY:2000-02-29\r\n"), &nine());
        let triggers: Vec<i64> = fired.iter().map(|firing| firing.trigger).collect();

        assert_eq!(
            vec![at("2026-02-28T08:00:00Z"), at("2027-02-28T08:00:00Z")],
            triggers
        );

        let mut book = BirthdayBook::new(paris());
        book.read("a", "a", card("BDAY:--0229\r\n").as_bytes());
        let from = at("2028-01-01T00:00:00Z");
        let leap = book.firings(&nine(), from, from, at("2029-01-01T00:00:00Z"));

        assert_eq!(vec![at("2028-02-29T08:00:00Z")], vec![leap[0].trigger]);
    }

    #[test]
    fn a_date_given_as_text_is_ignored() {
        let body = card("BDAY;VALUE=text:circa 1800\r\nANNIVERSARY;VALUE=text:June\r\n");
        let mut book = BirthdayBook::new(paris());
        book.read("a", "a", body.as_bytes());

        assert_eq!(0, book.len());
        assert!(firings(&body, &nine()).is_empty());
    }

    #[test]
    fn an_anniversary_fires_beside_a_birthday() {
        let body = card("BDAY:19901127\r\nANNIVERSARY:20100612\r\n");
        let fired = firings(&body, &nine());
        let kinds: Vec<(BirthdayKind, i64, Option<i16>)> = fired
            .iter()
            .map(|firing| (firing.kind, firing.trigger, firing.age))
            .collect();

        assert_eq!(
            vec![
                (
                    BirthdayKind::Anniversary,
                    at("2026-06-12T07:00:00Z"),
                    Some(16)
                ),
                (BirthdayKind::Birthday, at("2026-11-27T08:00:00Z"), Some(36)),
                (
                    BirthdayKind::Anniversary,
                    at("2027-06-12T07:00:00Z"),
                    Some(17)
                ),
                (BirthdayKind::Birthday, at("2027-11-27T08:00:00Z"), Some(37)),
            ],
            kinds
        );
    }

    #[test]
    fn a_date_fires_its_days_before_at_its_time() {
        let rule = BirthdayRule {
            at: Time::constant(18, 30, 0, 0),
            days_before: 1,
            late_days: 2,
        };
        let fired = firings(&card("BDAY:1990-01-01\r\n"), &rule);

        assert_eq!(at("2026-12-31T17:30:00Z"), fired[0].trigger);
        assert_eq!(at("2027-01-03T00:00:00+01:00"), fired[0].deadline);
        assert_eq!(Some(37), fired[0].age);
        assert_eq!(2, fired.len());
    }

    #[test]
    fn a_date_holding_a_time_or_a_vcard_3_date_fires() {
        let v3 = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Jane\r\nBDAY:1990-11-27\r\nEND:VCARD\r\n";
        let timed = card("BDAY:19901127T231000Z\r\n");

        for body in [v3, &timed] {
            let fired = firings(body, &nine());
            assert_eq!(at("2026-11-27T08:00:00Z"), fired[0].trigger, "{body}");
        }

        assert_eq!("", firings(v3, &nine())[0].uid);
    }

    #[test]
    fn a_date_naming_no_day_is_ignored() {
        for bday in ["BDAY:1990\r\n", "BDAY:1990-11\r\n", "BDAY:---27\r\n"] {
            assert!(firings(&card(bday), &nine()).is_empty(), "{bday}");
        }
    }

    #[test]
    fn a_firing_past_its_late_days_is_left_out() {
        let mut book = BirthdayBook::new(paris());
        book.read("a", "a", card("BDAY:--1127\r\n").as_bytes());
        let from = at("2026-11-27T00:00:00Z");
        let until = at("2026-11-30T00:00:00Z");

        let late = |rule: &BirthdayRule, now: &str| book.firings(rule, from, at(now), until).len();
        let two = BirthdayRule {
            late_days: 2,
            ..nine()
        };

        assert_eq!(1, late(&nine(), "2026-11-27T22:00:00Z"));
        assert_eq!(0, late(&nine(), "2026-11-27T23:00:00Z"));
        assert_eq!(1, late(&two, "2026-11-29T22:00:00Z"));
        assert_eq!(0, late(&two, "2026-11-29T23:00:00Z"));
    }

    #[test]
    fn a_firing_counts_its_late_days() {
        let fired = firings(&card("BDAY:--1127\r\n"), &nine());
        let day = |text: &str| text.parse::<Date>().unwrap();

        assert_eq!(0, fired[0].late(day("2026-11-27")));
        assert_eq!(1, fired[0].late(day("2026-11-28")));
    }

    #[test]
    fn a_body_that_is_no_vcard_is_forgotten() {
        let mut book = BirthdayBook::new(paris());
        book.read("a", "a", card("BDAY:--1127\r\n").as_bytes());
        book.read("a", "a", b"not a card");

        assert_eq!(0, book.len());
    }
}
