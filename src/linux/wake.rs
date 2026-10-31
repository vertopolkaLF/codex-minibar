//! Wakes the tray bridge thread out of its idle wait.
//!
//! On Windows the bridge sleeps in `MsgWaitForMultipleObjectsEx`, which tray
//! clicks interrupt because they arrive as window messages. On Linux tray and
//! single-instance events arrive on D-Bus and socket threads instead; they
//! call [`wake`] so the bridge reacts at once rather than on its next tick.

use std::{
    sync::{Condvar, Mutex},
    time::Duration,
};

static PENDING: Mutex<bool> = Mutex::new(false);
static SIGNAL: Condvar = Condvar::new();

/// Interrupts the current (or next) [`wait`].
pub fn wake() {
    if let Ok(mut pending) = PENDING.lock() {
        *pending = true;
        SIGNAL.notify_all();
    }
}

/// Sleeps until [`wake`] is called or `timeout` elapses.
pub fn wait(timeout: Duration) {
    let Ok(pending) = PENDING.lock() else {
        std::thread::sleep(timeout);
        return;
    };
    let mut pending = if *pending {
        pending
    } else {
        match SIGNAL.wait_timeout_while(pending, timeout, |pending| !*pending) {
            Ok((pending, _)) => pending,
            Err(_) => return,
        }
    };
    *pending = false;
}
