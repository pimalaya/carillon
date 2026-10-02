//! # Wizard
//!
//! The account generator: what a first configuration is discovered from,
//! prompted for and tested with, one module per backend.

pub mod configure;
#[cfg(feature = "dav")]
pub mod dav;
pub mod discover;
#[cfg(feature = "gcal")]
pub mod gcal;
#[cfg(feature = "gmail")]
pub mod gmail;
#[cfg(feature = "gpeople")]
pub mod gpeople;
#[cfg(feature = "imap")]
pub mod imap;
#[cfg(feature = "jmap")]
pub mod jmap;
#[cfg(feature = "maildir")]
pub mod local;
#[cfg(feature = "msgraph")]
pub mod msgraph;
#[cfg(network)]
pub mod search;
#[cfg(network)]
pub mod secret;
