//! # Hook
//!
//! The runner firing the desktop notification and the shell command a
//! change is configured to trigger.
//!
//! Which hook a change calls for is its backend's to answer, the tables
//! being named after the domain each backend holds; what arrives here is
//! the hook that answer resolved to, so one runner serves them all.
//!
//! Summaries and bodies are pimalaya-config notification templates, in
//! the shell's syntax (`$name`, `${name}`, `${name:-default}`), and the
//! same camelCase variables are exported as environment variables on the
//! spawned command, so both shapes template against one vocabulary.
//!
//! A failing hook is logged and never propagated, since a broken script
//! must not stop the watch.

use std::collections::BTreeMap;

use anyhow::{Result, bail};
#[cfg(alarm)]
use jiff::{Timestamp, tz::TimeZone};
use log::{trace, warn};
use pimalaya_config::notify::NotificationTemplate;

#[cfg(all(alarm, feature = "notify"))]
use crate::alarm::AlarmAction;
#[cfg(alarm)]
use crate::alarm::AlarmFiring;
use crate::{
    config::{FlagHook, Hook, HookCmd, ItemHook},
    event::{ItemSummary, WatchEvent},
};

/// The one variable every backend means the same way.
const ID_VAR: &str = "id";
/// The collection the event is about, whatever its backend calls it, so
/// one hook runner reads every domain under one name.
const COLLECTION_VAR: &str = "collection";
/// What a flag hook adds: the one flag its firing is about.
const FLAG_VAR: &str = "flag";
/// What an arrival adds, where its backend can read an envelope.
///
/// Nothing else resolves one, so nothing else may name these.
const ENVELOPE_VARS: &[&str] = &[
    "subject",
    "date",
    "sender",
    "senderName",
    "senderAddress",
    "recipient",
    "recipientName",
    "recipientAddress",
];
/// What an alarm hook adds: the firing, and the item holding it.
const ALARM_VARS: &[&str] = &[
    "etag",
    "uid",
    "kind",
    "summary",
    "recurrenceId",
    "alarm",
    "alarmUid",
    "action",
    "trigger",
    "at",
];
/// The sound an `AUDIO` alarm asks the notification for, from the
/// freedesktop sound naming specification.
#[cfg(all(alarm, feature = "notify"))]
const ALARM_SOUND: &str = "alarm-clock-elapsed";

/// The collection a watch is on: what its backend calls it, and which one
/// it is.
///
/// The name travels with the value because it is the backend's word, not
/// carillon's: mail watches a `mailbox`, CalDAV a `calendar`, CardDAV an
/// `addressbook`.
#[derive(Clone, Copy)]
pub struct HookCollection<'a> {
    /// The word the backend names its collection with.
    pub name: &'static str,
    /// The collection itself, as the account configured it.
    pub value: &'a str,
}

/// What a hook's notification may name, which is what its event carries
/// and no more.
///
/// Both halves of the contract: the loader expands a template against it
/// to refuse a name no firing could ever fill, and the runner seeds it
/// empty so a legitimate name absent from one item expands to nothing.
#[derive(Clone, Copy)]
pub struct Vocabulary {
    /// What this backend calls the collection it watches.
    collection: &'static str,
    /// Whether an arrival's envelope is resolved for this hook.
    envelope: bool,
    /// Whether this hook is about one flag.
    flag: bool,
    /// Whether this hook is about one alarm firing.
    alarm: bool,
}

impl Vocabulary {
    /// An item hook with nothing to resolve: a removal, an edit, or an
    /// arrival on a backend that reads no envelope.
    pub const fn item(collection: &'static str) -> Self {
        Self {
            collection,
            envelope: false,
            flag: false,
            alarm: false,
        }
    }

    /// An item hook on a backend that resolves an arrival's envelope.
    pub const fn resolved(collection: &'static str) -> Self {
        Self {
            collection,
            envelope: true,
            flag: false,
            alarm: false,
        }
    }

    /// A flag hook.
    pub const fn flag(collection: &'static str) -> Self {
        Self {
            collection,
            envelope: false,
            flag: true,
            alarm: false,
        }
    }

    /// An alarm hook.
    #[cfg(alarm)]
    pub const fn alarm(collection: &'static str) -> Self {
        Self {
            collection,
            envelope: false,
            flag: false,
            alarm: true,
        }
    }

