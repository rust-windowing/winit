#![cfg(any(windows_platform, macos_platform, x11_platform, wayland_platform))]

use crate::keyboard::KeyCode;

// TODO: Describe what this value contains for each platform

/// Additional methods for the [`KeyCode`] type that allow the user to access the platform-specific
/// scancode.
///
/// [`KeyCode`]: crate::keyboard::KeyCode
pub trait KeyCodeExtScancode {
    /// The raw value of the platform-specific physical key identifier.
    ///
    /// Returns `Some(key_id)` if the conversion was succesful; returns `None` otherwise.
    ///
    /// ## Platform-specific
    /// - **Windows:** A 16bit extended scancode
    /// - **Wayland/X11**: A 32-bit linux scancode, which is X11/Wayland keycode subtracted by 8.
    fn to_scancode(self) -> Option<u32>;

    /// Constructs a `KeyCode` from a platform-specific physical key identifier.
    ///
    /// Note that this conversion may be lossy, i.e. converting the returned `KeyCode` back
    /// using `to_scancode` might not yield the original value.
    ///
    /// ## Platform-specific
    /// - **Wayland/X11**: A 32-bit linux scancode. When building from X11/Wayland keycode subtract
    ///                    `8` to get the value you wanted.
    fn from_scancode(scancode: u32) -> KeyCode;
}
