//! Query logical keys from physical key codes under the current keyboard layout.
//!
//! This module provides the [`KeyCodeExtKeyMap`] trait which extends [`KeyCode`]
//! with methods to query what logical key a physical key produces.
//!
//! ## Platform Support
//!
//! - **macOS**: Fully supported via `UCKeyTranslate`.
//! - **Other platforms**: Not yet implemented, returns `Key::Unidentified`.

use crate::keyboard::{Key, KeyCode, ModifiersState};

/// Extension trait for [`KeyCode`] to query logical key mappings.
pub trait KeyCodeExtKeyMap {
    /// Returns the logical key that would be produced by this physical key
    /// with the given modifiers under the current keyboard layout.
    ///
    /// # Arguments
    ///
    /// * `modifiers` - The modifier key state (Shift, Ctrl, Alt, Meta)
    /// * `caps_lock` - Whether Caps Lock is toggled on
    /// * `num_lock` - Whether Num Lock is toggled on
    ///
    /// # Example
    ///
    /// ```ignore
    /// use winit::keyboard::{KeyCode, ModifiersState};
    /// use winit::platform::key_map::KeyCodeExtKeyMap;
    ///
    /// // Get what the 'A' key produces with Shift held
    /// let key = KeyCode::KeyA.physical_to_logical_key(ModifiersState::SHIFT, false, false);
    /// ```
    fn physical_to_logical_key(self, modifiers: ModifiersState, caps_lock: bool, num_lock: bool) -> Key;

    /// Returns the logical key without any modifiers applied.
    ///
    /// Equivalent to `physical_to_logical_key(ModifiersState::empty(), false, false)`.
    fn physical_to_logical_key_unmodified(self) -> Key;
}

impl KeyCodeExtKeyMap for KeyCode {
    #[inline]
    fn physical_to_logical_key(self, modifiers: ModifiersState, caps_lock: bool, num_lock: bool) -> Key {
        crate::platform_impl::platform::physical_to_logical_key(self, modifiers, caps_lock, num_lock)
    }

    #[inline]
    fn physical_to_logical_key_unmodified(self) -> Key {
        self.physical_to_logical_key(ModifiersState::empty(), false, false)
    }
}
