//! # Stream
//!
//! What `carillon watch` prints: one event per thing a watch observed,
//! while it runs, then one result once it stopped, as every long-lived
//! pimalaya command does. The account threads hand their events to the
//! command's own thread through a channel, which alone holds the printer,
//! so the lines never interleave.

use std::{fmt, sync::mpsc::Sender};

use anyhow::Error;
#[cfg(alarm)]
use jiff::Timestamp;
use pimalaya_cli::sanitize;
use schemars::JsonSchema;
use serde::Serialize;

#[cfg(alarm)]
use crate::alarm::AlarmFiring;
use crate::{
    config::Hook,
    error::{self, ErrorCode},
    event::{ItemSummary, WatchEvent},
    hook::{self, HookCollection, HookFailure, HookFired, HookReaction, command::HookCommands},
};

/// What `watch` prints once it stopped.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WatchOutput {
    /// What stopped the watch.
    pub stopped_by: WatchStop,
}

/// What stopped a watch.
#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum WatchStop {
    /// A termination signal: SIGINT (Ctrl+C), SIGTERM or SIGHUP.
    Signal,
}

impl fmt::Display for WatchOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.stopped_by {
            WatchStop::Signal => writeln!(f, "Watch stopped by a signal"),
        }
    }
}

/// A collection as an event names it: what its backend calls it, and
/// which one it is.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CollectionOutput {
    /// The word the backend names its collection with (`mailbox`,
    /// `calendar`, `addressbook`).
    pub name: &'static str,
    /// The collection, as the account configured it.
    pub value: String,
}

impl From<HookCollection<'_>> for CollectionOutput {
    fn from(collection: HookCollection<'_>) -> Self {
        Self {
            name: collection.name,
            value: collection.value.to_owned(),
        }
    }
}

impl fmt::Display for CollectionOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} `{}`", self.name, sanitize::text(&self.value))
    }
}

/// How a session learns about changes.
// NOTE: declared in every build, so the schema is one: a build leaving out
// the backends of a method leaves it unused.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum WatchMethod {
    /// IMAP IDLE.
    Idle,
    /// A poll every interval.
    Poll,
    /// A JMAP event stream.
    Push,
}

/// One thing a watch observed, tagged by `event`.
///
/// Its text is one line whatever its strings hold: what a server or a
/// store sent (an id, a flag, a summary, an error) and what the
/// configuration names (an account, a collection) pass through
/// pimalaya-cli's `sanitize::text`, as the family's text listings do.
// NOTE: declared in every build, so the schema is one: a build leaving out
// the calendar backends never fires an alarm.
#[allow(dead_code)]
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(tag = "event", rename_all = "kebab-case")]
pub enum WatchStreamEvent {
    /// A session of an account is established and about to wait for
    /// changes: connected, authenticated and its collections read.
    #[serde(rename_all = "camelCase")]
    SessionOpened {
        /// The account.
        account: String,
        /// The backend the session runs over, as configured.
        backend: &'static str,
        /// How it learns about changes.
        method: WatchMethod,
        /// The collections it watches.
        collections: Vec<CollectionOutput>,
    },
    /// A session of an account ended or could not open; it is reopened
    /// after a backoff.
    #[serde(rename_all = "camelCase")]
    SessionLost {
        /// The account.
        account: String,
        /// The stable code of the failure, `null` when it carries none.
        code: Option<ErrorCode>,
        /// The outermost error message.
        error: String,
        /// The messages of the causes, outermost first.
        sources: Vec<String>,
    },
    /// A change in a watched collection, as its hook receives it.
    #[serde(rename_all = "camelCase")]
    Changed {
        /// The account.
        account: String,
        /// The collection the change is in.
        collection: CollectionOutput,
        /// The change.
        change: WatchEvent,
    },
    /// A reaction of a hook failed; the watch goes on.
    #[serde(rename_all = "camelCase")]
    HookFailed {
        /// The account.
        account: String,
        /// The hook, as its table is named (`on-message-add`,
        /// `on-alarm-trigger`).
        hook: String,
        /// The backend's id of the item it fired for.
        id: String,
        /// The reaction that failed.
        reaction: HookReaction,
        /// The stable code of the failure, `null` when it carries none.
        code: Option<ErrorCode>,
        /// The outermost error message.
        error: String,
        /// The messages of the causes, outermost first.
        sources: Vec<String>,
    },
    /// An alarm of a watched calendar reached its trigger, and its hook
    /// fired.
    #[serde(rename_all = "camelCase")]
    AlarmFired {
        /// The account.
        account: String,
        /// The calendar holding the alarm.
        collection: CollectionOutput,
        /// The backend's id of the item holding the alarm.
        id: String,
        /// The `UID` of the event or task.
        uid: String,
        /// `event` or `task`.
        kind: &'static str,
        /// The `SUMMARY` of the component, `null` when it has none.
        summary: Option<String>,
        /// `display` or `audio`.
        action: &'static str,
        /// When it fired, RFC 3339 in UTC.
        trigger: String,
    },
}

