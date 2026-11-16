//! # Error codes
//!
//! A stable, machine-readable name for the failures a caller is expected
//! to act on, printed as `code` beside the message under `--json` and in an
//! `account check` row. The message stays for humans and may be reworded at
//! any time; the code is the contract. A failure with no code prints
//! `"code": null`. The mechanism (`CodedError`, `code_of`, `with_code`, the
//! report) lives in `pimalaya_cli::error`, which also raises `usage-invalid`
//! for a command line the grammar refuses; only the vocabulary and its
//! readers are carillon's.
//!
//! The account `-a` could not take is coded from pimalaya-config's
//! `AccountError`, and a backend the build leaves out is coded
//! `backend-not-built` when a command reaches it.
//!
//! A server's refusal takes a code of the vocabulary the family shares,
//! `permission-denied`, `quota-exceeded` and `rate-limited`, read from the
//! typed error of the protocol that answered it by that backend's reader,
//! `refusal_code` beside its client. A watched collection that does not
//! exist is `mailbox-not-found`, `calendar-not-found` or
//! `addressbook-not-found`, after the backend's word for it, wherever the
//! backend can tell a missing collection from another failure.
//!
//! A pimdir store's failure takes the code io-pimdir gives it
//! (`PimdirError::code`), spelled the same: carillon relays the codes
//! himalaya does, so a caller reading every pimdir app reads one
//! vocabulary, though a reader taking no lock meets only a few of them.

use std::fmt;

use anyhow::Error;
#[cfg(feature = "pimdir")]
use io_pimdir::client::PimdirError;
use pimalaya_cli::error::{CodedError, code_of, with_code};
use pimalaya_config::toml::AccountError;
use schemars::JsonSchema;
use serde::Serialize;

