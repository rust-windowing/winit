//! Incoming notifications from the GUI system.

/// Identifier of an input device.
///
/// Whenever you receive an event arising from a particular input device, this event contains a `DeviceId` which
/// identifies its origin. Note that devices may be virtual (representing an on-screen cursor and keyboard focus) or
/// physical. Virtual devices typically aggregate inputs from multiple physical devices.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId(u64);

impl DeviceId {
    /// Returns a dummy id, useful for unit testing.
    ///
    /// # Safety
    ///
    /// The only guarantee made about the return value of this function is that
    /// it will always be equal to itself and to future values returned by this function.
    /// No other guarantees are made. This may be equal to a real `DeviceId`.
    ///
    /// **Passing this into a winit function will result in undefined behavior.**
    pub const unsafe fn dummy() -> Self {
        DeviceId(0)
    }
}

impl From<u64> for DeviceId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<DeviceId> for u64 {
    fn from(value: DeviceId) -> Self {
        value.0
    }
}
