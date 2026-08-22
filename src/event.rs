//! The change vocabulary every backend speaks.
//!
//! A watch reports what changed in a watched mailbox, keyed by the
//! backend's own message id: a message arrived, one left, flags were
//! set or cleared. Nothing here is protocol-specific, so a hook is
//! written once and fires the same way whether the change came from an
//! IMAP IDLE, a JMAP push or a Maildir poll.
//!
//! An arrival carries no envelope: an IMAP watch learns of a new UID
//! without its subject, and reading one costs a fetch. The watcher
//! reports the arrival, and [`crate::resolve`] fills the envelope in
//! only when a hook asks for it.

use std::collections::BTreeSet;

/// A change in a watched mailbox, keyed by the backend's message id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WatchEvent {
    /// A message arrived.
    MessageAdded {
        /// The backend's id for the message: an IMAP UID, a JMAP
        /// `Email` id, a Maildir file name.
        id: String,
    },
    /// A message left the mailbox, expunged or moved away.
    MessageRemoved {
        /// The backend's id for the message.
        id: String,
    },
    /// Flags were set on a message.
    FlagsAdded {
        /// The backend's id for the message.
        id: String,
        /// The flags that appeared, as the backend spells them.
        flags: BTreeSet<String>,
    },
    /// Flags were cleared on a message.
    FlagsRemoved {
        /// The backend's id for the message.
        id: String,
        /// The flags that disappeared, as the backend spells them.
        flags: BTreeSet<String>,
    },
}

/// What a hook can say about a newly-arrived message, once resolved.
///
/// Every field is optional: a server may omit any envelope part, and
/// resolving is skipped entirely when no hook asks for it.
#[derive(Clone, Debug, Default)]
pub struct MessageSummary {
    /// The sender's personal name.
    pub from_name: Option<String>,
    /// The sender's `mailbox@host` address.
    pub from_addr: Option<String>,
    /// The first recipient's personal name.
    pub to_name: Option<String>,
    /// The first recipient's `mailbox@host` address.
    pub to_addr: Option<String>,
    /// The message subject.
    pub subject: Option<String>,
    /// The message date, as the backend rendered it.
    pub date: Option<String>,
}
