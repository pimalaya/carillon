//! # Wizard
//!
//! The account generator: what a first configuration is discovered from,
//! prompted for and tested with, one module per backend.

pub mod configure;
#[cfg(feature = "dav")]
pub mod dav;
pub mod discover;
#[cfg(feature = "imap")]
pub mod imap;
#[cfg(feature = "jmap")]
pub mod jmap;
#[cfg(feature = "maildir")]
pub mod local;
#[cfg(any(feature = "imap", feature = "jmap", feature = "dav"))]
pub mod search;
#[cfg(any(feature = "imap", feature = "jmap", feature = "dav"))]
pub mod secret;
