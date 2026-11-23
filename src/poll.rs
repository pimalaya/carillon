//! # Poll
//!
//! The interruptible sleep every polled backend and the reconnect backoff
//! wait on, which a requested shutdown ends at once, and the retry of a
//! round on a fresh connection.

use std::{
    sync::{
        Arc, Condvar, Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use anyhow::Result;
use log::debug;

/// Held to read a shutdown flag before waiting on [`RING`], and to ring
/// it, so a ring never falls between the read and the wait.
static RING_LOCK: Mutex<()> = Mutex::new(());

/// Rung by [`stop`], waking every sleep to read its shutdown flag again.
static RING: Condvar = Condvar::new();

/// Sleeps `total`, returning false as soon as a shutdown is requested
/// through [`stop`].
pub fn sleep(total: Duration, shutdown: &Arc<AtomicBool>) -> bool {
    let deadline = Instant::now() + total;
    let mut guard = RING_LOCK.lock().unwrap_or_else(PoisonError::into_inner);

    loop {
        if shutdown.load(Ordering::SeqCst) {
            return false;
        }

        let now = Instant::now();

        if now >= deadline {
            return true;
        }

        guard = RING
            .wait_timeout(guard, deadline - now)
            .unwrap_or_else(PoisonError::into_inner)
            .0;
    }
}

/// Requests a shutdown: sets `shutdown`, then wakes every sleep so the
/// ones reading it end.
pub fn stop(shutdown: &AtomicBool) {
    shutdown.store(true, Ordering::SeqCst);

    let _guard = RING_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    RING.notify_all();
}

/// Runs `round` on `client`, and once more on a fresh client from `open`
/// when it fails, unless a shutdown was requested meanwhile.
///
/// The connection sat idle since the last round, which a server is free
/// to close, and its credential may have expired, so a failed round is
/// given a fresh connection and a fresh credential before the session is
/// given up.
#[cfg_attr(not(any(feature = "jmap", dav, api)), allow(dead_code))]
pub fn retried<C>(
    client: &mut C,
    shutdown: &AtomicBool,
    open: impl FnOnce() -> Result<C>,
    mut round: impl FnMut(&mut C) -> Result<()>,
) -> Result<()> {
    let Err(err) = round(client) else {
        return Ok(());
    };

    if shutdown.load(Ordering::SeqCst) {
        return Ok(());
    }

    debug!("round failed, reconnecting: {err:#}");
    *client = open()?;
    round(client)
}
