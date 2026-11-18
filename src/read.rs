//! # Read
//!
//! The read every coroutine pump of carillon makes over a connection
//! carrying a read deadline: the deadline wakes it to look at the shutdown
//! flag, and a server that sent nothing for long enough is taken for dead
//! rather than held until TCP gives up. The pumps of [`crate::dav`] and
//! [`crate::imap`] read through [`bytes`], the JMAP event stream through
//! [`read`].

use std::{
    io::{self, Read},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};

/// How long a request may go unanswered, not a byte read, before its
/// connection is taken for dead and the request fails.
#[cfg(not(test))]
pub const SILENCE: Duration = Duration::from_secs(60);
/// The same, short enough for a test to wait it out.
#[cfg(test)]
pub const SILENCE: Duration = Duration::from_secs(2);

/// What one read gave.
pub enum ReadOutcome {
    /// That many bytes, at the start of the buffer.
    Bytes(usize),
    /// The peer closed the connection.
    Closed,
    /// A shutdown was requested.
    Shutdown,
    /// The server sent nothing for the silence allowed.
    Silent,
}

/// Reads `stream` into `buf`, waking at each read deadline to look at
/// `shutdown`, until bytes come, the peer closes, a shutdown is requested
/// or `silence` passed with nothing read.
pub fn read(
    stream: &mut (impl Read + ?Sized),
    buf: &mut [u8],
    shutdown: &AtomicBool,
    silence: Duration,
) -> io::Result<ReadOutcome> {
    let asked = Instant::now();

    loop {
        if shutdown.load(Ordering::SeqCst) {
            return Ok(ReadOutcome::Shutdown);
        }

        match stream.read(buf) {
            Ok(0) => return Ok(ReadOutcome::Closed),
            Ok(read) => return Ok(ReadOutcome::Bytes(read)),
            Err(err) if is_timeout(&err) && asked.elapsed() < silence => continue,
            Err(err) if is_timeout(&err) => return Ok(ReadOutcome::Silent),
            Err(err) => return Err(err),
        }
    }
}

/// The bytes a coroutine waits for, read as [`read`] reads them within
/// [`SILENCE`], anything else failing the request.
#[cfg_attr(not(any(feature = "imap", dav)), allow(dead_code))]
pub fn bytes(
    stream: &mut (impl Read + ?Sized),
    buf: &mut [u8],
    shutdown: &AtomicBool,
) -> Result<Vec<u8>> {
    match read(stream, buf, shutdown, SILENCE).context("read failed")? {
        ReadOutcome::Bytes(read) => Ok(buf[..read].to_vec()),
        ReadOutcome::Closed => bail!("Connection closed by peer"),
        ReadOutcome::Shutdown => bail!("Shutting down"),
        ReadOutcome::Silent => bail!("Server sent nothing for {} seconds", SILENCE.as_secs()),
    }
}

/// Whether an I/O error is the read deadline expiring, which is a
/// wakeup rather than a failure.
fn is_timeout(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    )
}
