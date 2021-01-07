#![cfg(any(
    target_os = "windows",
    target_os = "macos",
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]

use crate::keyboard::Key;

/// Additional methods for the `KeyEvent` which cannot be implemented on all
/// platforms.
pub trait KeyEventExtModifierSupplement {
    /// This value is affected by all modifiers including but not
    /// limited to <kbd>Shift</kbd>, <kbd>Ctrl</kbd>, and <kbd>Num Lock</kbd>.
    ///
    /// This is suitable for text input in a terminal application.
    ///
    /// `None` is returned if the input cannot be translated to a string.
    /// For example dead key input as well as <kbd>F1</kbd> and
    /// <kbd>Home</kbd> among others produce `None`.
    ///
    /// Note that the resulting string may contain multiple characters.
    /// For example on Windows when pressing <kbd>'</kbd> using
    /// a US-International layout, this will be `None` for the first
    /// keypress and will be `Some("''")` for the second keypress.
    /// It's important that this behaviour might be different on
    /// other platforms. For example Linux systems may emit a  
    /// `Some("'")` on the second keypress.
    fn char_with_all_modifers(&self) -> Option<&str>;

    /// This value ignores all modifiers including
    /// but not limited to <kbd>Shift</kbd>, <kbd>Caps Lock</kbd>,
    /// and <kbd>Ctrl</kbd>. In most cases this means that the
    /// unicode character in the resulting string is lowercase.
    ///
    /// This is useful for key-bindings / shortcut key combinations.
    ///
    /// In case `logical_key` reports `Dead`, this will still report the
    /// real key according to the current keyboard layout. This value
    /// cannot be `Dead`. Furthermore the `Character` variant will always
    /// contain a single-character String.
    fn key_without_modifers(&self) -> Key<'static>;
}
