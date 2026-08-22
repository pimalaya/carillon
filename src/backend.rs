//! Backend selection for cross-protocol commands.
//!
//! Same shape as [himalaya CLI's `Backend`]: `Auto` picks the first
//! configured backend in a fixed priority order; the named variants
//! pin the active backend to that protocol and bail when the account
//! has no matching config block.
//!
//! CalDAV and CardDAV are two names rather than one, since what a
//! collection holds is what its events are called, and a backend that
//! names its domain can refuse the hooks of another.
//!
//! [himalaya CLI's `Backend`]: https://github.com/pimalaya/himalaya/blob/master/src/backend.rs

use std::{fmt, str::FromStr};

use anyhow::{Error, bail};
use clap::Parser;

/// Backend selector for the `-b/--backend` CLI flag.
#[derive(Clone, Copy, Debug, Default, Parser, PartialEq, Eq)]
pub enum Backend {
    /// First configured block wins (priority: IMAP, JMAP, Maildir,
    /// CalDAV, CardDAV).
    #[default]
    Auto,
    /// Force IMAP; bail when the account has no `imap` block.
    Imap,
    /// Force JMAP; bail when the account has no `jmap` block.
    Jmap,
    /// Force Maildir; bail when the account has no `maildir` block.
    Maildir,
    /// Force CalDAV; bail when the account has no `caldav` block.
    Caldav,
    /// Force CardDAV; bail when the account has no `carddav` block.
    Carddav,
}

#[allow(unused)]
impl Backend {
    pub fn allows_imap(self) -> bool {
        matches!(self, Self::Auto | Self::Imap)
    }

    pub fn allows_jmap(self) -> bool {
        matches!(self, Self::Auto | Self::Jmap)
    }

    pub fn allows_maildir(self) -> bool {
        matches!(self, Self::Auto | Self::Maildir)
    }

    pub fn allows_caldav(self) -> bool {
        matches!(self, Self::Auto | Self::Caldav)
    }

    pub fn allows_carddav(self) -> bool {
        matches!(self, Self::Auto | Self::Carddav)
    }
}

impl FromStr for Backend {
    type Err = Error;

    fn from_str(backend: &str) -> Result<Self, Self::Err> {
        match backend {
            "auto" => Ok(Self::Auto),
            "imap" => Ok(Self::Imap),
            "jmap" => Ok(Self::Jmap),
            "maildir" => Ok(Self::Maildir),
            "caldav" => Ok(Self::Caldav),
            "carddav" => Ok(Self::Carddav),
            backend => bail!("Invalid backend {backend}"),
        }
    }
}

impl fmt::Display for Backend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Auto => write!(f, "auto"),
            Self::Imap => write!(f, "imap"),
            Self::Jmap => write!(f, "jmap"),
            Self::Maildir => write!(f, "maildir"),
            Self::Caldav => write!(f, "caldav"),
            Self::Carddav => write!(f, "carddav"),
        }
    }
}
