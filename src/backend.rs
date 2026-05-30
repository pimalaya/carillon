// This file is part of Mirador, a CLI to watch mailbox changes.
//
// Copyright (C) 2024-2026  soywod <pimalaya.org@posteo.net>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Backend selection for cross-protocol commands.
//!
//! Same shape as [himalaya CLI's `Backend`]: `Auto` picks the first
//! configured backend in a fixed priority order; the named variants
//! pin the active backend to that protocol and bail when the account
//! has no matching config block.
//!
//! [himalaya CLI's `Backend`]: https://github.com/pimalaya/himalaya/blob/master/src/backend.rs

use std::{fmt, str::FromStr};

use anyhow::{Error, bail};
use clap::Parser;

/// Backend selector for the `-b/--backend` CLI flag.
#[derive(Clone, Copy, Debug, Default, Parser, PartialEq, Eq)]
pub enum Backend {
    /// First configured block wins (priority: IMAP, JMAP, Maildir).
    #[default]
    Auto,
    /// Force IMAP; bail when the account has no `imap` block.
    Imap,
    /// Force JMAP; bail when the account has no `jmap` block.
    Jmap,
    /// Force Maildir; bail when the account has no `maildir` block.
    Maildir,
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
}

impl FromStr for Backend {
    type Err = Error;

    fn from_str(backend: &str) -> Result<Self, Self::Err> {
        match backend {
            "auto" => Ok(Self::Auto),
            "imap" => Ok(Self::Imap),
            "jmap" => Ok(Self::Jmap),
            "maildir" => Ok(Self::Maildir),
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
        }
    }
}
