//! The change signal a watch surfaces.
//!
//! A watch says only that something changed at an address, never what
//! changed. The signal carries no sender, subject, body, UID or change
//! kind. Enriching it is a consumer's job, since the consumer holds the
//! credentials and can go look. This mirrors JMAP's StateChange: enough to
//! address and dedup, and nothing more.
//!
//! The daemon fires its hooks from the parts it already holds: the
//! account, the watched mailbox, the source below, and the opaque state
//! token the coroutine surfaces. A hosted server would add an id and a
//! timestamp so a delivery can be signed and deduplicated; a desktop
//! notification needs neither.

/// Which kind of source rang the doorbell.
///
/// Tags a ring with its origin protocol so a consumer can route without
/// parsing the target string.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CarillonSource {
    /// An IMAP mailbox watched over a held IDLE connection.
    Imap,
}

impl CarillonSource {
    /// The lowercase wire string a consumer sees, as `CARILLON_SOURCE`.
    pub fn as_str(&self) -> &'static str {
        match self {
            CarillonSource::Imap => "imap",
        }
    }
}
