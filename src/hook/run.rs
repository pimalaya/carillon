// This file is part of Mirador, a CLI to watch mailbox changes.
//
// Copyright (C) 2024-2026  soywod <pimalaya.org@posteo.net>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Hook runner: dispatches to `notify-rust` for system notifications
//! and to `sh -c` for shell commands. Notification summary/body
//! strings are expanded with [`subst`] (shell-style `$name` /
//! `${name}`); shell commands receive the same placeholders as
//! environment variables, so the shell itself does the expansion
//! (`"$subject"`). Failures are logged but never panic the watch
//! loop.

use std::{collections::BTreeMap, process::Command};

use anyhow::{Context, Result};
use io_email::{envelope::Envelope, flag::Flag};
use log::{trace, warn};
use notify_rust::Notification;

use crate::hook::config::{FlagsHook, MessageHook, NotifyConfig};

/// Runs an envelope-level hook. Failures are logged at `warn` so a
/// broken script never crashes the watcher.
pub fn run_message_hook(hook: &MessageHook, ctx: &MessageContext<'_>) {
    let vars = ctx.template_vars();
    if let Some(notify) = &hook.notify
        && let Err(err) = fire_notification(notify, &vars)
    {
        warn!("notify hook failed: {err}");
    }
    if let Some(cmd) = &hook.cmd
        && let Err(err) = run_command(cmd, &vars)
    {
        warn!("cmd hook failed: {err}");
    }
}

/// Runs a flag-level hook against the given delta, honouring the
/// optional `flags` filter on the hook config.
pub fn run_flags_hook(hook: &FlagsHook, ctx: &FlagsContext<'_>) {
    if !hook.flags.is_empty() && !ctx.flags.iter().any(|f| matches_filter(&hook.flags, f)) {
        trace!("flags hook skipped: no matching flag in delta");
        return;
    }

    let vars = ctx.template_vars();
    if let Some(notify) = &hook.notify
        && let Err(err) = fire_notification(notify, &vars)
    {
        warn!("notify hook failed: {err}");
    }
    if let Some(cmd) = &hook.cmd
        && let Err(err) = run_command(cmd, &vars)
    {
        warn!("cmd hook failed: {err}");
    }
}

pub struct MessageContext<'a> {
    pub mailbox: &'a str,
    pub id: &'a str,
    pub envelope: Option<&'a Envelope>,
}

pub struct FlagsContext<'a> {
    pub mailbox: &'a str,
    pub id: &'a str,
    pub flags: &'a std::collections::BTreeSet<Flag>,
}

impl<'a> MessageContext<'a> {
    fn template_vars(&self) -> BTreeMap<&'static str, String> {
        let mut vars = BTreeMap::new();
        vars.insert("id", self.id.to_string());
        vars.insert("mailbox", self.mailbox.to_string());

        let Some(env) = self.envelope else {
            return vars;
        };

        vars.insert("subject", env.subject.clone());

        if let Some(sender) = env.from.first() {
            let name = sender.name.clone().unwrap_or_default();
            let address = sender.email.clone();
            let combined = if name.is_empty() {
                address.clone()
            } else {
                format!("{name} <{address}>")
            };
            vars.insert("sender", combined);
            vars.insert("sender_name", name);
            vars.insert("sender_address", address);
        }

        if let Some(recipient) = env.to.first() {
            let name = recipient.name.clone().unwrap_or_default();
            let address = recipient.email.clone();
            let combined = if name.is_empty() {
                address.clone()
            } else {
                format!("{name} <{address}>")
            };
            vars.insert("recipient", combined);
            vars.insert("recipient_name", name);
            vars.insert("recipient_address", address);
        }

        vars
    }
}

impl<'a> FlagsContext<'a> {
    fn template_vars(&self) -> BTreeMap<&'static str, String> {
        let mut vars = BTreeMap::new();
        vars.insert("id", self.id.to_string());
        vars.insert("mailbox", self.mailbox.to_string());
        let names: Vec<String> = self.flags.iter().map(|f| f.raw().to_string()).collect();
        vars.insert("flags", names.join(","));
        if let Some(first) = self.flags.iter().next() {
            vars.insert("flag", first.raw().to_string());
        }
        vars
    }
}

fn matches_filter(filter: &std::collections::BTreeSet<String>, flag: &Flag) -> bool {
    let raw = flag.raw();
    let stripped = raw.trim_start_matches(['\\', '$']);
    filter
        .iter()
        .any(|f| f.eq_ignore_ascii_case(raw) || f.eq_ignore_ascii_case(stripped))
}

fn fire_notification(config: &NotifyConfig, vars: &BTreeMap<&'static str, String>) -> Result<()> {
    let summary = subst::substitute(&config.summary, vars).context("expand summary template")?;
    let body = subst::substitute(&config.body, vars).context("expand body template")?;
    let mut n = Notification::new();
    n.summary(&summary);
    if !body.is_empty() {
        n.body(&body);
    }
    n.show()?;
    Ok(())
}

/// Spawns `sh -c <cmd>` with the template vars exported as
/// environment variables; the shell's own parameter expansion does
/// the substitution (`"$subject"` etc.). POSIX guarantees the
/// expansion is single-pass and not re-parsed as shell syntax, so
/// values containing `$id`, `;`, backticks, etc. are safe as long
/// as the user quotes the reference.
fn run_command(cmd: &str, vars: &BTreeMap<&'static str, String>) -> Result<()> {
    let status = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .envs(vars.iter().map(|(k, v)| (*k, v.as_str())))
        .status()?;
    if !status.success() {
        warn!("cmd `{cmd}` exited with {status}");
    }
    Ok(())
}