    /// The names this vocabulary holds.
    fn names(&self) -> impl Iterator<Item = &'static str> {
        let envelope: &[&str] = if self.envelope { ENVELOPE_VARS } else { &[] };
        let flag = self.flag.then_some(FLAG_VAR);
        let alarm: &[&str] = if self.alarm { ALARM_VARS } else { &[] };

        [ID_VAR, COLLECTION_VAR, self.collection]
            .into_iter()
            .chain(envelope.iter().copied())
            .chain(flag)
            .chain(alarm.iter().copied())
    }

    /// The names, rendered the way a template writes them, for an
    /// error that has to say what was allowed instead.
    fn rendered(&self) -> String {
        self.names()
            .map(|name| format!("${name}"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Refuses a notification naming anything its hook cannot fill, a
/// default or not: a template's default stands in for an empty value,
/// never for a name the hook does not have.
pub fn validate(
    notify: Option<&NotificationTemplate>,
    vocabulary: Vocabulary,
    hook: &str,
) -> Result<()> {
    let Some(notify) = notify else {
        return Ok(());
    };

    if let Err(err) = notify.check(|name| vocabulary.names().any(|known| known == name)) {
        bail!(
            "{hook}.notify: {err}. This hook can use {}",
            vocabulary.rendered()
        );
    }

    Ok(())
}

/// Fires `hook` for `event`, with the summary a resolved arrival carries.
pub fn run(
    hook: Hook<'_>,
    event: &WatchEvent,
    collection: HookCollection<'_>,
    summary: Option<&ItemSummary>,
) {
    trace!("dispatch event: {event:?}");

    match hook {
        Hook::Item(hook) => run_item_hook(hook, item_vars(event.id(), collection, summary)),
        #[allow(unreachable_patterns)]
        Hook::Flag(hook) => match event {
            WatchEvent::FlagAdded { id, flag, .. } | WatchEvent::FlagRemoved { id, flag, .. } => {
                run_flag_hook(hook, id, collection, flag)
            }
            // NOTE: unreachable by construction, a flag hook being what
            // only a flag event resolves to.
            event => warn!("flag hook resolved for {event:?}, skipping"),
        },
    }
}

/// Fires an item-level hook.
fn run_item_hook(hook: &ItemHook, vars: BTreeMap<&'static str, String>) {
    fire(hook.notify.as_ref(), hook.cmd.as_ref(), &vars);
}

/// Fires a flag-level hook for the flag that moved, through its filter.
fn run_flag_hook(hook: &FlagHook, id: &str, collection: HookCollection<'_>, flag: &str) {
    if !hook.flags.is_empty() && !matches_filter(hook, flag) {
        trace!("flag hook skipped: `{flag}` is not in the filter");
        return;
    }

    let mut vars = seeded(Vocabulary::flag(collection.name));
    vars.insert(ID_VAR, id.to_string());
    vars.insert(COLLECTION_VAR, collection.value.to_string());
    vars.insert(collection.name, collection.value.to_string());
    vars.insert(FLAG_VAR, flag.to_string());

    fire(hook.notify.as_ref(), hook.cmd.as_ref(), &vars);
}

/// The variables an item-level hook templates against.
///
/// The envelope ones carry a value only for a resolved arrival and are
/// present and empty otherwise: an envelope with no `From`, or a failed
/// resolution, leaves a gap rather than dropping the notification.
fn item_vars(
    id: &str,
    collection: HookCollection<'_>,
    summary: Option<&ItemSummary>,
) -> BTreeMap<&'static str, String> {
    let mut vars = seeded(Vocabulary::resolved(collection.name));
    vars.insert(ID_VAR, id.to_string());
    vars.insert(COLLECTION_VAR, collection.value.to_string());
    vars.insert(collection.name, collection.value.to_string());

    let Some(summary) = summary else {
        return vars;
    };

    if let Some(subject) = &summary.subject {
        vars.insert("subject", subject.clone());
    }

    if let Some(date) = &summary.date {
        vars.insert("date", date.clone());
    }

    insert_party(
        &mut vars,
        ("sender", "senderName", "senderAddress"),
        summary.from_name.as_deref(),
        summary.from_addr.as_deref(),
    );
    insert_party(
        &mut vars,
        ("recipient", "recipientName", "recipientAddress"),
        summary.to_name.as_deref(),
        summary.to_addr.as_deref(),
    );

    vars
}

/// A map holding every name `vocabulary` allows, all empty, for the
/// caller to overwrite with whatever it resolved.
fn seeded(vocabulary: Vocabulary) -> BTreeMap<&'static str, String> {
    vocabulary
        .names()
        .map(|name| (name, String::new()))
        .collect()
}

/// Inserts the three variables naming one party: combined, name, address.
fn insert_party(
    vars: &mut BTreeMap<&'static str, String>,
    keys: (&'static str, &'static str, &'static str),
    name: Option<&str>,
    address: Option<&str>,
) {
    let (combined_key, name_key, address_key) = keys;

    if let Some(name) = name {
        vars.insert(name_key, name.to_string());
    }

    if let Some(address) = address {
        vars.insert(address_key, address.to_string());
    }

    let combined = match (name, address) {
        (Some(name), Some(address)) => format!("{name} <{address}>"),
        (None, Some(address)) => address.to_string(),
        (Some(name), None) => name.to_string(),
        (None, None) => return,
    };

    vars.insert(combined_key, combined);
}

/// Fires `on-alarm-trigger` for one firing of an alarm of `collection`.
///
/// Times reach the hook as RFC 3339 in the system's zone, and an `AUDIO`
/// alarm's notification asks for the freedesktop alarm sound.
#[cfg(alarm)]
pub fn run_alarm(hook: &ItemHook, collection: HookCollection<'_>, firing: &AlarmFiring) {
    let zone = TimeZone::system();
    let mut vars = seeded(Vocabulary::alarm(collection.name));

    vars.insert(ID_VAR, firing.id.clone());
    vars.insert(COLLECTION_VAR, collection.value.to_string());
    vars.insert(collection.name, collection.value.to_string());
    vars.insert("etag", firing.etag.clone().unwrap_or_default());
    vars.insert("uid", firing.uid.clone());
    vars.insert("kind", firing.kind.as_str().to_string());
    vars.insert("summary", firing.summary.clone().unwrap_or_default());
    vars.insert(
        "recurrenceId",
        firing.occurrence.clone().unwrap_or_default(),
    );
    vars.insert("alarm", firing.alarm.to_string());
    vars.insert("alarmUid", firing.alarm_uid.clone().unwrap_or_default());
    vars.insert("action", firing.action.as_str().to_string());
    vars.insert("trigger", rfc3339(firing.trigger, &zone));
    vars.insert(
        "at",
        firing.at.map(|at| rfc3339(at, &zone)).unwrap_or_default(),
    );

    #[cfg(feature = "notify")]
    let sound = (firing.action == AlarmAction::Audio).then_some(ALARM_SOUND);
    #[cfg(not(feature = "notify"))]
    let sound = None;

    fire_with(hook.notify.as_ref(), hook.cmd.as_ref(), &vars, sound);
}

/// An instant as RFC 3339 with the offset `zone` has then, empty when it
/// is out of range.
#[cfg(alarm)]
fn rfc3339(instant: i64, zone: &TimeZone) -> String {
    match Timestamp::from_second(instant) {
        Ok(at) => at
            .to_zoned(zone.clone())
            .strftime("%Y-%m-%dT%H:%M:%S%:z")
            .to_string(),
        Err(_) => String::new(),
    }
}

/// Runs the two reactions a hook can carry, logging either failure.
fn fire(
    notify: Option<&NotificationTemplate>,
    cmd: Option<&HookCmd>,
    vars: &BTreeMap<&'static str, String>,
) {
    fire_with(notify, cmd, vars, None);
}

/// Runs the two reactions a hook can carry, the notification asking for
/// `sound` when one is given, logging either failure.
fn fire_with(
    notify: Option<&NotificationTemplate>,
    cmd: Option<&HookCmd>,
    vars: &BTreeMap<&'static str, String>,
    sound: Option<&'static str>,
) {
    if let Some(notify) = notify
        && let Err(err) = fire_notification(notify, vars, sound)
    {
        warn!("notify hook failed: {err:#}");
    }

    if let Some(cmd) = cmd
        && let Err(err) = run_command(cmd, vars)
    {
        warn!("cmd hook failed: {err:#}");
    }
}

/// Expands the template and shows the desktop notification.
fn fire_notification(
    template: &NotificationTemplate,
    vars: &BTreeMap<&'static str, String>,
    sound: Option<&'static str>,
) -> Result<()> {
    #[cfg_attr(not(feature = "notify"), allow(unused_mut))]
    let mut notification = template.render(|name| vars.get(name).cloned())?;

    #[cfg(feature = "notify")]
    if let Some(sound) = sound {
        notification.sound_name(sound);
    }

    #[cfg(not(feature = "notify"))]
    let _ = sound;

    if let Err(err) = notification.show() {
        bail!("Cannot show notification: {err}");
    }

    Ok(())
}

/// Spawns the hook command with the variables in its environment.
///
/// Both TOML shapes, a string handed to the platform shell and a list
/// spawned directly, are flattened into one [`std::process::Command`] at
/// deserialization time, so there is one runtime path here.
fn run_command(cmd: &HookCmd, vars: &BTreeMap<&'static str, String>) -> Result<()> {
    let status = cmd
        .clone()
        .0
        .envs(vars.iter().map(|(key, value)| (*key, value.as_str())))
        .status()?;

    if !status.success() {
        warn!("cmd hook exited with {status}");
    }

    Ok(())
}

/// Whether `flag` matches one of the filter's names, with or without an
/// IMAP backslash or a keyword dollar.
fn matches_filter(hook: &FlagHook, flag: &str) -> bool {
    let stripped = flag.trim_start_matches(['\\', '$']);

    hook.flags
        .iter()
        .any(|name| name.eq_ignore_ascii_case(flag) || name.eq_ignore_ascii_case(stripped))
}

#[cfg(test)]
mod tests {
    use crate::hook::*;

    fn notified(body: &str) -> NotificationTemplate {
        NotificationTemplate::new("something happened", body).expect("build the template")
    }

    /// The regression this validation exists for: a removal whose body
    /// asked for an envelope fired nothing and only warned.
    #[test]
    fn an_envelope_variable_is_refused_where_nothing_resolves_one() {
        let notify = notified("$subject");
        let err = validate(
            Some(&notify),
            Vocabulary::item("mailbox"),
            "imap.hook.on-message-removed",
        )
        .expect_err("an envelope name is refused");
        let err = format!("{err:#}");

        assert!(err.contains("on-message-removed.notify"), "got {err}");
        assert!(err.contains("`subject`"), "got {err}");
        assert!(err.contains("$id, $collection, $mailbox"), "got {err}");
    }

    #[test]
    fn the_variables_an_event_carries_are_accepted() {
        let notify = notified("$subject from $sender in $mailbox");
        validate(
            Some(&notify),
            Vocabulary::resolved("mailbox"),
            "imap.hook.on-message-added",
        )
        .expect("an arrival resolves an envelope");

        let notify = notified("$flag on $id");
        validate(
            Some(&notify),
            Vocabulary::flag("mailbox"),
            "imap.hook.on-flag-added",
        )
        .expect("a flag hook has its flag");

        let notify = notified("$flag");
        validate(
            Some(&notify),
            Vocabulary::item("calendar"),
            "caldav.hook.on-event-added",
        )
        .expect_err("an item hook has no flag");
    }

    /// A default stands in for an empty value, never for a name the
    /// hook does not have, so a misspelt name is still caught.
    #[test]
    fn a_default_does_not_excuse_an_unknown_name() {
        let notify = notified("${subject:-no subject}");
        validate(
            Some(&notify),
            Vocabulary::item("addressbook"),
            "carddav.hook.on-card-removed",
        )
        .expect_err("a card removal has no subject");

        let notify = notified("${subject:-no subject}");
        validate(
            Some(&notify),
            Vocabulary::resolved("mailbox"),
            "imap.hook.on-message-added",
        )
        .expect("an arrival has a subject");
    }

    /// The other half: a name the hook may use, absent from this one
    /// item, leaves a gap or its default rather than taking the
    /// notification down.
    #[cfg(feature = "notify")]
    #[test]
    fn an_absent_variable_expands_to_nothing_or_its_default() {
        use crate::event::ItemSummary;

        let mailbox = || HookCollection {
            name: "mailbox",
            value: "INBOX",
        };
        let template = NotificationTemplate::new("$subject from $sender", "${senderName:-?}")
            .expect("build the template");

        let vars = item_vars("42", mailbox(), None);
        let notification = template
            .render(|name| vars.get(name).cloned())
            .expect("render");
        assert_eq!(" from ", notification.summary);
        assert_eq!("?", notification.body);

        let summary = ItemSummary {
            from_name: Some(String::from("Alice")),
            from_addr: Some(String::from("alice@example.org")),
            ..Default::default()
        };
        let vars = item_vars("42", mailbox(), Some(&summary));
        let notification = template
            .render(|name| vars.get(name).cloned())
            .expect("render");
        assert_eq!(" from Alice <alice@example.org>", notification.summary);
        assert_eq!("Alice", notification.body);
    }

    /// One hook runner reads every domain: `$collection` is the value
    /// the account configured, whatever its backend calls it.
    #[test]
    fn every_hook_names_its_collection_under_one_name() {
        let calendar = HookCollection {
            name: "calendar",
            value: "work",
        };
        let vars = item_vars("/cal/alice/work/a.ics", calendar, None);

        assert_eq!("work", vars["collection"]);
        assert_eq!("work", vars["calendar"]);
        assert_eq!("/cal/alice/work/a.ics", vars["id"]);
        assert!(!vars.contains_key("mailbox"));

        let notify = notified("$collection");
        validate(
            Some(&notify),
            Vocabulary::item("addressbook"),
            "gpeople.hook.on-card-added",
        )
        .expect("every hook has its collection");
    }

    #[cfg(alarm)]
    #[test]
    fn an_alarm_time_reads_in_rfc_3339_with_its_offset() {
        let zone = TimeZone::get("Europe/Paris").expect("a bundled zone");
        let instant = "2026-11-05T07:45:00Z"
            .parse::<Timestamp>()
            .unwrap()
            .as_second();

        assert_eq!("2026-11-05T08:45:00+01:00", rfc3339(instant, &zone));
    }

    #[cfg(alarm)]
    #[test]
    fn an_alarm_hook_templates_against_its_firing() {
        let alarm = Vocabulary::alarm("calendar");
        let hook = "caldav.hook.on-alarm-trigger";

        validate(
            Some(&notified("$summary at $at ($kind, $action)")),
            alarm,
            hook,
        )
        .expect("a firing has a summary, a time, a kind and an action");
        validate(Some(&notified("$subject")), alarm, hook).expect_err("an alarm has no subject");
        validate(
            Some(&notified("$summary")),
            Vocabulary::item("calendar"),
            "caldav.hook.on-event-added",
        )
        .expect_err("an event addition has no summary");
    }

    #[cfg(all(alarm, unix))]
    #[test]
    fn an_alarm_command_reads_the_firing_in_its_environment() {
        use crate::alarm::{AlarmAction, AlarmKind};

        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("fired");
        let line = format!("printf '%s %s' \"$uid\" \"$trigger\" > '{}'", out.display());
        let hook = ItemHook {
            notify: None,
            cmd: Some(HookCmd(pimalaya_config::command::shell(&line))),
        };

        let trigger = "2026-11-05T07:45:00Z"
            .parse::<Timestamp>()
            .unwrap()
            .as_second();
        let firing = AlarmFiring {
            id: String::from("/cal/work/standup.ics"),
            etag: None,
            uid: String::from("standup"),
            kind: AlarmKind::Event,
            summary: None,
            alarm: 0,
            alarm_uid: None,
            over: None,
            occurrence: None,
            repeat: 0,
            action: AlarmAction::Display,
            trigger,
            at: None,
        };
        let collection = HookCollection {
            name: "calendar",
            value: "work",
        };

        run_alarm(&hook, collection, &firing);

        let fired = std::fs::read_to_string(&out).expect("the command ran");
        let (uid, at) = fired.split_once(' ').expect("two words");
        assert_eq!("standup", uid);
        assert_eq!(trigger, at.parse::<Timestamp>().unwrap().as_second());
    }
}
