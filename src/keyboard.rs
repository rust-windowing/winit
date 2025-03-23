//! Types related to the keyboard.

use bitflags::bitflags;
pub use keyboard_types::{Code as KeyCode, NamedKey};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
pub use smol_str::SmolStr;

/// Contains the platform-native physical key identifier
///
/// The exact values vary from platform to platform (which is part of why this is a per-platform
/// enum), but the values are primarily tied to the key's physical location on the keyboard.
///
/// This enum is primarily used to store raw keycodes when Winit doesn't map a given native
/// physical key identifier to a meaningful [`KeyCode`] variant. In the presence of identifiers we
/// haven't mapped for you yet, this lets you use use [`KeyCode`] to:
///
/// - Correctly match key press and release events.
/// - On non-Web platforms, support assigning keybinds to virtually any key through a UI.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum NativeKeyCode {
    Unidentified,
    /// An Android "scancode".
    Android(u32),
    /// A macOS "scancode".
    MacOS(u16),
    /// A Windows "scancode".
    Windows(u16),
    /// An XKB "keycode".
    Xkb(u32),
}

impl std::fmt::Debug for NativeKeyCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use NativeKeyCode::{Android, MacOS, Unidentified, Windows, Xkb};
        let mut debug_tuple;
        match self {
            Unidentified => {
                debug_tuple = f.debug_tuple("Unidentified");
            },
            Android(code) => {
                debug_tuple = f.debug_tuple("Android");
                debug_tuple.field(&format_args!("0x{code:04X}"));
            },
            MacOS(code) => {
                debug_tuple = f.debug_tuple("MacOS");
                debug_tuple.field(&format_args!("0x{code:04X}"));
            },
            Windows(code) => {
                debug_tuple = f.debug_tuple("Windows");
                debug_tuple.field(&format_args!("0x{code:04X}"));
            },
            Xkb(code) => {
                debug_tuple = f.debug_tuple("Xkb");
                debug_tuple.field(&format_args!("0x{code:04X}"));
            },
        }
        debug_tuple.finish()
    }
}

/// Contains the platform-native logical key identifier
///
/// Exactly what that means differs from platform to platform, but the values are to some degree
/// tied to the currently active keyboard layout. The same key on the same keyboard may also report
/// different values on different platforms, which is one of the reasons this is a per-platform
/// enum.
///
/// This enum is primarily used to store raw keysym when Winit doesn't map a given native logical
/// key identifier to a meaningful [`Key`] variant. This lets you use [`Key`], and let the user
/// define keybinds which work in the presence of identifiers we haven't mapped for you yet.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum NativeKey {
    Unidentified,
    /// An Android "keycode", which is similar to a "virtual-key code" on Windows.
    Android(u32),
    /// A macOS "scancode". There does not appear to be any direct analogue to either keysyms or
    /// "virtual-key" codes in macOS, so we report the scancode instead.
    MacOS(u16),
    /// A Windows "virtual-key code".
    Windows(u16),
    /// An XKB "keysym".
    Xkb(u32),
    /// A "key value string".
    Web(SmolStr),
}

impl std::fmt::Debug for NativeKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use NativeKey::{Android, MacOS, Unidentified, Web, Windows, Xkb};
        let mut debug_tuple;
        match self {
            Unidentified => {
                debug_tuple = f.debug_tuple("Unidentified");
            },
            Android(code) => {
                debug_tuple = f.debug_tuple("Android");
                debug_tuple.field(&format_args!("0x{code:04X}"));
            },
            MacOS(code) => {
                debug_tuple = f.debug_tuple("MacOS");
                debug_tuple.field(&format_args!("0x{code:04X}"));
            },
            Windows(code) => {
                debug_tuple = f.debug_tuple("Windows");
                debug_tuple.field(&format_args!("0x{code:04X}"));
            },
            Xkb(code) => {
                debug_tuple = f.debug_tuple("Xkb");
                debug_tuple.field(&format_args!("0x{code:04X}"));
            },
            Web(code) => {
                debug_tuple = f.debug_tuple("Web");
                debug_tuple.field(code);
            },
        }
        debug_tuple.finish()
    }
}