impl fmt::Display for WatchStreamEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SessionOpened {
                account,
                backend,
                method,
                collections,
            } => {
                let account = sanitize::text(account);
                let collections: Vec<_> = collections.iter().map(ToString::to_string).collect();
                let method = match method {
                    WatchMethod::Idle => "idling",
                    WatchMethod::Poll => "polling",
                    WatchMethod::Push => "pushed",
                };
                let collections = collections.join(", ");
                writeln!(
                    f,
                    "[{account}] watching {collections} over {backend}, {method}"
                )
            }
            Self::SessionLost {
                account,
                code,
                error,
                sources,
            } => {
                write!(f, "[{}] session lost", sanitize::text(account))?;
                if let Some(code) = code {
                    write!(f, " ({code})")?;
                }
                writeln!(f, ": {}", chain(error, sources))
            }
            Self::Changed {
                account,
                collection,
                change,
            } => writeln!(f, "[{}] {collection}: {change}", sanitize::text(account)),
            Self::HookFailed {
                account,
                hook,
                id,
                reaction,
                code,
                error,
                sources,
            } => {
                let reaction = match reaction {
                    HookReaction::Notify => "notify",
                    HookReaction::Command => "command",
                };
                let account = sanitize::text(account);
                let id = sanitize::text(id);
                write!(f, "[{account}] {hook} {reaction} failed for {id}")?;
                if let Some(code) = code {
                    write!(f, " ({code})")?;
                }
                writeln!(f, ": {}", chain(error, sources))
            }
            Self::AlarmFired {
                account,
                collection,
                kind,
                uid,
                summary,
                trigger,
                ..
            } => {
                let account = sanitize::text(account);
                let what = sanitize::text(summary.as_deref().unwrap_or(uid));
                writeln!(
                    f,
                    "[{account}] {collection}: alarm of {kind} {what} at {trigger}"
                )
            }
        }
    }
}

/// An error and its causes on one line, as `{err:#}` renders them, a
/// server's text sanitized.
fn chain(error: &str, sources: &[String]) -> String {
    let mut line = sanitize::text(error).into_owned();
    for source in sources {
        line.push_str(": ");
        line.push_str(&sanitize::text(source));
    }
    line
}

/// The code, the message and the causes of `err`.
fn parts(err: &Error) -> (Option<ErrorCode>, String, Vec<String>) {
    let sources = err.chain().skip(1).map(ToString::to_string).collect();
    (error::code(err), err.to_string(), sources)
}

/// Where the threads of one account send what they observed.
///
/// A send to a command that stopped reading is dropped: it is shutting
/// down, and the thread finds out from the shutdown flag.
#[derive(Clone)]
pub struct WatchEvents {
    account: String,
    sender: Sender<WatchStreamEvent>,
    commands: HookCommands,
}

impl WatchEvents {
    /// The sender of `account`'s events, its hook commands run by
    /// `commands`.
    pub fn new(
        account: impl Into<String>,
        sender: Sender<WatchStreamEvent>,
        commands: HookCommands,
    ) -> Self {
        Self {
            account: account.into(),
            sender,
            commands,
        }
    }

    fn send(&self, event: WatchStreamEvent) {
        let _ = self.sender.send(event);
    }

    /// A session over `backend` is established and watches `collections`.
    pub fn opened(
        &self,
        backend: &'static str,
        method: WatchMethod,
        collections: Vec<HookCollection<'_>>,
    ) {
        self.send(WatchStreamEvent::SessionOpened {
            account: self.account.clone(),
            backend,
            method,
            collections: collections.into_iter().map(Into::into).collect(),
        });
    }

    /// A session ended on `err`.
    pub fn lost(&self, err: &Error) {
        let (code, error, sources) = parts(err);

        self.send(WatchStreamEvent::SessionLost {
            account: self.account.clone(),
            code,
            error,
            sources,
        });
    }

    /// Reports `change` in `collection`, then fires `hook` when one is
    /// configured for it, reporting the reactions that failed and handing
    /// its command to the commands.
    pub fn react(
        &self,
        hook: Option<Hook<'_>>,
        change: &WatchEvent,
        collection: HookCollection<'_>,
        summary: Option<&ItemSummary>,
    ) {
        self.send(WatchStreamEvent::Changed {
            account: self.account.clone(),
            collection: collection.into(),
            change: change.clone(),
        });

        if let Some(hook) = hook {
            let fired = hook::run(hook, change, collection, summary);
            self.fired(change.hook(), change.id(), fired);
        }
    }

    /// Reports the alarm `firing` of `collection`, then what its hook
    /// `fired`.
    #[cfg(alarm)]
    pub fn alarm(&self, collection: HookCollection<'_>, firing: &AlarmFiring, fired: HookFired) {
        let trigger = Timestamp::from_second(firing.trigger)
            .map(|at| at.strftime("%Y-%m-%dT%H:%M:%SZ").to_string())
            .unwrap_or_default();

        self.send(WatchStreamEvent::AlarmFired {
            account: self.account.clone(),
            collection: collection.into(),
            id: firing.id.clone(),
            uid: firing.uid.clone(),
            kind: firing.kind.as_str(),
            summary: firing.summary.clone(),
            action: firing.action.as_str(),
            trigger,
        });

        self.fired(String::from("on-alarm-trigger"), &firing.id, fired);
    }

