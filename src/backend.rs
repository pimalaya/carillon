//! The source a watch connects to.
//!
//! A backend describes what to watch as plain connection parameters.
//! Credentials and consumer fan-out live elsewhere: this is only the
//! address.

/// Connection parameters for an IMAP mailbox watch.
pub struct CarillonImapBackend {
    /// IMAP server host.
    pub host: String,
    /// IMAP server port.
    pub port: u16,
    /// Login, the authentication identity.
    pub login: String,
    /// Mailbox to watch.
    pub mailbox: String,
}
