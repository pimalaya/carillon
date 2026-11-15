//! # Stream
//!
//! What `carillon watch` prints: one event per thing a watch observed,
//! while it runs, then one result once it stopped, as every long-lived
//! pimalaya command does. The account threads hand their events to the
//! command's own thread through a channel, which alone holds the printer,
//! so the lines never interleave.

use std::{fmt, sync::mpsc::Sender};

use anyhow::Error;
use schemars::JsonSchema;
use serde::Serialize;

#[cfg(alarm)]
use crate::alarm::AlarmFiring;
use crate::{
    config::Hook,
    error::{self, ErrorCode},
    event::{ItemSummary, WatchEvent},
    hook::{self, HookCollection, HookFailure, HookReaction},
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
        write!(f, "{} `{}`", self.name, self.value)
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
        /// When it fired, RFC 3339 in the system's zone.
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
                write!(f, "[{account}] session lost")?;
                if let Some(code) = code {
                    write!(f, " ({code})")?;
                }
                writeln!(f, ": {}", chain(error, sources))
            }
            Self::Changed {
                account,
                collection,
                change,
            } => writeln!(f, "[{account}] {collection}: {change}"),
            Self::HookFailed {
                account,
                reaction,
                code,
                error,
                sources,
            } => {
                let reaction = match reaction {
                    HookReaction::Notify => "notify",
                    HookReaction::Command => "command",
                };
                write!(f, "[{account}] {reaction} hook failed")?;
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
                let what = summary.as_deref().unwrap_or(uid);
                writeln!(
                    f,
                    "[{account}] {collection}: alarm of {kind} {what} at {trigger}"
                )
            }
        }
    }
}

/// An error and its causes on one line, as `{err:#}` renders them.
fn chain(error: &str, sources: &[String]) -> String {
    let mut line = error.to_owned();
    for source in sources {
        line.push_str(": ");
        line.push_str(source);
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
}

impl WatchEvents {
    /// The sender of `account`'s events.
    pub fn new(account: impl Into<String>, sender: Sender<WatchStreamEvent>) -> Self {
        Self {
            account: account.into(),
            sender,
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
    /// configured for it, reporting the reactions that failed.
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
            self.failed(hook::run(hook, change, collection, summary));
        }
    }

    /// Reports the alarm `firing` of `collection`, its hook having fired
    /// with the reactions `failures` failed.
    #[cfg(alarm)]
    pub fn alarm(
        &self,
        collection: HookCollection<'_>,
        firing: &AlarmFiring,
        failures: Vec<HookFailure>,
    ) {
        let zone = jiff::tz::TimeZone::system();

        self.send(WatchStreamEvent::AlarmFired {
            account: self.account.clone(),
            collection: collection.into(),
            id: firing.id.clone(),
            uid: firing.uid.clone(),
            kind: firing.kind.as_str(),
            summary: firing.summary.clone(),
            action: firing.action.as_str(),
            trigger: hook::rfc3339(firing.trigger, &zone),
        });

        self.failed(failures);
    }

    /// Reports each reaction of a hook that failed.
    fn failed(&self, failures: Vec<HookFailure>) {
        for failure in failures {
            let (code, error, sources) = parts(&failure.error);

            self.send(WatchStreamEvent::HookFailed {
                account: self.account.clone(),
                reaction: failure.reaction,
                code,
                error,
                sources,
            });
        }
    }
}