use crate::event::WatchDomain;

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
        not(any(dav, feature = "imap", feature = "jmap", feature = "msgraph")),
        allow(dead_code)
    )]
    QuotaExceeded,
    /// The server throttled the request: an HTTP 429, or a Google 403
    /// naming a rate limit or a quota. carillon sends no request twice.
    #[cfg_attr(not(any(dav, feature = "jmap", api)), allow(dead_code))]
    RateLimited,
    /// The mailbox a backend watches does not exist: nothing was watched.
    #[cfg_attr(
        not(any(
            feature = "imap",
            feature = "jmap",
            feature = "maildir",
            feature = "msgraph",
            feature = "gmail"
        )),
        allow(dead_code)
    )]
    MailboxNotFound,
    /// The calendar a backend watches does not exist: nothing was watched.
    #[cfg_attr(
        not(any(
            feature = "jmap",
            dav,
            feature = "msgraph",
            feature = "gcal",
            feature = "pimdir"
        )),
        allow(dead_code)
    )]
    CalendarNotFound,
    /// The addressbook a backend watches does not exist: nothing was
    /// watched.
    #[cfg_attr(
        not(any(feature = "jmap", dav, feature = "msgraph", feature = "gpeople")),
        allow(dead_code)
    )]
    AddressbookNotFound,
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
    /// Another writer holds the pimdir store's write lock. Retry.
    #[cfg_attr(not(feature = "pimdir"), allow(dead_code))]
    StoreBusy,
    /// A sync owns the pimdir store: retry once it ends.
    #[cfg_attr(not(feature = "pimdir"), allow(dead_code))]
    StoreOwned,
    /// A producer is staging a body in the pimdir store: retry.
    #[cfg_attr(not(feature = "pimdir"), allow(dead_code))]
    StoreStaging,
    /// A pimdir store verb is between two of its chunks: retry.
    #[cfg_attr(not(feature = "pimdir"), allow(dead_code))]
    StoreInFlight,
    /// The pimdir store is not the one that answered a queue row's
    /// handle, a copy restored since.
    #[cfg_attr(not(feature = "pimdir"), allow(dead_code))]
    StoreReset,
    /// The lock file of the pimdir store was removed or replaced under a
    /// write. Retry.
    #[cfg_attr(not(feature = "pimdir"), allow(dead_code))]
    LockReplaced,
    /// The pimdir collection already holds the item, or has its creation
    /// queued.
    #[cfg_attr(not(feature = "pimdir"), allow(dead_code))]
    ItemDuplicate,
    /// The pimdir store cannot hold the flag.
    #[cfg_attr(not(feature = "pimdir"), allow(dead_code))]
    FlagInvalid,
    /// The pimdir action's payload is malformed.
    #[cfg_attr(not(feature = "pimdir"), allow(dead_code))]
    ActionMalformed,
    /// The pimdir conflict holds a body nobody has read.
    #[cfg_attr(not(feature = "pimdir"), allow(dead_code))]
    ConflictUnread,
    /// A source of the pimdir store does not support the write.
    #[cfg_attr(not(feature = "pimdir"), allow(dead_code))]
    Unsupported,
    /// No source of the pimdir store can perform the intent.
    #[cfg_attr(not(feature = "pimdir"), allow(dead_code))]
    NoPerformer,
    /// Several sources of the pimdir store can perform the intent and
    /// none is chosen.
    #[cfg_attr(not(feature = "pimdir"), allow(dead_code))]
    AmbiguousPerformer,
    /// The pimdir store has no search index yet: its sync engine builds
    /// it.
    #[cfg_attr(not(feature = "pimdir"), allow(dead_code))]
    IndexMissing,
    /// The pimdir search index was built by another version: its sync
    /// engine rebuilds it.
    #[cfg_attr(not(feature = "pimdir"), allow(dead_code))]
    IndexOutdated,
    /// The pimdir search index is of another draft or shape.
    #[cfg_attr(not(feature = "pimdir"), allow(dead_code))]
    IndexVersion,
    /// The zone the pimdir search index reads floating times in is not
    /// an IANA zone.
    #[cfg_attr(not(feature = "pimdir"), allow(dead_code))]
    IndexZoneUnknown,
    /// The SQLite linked lacks what the pimdir search index needs (FTS5,
    /// 3.43 or later).
    #[cfg_attr(not(feature = "pimdir"), allow(dead_code))]
    IndexFts5Missing,
    /// The pimdir search query is malformed or names an unknown field or
    /// value.
    #[cfg_attr(not(feature = "pimdir"), allow(dead_code))]
    QueryRefused,
    /// The pimdir search index could not answer an accepted query.
    #[cfg_attr(not(feature = "pimdir"), allow(dead_code))]
    QueryUnanswerable,
    /// The pimdir tag names no flag, or adds and removes one flag.
    #[cfg_attr(not(feature = "pimdir"), allow(dead_code))]
    TagRefused,
    /// No account carries the name `-a` gave.
    AccountMissing,
    /// No account is marked default, and `-a` named none.
    AccountDefaultMissing,
    /// Several accounts are marked default, and `-a` named none.
    AccountDefaultSeveral,
    /// The account declares a backend this build leaves out, which a
    /// command reached: a build with its cargo feature serves it.
    #[cfg_attr(
        all(
            feature = "imap",
            feature = "jmap",
            feature = "maildir",
            feature = "caldav",
            feature = "carddav",
            feature = "msgraph",
            feature = "gmail",
            feature = "gcal",
            feature = "gpeople",
            feature = "pimdir"
        ),
        allow(dead_code)
    )]
    BackendNotBuilt,
}

