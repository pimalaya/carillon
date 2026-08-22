//! Hook runner: fires the desktop notification and the shell command a
//! change is configured to trigger.
//!
//! Which hook a change calls for is its backend's to answer, since the
//! tables are named after the domain each backend holds; what arrives
//! here is the hook that answer resolved to, so one runner serves them
//! all.
//!
//! Notification summaries and bodies are templates expanded with
//! [`subst`] (shell-style `$name` / `${name}`); the same variables are
//! exported as environment variables on the spawned command, so both
//! shapes template against one vocabulary. A failing hook is logged and
//! never propagated, since a broken script must not stop the watch.

use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use log::{trace, warn};
use notify_rust::Notification;
use subst::VariableMap;

use crate::{
    config::{FlagHook, Hook, HookCmd, ItemHook, NotifyConfig},
    event::{ItemSummary, WatchEvent},
};

/// The variables every hook can fill, whatever reported the change.
const COMMON_VARS: &[&str] = &["id", "collection"];
/// What a flag hook adds: the one flag its firing is about.
const FLAG_VARS: &[&str] = &["flag"];
/// What an arrival adds, where the backend can read one. Nothing else
/// resolves an envelope, so nothing else may name these.
const ENVELOPE_VARS: &[&str] = &[
    "subject",
    "date",
    "sender",
    "sender_name",
    "sender_address",
    "recipient",
    "recipient_name",
    "recipient_address",
];

