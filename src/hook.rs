//! Hook runner: a content-free desktop notification and shell command.
//!
//! The ring carries no message content, so a hook only ever knows the
//! account, mailbox, source, and opaque state. Those are substituted into
//! notification templates (shell-style `$name` / `${name}`) and exported to
//! a command as `CARILLON_*` environment variables. A failure is logged,
//! never fatal, so one hook never stalls the watch.

use std::collections::HashMap;

use anyhow::{Context, Result};
use log::warn;
use notify_rust::Notification;

use crate::config::{HookCmd, HookConfig, NotifyConfig};

/// The content-free fields a hook can act on.
pub struct HookContext<'a> {
    pub account: &'a str,
    pub mailbox: &'a str,
    pub source: &'a str,
    pub state: &'a str,
}

impl HookContext<'_> {
    fn vars(&self) -> HashMap<String, String> {
        HashMap::from([
            (String::from("account"), self.account.to_string()),
            (String::from("mailbox"), self.mailbox.to_string()),
            (String::from("source"), self.source.to_string()),
            (String::from("state"), self.state.to_string()),
        ])
    }
}

/// Fires a hook's notification and command, logging rather than
/// propagating any failure.
pub fn run(hook: &HookConfig, ctx: &HookContext<'_>) {
    let vars = ctx.vars();
    if let Some(notify) = &hook.notify
        && let Err(err) = fire_notification(notify, &vars)
    {
        warn!("notify hook failed: {err:#}");
    }
    if let Some(cmd) = &hook.cmd
        && let Err(err) = run_command(cmd, ctx)
    {
        warn!("cmd hook failed: {err:#}");
    }
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

/// Runs the shell command with the change fields in the environment.
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
