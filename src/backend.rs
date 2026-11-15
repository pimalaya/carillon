//! # Backend
//!
//! The backend selection cross-protocol commands share, in the shape
//! [himalaya CLI's `Backend`] has: `Auto` takes the first configured
//! block in a fixed priority order, a named variant pins that protocol
//! and bails when the account declares none.
//!
//! CalDAV and CardDAV are two names rather than one, since what a
//! collection holds is what its events are called, and a backend that
//! names its domain can refuse the hooks of another.
//!
//! [himalaya CLI's `Backend`]: https://github.com/pimalaya/himalaya/blob/master/src/backend.rs

use std::fmt;

use clap::ValueEnum;

/// Backend selector for the `-b/--backend` CLI flag, each variant one of
/// its possible values whatever the build: a backend the build leaves out
/// is still named, a command reaching it failing `backend-not-built`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum Backend {
    /// First configured block wins (priority: IMAP, JMAP, Maildir,
    /// CalDAV, CardDAV, Microsoft Graph, Gmail, Google Calendar, Google
    /// People, pimdir).
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
    /// Force Microsoft Graph; bail when the account has no `msgraph` block.
    Msgraph,
    /// Force Gmail; bail when the account has no `gmail` block.
    Gmail,
    /// Force Google Calendar; bail when the account has no `gcal` block.
    Gcal,
    /// Force Google People; bail when the account has no `gpeople` block.
    Gpeople,
    /// Force pimdir; bail when the account has no `pimdir` block.
    Pimdir,
}

#[allow(unused)]
impl Backend {
    /// Whether an `imap` block may be used.
    pub fn allows_imap(self) -> bool {
        matches!(self, Self::Auto | Self::Imap)
    }

    /// Whether a `jmap` block may be used.
    pub fn allows_jmap(self) -> bool {
        matches!(self, Self::Auto | Self::Jmap)
    }

    /// Whether a `maildir` block may be used.
    pub fn allows_maildir(self) -> bool {
        matches!(self, Self::Auto | Self::Maildir)
    }

    /// Whether a `caldav` block may be used.
    pub fn allows_caldav(self) -> bool {
        matches!(self, Self::Auto | Self::Caldav)
    }

    /// Whether a `carddav` block may be used.
    pub fn allows_carddav(self) -> bool {
        matches!(self, Self::Auto | Self::Carddav)
    }

    /// Whether an `msgraph` block may be used.
    pub fn allows_msgraph(self) -> bool {
        matches!(self, Self::Auto | Self::Msgraph)
    }

    /// Whether a `gmail` block may be used.
    pub fn allows_gmail(self) -> bool {
        matches!(self, Self::Auto | Self::Gmail)
    }

    /// Whether a `gcal` block may be used.
    pub fn allows_gcal(self) -> bool {
        matches!(self, Self::Auto | Self::Gcal)
    }

    /// Whether a `gpeople` block may be used.
    pub fn allows_gpeople(self) -> bool {
        matches!(self, Self::Auto | Self::Gpeople)
    }

    /// Whether a `pimdir` block may be used.
    pub fn allows_pimdir(self) -> bool {
        matches!(self, Self::Auto | Self::Pimdir)
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
            Self::Msgraph => write!(f, "msgraph"),
            Self::Gmail => write!(f, "gmail"),
            Self::Gcal => write!(f, "gcal"),
            Self::Gpeople => write!(f, "gpeople"),
            Self::Pimdir => write!(f, "pimdir"),
        }
    }
}
