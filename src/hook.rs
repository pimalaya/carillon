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

use anyhow::{Context, Result};
use log::{trace, warn};
use notify_rust::Notification;

use crate::{
    config::{FlagHook, Hook, HookCmd, ItemHook, NotifyConfig},
    event::{ItemSummary, WatchEvent},
};

/// Fires `hook` for `event`, with `summary` filled in when an arrival
/// was resolved.
pub fn run(hook: Hook<'_>, event: &WatchEvent, collection: &str, summary: Option<&ItemSummary>) {
    trace!("dispatch event: {event:?}");

    match hook {
        Hook::Item(hook) => run_item_hook(hook, item_vars(event.id(), collection, summary)),
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

    let mut vars = BTreeMap::new();
    vars.insert("id", id.to_string());
    vars.insert("collection", collection.to_string());
    vars.insert("flag", flag.to_string());

    fire(hook.notify.as_ref(), hook.cmd.as_ref(), &vars);
}

/// The variables an item-level hook templates against. The envelope
/// ones are present only for a resolved arrival.
fn item_vars(
    id: &str,
    collection: &str,
    summary: Option<&ItemSummary>,
) -> BTreeMap<&'static str, String> {
    let mut vars = BTreeMap::new();
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
