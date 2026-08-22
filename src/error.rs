//! Watch error type.

use thiserror::Error;

/// The result of running a watch.
pub type CarillonWatchResult<T> = Result<T, CarillonWatchError>;

/// What can go wrong opening or running a watch.
#[derive(Debug, Error)]
pub enum CarillonWatchError {
    /// The IMAP conversation failed (transport dropped, authentication
    /// rejected, an unexpected server response). The message carries the
    /// underlying cause chain.
    #[error("{0}")]
    Imap(String),
}
