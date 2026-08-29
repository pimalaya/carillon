//! # Event
//!
//! The change vocabulary every backend speaks, so that one hook runner
//! serves them all.
//!
//! What a change is about travels with it as a [`WatchDomain`], which is
//! what lets a hook be named after a message, a card, an event or a task
//! while the runner below stays one shape.
//!
//! Not every backend reports every kind of change, which is the protocol
//! talking rather than a gap: mail is immutable, so nothing mail reports
//! an edit, and a WebDAV poll reads etags, so flags are unknown to it
//! rather than empty.

/// What a change is about, which is the noun its hook is named after.
///
/// A backend fills it from what it holds: mail is always a message, a
/// CardDAV member a card, and a CalDAV member an event or a task.
// NOTE: which domains can be constructed depends on the backends
// compiled in, so a reduced feature set leaves some unused.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatchDomain {
    /// A mail message, whichever of IMAP, JMAP and Maildir carries it.
    Message,
    /// A vCard in a CardDAV addressbook.
    Card,
    /// A VEVENT in a CalDAV calendar.
    Event,
    /// A VTODO in a CalDAV calendar.
    Task,
}

/// A change in a watched collection, keyed by the backend's own id.
// NOTE: same reason as above, for the variants.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WatchEvent {
    /// An item appeared in the collection.
    ItemAdded {
        /// What the item is, which names the hook that fires.
        domain: WatchDomain,
        /// The backend's id: an IMAP UID, a JMAP `Email` id, a Maildir
        /// file name, a WebDAV href.
        id: String,
    },
    /// An item left the collection, deleted or moved away.
    ItemRemoved {
        /// What the item was, remembered from when it was still there.
        domain: WatchDomain,
        /// The backend's id for the item.
        id: String,
    },
    /// An item's content changed where it stands.
    ///
    /// Only a backend holding mutable items reports this: a message is
    /// immutable, so IMAP, JMAP and Maildir never do, while a WebDAV
    /// card or event is edited in place and its etag moves.
    ItemChanged {
        /// What the item is.
        domain: WatchDomain,
        /// The backend's id for the item.
        id: String,
    },
    /// One flag was set on an item.
    ///
    /// A delta setting several flags reports one event each, so that a
    /// hook always knows which flag it fired for. Only a backend that
    /// has flags reports this, which WebDAV does not.
    FlagAdded {
        /// What the item is, which for a flag is always a message.
        domain: WatchDomain,
        /// The backend's id for the item.
        id: String,
        /// The flag that appeared, under its shared name.
        flag: String,
    },
    /// One flag was cleared on an item.
    FlagRemoved {
        /// What the item is.
        domain: WatchDomain,
        /// The backend's id for the item.
        id: String,
        /// The flag that disappeared, under its shared name.
        flag: String,
    },
}

impl WatchEvent {
    /// The backend's id for the item this change is about.
    pub fn id(&self) -> &str {
        match self {
            Self::ItemAdded { id, .. }
            | Self::ItemRemoved { id, .. }
            | Self::ItemChanged { id, .. }
            | Self::FlagAdded { id, .. }
            | Self::FlagRemoved { id, .. } => id,
        }
    }
}

/// What a hook can say about an item, once resolved.
///
/// The per-kind summary pimdir calls `meta`: mail fills the envelope
/// fields, another kind would fill its own. Every field is optional,
/// and nothing is resolved unless a hook asks.
#[derive(Clone, Debug, Default)]
pub struct ItemSummary {
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