    /// Reports the reactions the hook `hook` fired for the item `id` failed
    /// at once, and runs its command, one at a time per command of the
    /// account, reporting its failure.
    fn fired(&self, hook: String, id: &str, fired: HookFired) {
        self.failed(&hook, id, fired.failures);

        let Some(job) = fired.command else {
            return;
        };

        let key = format!("{}\n{:?}", self.account, job.command);
        let events = self.clone();
        let id = id.to_owned();

        self.commands.submit(key, job, move |error| {
            let reaction = HookReaction::Command;
            events.failed(&hook, &id, vec![HookFailure { reaction, error }]);
        });
    }

    /// Reports each reaction of the hook `hook` fired for the item `id`
    /// that failed.
    fn failed(&self, hook: &str, id: &str, failures: Vec<HookFailure>) {
        for failure in failures {
            let (code, error, sources) = parts(&failure.error);

            self.send(WatchStreamEvent::HookFailed {
                account: self.account.clone(),
                hook: hook.to_owned(),
                id: id.to_owned(),
                reaction: failure.reaction,
                code,
                error,
                sources,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        event::{WatchDomain, WatchEvent},
        stream::*,
    };

    /// An alarm prints when it fired as an instant in UTC, whatever the
    /// system's zone, and a hook failing for it names the hook and the
    /// item.
    #[cfg(alarm)]
    #[test]
    fn an_alarm_fires_at_its_utc_instant() {
        use std::sync::mpsc::channel;

        use crate::{
            alarm::{AlarmAction, AlarmFiring, AlarmKind},
            hook::{HookCollection, HookFailure, HookFired, HookReaction, command::HookCommands},
        };

        let (sender, receiver) = channel();
        let events = WatchEvents::new("perso", sender, HookCommands::start().unwrap());
        let collection = HookCollection {
            name: "calendar",
            value: "work",
        };
        let firing = AlarmFiring {
            id: String::from("a.ics"),
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
            trigger: 1_763_624_700,
            at: None,
        };
        let fired = HookFired {
            failures: vec![HookFailure {
                reaction: HookReaction::Notify,
                error: anyhow::anyhow!("No notification daemon"),
            }],
            command: None,
        };

        events.alarm(collection, &firing, fired);

        let WatchStreamEvent::AlarmFired { trigger, .. } = receiver.recv().unwrap() else {
            panic!("expected alarm-fired");
        };
        assert_eq!(trigger, "2025-11-20T07:45:00Z");

        let WatchStreamEvent::HookFailed { hook, id, .. } = receiver.recv().unwrap() else {
            panic!("expected hook-failed");
        };
        assert_eq!(hook, "on-alarm-trigger");
        assert_eq!(id, "a.ics");
    }

    /// What a server or a store sent cannot drive the terminal nor break
    /// the line of a text event: its control and bidi characters print as
    /// U+FFFD, as `sanitize::text` renders them.
    #[test]
    fn a_text_event_sanitizes_what_it_was_sent() {
        let changed = WatchStreamEvent::Changed {
            account: String::from("per\x1bso"),
            collection: CollectionOutput {
                name: "mailbox",
                value: String::from("IN\x1b[2JBOX"),
            },
            change: WatchEvent::FlagAdded {
                domain: WatchDomain::Message,
                id: String::from("4\n2"),
                flag: String::from("Se\u{202e}en"),
            },
        };
        assert_eq!(
            "[per\u{fffd}so] mailbox `IN\u{fffd}[2JBOX`: flag Se\u{fffd}en added on message 4\u{fffd}2\n",
            changed.to_string()
        );

        let fired = WatchStreamEvent::AlarmFired {
            account: String::from("perso"),
            collection: CollectionOutput {
                name: "calendar",
                value: String::from("work"),
            },
            id: String::from("a.ics"),
            uid: String::from("standup"),
            kind: "event",
            summary: Some(String::from("Stand\x07up\nrm -rf ~")),
            action: "display",
            trigger: String::from("2026-11-15T08:45:00+01:00"),
        };
        assert_eq!(
            "[perso] calendar `work`: alarm of event Stand\u{fffd}up\u{fffd}rm -rf ~ at 2026-11-15T08:45:00+01:00\n",
            fired.to_string()
        );

        let lost = WatchStreamEvent::SessionLost {
            account: String::from("perso"),
            code: None,
            error: String::from("imap watch failed"),
            sources: vec![String::from("NO \x1b]52;c;cm0=\x07")],
        };
        assert_eq!(
            "[perso] session lost: imap watch failed: NO \u{fffd}]52;c;cm0=\u{fffd}\n",
            lost.to_string()
        );
    }
}
