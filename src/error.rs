//! # Error codes
//!
//! A stable, machine-readable name for the failures a caller is expected
//! to act on, printed as `code` beside the message under `--json` and in a
//! `check` row. The message stays for humans and may be reworded at any
//! time; the code is the contract. A failure with no code prints `"code":
//! null`. The mechanism (`CodedError`, `code_of`, the report) lives in
//! `pimalaya_cli::error`, which also raises `usage-invalid` for a command
//! line the grammar refuses; only the vocabulary is carillon's.
//!
//! A server's refusal takes a code of the vocabulary the family shares,
//! `permission-denied`, `quota-exceeded` and `rate-limited`. A pimdir
//! store's failure takes the code io-pimdir gives it (`PimdirError::code`),
//! spelled the same; the codes of a store's owner (`store-busy`,
//! `store-owned`, `lock-replaced`) never reach a reader taking no lock.

use anyhow::Error;
use pimalaya_cli::error::{CodedError, code_of};
use schemars::JsonSchema;
use serde::Serialize;

/// The stable codes, each serialised in kebab-case.
// NOTE: declared in every build, so the schema is one; no reader raises a
// code yet.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ErrorCode {
    /// The server refused the account: an HTTP 403 that is no rate limit
    /// and, on DAV, names no precondition but `need-privileges` (RFC 3744
    /// §7.1.1), or an IMAP `NOPERM` (RFC 5530 §3).
    PermissionDenied,
    /// The account is over its quota: an HTTP 507, a DAV
    /// `quota-not-exceeded` (RFC 4331 §6), Graph's `ErrorQuotaExceeded`, or
    /// an IMAP `OVERQUOTA` (RFC 5530 §3).
    QuotaExceeded,
    /// The server throttled the request: an HTTP 429, or a Google 403
    /// naming a rate limit or a quota. carillon sends no request twice.
    RateLimited,
    /// The pimdir store is of another draft than the one carillon reads:
    /// its owner reinstalls it.
    StoreVersion,
    /// The pimdir root holds no store yet: its sync engine creates it.
    StoreUncreated,
    /// The pimdir root holds a database another program wrote.
    StoreForeign,
    /// The pimdir store's version records disagree.
    StoreCorrupt,
    /// The pimdir store holds no schema yet.
    StoreNotCurrent,
    /// The pimdir store names its bodies by a hash carillon does not know.
    StoreHashAlgo,
    /// Another user can reach the pimdir store, or a link stands where its
    /// files belong.
    StoreExposed,
    /// Another sync engine version wrote the pimdir store's sync state.
    SyncVersion,
}

/// A failure carrying `code`, with its own message.
pub fn coded(code: ErrorCode, message: impl Into<String>) -> Error {
    CodedError::new(code, message).into()
}

/// The code of `err`: the one a coded error in its chain carries.
pub fn code(err: &Error) -> Option<ErrorCode> {
    code_of(err)
}

/// The failure as it is reported: `err` itself when the report reads its
/// code already, else its messages over a coded innermost one, so the
/// code a protocol's typed error names reaches the failure object.
pub fn with_code(err: Error) -> Error {
    if code_of::<ErrorCode>(&err).is_some() {
        return err;
    }

    let Some(code) = code(&err) else {
        return err;
    };

    chained(Some(code), err.chain().map(ToString::to_string).collect())
}

/// The failure reading `messages`, outermost first, its innermost one
/// carrying `code`.
fn chained(code: Option<ErrorCode>, mut messages: Vec<String>) -> Error {
    let innermost = messages.pop().unwrap_or_default();
    let err = match code {
        Some(code) => coded(code, innermost),
        None => Error::msg(innermost),
    };

    messages
        .into_iter()
        .rev()
        .fold(err, |err, message| err.context(message))
}

#[cfg(all(test, feature = "pimdir"))]
mod tests {
    use std::path::PathBuf;

    use io_pimdir::client::{PimdirError, PimdirNotCurrent};

    use super::*;

    /// Each store code is spelled as io-pimdir 0.12's `PimdirError::code`
    /// spells the failure it names.
    #[test]
    fn a_store_code_keeps_io_pimdirs_spelling() {
        let store = [
            (
                PimdirError::Version { found: None },
                ErrorCode::StoreVersion,
            ),
            (PimdirError::Uncreated, ErrorCode::StoreUncreated),
            (PimdirError::Foreign, ErrorCode::StoreForeign),
            (
                PimdirError::VersionMismatch {
                    user_version: 1,
                    store_meta: 2,
                },
                ErrorCode::StoreCorrupt,
            ),
            (
                PimdirError::NotCurrent(PimdirNotCurrent::Version { found: 0 }),
                ErrorCode::StoreNotCurrent,
            ),
            (
                PimdirError::HashAlgo {
                    found: "sha1".into(),
                },
                ErrorCode::StoreHashAlgo,
            ),
            (
                PimdirError::Exposed(PathBuf::from("pimdir.db")),
                ErrorCode::StoreExposed,
            ),
            (
                PimdirError::SyncVersion { found: "0".into() },
                ErrorCode::SyncVersion,
            ),
        ];

        for (err, ours) in store {
            assert_eq!(serde_json::to_value(ours).unwrap(), err.code());
        }
    }
}