impl From<NativeKeyCode> for NativeKey {
    #[inline]
    fn from(code: NativeKeyCode) -> Self {
        match code {
            NativeKeyCode::Unidentified => NativeKey::Unidentified,
            NativeKeyCode::Android(x) => NativeKey::Android(x),
            NativeKeyCode::MacOS(x) => NativeKey::MacOS(x),
            NativeKeyCode::Windows(x) => NativeKey::Windows(x),
            NativeKeyCode::Xkb(x) => NativeKey::Xkb(x),
        }
    }
}

impl PartialEq<NativeKey> for NativeKeyCode {
    #[allow(clippy::cmp_owned)] // uses less code than direct match; target is stack allocated
    #[inline]
    fn eq(&self, rhs: &NativeKey) -> bool {
        NativeKey::from(*self) == *rhs
    }
}

impl PartialEq<NativeKeyCode> for NativeKey {
    #[inline]
    fn eq(&self, rhs: &NativeKeyCode) -> bool {
        rhs == self
    }
}

/// Represents the location of a physical key.
///
/// Winit will not emit [`KeyCode::Unidentified`] when it cannot recognize the key, instead it will
/// emit [`PhysicalKey::Unidentified`] with additional data about the key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum PhysicalKey {
    /// A known key code
    Code(KeyCode),
    /// This variant is used when the key cannot be translated to a [`KeyCode`]
    ///
    /// The native keycode is provided (if available) so you're able to more reliably match
    /// key-press and key-release events by hashing the [`PhysicalKey`]. It is also possible to use
    /// this for keybinds for non-standard keys, but such keybinds are tied to a given platform.
    Unidentified(NativeKeyCode),
}

impl From<KeyCode> for PhysicalKey {
    #[inline]
    fn from(code: KeyCode) -> Self {
        PhysicalKey::Code(code)
    }
}

impl From<PhysicalKey> for KeyCode {
    #[inline]
    fn from(key: PhysicalKey) -> Self {
        match key {
            PhysicalKey::Code(code) => code,
            PhysicalKey::Unidentified(_) => KeyCode::Unidentified,
        }
    }
}

impl From<NativeKeyCode> for PhysicalKey {
    #[inline]
    fn from(code: NativeKeyCode) -> Self {
        PhysicalKey::Unidentified(code)
    }
}

impl PartialEq<KeyCode> for PhysicalKey {
    #[inline]
    fn eq(&self, rhs: &KeyCode) -> bool {
        match self {
            PhysicalKey::Code(ref code) => code == rhs,
            _ => false,
        }
    }
}

impl PartialEq<PhysicalKey> for KeyCode {
    #[inline]
    fn eq(&self, rhs: &PhysicalKey) -> bool {
        rhs == self
    }
}

impl PartialEq<NativeKeyCode> for PhysicalKey {
    #[inline]
    fn eq(&self, rhs: &NativeKeyCode) -> bool {
        match self {
            PhysicalKey::Unidentified(ref code) => code == rhs,
            _ => false,
        }
    }
}

impl PartialEq<PhysicalKey> for NativeKeyCode {
    #[inline]
    fn eq(&self, rhs: &PhysicalKey) -> bool {
        rhs == self
    }
}

/// Key represents the meaning of a keypress.
///
/// This is a superset of the UI Events Specification's [`KeyboardEvent.key`] with
/// additions:
/// - All simple variants are wrapped under the `Named` variant
/// - The `Unidentified` variant here, can still identify a key through it's `NativeKeyCode`.
/// - The `Dead` variant here, can specify the character which is inserted when pressing the
///   dead-key twice.
///
/// [`KeyboardEvent.key`]: https://w3c.github.io/uievents-key/
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Key<Str = SmolStr> {
    /// A simple (unparameterised) action
    Named(NamedKey),

    /// A key string that corresponds to the character typed by the user, taking into account the
    /// user’s current locale setting, and any system-level keyboard mapping overrides that are in
    /// effect.
    Character(Str),

    /// This variant is used when the key cannot be translated to any other variant.
    ///
    /// The native key is provided (if available) in order to allow the user to specify keybindings
    /// for keys which are not defined by this API, mainly through some sort of UI.
    Unidentified(NativeKey),

    /// Contains the text representation of the dead-key when available.
    ///
    /// ## Platform-specific
    /// - **Web:** Always contains `None`
    Dead(Option<char>),
}

