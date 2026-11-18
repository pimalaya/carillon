//! # Poll
//!
//! The interruptible sleep every polled backend waits on between two
//! rounds, and the retry of a round on a fresh connection.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use anyhow::Result;
use log::debug;

/// How long a sleep runs at a time, so a shutdown is noticed promptly.
const POLL_STEP: Duration = Duration::from_millis(200);

/// Sleeps `total` in small steps, returning false as soon as a shutdown
/// is requested.
pub fn sleep(total: Duration, shutdown: &Arc<AtomicBool>) -> bool {
    let mut left = total;

    while left > Duration::ZERO {
        if shutdown.load(Ordering::SeqCst) {
            return false;
        }

        let step = left.min(POLL_STEP);
        thread::sleep(step);
        left -= step;
    }

    !shutdown.load(Ordering::SeqCst)
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
