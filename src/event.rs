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
//!
//! A change is also what `watch` prints, as the `change` of its `changed`
//! event, tagged by `kind`.

use std::fmt;

use pimalaya_cli::sanitize;
use schemars::JsonSchema;
use serde::Serialize;

/// What a change is about, which is the noun its hook is named after.
///
/// A backend fills it from what it holds: mail is always a message, a
/// CardDAV member or a JMAP `ContactCard` a card, a CalDAV member an event
/// or a task, and a JMAP `CalendarEvent` an event.
// NOTE: which domains can be constructed depends on the backends
// compiled in, so a reduced feature set leaves some unused.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum WatchDomain {
    /// A mail message, whichever of IMAP, JMAP and Maildir carries it.
    Message,
    /// A contact in an addressbook, over CardDAV or JMAP.
    Card,
    /// A calendar event, a CalDAV VEVENT or a JMAP `CalendarEvent`.
    Event,
    /// A VTODO in a CalDAV calendar.
    Task,
}

impl WatchDomain {
    /// What a collection of this domain is called, which is also the key
    /// a backend configures it under and the variable a hook names it by.
    // NOTE: only the backends serving several domains ask, which a
    // reduced feature set may not compile in.
    #[allow(dead_code)]
    pub const fn collection_name(self) -> &'static str {
        match self {
            Self::Message => "mailbox",
            Self::Card => "addressbook",
            Self::Event | Self::Task => "calendar",
        }
    }
}

/// A change in a watched collection, keyed by the backend's own id, as
/// a hook receives it and a watch prints it, tagged by `kind`.
// NOTE: same reason as above, for the variants.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum WatchEvent {
    /// An item appeared in the collection.
    ItemAdded {
        /// What the item is, which names the hook that fires.
        domain: WatchDomain,
        /// The backend's id: an IMAP UID, a JMAP object id, a Maildir
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
    /// immutable, so no mail backend does but Microsoft Graph, whose
    /// drafts are edited in place, while a card or an event is edited in
    /// place, its WebDAV etag, JMAP state or Graph `changeKey` moving.
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

    /// The name of the hook this change fires, `on-message-add` or
    /// `on-flag-remove`.
    pub fn hook(&self) -> String {
        match self {
            Self::ItemAdded { domain, .. } => format!("on-{domain}-add"),
            Self::ItemRemoved { domain, .. } => format!("on-{domain}-remove"),
            Self::ItemChanged { domain, .. } => format!("on-{domain}-change"),
            Self::FlagAdded { .. } => String::from("on-flag-add"),
            Self::FlagRemoved { .. } => String::from("on-flag-remove"),
        }
    }

    /// What the item this change is about is.
    // NOTE: only a backend watching several domains asks, which a reduced
    // feature set may not compile in.
    #[allow(dead_code)]
    pub fn domain(&self) -> WatchDomain {
        match self {
            Self::ItemAdded { domain, .. }
            | Self::ItemRemoved { domain, .. }
            | Self::ItemChanged { domain, .. }
            | Self::FlagAdded { domain, .. }
            | Self::FlagRemoved { domain, .. } => *domain,
        }
    }
}

/// The noun of the domain, as a hook is named after it.
impl fmt::Display for WatchDomain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Message => f.write_str("message"),
            Self::Card => f.write_str("card"),
            Self::Event => f.write_str("event"),
            Self::Task => f.write_str("task"),
        }
    }
}

/// The change in words: `message 42 added`, `flag Seen added on message
/// 42`, the id and the flag a server sent sanitized.
impl fmt::Display for WatchEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let id = sanitize::text(self.id());

        match self {
            Self::ItemAdded { domain, .. } => write!(f, "{domain} {id} added"),
            Self::ItemRemoved { domain, .. } => write!(f, "{domain} {id} removed"),
            Self::ItemChanged { domain, .. } => write!(f, "{domain} {id} changed"),
            Self::FlagAdded { domain, flag, .. } => {
                let flag = sanitize::text(flag);
                write!(f, "flag {flag} added on {domain} {id}")
            }
            Self::FlagRemoved { domain, flag, .. } => {
                let flag = sanitize::text(flag);
                write!(f, "flag {flag} removed from {domain} {id}")
            }
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
