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
//! `permission-denied`, `quota-exceeded` and `rate-limited`, read from the
//! typed error of the protocol that answered it by that backend's reader,
//! `refusal_code` beside its client. A pimdir store's failure takes the
//! code io-pimdir gives it (`PimdirError::code`), spelled the same; the
//! codes of a store's owner (`store-busy`, `store-owned`, `lock-replaced`)
//! never reach a reader taking no lock.

use std::fmt;

use anyhow::Error;
#[cfg(feature = "pimdir")]
use io_pimdir::client::PimdirError;
use pimalaya_cli::error::{CodedError, code_of};
use schemars::JsonSchema;
use serde::Serialize;

/// The stable codes, each serialised in kebab-case.
// NOTE: declared in every build, so the schema is one: a build leaving out
// every backend raising a code leaves that code unraised.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ErrorCode {
    /// The server refused the account: an HTTP 403 that is no rate limit
    /// and, on DAV, names no precondition but `need-privileges` (RFC 3744
    /// §7.1.1), or an IMAP `NOPERM` (RFC 5530 §3).
    #[cfg_attr(not(network), allow(dead_code))]
    PermissionDenied,
    /// The account is over its quota: an HTTP 507, a DAV
    /// `quota-not-exceeded` (RFC 4331 §6), Graph's `ErrorQuotaExceeded`, or
    /// an IMAP `OVERQUOTA` (RFC 5530 §3).
    #[cfg_attr(
        not(any(
            feature = "dav",
            feature = "imap",
            feature = "jmap",
            feature = "msgraph"
        )),
        allow(dead_code)
    )]
    QuotaExceeded,
    /// The server throttled the request: an HTTP 429, or a Google 403
    /// naming a rate limit or a quota. carillon sends no request twice.
    #[cfg_attr(not(any(feature = "dav", feature = "jmap", api)), allow(dead_code))]
    RateLimited,
    /// The pimdir store is of another draft than the one carillon reads:
    /// its owner reinstalls it.
    #[cfg_attr(not(feature = "pimdir"), allow(dead_code))]
    StoreVersion,
    /// The pimdir root holds no store yet: its sync engine creates it.
    #[cfg_attr(not(feature = "pimdir"), allow(dead_code))]
    StoreUncreated,
    /// The pimdir root holds a database another program wrote.
    #[cfg_attr(not(feature = "pimdir"), allow(dead_code))]
    StoreForeign,
    /// The pimdir store's version records disagree.
    #[cfg_attr(not(feature = "pimdir"), allow(dead_code))]
    StoreCorrupt,
    /// The pimdir store holds no schema yet.
    #[cfg_attr(not(feature = "pimdir"), allow(dead_code))]
    StoreNotCurrent,
    /// The pimdir store names its bodies by a hash carillon does not know.
    #[cfg_attr(not(feature = "pimdir"), allow(dead_code))]
    StoreHashAlgo,
    /// Another user can reach the pimdir store, or a link stands where its
    /// files belong.
    #[cfg_attr(not(feature = "pimdir"), allow(dead_code))]
    StoreExposed,
    /// Another sync engine version wrote the pimdir store's sync state.
    #[cfg_attr(not(feature = "pimdir"), allow(dead_code))]
    SyncVersion,
}

/// The store codes carillon relays, by io-pimdir's spelling, which
/// serialises each.
#[cfg(feature = "pimdir")]
const PIMDIR_CODES: &[(&str, ErrorCode)] = &[
    ("store-version", ErrorCode::StoreVersion),
    ("store-uncreated", ErrorCode::StoreUncreated),
    ("store-foreign", ErrorCode::StoreForeign),
    ("store-corrupt", ErrorCode::StoreCorrupt),
    ("store-not-current", ErrorCode::StoreNotCurrent),
    ("store-hash-algo", ErrorCode::StoreHashAlgo),
    ("store-exposed", ErrorCode::StoreExposed),
    ("sync-version", ErrorCode::SyncVersion),
];

impl ErrorCode {
    /// The code of a store failure, `None` for one the caller cannot act
    /// on (an SQL or I/O failure).
    #[cfg(feature = "pimdir")]
    pub fn of_pimdir(err: &PimdirError) -> Option<Self> {
        let code = err.code();
        PIMDIR_CODES
            .iter()
            .find(|(pimdir, _)| *pimdir == code)
            .map(|(_, ours)| *ours)
    }
}

/// The code in its serialised spelling, as a log line names it.
impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match serde_json::to_value(self) {
            Ok(serde_json::Value::String(code)) => f.write_str(&code),
            _ => Err(fmt::Error),
        }
    }
}

