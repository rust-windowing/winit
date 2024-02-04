//! Types needed to define the event loop.

use std::sync::atomic::{AtomicU64, Ordering};

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
