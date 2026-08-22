//! The change vocabulary every backend speaks.
//!
//! A watch reports what changed in a watched collection, keyed by the
//! backend's own id: an item arrived, one left, one was edited, flags
//! moved. Nothing here is protocol-specific, so a hook is written once
//! and fires the same way whether the change came from an IMAP IDLE, a
//! JMAP poll, a Maildir listing or a WebDAV sync-collection.
//!
//! Not every backend can report every event, and that is a property of
//! the protocol rather than a gap: mail is immutable, so nothing mail
//! reports an edit, and WebDAV has no flags.
//!
//! An arrival carries no envelope: an IMAP watch learns of a new UID
//! without its subject, and reading one costs a fetch. The watcher
//! reports the arrival, and the backend fills the envelope in only
//! when a hook asks for it.

use std::collections::BTreeSet;

/// A change in a watched collection, keyed by the backend's own id.
///
/// The vocabulary is deliberately not mail-shaped: an item is a
/// message, a contact or a calendar event depending on the backend
/// reporting it, and a hook is written once for all of them.
// NOTE: which variants exist is the vocabulary's business; which of
// them can be constructed depends on the backends compiled in, so a
// reduced feature set leaves some unused by construction.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WatchEvent {
    /// An item appeared in the collection.
    ItemAdded {
        /// The backend's id: an IMAP UID, a JMAP `Email` id, a Maildir
        /// file name, a WebDAV href.
        id: String,
    },
    /// An item left the collection, deleted or moved away.
    ItemRemoved {
        /// The backend's id for the item.
        id: String,
    },
    /// An item's content changed where it stands.
    ///
    /// Only a backend holding mutable items reports this: a message is
    /// immutable, so IMAP, JMAP and Maildir never do, while a WebDAV
    /// contact or event is edited in place and its etag moves.
    ItemChanged {
        /// The backend's id for the item.
        id: String,
    },
    /// Flags were set on an item.
    ///
    /// Only a backend that has flags reports this, which WebDAV does
    /// not.
    FlagsAdded {
        /// The backend's id for the item.
        id: String,
        /// The flags that appeared, under their shared names.
        flags: BTreeSet<String>,
    },
    /// Flags were cleared on an item.
    FlagsRemoved {
        /// The backend's id for the item.
        id: String,
        /// The flags that disappeared, under their shared names.
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
