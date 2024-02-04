//! Types needed to define the event loop.

use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(not(web_platform))]
use std::time::{Duration, Instant};
#[cfg(web_platform)]
use web_time::{Duration, Instant};

/// A unique identifier of the winit's async request.
///
/// This could be used to identify the async request once it's done
/// and a specific action must be taken.
///
/// One of the handling scenarious could be to maintain a working list
/// containing [`AsyncRequestSerial`] and some closure associated with it.
/// Then once event is arriving the working list is being traversed and a job
/// executed and removed from the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsyncRequestSerial {
    serial: u64,
}

impl AsyncRequestSerial {
    /// Get the next serial in the sequence.
    pub fn get() -> Self {
        static CURRENT_SERIAL: AtomicU64 = AtomicU64::new(0);
        // NOTE: we rely on wrap around here, while the user may just request
        // in the loop u64::MAX times that's issue is considered on them.
        let serial = CURRENT_SERIAL.fetch_add(1, Ordering::Relaxed);
        Self { serial }
    }
}

/// Set through [`EventLoopWindowTarget::set_control_flow()`].
///
/// Indicates the desired behavior of the event loop after [`Event::AboutToWait`] is emitted.
///
/// Defaults to [`Wait`].
///
/// [`Wait`]: Self::Wait
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum ControlFlow {
    /// When the current loop iteration finishes, immediately begin a new iteration regardless of
    /// whether or not new events are available to process.
    Poll,

    /// When the current loop iteration finishes, suspend the thread until another event arrives.
    #[default]
    Wait,

    /// When the current loop iteration finishes, suspend the thread until either another event
    /// arrives or the given time is reached.
    ///
    /// Useful for implementing efficient timers. Applications which want to render at the display's
    /// native refresh rate should instead use [`Poll`] and the VSync functionality of a graphics API
    /// to reduce odds of missed frames.
    ///
    /// [`Poll`]: Self::Poll
    WaitUntil(Instant),
}

impl ControlFlow {
    /// Creates a [`ControlFlow`] that waits until a timeout has expired.
    ///
    /// In most cases, this is set to [`WaitUntil`]. However, if the timeout overflows, it is
    /// instead set to [`Wait`].
    ///
    /// [`WaitUntil`]: Self::WaitUntil
    /// [`Wait`]: Self::Wait
    pub fn wait_duration(timeout: Duration) -> Self {
        match Instant::now().checked_add(timeout) {
            Some(instant) => Self::WaitUntil(instant),
            None => Self::Wait,
        }
    }
}