/// The store codes carillon relays, by io-pimdir's spelling, which
/// serialises each: himalaya's table (himalaya/src/error.rs).
#[cfg(feature = "pimdir")]
const PIMDIR_CODES: &[(&str, ErrorCode)] = &[
    ("store-version", ErrorCode::StoreVersion),
    ("store-uncreated", ErrorCode::StoreUncreated),
    ("store-foreign", ErrorCode::StoreForeign),
    ("store-corrupt", ErrorCode::StoreCorrupt),
    ("store-not-current", ErrorCode::StoreNotCurrent),
    ("store-hash-algo", ErrorCode::StoreHashAlgo),
    ("sync-version", ErrorCode::SyncVersion),
    ("store-busy", ErrorCode::StoreBusy),
    ("store-owned", ErrorCode::StoreOwned),
    ("store-staging", ErrorCode::StoreStaging),
    ("store-in-flight", ErrorCode::StoreInFlight),
    ("store-exposed", ErrorCode::StoreExposed),
    ("store-reset", ErrorCode::StoreReset),
    ("lock-replaced", ErrorCode::LockReplaced),
    ("item-duplicate", ErrorCode::ItemDuplicate),
    ("flag-invalid", ErrorCode::FlagInvalid),
    ("action-malformed", ErrorCode::ActionMalformed),
    ("conflict-unread", ErrorCode::ConflictUnread),
    ("unsupported", ErrorCode::Unsupported),
    ("no-performer", ErrorCode::NoPerformer),
    ("ambiguous-performer", ErrorCode::AmbiguousPerformer),
    ("index-missing", ErrorCode::IndexMissing),
    ("index-outdated", ErrorCode::IndexOutdated),
    ("index-version", ErrorCode::IndexVersion),
    ("index-zone-unknown", ErrorCode::IndexZoneUnknown),
    ("index-fts5-missing", ErrorCode::IndexFts5Missing),
    ("query-refused", ErrorCode::QueryRefused),
    ("query-unanswerable", ErrorCode::QueryUnanswerable),
    ("tag-refused", ErrorCode::TagRefused),
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

/// The failure of a command reaching the `backend` block of an account,
/// which this build leaves out with its cargo feature, named as the block.
#[cfg(not(all(
    feature = "imap",
    feature = "jmap",
    feature = "maildir",
    feature = "caldav",
    feature = "carddav",
    feature = "msgraph",
    feature = "gmail",
    feature = "gcal",
    feature = "gpeople",
    feature = "pimdir"
)))]
pub fn backend_not_built(backend: &str) -> Error {
    let message = format!(
        "This build leaves out the `{backend}` backend, rebuild with \
         `--features {backend}`"
    );

    CodedError::new(ErrorCode::BackendNotBuilt, message).into()
}

/// The code of a watched collection of `domain` that does not exist, after
/// the word its backend names it by.
fn not_found_code(domain: WatchDomain) -> ErrorCode {
    match domain {
        WatchDomain::Message => ErrorCode::MailboxNotFound,
        WatchDomain::Card => ErrorCode::AddressbookNotFound,
        WatchDomain::Event | WatchDomain::Task => ErrorCode::CalendarNotFound,
    }
}

/// The failure of a watched collection of `domain` that does not exist.
#[cfg_attr(
    not(any(
        feature = "jmap",
        feature = "gmail",
        feature = "gcal",
        feature = "gpeople",
        feature = "pimdir"
    )),
    allow(dead_code)
)]
pub fn not_found(domain: WatchDomain, message: impl Into<String>) -> Error {
    CodedError::new(not_found_code(domain), message.into()).into()
}

/// `err`, a backend's answer that the watched collection of `domain` does
/// not exist, coded so, its messages kept.
#[cfg_attr(
    not(any(feature = "maildir", dav, feature = "msgraph")),
    allow(dead_code)
)]
pub fn not_found_from(domain: WatchDomain, err: Error) -> Error {
    with_code(err, |_| Some(not_found_code(domain)))
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
        account_code,
        #[cfg(dav)]
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

/// The code of the account `-a` could not take.
fn account_code(cause: &(dyn std::error::Error + 'static)) -> Option<ErrorCode> {
    match cause.downcast_ref::<AccountError>()? {
        AccountError::Missing { .. } => Some(ErrorCode::AccountMissing),
        AccountError::NoDefault => Some(ErrorCode::AccountDefaultMissing),
        AccountError::SeveralDefaults(_) => Some(ErrorCode::AccountDefaultSeveral),
    }
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

#[cfg(all(test, feature = "pimdir"))]
mod tests {
    use std::path::PathBuf;

    use io_pimdir::client::{PimdirError, PimdirNotCurrent};

    use super::*;

    /// Each store code is spelled as io-pimdir 0.14's `PimdirError::code`
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

    /// Every code of himalaya's relay table is relayed, under io-pimdir's
    /// spelling, the owner's codes included.
    #[test]
    fn the_relayed_pimdir_codes_are_himalayas() {
        assert_eq!(29, PIMDIR_CODES.len());

        for (pimdir, ours) in PIMDIR_CODES {
            assert_eq!(serde_json::to_value(ours).unwrap(), *pimdir);
        }

        let busy = ErrorCode::of_pimdir(&PimdirError::Busy);
        assert_eq!(Some(ErrorCode::StoreBusy), busy);
    }
}
