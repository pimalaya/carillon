//! Hook runner: fires the desktop notification and the shell command a
//! change is configured to trigger.
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
    config::{FlagsHook, HookCmd, HooksConfig, ItemHook, NotifyConfig},
    event::{MessageSummary, WatchEvent},
};

/// Fires whichever hook `event` calls for, with `summary` filled in
/// when an arrival was resolved.
pub fn run(
    hooks: &HooksConfig,
    event: &WatchEvent,
    mailbox: &str,
    summary: Option<&MessageSummary>,
) {
    trace!("dispatch event: {event:?}");

    match event {
        WatchEvent::ItemAdded { id } => {
            let Some(hook) = &hooks.on_item_added else {
                return;
            };
            run_item_hook(hook, item_vars(id, mailbox, summary));
        }
        WatchEvent::ItemRemoved { id } => {
            let Some(hook) = &hooks.on_item_removed else {
                return;
            };
            run_item_hook(hook, item_vars(id, mailbox, None));
        }
        WatchEvent::ItemChanged { id } => {
            let Some(hook) = &hooks.on_item_changed else {
                return;
            };
            run_item_hook(hook, item_vars(id, mailbox, None));
        }
        WatchEvent::FlagsAdded { id, flags } => {
            let Some(hook) = &hooks.on_flags_added else {
                return;
            };
            run_flags_hook(hook, id, mailbox, flags);
        }
        WatchEvent::FlagsRemoved { id, flags } => {
            let Some(hook) = &hooks.on_flags_removed else {
                return;
            };
            run_flags_hook(hook, id, mailbox, flags);
        }
    }
}

/// Fires an item-level hook.
fn run_item_hook(hook: &ItemHook, vars: BTreeMap<&'static str, String>) {
    fire(hook.notify.as_ref(), hook.cmd.as_ref(), &vars);
}

/// Fires a flag-level hook, honouring its optional flag filter.
fn run_flags_hook(
    hook: &FlagsHook,
    id: &str,
    mailbox: &str,
    flags: &std::collections::BTreeSet<String>,
) {
    if !hook.flags.is_empty() && !flags.iter().any(|flag| matches_filter(&hook.flags, flag)) {
        trace!("flags hook skipped: no matching flag in delta");
        return;
    }

    let mut vars = BTreeMap::new();
    vars.insert("id", id.to_string());
    vars.insert("mailbox", mailbox.to_string());
    vars.insert("flags", flags.iter().cloned().collect::<Vec<_>>().join(","));

    if let Some(first) = flags.iter().next() {
        vars.insert("flag", first.clone());
    }

    fire(hook.notify.as_ref(), hook.cmd.as_ref(), &vars);
}

/// The variables an item-level hook templates against. The envelope
/// ones are present only for a resolved arrival.
fn item_vars(
    id: &str,
    mailbox: &str,
    summary: Option<&MessageSummary>,
) -> BTreeMap<&'static str, String> {
    let mut vars = BTreeMap::new();
    vars.insert("id", id.to_string());
    vars.insert("mailbox", mailbox.to_string());

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
fn matches_filter(filter: &std::collections::BTreeSet<String>, flag: &str) -> bool {
    let stripped = flag.trim_start_matches(['\\', '$']);

    filter
        .iter()
        .any(|name| name.eq_ignore_ascii_case(flag) || name.eq_ignore_ascii_case(stripped))
}
