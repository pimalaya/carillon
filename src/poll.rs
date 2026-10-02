//! # Poll
//!
//! The interruptible sleep every polled backend waits on between two
//! rounds.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

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