impl From<NamedKey> for Key {
    #[inline]
    fn from(action: NamedKey) -> Self {
        Key::Named(action)
    }
}

impl From<NativeKey> for Key {
    #[inline]
    fn from(code: NativeKey) -> Self {
        Key::Unidentified(code)
    }
}

impl<Str> PartialEq<NamedKey> for Key<Str> {
    #[inline]
    fn eq(&self, rhs: &NamedKey) -> bool {
        match self {
            Key::Named(ref a) => a == rhs,
            _ => false,
        }
    }
}

impl<Str: PartialEq<str>> PartialEq<str> for Key<Str> {
    #[inline]
    fn eq(&self, rhs: &str) -> bool {
        match self {
            Key::Character(ref s) => s == rhs,
            _ => false,
        }
    }
}

impl<Str: PartialEq<str>> PartialEq<&str> for Key<Str> {
    #[inline]
    fn eq(&self, rhs: &&str) -> bool {
        self == *rhs
    }
}

impl<Str> PartialEq<NativeKey> for Key<Str> {
    #[inline]
    fn eq(&self, rhs: &NativeKey) -> bool {
        match self {
            Key::Unidentified(ref code) => code == rhs,
            _ => false,
        }
    }
}

impl<Str> PartialEq<Key<Str>> for NativeKey {
    #[inline]
    fn eq(&self, rhs: &Key<Str>) -> bool {
        rhs == self
    }
}

impl Key<SmolStr> {
    /// Convert `Key::Character(SmolStr)` to `Key::Character(&str)` so you can more easily match on
    /// `Key`. All other variants remain unchanged.
    pub fn as_ref(&self) -> Key<&str> {
        match self {
            Key::Named(a) => Key::Named(*a),
            Key::Character(ch) => Key::Character(ch.as_str()),
            Key::Dead(d) => Key::Dead(*d),
            Key::Unidentified(u) => Key::Unidentified(u.clone()),
        }
    }
}

impl Key {
    /// Convert a key to its approximate textual equivalent.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(web_platform)]
    /// # wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);
    /// # #[cfg_attr(web_platform, wasm_bindgen_test::wasm_bindgen_test)]
    /// # fn main() {
    /// use winit::keyboard::{Key, NamedKey};
    ///
    /// assert_eq!(Key::Character("a".into()).to_text(), Some("a"));
    /// assert_eq!(Key::Named(NamedKey::Enter).to_text(), Some("\r"));
    /// assert_eq!(Key::Named(NamedKey::F20).to_text(), None);
    /// # }
    /// ```
    pub fn to_text(&self) -> Option<&str> {
        match self {
            Key::Named(action) => match action {
                NamedKey::Enter => Some("\r"),
                NamedKey::Backspace => Some("\x08"),
                NamedKey::Tab => Some("\t"),
                NamedKey::Escape => Some("\x1b"),
                _ => None,
            },
            Key::Character(ch) => Some(ch.as_str()),
            _ => None,
        }
    }
}