/// What a hook's notification may name, which is what its event
/// carries and no more.
///
/// It is both halves of the contract: the loader expands a template
/// against it to refuse a name no firing could ever fill, and the
/// runner seeds it empty so a name that is legitimate but absent from
/// one item expands to nothing rather than dropping the notification.
#[derive(Clone, Copy)]
pub struct Vocabulary(&'static [&'static [&'static str]]);

/// An item hook on a backend that resolves an arrival's envelope.
pub const RESOLVED_ITEM: Vocabulary = Vocabulary(&[COMMON_VARS, ENVELOPE_VARS]);
/// An item hook with nothing to resolve: a removal, an edit, or an
/// arrival on a backend that reads no envelope.
pub const ITEM: Vocabulary = Vocabulary(&[COMMON_VARS]);
/// A flag hook.
pub const FLAG: Vocabulary = Vocabulary(&[COMMON_VARS, FLAG_VARS]);

impl Vocabulary {
    /// The names this vocabulary holds.
    fn names(&self) -> impl Iterator<Item = &'static str> {
        self.0.iter().copied().flatten().copied()
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

impl<'a> VariableMap<'a> for Vocabulary {
    type Value = &'static str;

    fn get(&'a self, key: &str) -> Option<Self::Value> {
        self.names().find(|name| *name == key)
    }
}

/// Refuses a notification naming anything its hook cannot fill.
///
/// The check is the expansion itself, run against the vocabulary and
/// nothing else, so it refuses exactly what a firing would and lets a
/// `${name:default}` through, a default being how a template says it
/// can do without the value.
pub fn validate(notify: Option<&NotifyConfig>, vocabulary: Vocabulary, hook: &str) -> Result<()> {
    let Some(notify) = notify else {
        return Ok(());
    };

    for (part, template) in [("summary", &notify.summary), ("body", &notify.body)] {
        if let Err(err) = subst::substitute(template, &vocabulary) {
            bail!(
                "{hook}.notify.{part}: {err}. This hook can use {}",
                vocabulary.rendered()
            );
        }
    }

    Ok(())
}

/// Fires `hook` for `event`, with `summary` filled in when an arrival
/// was resolved.
pub fn run(hook: Hook<'_>, event: &WatchEvent, collection: &str, summary: Option<&ItemSummary>) {
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

/// Fires a flag-level hook for the one flag that moved, honouring its
/// optional filter.
fn run_flag_hook(hook: &FlagHook, id: &str, collection: &str, flag: &str) {
    if !hook.flags.is_empty() && !matches_filter(hook, flag) {
        trace!("flag hook skipped: `{flag}` is not in the filter");
        return;
    }

    let mut vars = seeded(FLAG);
    vars.insert("id", id.to_string());
    vars.insert("collection", collection.to_string());
    vars.insert("flag", flag.to_string());

    fire(hook.notify.as_ref(), hook.cmd.as_ref(), &vars);
}

/// The variables an item-level hook templates against.
///
/// The envelope ones carry a value only for a resolved arrival, and
/// are present and empty otherwise: an envelope with no `From`, or an
/// arrival whose resolution failed, must leave a gap in the
/// notification rather than take the whole notification down.
fn item_vars(
    id: &str,
    collection: &str,
    summary: Option<&ItemSummary>,
) -> BTreeMap<&'static str, String> {
    let mut vars = seeded(RESOLVED_ITEM);
    vars.insert("id", id.to_string());
    vars.insert("collection", collection.to_string());

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
        ("sender", "sender_name", "sender_address"),
        summary.from_name.as_deref(),
        summary.from_addr.as_deref(),
    );
    insert_party(
        &mut vars,
        ("recipient", "recipient_name", "recipient_address"),
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

/// Inserts the three variables naming one party: the combined form, the
/// personal name and the address.
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

/// Runs the two reactions a hook can carry, logging either failure.
fn fire(
    notify: Option<&NotifyConfig>,
    cmd: Option<&HookCmd>,
    vars: &BTreeMap<&'static str, String>,
) {
    if let Some(notify) = notify
        && let Err(err) = fire_notification(notify, vars)
    {
        warn!("notify hook failed: {err:#}");
    }

    if let Some(cmd) = cmd
        && let Err(err) = run_command(cmd, vars)
    {
        warn!("cmd hook failed: {err:#}");
    }
}

/// Expands the templates and shows the desktop notification.
fn fire_notification(config: &NotifyConfig, vars: &BTreeMap<&'static str, String>) -> Result<()> {
    let summary = subst::substitute(&config.summary, vars).context("cannot expand summary")?;
    let body = subst::substitute(&config.body, vars).context("cannot expand body")?;

    let mut notification = Notification::new();
    notification.summary(&summary);

    if !body.is_empty() {
        notification.body(&body);
    }

    notification.show()?;

    Ok(())
}

/// Spawns the hook command with the variables in its environment.
///
/// Both TOML shapes (a string handed to the platform shell, a list
/// spawned directly) are flattened into one [`std::process::Command`]
/// at deserialization time, so the runtime path is uniform here.
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
    use crate::{event::ItemSummary, hook::*};

    fn notified(body: &str) -> NotifyConfig {
        NotifyConfig {
            summary: String::from("something happened"),
            body: String::from(body),
        }
    }

    /// The regression this validation exists for: a removal whose body
    /// asks for an envelope fired nothing and only warned, because an
    /// expunged message has no envelope to read.
    #[test]
    fn an_envelope_variable_is_refused_where_nothing_resolves_one() {
        let notify = notified("$subject");
        let err = validate(Some(&notify), ITEM, "imap.hook.on-message-removed")
            .expect_err("an envelope name is refused");
        let err = format!("{err:#}");

        assert!(err.contains("on-message-removed.notify.body"), "got {err}");
        assert!(err.contains("$subject"), "got {err}");
        assert!(err.contains("$id, $collection"), "got {err}");
    }

    #[test]
    fn the_variables_an_event_carries_are_accepted() {
        let notify = notified("$subject from $sender in $collection");
        validate(Some(&notify), RESOLVED_ITEM, "imap.hook.on-message-added")
            .expect("an arrival resolves an envelope");

        let notify = notified("$flag on $id");
        validate(Some(&notify), FLAG, "imap.hook.on-flag-added").expect("a flag hook has its flag");

        let notify = notified("$flag");
        validate(Some(&notify), ITEM, "dav.hook.on-item-added")
            .expect_err("an item hook has no flag");
    }

    /// A default is how a template says it can do without the value,
    /// so it is not a claim that the variable exists.
    #[test]
    fn a_default_stands_in_for_any_name() {
        let notify = notified("${subject:no subject}");
        validate(Some(&notify), ITEM, "dav.hook.on-item-removed").expect("a default is enough");
    }

    /// The other half: a name the hook may use, absent from this one
    /// item, leaves a gap rather than taking the notification down.
    #[test]
    fn an_absent_variable_expands_to_nothing() {
        let vars = item_vars("42", "INBOX", None);
        let expanded = subst::substitute("$subject from $sender", &vars).expect("expands");
        assert_eq!(" from ", expanded);

        let summary = ItemSummary {
            from_addr: Some(String::from("alice@example.org")),
            ..Default::default()
        };
        let vars = item_vars("42", "INBOX", Some(&summary));
        let expanded = subst::substitute("$subject from $sender", &vars).expect("expands");
        assert_eq!(" from alice@example.org", expanded);
    }
}