/// A failure carrying `code`, with its own message.
pub fn coded(code: ErrorCode, message: impl Into<String>) -> Error {
    CodedError::new(code, message).into()
}

/// The code of `err`: the one a coded error in its chain carries, else
/// the one a reader finds in a typed cause.
pub fn code(err: &Error) -> Option<ErrorCode> {
    code_of(err).or_else(|| typed_code(err))
}

/// A reader of one backend's typed errors, coding the failures it knows.
type Reader = fn(&(dyn std::error::Error + 'static)) -> Option<ErrorCode>;

/// The code the typed error of the protocol or the store that failed
/// carries: the first cause of `err`'s chain a reader codes.
fn typed_code(err: &Error) -> Option<ErrorCode> {
    let readers: &[Reader] = &[
        #[cfg(feature = "dav")]
        crate::dav::refusal_code,
        #[cfg(feature = "gcal")]
        crate::gcal::refusal_code,
        #[cfg(feature = "gmail")]
        crate::gmail::refusal_code,
        #[cfg(feature = "gpeople")]
        crate::gpeople::refusal_code,
        #[cfg(feature = "imap")]
        crate::imap::refusal_code,
        #[cfg(feature = "jmap")]
        crate::jmap::refusal_code,
        #[cfg(feature = "msgraph")]
        crate::msgraph::refusal_code,
        #[cfg(feature = "pimdir")]
        crate::pimdir::store_code,
    ];

    err.chain()
        .find_map(|cause| readers.iter().find_map(|read| read(cause)))
}

/// A Google API error as io-gmail, io-gcal and io-gpeople keep it.
#[cfg(any(feature = "gmail", feature = "gcal", feature = "gpeople"))]
pub struct GoogleAnswer<'a> {
    /// The effective HTTP status.
    pub status: u16,
    /// The message of the envelope, or the raw body.
    pub message: &'a str,
    /// The reasons of `error.errors[]` (`rateLimitExceeded`).
    pub reasons: &'a [String],
    /// The reasons of `error.details[]` (`RATE_LIMIT_EXCEEDED`).
    pub detail_reasons: &'a [String],
    /// The canonical status (`RESOURCE_EXHAUSTED`).
    pub google_status: Option<&'a str>,
}

/// The code of a Google refusal: a 429, or a 403 naming a rate limit or a
/// quota, is `rate-limited`; any other 403 is `permission-denied`.
#[cfg(any(feature = "gmail", feature = "gcal", feature = "gpeople"))]
pub fn google_code(answer: &GoogleAnswer<'_>) -> Option<ErrorCode> {
    match answer.status {
        429 => Some(ErrorCode::RateLimited),
        403 if names_rate_limit(answer) => Some(ErrorCode::RateLimited),
        403 => Some(ErrorCode::PermissionDenied),
        _ => None,
    }
}

/// Whether a Google answer names a rate limit or a quota, by its
/// canonical status, by reason (`rateLimitExceeded`,
/// `userRateLimitExceeded`, `quotaExceeded`, `dailyLimitExceeded`,
/// `RATE_LIMIT_EXCEEDED`) or by its wording, a per-minute or a daily quota
/// included.
#[cfg(any(feature = "gmail", feature = "gcal", feature = "gpeople"))]
fn names_rate_limit(answer: &GoogleAnswer<'_>) -> bool {
    const REASONS: [&str; 5] = [
        "rateLimitExceeded",
        "userRateLimitExceeded",
        "quotaExceeded",
        "dailyLimitExceeded",
        "RATE_LIMIT_EXCEEDED",
    ];

    let reasons = || answer.reasons.iter().chain(answer.detail_reasons);
    let mut text = String::from(answer.message);
    for reason in reasons() {
        text.push(' ');
        text.push_str(reason);
    }
    let squeezed: String = text
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    let daily = squeezed.contains("dailylimit") || squeezed.contains("perday");

    answer.google_status == Some("RESOURCE_EXHAUSTED")
        || reasons().any(|reason| REASONS.contains(&reason.as_str()))
        || squeezed.contains("ratelimitexceeded")
        || squeezed.contains("quotaexceeded")
        || squeezed.contains("perminute")
        || squeezed.contains("per100seconds")
        || (daily && !squeezed.contains("unauthenticated"))
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
    /// spells the failure it names, and is the code that failure reads as.
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
            assert_eq!(Some(ours), ErrorCode::of_pimdir(&err), "{err}");
        }
    }
}