/// The location of the key on the keyboard.
///
/// Certain physical keys on the keyboard can have the same value, but are in different locations.
/// For instance, the Shift key can be on the left or right side of the keyboard, or the number
/// keys can be above the letters or on the numpad. This enum allows the user to differentiate
/// them.
///
/// See the documentation for the [`location`] field on the [`KeyEvent`] struct for more
/// information.
///
/// [`location`]: ../event/struct.KeyEvent.html#structfield.location
/// [`KeyEvent`]: crate::event::KeyEvent
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum KeyLocation {
    /// The key is in its "normal" location on the keyboard.
    ///
    /// For instance, the "1" key above the "Q" key on a QWERTY keyboard will use this location.
    /// This invariant is also returned when the location of the key cannot be identified.
    ///
    /// ![Standard 1 key](https://raw.githubusercontent.com/rust-windowing/winit/master/docs/res/keyboard_standard_1_key.svg)
    ///
    /// <sub>
    ///   For image attribution, see the
    ///   <a href="https://github.com/rust-windowing/winit/blob/master/docs/res/ATTRIBUTION.md">
    ///     ATTRIBUTION.md
    ///   </a>
    ///   file.
    /// </sub>
    Standard,

    /// The key is on the left side of the keyboard.
    ///
    /// For instance, the left Shift key below the Caps Lock key on a QWERTY keyboard will use this
    /// location.
    ///
    /// ![Left Shift key](https://raw.githubusercontent.com/rust-windowing/winit/master/docs/res/keyboard_left_shift_key.svg)
    ///
    /// <sub>
    ///   For image attribution, see the
    ///   <a href="https://github.com/rust-windowing/winit/blob/master/docs/res/ATTRIBUTION.md">
    ///     ATTRIBUTION.md
    ///   </a>
    ///   file.
    /// </sub>
    Left,

    /// The key is on the right side of the keyboard.
    ///
    /// For instance, the right Shift key below the Enter key on a QWERTY keyboard will use this
    /// location.
    ///
    /// ![Right Shift key](https://raw.githubusercontent.com/rust-windowing/winit/master/docs/res/keyboard_right_shift_key.svg)
    ///
    /// <sub>
    ///   For image attribution, see the
    ///   <a href="https://github.com/rust-windowing/winit/blob/master/docs/res/ATTRIBUTION.md">
    ///     ATTRIBUTION.md
    ///   </a>
    ///   file.
    /// </sub>
    Right,

    /// The key is on the numpad.
    ///
    /// For instance, the "1" key on the numpad will use this location.
    ///
    /// ![Numpad 1 key](https://raw.githubusercontent.com/rust-windowing/winit/master/docs/res/keyboard_numpad_1_key.svg)
    ///
    /// <sub>
    ///   For image attribution, see the
    ///   <a href="https://github.com/rust-windowing/winit/blob/master/docs/res/ATTRIBUTION.md">
    ///     ATTRIBUTION.md
    ///   </a>
    ///   file.
    /// </sub>
    Numpad,
}

bitflags! {
    /// Represents the current state of the keyboard modifiers
    ///
    /// Each flag represents a modifier and is set if this modifier is active.
    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    pub struct ModifiersState: u32 {
        /// The "shift" key.
        const SHIFT = 0b100;
        /// The "control" key.
        const CONTROL = 0b100 << 3;
        /// The "alt" key.
        const ALT = 0b100 << 6;
        /// This is the "windows" key on PC and "command" key on Mac.
        const META = 0b100 << 9;
        #[deprecated = "use META instead"]
        const SUPER = Self::META.bits();
    }
}

impl ModifiersState {
    /// Returns `true` if the shift key is pressed.
    pub fn shift_key(&self) -> bool {
        self.intersects(Self::SHIFT)
    }

    /// Returns `true` if the control key is pressed.
    pub fn control_key(&self) -> bool {
        self.intersects(Self::CONTROL)
    }

    /// Returns `true` if the alt key is pressed.
    pub fn alt_key(&self) -> bool {
        self.intersects(Self::ALT)
    }

    /// Returns `true` if the meta key is pressed.
    pub fn meta_key(&self) -> bool {
        self.intersects(Self::META)
    }
}

/// The state of the particular modifiers key.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ModifiersKeyState {
    /// The particular key is pressed.
    Pressed,
    /// The state of the key is unknown.
    #[default]
    Unknown,
}

// NOTE: the exact modifier key is not used to represent modifiers state in the
// first place due to a fact that modifiers state could be changed without any
// key being pressed and on some platforms like Wayland/X11 which key resulted
// in modifiers change is hidden, also, not that it really matters.
//
// The reason this API is even exposed is mostly to provide a way for users
// to treat modifiers differently based on their position, which is required
// on macOS due to their AltGr/Option situation.
bitflags! {
    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    pub(crate) struct ModifiersKeys: u8 {
        const LSHIFT   = 0b0000_0001;
        const RSHIFT   = 0b0000_0010;
        const LCONTROL = 0b0000_0100;
        const RCONTROL = 0b0000_1000;
        const LALT     = 0b0001_0000;
        const RALT     = 0b0010_0000;
        const LMETA    = 0b0100_0000;
        const RMETA    = 0b1000_0000;
        #[deprecated = "use LMETA instead"]
        const LSUPER   = Self::LMETA.bits();
        #[deprecated = "use RMETA instead"]
        const RSUPER   = Self::RMETA.bits();
    }
}
