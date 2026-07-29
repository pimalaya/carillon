//! Hook runner: an enriched desktop notification and a raw shell command.
//!
//! The notify consumer is terminal, so it is enriched locally (see
//! [`crate::resolve`]): the daemon fires one notification per newly-arrived
//! message, expanding its sender, subject, and date into the notification
//! templates. The cmd consumer is deliberately not enriched — its role is to
//! kick a sync tool, which re-derives everything itself — so it only ever
//! sees the raw content-free event as `CARILLON_*` environment variables. A
//! failure is logged, never fatal, so one hook never stalls the watch.

use std::collections::HashMap;

use anyhow::{Context, Result};
use log::warn;
use notify_rust::Notification;

use crate::{
    config::{HookCmd, HookConfig, NotifyConfig},
    resolve::CarillonImapMessage,
};

/// The content-free fields every hook can act on.
pub struct HookContext<'a> {
    pub account: &'a str,
    pub mailbox: &'a str,
    pub source: &'a str,
    pub state: &'a str,
}

/// Fires a hook's notification (once per new message) and command, logging
/// rather than propagating any failure. `new_messages` is what the local
/// resolver found; it is empty when the change carried no new mail or when
/// the watch has no notify hook.
pub fn run(hook: &HookConfig, ctx: &HookContext<'_>, new_messages: &[CarillonImapMessage]) {
    if let Some(notify) = &hook.notify {
        for message in new_messages {
            let vars = notify_vars(ctx, message, new_messages.len());
            if let Err(err) = fire_notification(notify, &vars) {
                warn!("notify hook failed: {err:#}");
            }
        }
    }
    if let Some(cmd) = &hook.cmd
        && let Err(err) = run_command(cmd, ctx)
    {
        warn!("cmd hook failed: {err:#}");
    }
}

/// Builds the substitution variables for one new message's notification.
/// Every documented key is always present (empty when the server omitted it)
/// so a template never fails on an undefined variable.
fn notify_vars(
    ctx: &HookContext<'_>,
    message: &CarillonImapMessage,
    count: usize,
) -> HashMap<String, String> {
    let from_name = message.from_name.clone().unwrap_or_default();
    let from_addr = message.from_addr.clone().unwrap_or_default();
    let from = message
        .from_name
        .clone()
        .or_else(|| message.from_addr.clone())
        .unwrap_or_default();
    HashMap::from([
        (String::from("account"), ctx.account.to_string()),
        (String::from("mailbox"), ctx.mailbox.to_string()),
        (String::from("source"), ctx.source.to_string()),
        (String::from("state"), ctx.state.to_string()),
        (String::from("from"), from),
        (String::from("from_name"), from_name),
        (String::from("from_addr"), from_addr),
        (
            String::from("subject"),
            message.subject.clone().unwrap_or_default(),
        ),
        (
            String::from("date"),
            message.date.clone().unwrap_or_default(),
        ),
        (String::from("count"), count.to_string()),
    ])
}

/// Shows a desktop notification with its templates expanded.
fn fire_notification(notify: &NotifyConfig, vars: &HashMap<String, String>) -> Result<()> {
    let summary = subst::substitute(&notify.summary, vars).context("cannot expand summary")?;
    let body = subst::substitute(&notify.body, vars).context("cannot expand body")?;
    Notification::new()
        .summary(&summary)
        .body(&body)
        .show()
        .context("cannot show desktop notification")?;
    Ok(())
}

/// Runs the shell command with the raw change fields in the environment.
fn run_command(cmd: &HookCmd, ctx: &HookContext<'_>) -> Result<()> {
    let mut command = cmd.clone().0;
    command
        .env("CARILLON_ACCOUNT", ctx.account)
        .env("CARILLON_MAILBOX", ctx.mailbox)
        .env("CARILLON_SOURCE", ctx.source)
        .env("CARILLON_STATE", ctx.state);
    let status = command.status().context("cannot spawn hook command")?;
    if !status.success() {
        anyhow::bail!("hook command exited with {status}");
    }
    Ok(())
}
