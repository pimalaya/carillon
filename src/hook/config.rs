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

//! Hook configuration: per-event-kind notification and shell-command
//! triggers. Each field of [`HooksConfig`] maps 1:1 to a
//! [`io_email::event::WatchEvent`] kind; flag hooks carry an optional
//! flag-name filter so users can wire (for example)
//! `[on-flags-added] flags = ["Seen"]` to fire only when `\Seen` lands.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// Per-account hook configuration: one optional hook per watch
/// event kind.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct HooksConfig {
    pub on_message_added: Option<MessageHook>,
    pub on_message_removed: Option<MessageHook>,
    pub on_flags_added: Option<FlagsHook>,
    pub on_flags_removed: Option<FlagsHook>,
}

/// Hook that fires for envelope-level events (added or removed).
/// Placeholders use shell-style `$name` / `${name}` syntax in both
/// the notification summary/body and the shell command (where the
/// shell itself does the expansion, so quote them as `"$subject"`).
/// Available names: `id`, `mailbox`, and (for `on-message-added`
/// only) `subject`, `sender`, `sender_name`, `sender_address`,
/// `recipient`, `recipient_name`, `recipient_address`.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct MessageHook {
    pub notify: Option<NotifyConfig>,
    pub cmd: Option<String>,
}

/// Hook that fires for flag-level events (added or removed). `flags`
/// optionally restricts firing to deltas whose IANA-classified flag
/// raw name matches one of the listed names (case-insensitive; both
/// `Seen` and `\Seen` work).
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct FlagsHook {
    pub notify: Option<NotifyConfig>,
    pub cmd: Option<String>,
    #[serde(default)]
    pub flags: BTreeSet<String>,
}

/// Desktop notification payload: a one-line summary and an optional
/// multi-line body.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct NotifyConfig {
    pub summary: String,
    #[serde(default)]
    pub body: String,
}
