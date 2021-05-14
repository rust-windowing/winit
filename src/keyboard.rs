//! Types related to the keyboard.

// This file contains a substantial portion of the UI Events Specification by the W3C. In
// particular, the variant names within `Key` and `KeyCode` and their documentation are modified
// versions of contents of the aforementioned specification.
//
// The original documents are:
//
// ### For `Key`
// UI Events KeyboardEvent key Values
// https://www.w3.org/TR/2017/CR-uievents-key-20170601/
// Copyright © 2017 W3C® (MIT, ERCIM, Keio, Beihang).
//
// ### For `KeyCode`
// UI Events KeyboardEvent code Values
// https://www.w3.org/TR/2017/CR-uievents-code-20170601/
// Copyright © 2017 W3C® (MIT, ERCIM, Keio, Beihang).
//
// These documents were used under the terms of the following license. This W3C license as well as
// the W3C short notice apply to the `Key` and `KeyCode` enums and their variants and the
// documentation attached to their variants.

// --------- BEGGINING OF W3C LICENSE --------------------------------------------------------------
//
// License
//
// By obtaining and/or copying this work, you (the licensee) agree that you have read, understood,
// and will comply with the following terms and conditions.
//
// Permission to copy, modify, and distribute this work, with or without modification, for any
// purpose and without fee or royalty is hereby granted, provided that you include the following on
// ALL copies of the work or portions thereof, including modifications:
//
// - The full text of this NOTICE in a location viewable to users of the redistributed or derivative
//   work.
// - Any pre-existing intellectual property disclaimers, notices, or terms and conditions. If none
//   exist, the W3C Software and Document Short Notice should be included.
// - Notice of any changes or modifications, through a copyright statement on the new code or
//   document such as "This software or document includes material copied from or derived from
//   [title and URI of the W3C document]. Copyright © [YEAR] W3C® (MIT, ERCIM, Keio, Beihang)."
//
// Disclaimers
//
// THIS WORK IS PROVIDED "AS IS," AND COPYRIGHT HOLDERS MAKE NO REPRESENTATIONS OR WARRANTIES,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO, WARRANTIES OF MERCHANTABILITY OR FITNESS FOR
// ANY PARTICULAR PURPOSE OR THAT THE USE OF THE SOFTWARE OR DOCUMENT WILL NOT INFRINGE ANY THIRD
// PARTY PATENTS, COPYRIGHTS, TRADEMARKS OR OTHER RIGHTS.
//
// COPYRIGHT HOLDERS WILL NOT BE LIABLE FOR ANY DIRECT, INDIRECT, SPECIAL OR CONSEQUENTIAL DAMAGES
// ARISING OUT OF ANY USE OF THE SOFTWARE OR DOCUMENT.
//
// The name and trademarks of copyright holders may NOT be used in advertising or publicity
// pertaining to the work without specific, written prior permission. Title to copyright in this
// work will at all times remain with copyright holders.
//
// --------- END OF W3C LICENSE --------------------------------------------------------------------

// --------- BEGGINING OF W3C SHORT NOTICE ---------------------------------------------------------
//
// winit: https://github.com/rust-windowing/winit
//
// Copyright © 2021 World Wide Web Consortium, (Massachusetts Institute of Technology, European
// Research Consortium for Informatics and Mathematics, Keio University, Beihang). All Rights
// Reserved. This work is distributed under the W3C® Software License [1] in the hope that it will
// be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE.
//
// [1] http://www.w3.org/Consortium/Legal/copyright-software
//
// --------- END OF W3C SHORT NOTICE ---------------------------------------------------------------

use nameof::name_of;

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
    /// Returns `true` if the super key is pressed.
    pub fn super_key(&self) -> bool {
        self.intersects(Self::SUPER)
    }
}

bitflags! {
    /// Represents the current state of the keyboard modifiers
    ///
    /// Each flag represents a modifier and is set if this modifier is active.
    #[derive(Default)]
    pub struct ModifiersState: u32 {
        // left and right modifiers are currently commented out, but we should be able to support
        // them in a future release
        /// The "shift" key.
        const SHIFT = 0b100 << 0;
        // const LSHIFT = 0b010 << 0;
        // const RSHIFT = 0b001 << 0;
        /// The "control" key.
        const CONTROL = 0b100 << 3;
        // const LCTRL = 0b010 << 3;
        // const RCTRL = 0b001 << 3;
        /// The "alt" key.
        const ALT = 0b100 << 6;
        // const LALT = 0b010 << 6;
        // const RALT = 0b001 << 6;
        /// This is the "windows" key on PC and "command" key on Mac.
        const SUPER = 0b100 << 9;
        // const LSUPER  = 0b010 << 9;
        // const RSUPER  = 0b001 << 9;
    }
}

#[cfg(feature = "serde")]
mod modifiers_serde {
    use super::ModifiersState;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Default, Serialize, Deserialize)]
    #[serde(default)]
    #[serde(rename = "ModifiersState")]
    pub struct ModifiersStateSerialize {
        pub shift_key: bool,
        pub control_key: bool,
        pub alt_key: bool,
        pub super_key: bool,
    }

    impl Serialize for ModifiersState {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let s = ModifiersStateSerialize {
                shift_key: self.shift_key(),
                control_key: self.control_key(),
                alt_key: self.alt_key(),
                super_key: self.super_key(),
            };
            s.serialize(serializer)
        }
    }

    impl<'de> Deserialize<'de> for ModifiersState {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            let ModifiersStateSerialize {
                shift_key,
                control_key,
                alt_key,
                super_key,
            } = ModifiersStateSerialize::deserialize(deserializer)?;
            let mut m = ModifiersState::empty();
            m.set(ModifiersState::SHIFT, shift_key);
            m.set(ModifiersState::CONTROL, control_key);
            m.set(ModifiersState::ALT, alt_key);
            m.set(ModifiersState::SUPER, super_key);
            Ok(m)
        }
    }
}

/// Contains the platform-native physical key identifier (aka scancode)
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum NativeKeyCode {
    Unidentified,
    Windows(u16),
    MacOS(u32),
    XKB(u32),
    Web(&'static str),
}
impl std::fmt::Debug for NativeKeyCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use NativeKeyCode::{MacOS, Unidentified, Web, Windows, XKB};
        let mut debug_tuple;
        match self {
            Unidentified => {
                debug_tuple = f.debug_tuple(name_of!(Unidentified));
            }
            Windows(v) => {
                debug_tuple = f.debug_tuple(name_of!(Windows));
                debug_tuple.field(&format_args!("0x{:04X}", v));
            }
            MacOS(v) => {
                debug_tuple = f.debug_tuple(name_of!(MacOS));
                debug_tuple.field(v);
            }
            XKB(v) => {
                debug_tuple = f.debug_tuple(name_of!(XKB));
                debug_tuple.field(v);
            }
            Web(v) => {
                debug_tuple = f.debug_tuple(name_of!(Web));
                debug_tuple.field(v);
            }
        }
        debug_tuple.finish()
    }
}

/// Represents the location of a physical key.
///
/// This mostly conforms to the UI Events Specification's [`KeyboardEvent.code`] with a few
/// exceptions:
/// - The keys that the specification calls "MetaLeft" and "MetaRight" are named "SuperLeft" and
///   "SuperRight" here.
/// - The key that the specification calls "Super" is reported as `Unidentified` here.
/// - The `Unidentified` variant here, can still identifiy a key through it's `NativeKeyCode`.
///
/// [`KeyboardEvent.code`]: https://w3c.github.io/uievents-code/#code-value-tables
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum KeyCode {
    /// This variant is used when the key cannot be translated to any
    /// other variant.
    ///
    /// The native scancode is provided (if available) in order
    /// to allow the user to specify keybindings for keys which
    /// are not defined by this API.
    Unidentified(NativeKeyCode),
    /// <kbd>`</kbd> on a US keyboard. This is the <kbd>半角</kbd>/<kbd>全角</kbd>/<kbd>漢字</kbd>
    /// (hankaku/zenkaku/kanji) key on Japanese keyboards
    Backquote,
    /// Used for both the US <kbd>\\</kbd> (on the 101-key layout) and also for the key
    /// located between the <kbd>"</kbd> and <kbd>Enter</kbd> keys on row C of the 102-,
    /// 104- and 106-key layouts.
    /// Labeled <kbd>#</kbd> on a UK (102) keyboard.
    Backslash,
    /// <kbd>[</kbd> on a US keyboard.
    BracketLeft,
    /// <kbd>]</kbd> on a US keyboard.
    BracketRight,
    /// <kbd>,</kbd> on a US keyboard.
    Comma,
    /// <kbd>0</kbd> on a US keyboard.
    Digit0,
    /// <kbd>1</kbd> on a US keyboard.
    Digit1,
    /// <kbd>2</kbd> on a US keyboard.
    Digit2,
    /// <kbd>3</kbd> on a US keyboard.
    Digit3,
    /// <kbd>4</kbd> on a US keyboard.
    Digit4,
    /// <kbd>5</kbd> on a US keyboard.
    Digit5,
    /// <kbd>6</kbd> on a US keyboard.
    Digit6,
    /// <kbd>7</kbd> on a US keyboard.
    Digit7,
    /// <kbd>8</kbd> on a US keyboard.
    Digit8,
    /// <kbd>9</kbd> on a US keyboard.
    Digit9,
    /// <kbd>=</kbd> on a US keyboard.
    Equal,
    /// Located between the left <kbd>Shift</kbd> and <kbd>Z</kbd> keys.
    /// Labeled <kbd>\\</kbd> on a UK keyboard.
    IntlBackslash,
    /// Located between the <kbd>/</kbd> and right <kbd>Shift</kbd> keys.
    /// Labeled <kbd>\\</kbd> (ro) on a Japanese keyboard.
    IntlRo,
    /// Located between the <kbd>=</kbd> and <kbd>Backspace</kbd> keys.
    /// Labeled <kbd>¥</kbd> (yen) on a Japanese keyboard. <kbd>\\</kbd> on a
    /// Russian keyboard.
    IntlYen,
    /// <kbd>a</kbd> on a US keyboard.
    /// Labeled <kbd>q</kbd> on an AZERTY (e.g., French) keyboard.
    KeyA,
    /// <kbd>b</kbd> on a US keyboard.
    KeyB,
    /// <kbd>c</kbd> on a US keyboard.
    KeyC,
    /// <kbd>d</kbd> on a US keyboard.
    KeyD,
    /// <kbd>e</kbd> on a US keyboard.
    KeyE,
    /// <kbd>f</kbd> on a US keyboard.
    KeyF,
    /// <kbd>g</kbd> on a US keyboard.
    KeyG,
    /// <kbd>h</kbd> on a US keyboard.
    KeyH,
    /// <kbd>i</kbd> on a US keyboard.
    KeyI,
    /// <kbd>j</kbd> on a US keyboard.
    KeyJ,
    /// <kbd>k</kbd> on a US keyboard.
    KeyK,
    /// <kbd>l</kbd> on a US keyboard.
    KeyL,
    /// <kbd>m</kbd> on a US keyboard.
    KeyM,
    /// <kbd>n</kbd> on a US keyboard.
    KeyN,
    /// <kbd>o</kbd> on a US keyboard.
    KeyO,
    /// <kbd>p</kbd> on a US keyboard.
    KeyP,
    /// <kbd>q</kbd> on a US keyboard.
    /// Labeled <kbd>a</kbd> on an AZERTY (e.g., French) keyboard.
    KeyQ,
    /// <kbd>r</kbd> on a US keyboard.
    KeyR,
    /// <kbd>s</kbd> on a US keyboard.
    KeyS,
    /// <kbd>t</kbd> on a US keyboard.
    KeyT,
    /// <kbd>u</kbd> on a US keyboard.
    KeyU,
    /// <kbd>v</kbd> on a US keyboard.
    KeyV,
    /// <kbd>w</kbd> on a US keyboard.
    /// Labeled <kbd>z</kbd> on an AZERTY (e.g., French) keyboard.
    KeyW,
    /// <kbd>x</kbd> on a US keyboard.
    KeyX,
    /// <kbd>y</kbd> on a US keyboard.
    /// Labeled <kbd>z</kbd> on a QWERTZ (e.g., German) keyboard.
    KeyY,
    /// <kbd>z</kbd> on a US keyboard.
    /// Labeled <kbd>w</kbd> on an AZERTY (e.g., French) keyboard, and <kbd>y</kbd> on a
    /// QWERTZ (e.g., German) keyboard.
    KeyZ,
    /// <kbd>-</kbd> on a US keyboard.
    Minus,
    /// <kbd>.</kbd> on a US keyboard.
    Period,
    /// <kbd>'</kbd> on a US keyboard.
    Quote,
    /// <kbd>;</kbd> on a US keyboard.
    Semicolon,
    /// <kbd>/</kbd> on a US keyboard.
    Slash,
    /// <kbd>Alt</kbd>, <kbd>Option</kbd>, or <kbd>⌥</kbd>.
    AltLeft,
    /// <kbd>Alt</kbd>, <kbd>Option</kbd>, or <kbd>⌥</kbd>.
    /// This is labeled <kbd>AltGr</kbd> on many keyboard layouts.
    AltRight,
    /// <kbd>Backspace</kbd> or <kbd>⌫</kbd>.
    /// Labeled <kbd>Delete</kbd> on Apple keyboards.
    Backspace,
    /// <kbd>CapsLock</kbd> or <kbd>⇪</kbd>
    CapsLock,
    /// The application context menu key, which is typically found between the right
    /// <kbd>Super</kbd> key and the right <kbd>Control</kbd> key.
    ContextMenu,
    /// <kbd>Control</kbd> or <kbd>⌃</kbd>
    ControlLeft,
    /// <kbd>Control</kbd> or <kbd>⌃</kbd>
    ControlRight,
    /// <kbd>Enter</kbd> or <kbd>↵</kbd>. Labeled <kbd>Return</kbd> on Apple keyboards.
    Enter,
    /// The Windows, <kbd>⌘</kbd>, <kbd>Command</kbd>, or other OS symbol key.
    SuperLeft,
    /// The Windows, <kbd>⌘</kbd>, <kbd>Command</kbd>, or other OS symbol key.
    SuperRight,
    /// <kbd>Shift</kbd> or <kbd>⇧</kbd>
    ShiftLeft,
    /// <kbd>Shift</kbd> or <kbd>⇧</kbd>
    ShiftRight,
    /// <kbd> </kbd> (space)
    Space,
    /// <kbd>Tab</kbd> or <kbd>⇥</kbd>
    Tab,
    /// Japanese: <kbd>変</kbd> (henkan)
    Convert,
    /// Japanese: <kbd>カタカナ</kbd>/<kbd>ひらがな</kbd>/<kbd>ローマ字</kbd> (katakana/hiragana/romaji)
    KanaMode,
    /// Korean: HangulMode <kbd>한/영</kbd> (han/yeong)
    ///
    /// Japanese (Mac keyboard): <kbd>か</kbd> (kana)
    Lang1,
    /// Korean: Hanja <kbd>한</kbd> (hanja)
    ///
    /// Japanese (Mac keyboard): <kbd>英</kbd> (eisu)
    Lang2,
    /// Japanese (word-processing keyboard): Katakana
    Lang3,
    /// Japanese (word-processing keyboard): Hiragana
    Lang4,
    /// Japanese (word-processing keyboard): Zenkaku/Hankaku
    Lang5,
    /// Japanese: <kbd>無変換</kbd> (muhenkan)
    NonConvert,
    /// <kbd>⌦</kbd>. The forward delete key.
    /// Note that on Apple keyboards, the key labelled <kbd>Delete</kbd> on the main part of
    /// the keyboard is encoded as [`Backspace`].
    ///
    /// [`Backspace`]: Self::Backspace
    Delete,
    /// <kbd>Page Down</kbd>, <kbd>End</kbd>, or <kbd>↘</kbd>
    End,
    /// <kbd>Help</kbd>. Not present on standard PC keyboards.
    Help,
    /// <kbd>Home</kbd> or <kbd>↖</kbd>
    Home,
    /// <kbd>Insert</kbd> or <kbd>Ins</kbd>. Not present on Apple keyboards.
    Insert,
    /// <kbd>Page Down</kbd>, <kbd>PgDn</kbd>, or <kbd>⇟</kbd>
    PageDown,
    /// <kbd>Page Up</kbd>, <kbd>PgUp</kbd>, or <kbd>⇞</kbd>
    PageUp,
    /// <kbd>↓</kbd>
    ArrowDown,
    /// <kbd>←</kbd>
    ArrowLeft,
    /// <kbd>→</kbd>
    ArrowRight,
    /// <kbd>↑</kbd>
    ArrowUp,
    /// On the Mac, this is used for the numpad <kbd>Clear</kbd> key.
    NumLock,
    /// <kbd>0 Ins</kbd> on a keyboard. <kbd>0</kbd> on a phone or remote control
    Numpad0,
    /// <kbd>1 End</kbd> on a keyboard. <kbd>1</kbd> or <kbd>1 QZ</kbd> on a phone or remote control
    Numpad1,
    /// <kbd>2 ↓</kbd> on a keyboard. <kbd>2 ABC</kbd> on a phone or remote control
    Numpad2,
    /// <kbd>3 PgDn</kbd> on a keyboard. <kbd>3 DEF</kbd> on a phone or remote control
    Numpad3,
    /// <kbd>4 ←</kbd> on a keyboard. <kbd>4 GHI</kbd> on a phone or remote control
    Numpad4,
    /// <kbd>5</kbd> on a keyboard. <kbd>5 JKL</kbd> on a phone or remote control
    Numpad5,
    /// <kbd>6 →</kbd> on a keyboard. <kbd>6 MNO</kbd> on a phone or remote control
    Numpad6,
    /// <kbd>7 Home</kbd> on a keyboard. <kbd>7 PQRS</kbd> or <kbd>7 PRS</kbd> on a phone
    /// or remote control
    Numpad7,
    /// <kbd>8 ↑</kbd> on a keyboard. <kbd>8 TUV</kbd> on a phone or remote control
    Numpad8,
    /// <kbd>9 PgUp</kbd> on a keyboard. <kbd>9 WXYZ</kbd> or <kbd>9 WXY</kbd> on a phone
    /// or remote control
    Numpad9,
    /// <kbd>+</kbd>
    NumpadAdd,
    /// Found on the Microsoft Natural Keyboard.
    NumpadBackspace,
    /// <kbd>C</kbd> or <kbd>A</kbd> (All Clear). Also for use with numpads that have a
    /// <kbd>Clear</kbd> key that is separate from the <kbd>NumLock</kbd> key. On the Mac, the
    /// numpad <kbd>Clear</kbd> key is encoded as [`NumLock`].
    ///
    /// [`NumLock`]: Self::NumLock
    NumpadClear,
    /// <kbd>C</kbd> (Clear Entry)
    NumpadClearEntry,
    /// <kbd>,</kbd> (thousands separator). For locales where the thousands separator
    /// is a "." (e.g., Brazil), this key may generate a <kbd>.</kbd>.
    NumpadComma,
    /// <kbd>. Del</kbd>. For locales where the decimal separator is "," (e.g.,
    /// Brazil), this key may generate a <kbd>,</kbd>.
    NumpadDecimal,
    /// <kbd>/</kbd>
    NumpadDivide,
    NumpadEnter,
    /// <kbd>=</kbd>
    NumpadEqual,
    /// <kbd>#</kbd> on a phone or remote control device. This key is typically found
    /// below the <kbd>9</kbd> key and to the right of the <kbd>0</kbd> key.
    NumpadHash,
    /// <kbd>M</kbd> Add current entry to the value stored in memory.
    NumpadMemoryAdd,
    /// <kbd>M</kbd> Clear the value stored in memory.
    NumpadMemoryClear,
    /// <kbd>M</kbd> Replace the current entry with the value stored in memory.
    NumpadMemoryRecall,
    /// <kbd>M</kbd> Replace the value stored in memory with the current entry.
    NumpadMemoryStore,
    /// <kbd>M</kbd> Subtract current entry from the value stored in memory.
    NumpadMemorySubtract,
    /// <kbd>*</kbd> on a keyboard. For use with numpads that provide mathematical
    /// operations (<kbd>+</kbd>, <kbd>-</kbd> <kbd>*</kbd> and <kbd>/</kbd>).
    ///
    /// Use `NumpadStar` for the <kbd>*</kbd> key on phones and remote controls.
    NumpadMultiply,
    /// <kbd>(</kbd> Found on the Microsoft Natural Keyboard.
    NumpadParenLeft,
    /// <kbd>)</kbd> Found on the Microsoft Natural Keyboard.
    NumpadParenRight,
    /// <kbd>*</kbd> on a phone or remote control device.
    ///
    /// This key is typically found below the <kbd>7</kbd> key and to the left of
    /// the <kbd>0</kbd> key.
    ///
    /// Use <kbd>"NumpadMultiply"</kbd> for the <kbd>*</kbd> key on
    /// numeric keypads.
    NumpadStar,
    /// <kbd>-</kbd>
    NumpadSubtract,
    /// <kbd>Esc</kbd> or <kbd>⎋</kbd>
    Escape,
    /// <kbd>Fn</kbd> This is typically a hardware key that does not generate a separate code.
    Fn,
    /// <kbd>FLock</kbd> or <kbd>FnLock</kbd>. Function Lock key. Found on the Microsoft
    /// Natural Keyboard.
    FnLock,
    /// <kbd>PrtScr SysRq</kbd> or <kbd>Print Screen</kbd>
    PrintScreen,
    /// <kbd>Scroll Lock</kbd>
    ScrollLock,
    /// <kbd>Pause Break</kbd>
    Pause,
    /// Some laptops place this key to the left of the <kbd>↑</kbd> key.
    BrowserBack,
    BrowserFavorites,
    /// Some laptops place this key to the right of the <kbd>↑</kbd> key.
    BrowserForward,
    BrowserHome,
    BrowserRefresh,
    BrowserSearch,
    BrowserStop,
    /// <kbd>Eject</kbd> or <kbd>⏏</kbd>. This key is placed in the function section on some Apple
    /// keyboards.
    Eject,
    /// Sometimes labelled <kbd>My Computer</kbd> on the keyboard
    LaunchApp1,
    /// Sometimes labelled <kbd>Calculator</kbd> on the keyboard
    LaunchApp2,
    LaunchMail,
    MediaPlayPause,
    MediaSelect,
    MediaStop,
    MediaTrackNext,
    MediaTrackPrevious,
    /// This key is placed in the function section on some Apple keyboards, replacing the
    /// <kbd>Eject</kbd> key.
    Power,
    Sleep,
    AudioVolumeDown,
    AudioVolumeMute,
    AudioVolumeUp,
    WakeUp,
    Hyper,
    Turbo,
    Abort,
    Resume,
    Suspend,
    /// Found on Sun’s USB keyboard.
    Again,
    /// Found on Sun’s USB keyboard.
    Copy,
    /// Found on Sun’s USB keyboard.
    Cut,
    /// Found on Sun’s USB keyboard.
    Find,
    /// Found on Sun’s USB keyboard.
    Open,
    /// Found on Sun’s USB keyboard.
    Paste,
    /// Found on Sun’s USB keyboard.
    Props,
    /// Found on Sun’s USB keyboard.
    Select,
    /// Found on Sun’s USB keyboard.
    Undo,
    /// Use for dedicated <kbd>ひらがな</kbd> key found on some Japanese word processing keyboards.
    Hiragana,
    /// Use for dedicated <kbd>カタカナ</kbd> key found on some Japanese word processing keyboards.
    Katakana,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F1,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F2,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F3,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F4,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F5,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F6,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F7,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F8,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F9,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F10,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F11,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F12,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F13,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F14,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F15,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F16,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F17,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F18,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F19,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F20,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F21,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F22,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F23,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F24,
    /// General-purpose function key.
    F25,
    /// General-purpose function key.
    F26,
    /// General-purpose function key.
    F27,
    /// General-purpose function key.
    F28,
    /// General-purpose function key.
    F29,
    /// General-purpose function key.
    F30,
    /// General-purpose function key.
    F31,
    /// General-purpose function key.
    F32,
    /// General-purpose function key.
    F33,
    /// General-purpose function key.
    F34,
    /// General-purpose function key.
    F35,
}

impl KeyCode {
    pub fn from_key_code_attribute_value(kcav: &str) -> Self {
        match kcav {
            "Backquote" => KeyCode::Backquote,
            "Backslash" => KeyCode::Backslash,
            "BracketLeft" => KeyCode::BracketLeft,
            "BracketRight" => KeyCode::BracketRight,
            "Comma" => KeyCode::Comma,
            "Digit0" => KeyCode::Digit0,
            "Digit1" => KeyCode::Digit1,
            "Digit2" => KeyCode::Digit2,
            "Digit3" => KeyCode::Digit3,
            "Digit4" => KeyCode::Digit4,
            "Digit5" => KeyCode::Digit5,
            "Digit6" => KeyCode::Digit6,
            "Digit7" => KeyCode::Digit7,
            "Digit8" => KeyCode::Digit8,
            "Digit9" => KeyCode::Digit9,
            "Equal" => KeyCode::Equal,
            "IntlBackslash" => KeyCode::IntlBackslash,
            "IntlRo" => KeyCode::IntlRo,
            "IntlYen" => KeyCode::IntlYen,
            "KeyA" => KeyCode::KeyA,
            "KeyB" => KeyCode::KeyB,
            "KeyC" => KeyCode::KeyC,
            "KeyD" => KeyCode::KeyD,
            "KeyE" => KeyCode::KeyE,
            "KeyF" => KeyCode::KeyF,
            "KeyG" => KeyCode::KeyG,
            "KeyH" => KeyCode::KeyH,
            "KeyI" => KeyCode::KeyI,
            "KeyJ" => KeyCode::KeyJ,
            "KeyK" => KeyCode::KeyK,
            "KeyL" => KeyCode::KeyL,
            "KeyM" => KeyCode::KeyM,
            "KeyN" => KeyCode::KeyN,
            "KeyO" => KeyCode::KeyO,
            "KeyP" => KeyCode::KeyP,
            "KeyQ" => KeyCode::KeyQ,
            "KeyR" => KeyCode::KeyR,
            "KeyS" => KeyCode::KeyS,
            "KeyT" => KeyCode::KeyT,
            "KeyU" => KeyCode::KeyU,
            "KeyV" => KeyCode::KeyV,
            "KeyW" => KeyCode::KeyW,
            "KeyX" => KeyCode::KeyX,
            "KeyY" => KeyCode::KeyY,
            "KeyZ" => KeyCode::KeyZ,
            "Minus" => KeyCode::Minus,
            "Period" => KeyCode::Period,
            "Quote" => KeyCode::Quote,
            "Semicolon" => KeyCode::Semicolon,
            "Slash" => KeyCode::Slash,
            "AltLeft" => KeyCode::AltLeft,
            "AltRight" => KeyCode::AltRight,
            "Backspace" => KeyCode::Backspace,
            "CapsLock" => KeyCode::CapsLock,
            "ContextMenu" => KeyCode::ContextMenu,
            "ControlLeft" => KeyCode::ControlLeft,
            "ControlRight" => KeyCode::ControlRight,
            "Enter" => KeyCode::Enter,
            "MetaLeft" => KeyCode::SuperLeft,
            "MetaRight" => KeyCode::SuperRight,
            "ShiftLeft" => KeyCode::ShiftLeft,
            "ShiftRight" => KeyCode::ShiftRight,
            " " => KeyCode::Space,
            "Tab" => KeyCode::Tab,
            "Convert" => KeyCode::Convert,
            "KanaMode" => KeyCode::KanaMode,
            "Lang1" => KeyCode::Lang1,
            "Lang2" => KeyCode::Lang2,
            "Lang3" => KeyCode::Lang3,
            "Lang4" => KeyCode::Lang4,
            "Lang5" => KeyCode::Lang5,
            "NonConvert" => KeyCode::NonConvert,
            "Delete" => KeyCode::Delete,
            "End" => KeyCode::End,
            "Help" => KeyCode::Help,
            "Home" => KeyCode::Home,
            "Insert" => KeyCode::Insert,
            "PageDown" => KeyCode::PageDown,
            "PageUp" => KeyCode::PageUp,
            "ArrowDown" => KeyCode::ArrowDown,
            "ArrowLeft" => KeyCode::ArrowLeft,
            "ArrowRight" => KeyCode::ArrowRight,
            "ArrowUp" => KeyCode::ArrowUp,
            "NumLock" => KeyCode::NumLock,
            "Numpad0" => KeyCode::Numpad0,
            "Numpad1" => KeyCode::Numpad1,
            "Numpad2" => KeyCode::Numpad2,
            "Numpad3" => KeyCode::Numpad3,
            "Numpad4" => KeyCode::Numpad4,
            "Numpad5" => KeyCode::Numpad5,
            "Numpad6" => KeyCode::Numpad6,
            "Numpad7" => KeyCode::Numpad7,
            "Numpad8" => KeyCode::Numpad8,
            "Numpad9" => KeyCode::Numpad9,
            "NumpadAdd" => KeyCode::NumpadAdd,
            "NumpadBackspace" => KeyCode::NumpadBackspace,
            "NumpadClear" => KeyCode::NumpadClear,
            "NumpadClearEntry" => KeyCode::NumpadClearEntry,
            "NumpadComma" => KeyCode::NumpadComma,
            "NumpadDecimal" => KeyCode::NumpadDecimal,
            "NumpadDivide" => KeyCode::NumpadDivide,
            "NumpadEnter" => KeyCode::NumpadEnter,
            "NumpadEqual" => KeyCode::NumpadEqual,
            "NumpadHash" => KeyCode::NumpadHash,
            "NumpadMemoryAdd" => KeyCode::NumpadMemoryAdd,
            "NumpadMemoryClear" => KeyCode::NumpadMemoryClear,
            "NumpadMemoryRecall" => KeyCode::NumpadMemoryRecall,
            "NumpadMemoryStore" => KeyCode::NumpadMemoryStore,
            "NumpadMemorySubtract" => KeyCode::NumpadMemorySubtract,
            "NumpadMultiply" => KeyCode::NumpadMultiply,
            "NumpadParenLeft" => KeyCode::NumpadParenLeft,
            "NumpadParenRight" => KeyCode::NumpadParenRight,
            "NumpadStar" => KeyCode::NumpadStar,
            "NumpadSubtract" => KeyCode::NumpadSubtract,
            "Escape" => KeyCode::Escape,
            "Fn" => KeyCode::Fn,
            "FnLock" => KeyCode::FnLock,
            "PrintScreen" => KeyCode::PrintScreen,
            "ScrollLock" => KeyCode::ScrollLock,
            "Pause" => KeyCode::Pause,
            "BrowserBack" => KeyCode::BrowserBack,
            "BrowserFavorites" => KeyCode::BrowserFavorites,
            "BrowserForward" => KeyCode::BrowserForward,
            "BrowserHome" => KeyCode::BrowserHome,
            "BrowserRefresh" => KeyCode::BrowserRefresh,
            "BrowserSearch" => KeyCode::BrowserSearch,
            "BrowserStop" => KeyCode::BrowserStop,
            "Eject" => KeyCode::Eject,
            "LaunchApp1" => KeyCode::LaunchApp1,
            "LaunchApp2" => KeyCode::LaunchApp2,
            "LaunchMail" => KeyCode::LaunchMail,
            "MediaPlayPause" => KeyCode::MediaPlayPause,
            "MediaSelect" => KeyCode::MediaSelect,
            "MediaStop" => KeyCode::MediaStop,
            "MediaTrackNext" => KeyCode::MediaTrackNext,
            "MediaTrackPrevious" => KeyCode::MediaTrackPrevious,
            "Power" => KeyCode::Power,
            "Sleep" => KeyCode::Sleep,
            "AudioVolumeDown" => KeyCode::AudioVolumeDown,
            "AudioVolumeMute" => KeyCode::AudioVolumeMute,
            "AudioVolumeUp" => KeyCode::AudioVolumeUp,
            "WakeUp" => KeyCode::WakeUp,
            "Hyper" => KeyCode::Hyper,
            "Turbo" => KeyCode::Turbo,
            "Abort" => KeyCode::Abort,
            "Resume" => KeyCode::Resume,
            "Suspend" => KeyCode::Suspend,
            "Again" => KeyCode::Again,
            "Copy" => KeyCode::Copy,
            "Cut" => KeyCode::Cut,
            "Find" => KeyCode::Find,
            "Open" => KeyCode::Open,
            "Paste" => KeyCode::Paste,
            "Props" => KeyCode::Props,
            "Select" => KeyCode::Select,
            "Undo" => KeyCode::Undo,
            "Hiragana" => KeyCode::Hiragana,
            "Katakana" => KeyCode::Katakana,
            "F1" => KeyCode::F1,
            "F2" => KeyCode::F2,
            "F3" => KeyCode::F3,
            "F4" => KeyCode::F4,
            "F5" => KeyCode::F5,
            "F6" => KeyCode::F6,
            "F7" => KeyCode::F7,
            "F8" => KeyCode::F8,
            "F9" => KeyCode::F9,
            "F10" => KeyCode::F10,
            "F11" => KeyCode::F11,
            "F12" => KeyCode::F12,
            "F13" => KeyCode::F13,
            "F14" => KeyCode::F14,
            "F15" => KeyCode::F15,
            "F16" => KeyCode::F16,
            "F17" => KeyCode::F17,
            "F18" => KeyCode::F18,
            "F19" => KeyCode::F19,
            "F20" => KeyCode::F20,
            "F21" => KeyCode::F21,
            "F22" => KeyCode::F22,
            "F23" => KeyCode::F23,
            "F24" => KeyCode::F24,
            "F25" => KeyCode::F25,
            "F26" => KeyCode::F26,
            "F27" => KeyCode::F27,
            "F28" => KeyCode::F28,
            "F29" => KeyCode::F29,
            "F30" => KeyCode::F30,
            "F31" => KeyCode::F31,
            "F32" => KeyCode::F32,
            "F33" => KeyCode::F33,
            "F34" => KeyCode::F34,
            "F35" => KeyCode::F35,
            // TODO: Fix unbounded leak
            string @ _ => KeyCode::Unidentified(NativeKeyCode::Web(Box::leak(
                String::from(string).into_boxed_str(),
            ))),
        }
    }

    pub fn to_key_code_attribute_value(&self) -> &'static str {
        match self {
            KeyCode::Unidentified(_) => "Unidentified",
            KeyCode::Backquote => "Backquote",
            KeyCode::Backslash => "Backslash",
            KeyCode::BracketLeft => "BracketLeft",
            KeyCode::BracketRight => "BracketRight",
            KeyCode::Comma => "Comma",
            KeyCode::Digit0 => "Digit0",
            KeyCode::Digit1 => "Digit1",
            KeyCode::Digit2 => "Digit2",
            KeyCode::Digit3 => "Digit3",
            KeyCode::Digit4 => "Digit4",
            KeyCode::Digit5 => "Digit5",
            KeyCode::Digit6 => "Digit6",
            KeyCode::Digit7 => "Digit7",
            KeyCode::Digit8 => "Digit8",
            KeyCode::Digit9 => "Digit9",
            KeyCode::Equal => "Equal",
            KeyCode::IntlBackslash => "IntlBackslash",
            KeyCode::IntlRo => "IntlRo",
            KeyCode::IntlYen => "IntlYen",
            KeyCode::KeyA => "KeyA",
            KeyCode::KeyB => "KeyB",
            KeyCode::KeyC => "KeyC",
            KeyCode::KeyD => "KeyD",
            KeyCode::KeyE => "KeyE",
            KeyCode::KeyF => "KeyF",
            KeyCode::KeyG => "KeyG",
            KeyCode::KeyH => "KeyH",
            KeyCode::KeyI => "KeyI",
            KeyCode::KeyJ => "KeyJ",
            KeyCode::KeyK => "KeyK",
            KeyCode::KeyL => "KeyL",
            KeyCode::KeyM => "KeyM",
            KeyCode::KeyN => "KeyN",
            KeyCode::KeyO => "KeyO",
            KeyCode::KeyP => "KeyP",
            KeyCode::KeyQ => "KeyQ",
            KeyCode::KeyR => "KeyR",
            KeyCode::KeyS => "KeyS",
            KeyCode::KeyT => "KeyT",
            KeyCode::KeyU => "KeyU",
            KeyCode::KeyV => "KeyV",
            KeyCode::KeyW => "KeyW",
            KeyCode::KeyX => "KeyX",
            KeyCode::KeyY => "KeyY",
            KeyCode::KeyZ => "KeyZ",
            KeyCode::Minus => "Minus",
            KeyCode::Period => "Period",
            KeyCode::Quote => "Quote",
            KeyCode::Semicolon => "Semicolon",
            KeyCode::Slash => "Slash",
            KeyCode::AltLeft => "AltLeft",
            KeyCode::AltRight => "AltRight",
            KeyCode::Backspace => "Backspace",
            KeyCode::CapsLock => "CapsLock",
            KeyCode::ContextMenu => "ContextMenu",
            KeyCode::ControlLeft => "ControlLeft",
            KeyCode::ControlRight => "ControlRight",
            KeyCode::Enter => "Enter",
            KeyCode::SuperLeft => "MetaLeft",
            KeyCode::SuperRight => "MetaRight",
            KeyCode::ShiftLeft => "ShiftLeft",
            KeyCode::ShiftRight => "ShiftRight",
            KeyCode::Space => " ",
            KeyCode::Tab => "Tab",
            KeyCode::Convert => "Convert",
            KeyCode::KanaMode => "KanaMode",
            KeyCode::Lang1 => "Lang1",
            KeyCode::Lang2 => "Lang2",
            KeyCode::Lang3 => "Lang3",
            KeyCode::Lang4 => "Lang4",
            KeyCode::Lang5 => "Lang5",
            KeyCode::NonConvert => "NonConvert",
            KeyCode::Delete => "Delete",
            KeyCode::End => "End",
            KeyCode::Help => "Help",
            KeyCode::Home => "Home",
            KeyCode::Insert => "Insert",
            KeyCode::PageDown => "PageDown",
            KeyCode::PageUp => "PageUp",
            KeyCode::ArrowDown => "ArrowDown",
            KeyCode::ArrowLeft => "ArrowLeft",
            KeyCode::ArrowRight => "ArrowRight",
            KeyCode::ArrowUp => "ArrowUp",
            KeyCode::NumLock => "NumLock",
            KeyCode::Numpad0 => "Numpad0",
            KeyCode::Numpad1 => "Numpad1",
            KeyCode::Numpad2 => "Numpad2",
            KeyCode::Numpad3 => "Numpad3",
            KeyCode::Numpad4 => "Numpad4",
            KeyCode::Numpad5 => "Numpad5",
            KeyCode::Numpad6 => "Numpad6",
            KeyCode::Numpad7 => "Numpad7",
            KeyCode::Numpad8 => "Numpad8",
            KeyCode::Numpad9 => "Numpad9",
            KeyCode::NumpadAdd => "NumpadAdd",
            KeyCode::NumpadBackspace => "NumpadBackspace",
            KeyCode::NumpadClear => "NumpadClear",
            KeyCode::NumpadClearEntry => "NumpadClearEntry",
            KeyCode::NumpadComma => "NumpadComma",
            KeyCode::NumpadDecimal => "NumpadDecimal",
            KeyCode::NumpadDivide => "NumpadDivide",
            KeyCode::NumpadEnter => "NumpadEnter",
            KeyCode::NumpadEqual => "NumpadEqual",
            KeyCode::NumpadHash => "NumpadHash",
            KeyCode::NumpadMemoryAdd => "NumpadMemoryAdd",
            KeyCode::NumpadMemoryClear => "NumpadMemoryClear",
            KeyCode::NumpadMemoryRecall => "NumpadMemoryRecall",
            KeyCode::NumpadMemoryStore => "NumpadMemoryStore",
            KeyCode::NumpadMemorySubtract => "NumpadMemorySubtract",
            KeyCode::NumpadMultiply => "NumpadMultiply",
            KeyCode::NumpadParenLeft => "NumpadParenLeft",
            KeyCode::NumpadParenRight => "NumpadParenRight",
            KeyCode::NumpadStar => "NumpadStar",
            KeyCode::NumpadSubtract => "NumpadSubtract",
            KeyCode::Escape => "Escape",
            KeyCode::Fn => "Fn",
            KeyCode::FnLock => "FnLock",
            KeyCode::PrintScreen => "PrintScreen",
            KeyCode::ScrollLock => "ScrollLock",
            KeyCode::Pause => "Pause",
            KeyCode::BrowserBack => "BrowserBack",
            KeyCode::BrowserFavorites => "BrowserFavorites",
            KeyCode::BrowserForward => "BrowserForward",
            KeyCode::BrowserHome => "BrowserHome",
            KeyCode::BrowserRefresh => "BrowserRefresh",
            KeyCode::BrowserSearch => "BrowserSearch",
            KeyCode::BrowserStop => "BrowserStop",
            KeyCode::Eject => "Eject",
            KeyCode::LaunchApp1 => "LaunchApp1",
            KeyCode::LaunchApp2 => "LaunchApp2",
            KeyCode::LaunchMail => "LaunchMail",
            KeyCode::MediaPlayPause => "MediaPlayPause",
            KeyCode::MediaSelect => "MediaSelect",
            KeyCode::MediaStop => "MediaStop",
            KeyCode::MediaTrackNext => "MediaTrackNext",
            KeyCode::MediaTrackPrevious => "MediaTrackPrevious",
            KeyCode::Power => "Power",
            KeyCode::Sleep => "Sleep",
            KeyCode::AudioVolumeDown => "AudioVolumeDown",
            KeyCode::AudioVolumeMute => "AudioVolumeMute",
            KeyCode::AudioVolumeUp => "AudioVolumeUp",
            KeyCode::WakeUp => "WakeUp",
            KeyCode::Hyper => "Hyper",
            KeyCode::Turbo => "Turbo",
            KeyCode::Abort => "Abort",
            KeyCode::Resume => "Resume",
            KeyCode::Suspend => "Suspend",
            KeyCode::Again => "Again",
            KeyCode::Copy => "Copy",
            KeyCode::Cut => "Cut",
            KeyCode::Find => "Find",
            KeyCode::Open => "Open",
            KeyCode::Paste => "Paste",
            KeyCode::Props => "Props",
            KeyCode::Select => "Select",
            KeyCode::Undo => "Undo",
            KeyCode::Hiragana => "Hiragana",
            KeyCode::Katakana => "Katakana",
            KeyCode::F1 => "F1",
            KeyCode::F2 => "F2",
            KeyCode::F3 => "F3",
            KeyCode::F4 => "F4",
            KeyCode::F5 => "F5",
            KeyCode::F6 => "F6",
            KeyCode::F7 => "F7",
            KeyCode::F8 => "F8",
            KeyCode::F9 => "F9",
            KeyCode::F10 => "F10",
            KeyCode::F11 => "F11",
            KeyCode::F12 => "F12",
            KeyCode::F13 => "F13",
            KeyCode::F14 => "F14",
            KeyCode::F15 => "F15",
            KeyCode::F16 => "F16",
            KeyCode::F17 => "F17",
            KeyCode::F18 => "F18",
            KeyCode::F19 => "F19",
            KeyCode::F20 => "F20",
            KeyCode::F21 => "F21",
            KeyCode::F22 => "F22",
            KeyCode::F23 => "F23",
            KeyCode::F24 => "F24",
            KeyCode::F25 => "F25",
            KeyCode::F26 => "F26",
            KeyCode::F27 => "F27",
            KeyCode::F28 => "F28",
            KeyCode::F29 => "F29",
            KeyCode::F30 => "F30",
            KeyCode::F31 => "F31",
            KeyCode::F32 => "F32",
            KeyCode::F33 => "F33",
            KeyCode::F34 => "F34",
            KeyCode::F35 => "F35",
        }
    }
}

/// Key represents the meaning of a keypress.
///
/// This mostly conforms to the UI Events Specification's [`KeyboardEvent.key`] with a few
/// exceptions:
/// - The `Super` variant here, is named `Meta` in the aforementioned specification. (There's
///   another key which the specification calls `Super`. That does not exist here.)
/// - The `Space` variant here, can be identified by the character it generates in the
///   specificaiton.
/// - The `Unidentified` variant here, can still identifiy a key through it's `NativeKeyCode`.
/// - The `Dead` variant here, can specify the character which is inserted when pressing the
///   dead-key twice.
///
/// [`KeyboardEvent.key`]: https://w3c.github.io/uievents-key/
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Key<'a> {
    /// A key string that corresponds to the character typed by the user, taking into account the
    /// user’s current locale setting, and any system-level keyboard mapping overrides that are in
    /// effect.
    Character(&'a str),

    /// This variant is used when the key cannot be translated to any other variant.
    ///
    /// The native scancode is provided (if available) in order to allow the user to specify
    /// keybindings for keys which are not defined by this API.
    Unidentified(NativeKeyCode),

    /// Contains the text representation of the dead-key when available.
    ///
    /// ## Platform-specific
    /// - **Web:** Always contains `None`
    Dead(Option<char>),

    /// The `Alt` (Alternative) key.
    ///
    /// This key enables the alternate modifier function for interpreting concurrent or subsequent
    /// keyboard input. This key value is also used for the Apple <kbd>Option</kbd> key.
    Alt,
    /// The Alternate Graphics (<kbd>AltGr</kbd> or <kbd>AltGraph</kbd>) key.
    ///
    /// This key is used enable the ISO Level 3 shift modifier (the standard `Shift` key is the
    /// level 2 modifier).
    AltGraph,
    /// The `Caps Lock` (Capital) key.
    ///
    /// Toggle capital character lock function for interpreting subsequent keyboard input event.
    CapsLock,
    /// The `Control` or `Ctrl` key.
    ///
    /// Used to enable control modifier function for interpreting concurrent or subsequent keyboard
    /// input.
    Control,
    /// The Function switch `Fn` key. Activating this key simultaneously with another key changes
    /// that key’s value to an alternate character or function. This key is often handled directly
    /// in the keyboard hardware and does not usually generate key events.
    Fn,
    /// The Function-Lock (`FnLock` or `F-Lock`) key. Activating this key switches the mode of the
    /// keyboard to changes some keys' values to an alternate character or function. This key is
    /// often handled directly in the keyboard hardware and does not usually generate key events.
    FnLock,
    /// The `NumLock` or Number Lock key. Used to toggle numpad mode function for interpreting
    /// subsequent keyboard input.
    NumLock,
    /// Toggle between scrolling and cursor movement modes.
    ScrollLock,
    /// Used to enable shift modifier function for interpreting concurrent or subsequent keyboard
    /// input.
    Shift,
    /// The Symbol modifier key (used on some virtual keyboards).
    Symbol,
    SymbolLock,
    Hyper,
    /// Used to enable "super" modifier function for interpreting concurrent or subsequent keyboard
    /// input. This key value is used for the "Windows Logo" key and the Apple `Command` or `⌘` key.
    ///
    /// Note: In some contexts (e.g. the Web) this is referred to as the "Meta" key.
    Super,
    /// The `Enter` or `↵` key. Used to activate current selection or accept current input. This key
    /// value is also used for the `Return` (Macintosh numpad) key. This key value is also used for
    /// the Android `KEYCODE_DPAD_CENTER`.
    Enter,
    /// The Horizontal Tabulation `Tab` key.
    Tab,
    /// Used in text to insert a space between words. Usually located below the character keys.
    Space,
    /// Navigate or traverse downward. (`KEYCODE_DPAD_DOWN`)
    ArrowDown,
    /// Navigate or traverse leftward. (`KEYCODE_DPAD_LEFT`)
    ArrowLeft,
    /// Navigate or traverse rightward. (`KEYCODE_DPAD_RIGHT`)
    ArrowRight,
    /// Navigate or traverse upward. (`KEYCODE_DPAD_UP`)
    ArrowUp,
    /// The End key, used with keyboard entry to go to the end of content (`KEYCODE_MOVE_END`).
    End,
    /// The Home key, used with keyboard entry, to go to start of content (`KEYCODE_MOVE_HOME`).
    /// For the mobile phone `Home` key (which goes to the phone’s main screen), use [`GoHome`].
    ///
    /// [`GoHome`]: Self::GoHome
    Home,
    /// Scroll down or display next page of content.
    PageDown,
    /// Scroll up or display previous page of content.
    PageUp,
    /// Used to remove the character to the left of the cursor. This key value is also used for
    /// the key labeled `Delete` on MacOS keyboards.
    Backspace,
    /// Remove the currently selected input.
    Clear,
    /// Copy the current selection. (`APPCOMMAND_COPY`)
    Copy,
    /// The Cursor Select key.
    CrSel,
    /// Cut the current selection. (`APPCOMMAND_CUT`)
    Cut,
    /// Used to delete the character to the right of the cursor. This key value is also used for the
    /// key labeled `Delete` on MacOS keyboards when `Fn` is active.
    Delete,
    /// The Erase to End of Field key. This key deletes all characters from the current cursor
    /// position to the end of the current field.
    EraseEof,
    /// The Extend Selection (Exsel) key.
    ExSel,
    /// Toggle between text modes for insertion or overtyping.
    /// (`KEYCODE_INSERT`)
    Insert,
    /// The Paste key. (`APPCOMMAND_PASTE`)
    Paste,
    /// Redo the last action. (`APPCOMMAND_REDO`)
    Redo,
    /// Undo the last action. (`APPCOMMAND_UNDO`)
    Undo,
    /// The Accept (Commit, OK) key. Accept current option or input method sequence conversion.
    Accept,
    /// Redo or repeat an action.
    Again,
    /// The Attention (Attn) key.
    Attn,
    Cancel,
    /// Show the application’s context menu.
    /// This key is commonly found between the right `Super` key and the right `Control` key.
    ContextMenu,
    /// The `Esc` key. This key was originally used to initiate an escape sequence, but is
    /// now more generally used to exit or "escape" the current context, such as closing a dialog
    /// or exiting full screen mode.
    Escape,
    Execute,
    /// Open the Find dialog. (`APPCOMMAND_FIND`)
    Find,
    /// Open a help dialog or toggle display of help information. (`APPCOMMAND_HELP`,
    /// `KEYCODE_HELP`)
    Help,
    /// Pause the current state or application (as appropriate).
    ///
    /// Note: Do not use this value for the `Pause` button on media controllers. Use `"MediaPause"`
    /// instead.
    Pause,
    /// Play or resume the current state or application (as appropriate).
    ///
    /// Note: Do not use this value for the `Play` button on media controllers. Use `"MediaPlay"`
    /// instead.
    Play,
    /// The properties (Props) key.
    Props,
    Select,
    /// The ZoomIn key. (`KEYCODE_ZOOM_IN`)
    ZoomIn,
    /// The ZoomOut key. (`KEYCODE_ZOOM_OUT`)
    ZoomOut,
    /// The Brightness Down key. Typically controls the display brightness.
    /// (`KEYCODE_BRIGHTNESS_DOWN`)
    BrightnessDown,
    /// The Brightness Up key. Typically controls the display brightness. (`KEYCODE_BRIGHTNESS_UP`)
    BrightnessUp,
    /// Toggle removable media to eject (open) and insert (close) state. (`KEYCODE_MEDIA_EJECT`)
    Eject,
    LogOff,
    /// Toggle power state. (`KEYCODE_POWER`)
    /// Note: Note: Some devices might not expose this key to the operating environment.
    Power,
    /// The `PowerOff` key. Sometime called `PowerDown`.
    PowerOff,
    /// Initiate print-screen function.
    PrintScreen,
    /// The Hibernate key. This key saves the current state of the computer to disk so that it can
    /// be restored. The computer will then shutdown.
    Hibernate,
    /// The Standby key. This key turns off the display and places the computer into a low-power
    /// mode without completely shutting down. It is sometimes labelled `Suspend` or `Sleep` key.
    /// (`KEYCODE_SLEEP`)
    Standby,
    /// The WakeUp key. (`KEYCODE_WAKEUP`)
    WakeUp,
    /// Initate the multi-candidate mode.
    AllCandidates,
    Alphanumeric,
    /// Initiate the Code Input mode to allow characters to be entered by
    /// their code points.
    CodeInput,
    /// The Compose key, also known as "Multi_key" on the X Window System. This key acts in a
    /// manner similar to a dead key, triggering a mode where subsequent key presses are combined to
    /// produce a different character.
    Compose,
    /// Convert the current input method sequence.
    Convert,
    /// The Final Mode `Final` key used on some Asian keyboards, to enable the final mode for IMEs.
    FinalMode,
    /// Switch to the first character group. (ISO/IEC 9995)
    GroupFirst,
    /// Switch to the last character group. (ISO/IEC 9995)
    GroupLast,
    /// Switch to the next character group. (ISO/IEC 9995)
    GroupNext,
    /// Switch to the previous character group. (ISO/IEC 9995)
    GroupPrevious,
    /// Toggle between or cycle through input modes of IMEs.
    ModeChange,
    NextCandidate,
    /// Accept current input method sequence without
    /// conversion in IMEs.
    NonConvert,
    PreviousCandidate,
    Process,
    SingleCandidate,
    /// Toggle between Hangul and English modes.
    HangulMode,
    HanjaMode,
    JunjaMode,
    /// The Eisu key. This key may close the IME, but its purpose is defined by the current IME.
    /// (`KEYCODE_EISU`)
    Eisu,
    /// The (Half-Width) Characters key.
    Hankaku,
    /// The Hiragana (Japanese Kana characters) key.
    Hiragana,
    /// The Hiragana/Katakana toggle key. (`KEYCODE_KATAKANA_HIRAGANA`)
    HiraganaKatakana,
    /// The Kana Mode (Kana Lock) key. This key is used to enter hiragana mode (typically from
    /// romaji mode).
    KanaMode,
    /// The Kanji (Japanese name for ideographic characters of Chinese origin) Mode key. This key is
    /// typically used to switch to a hiragana keyboard for the purpose of converting input into
    /// kanji. (`KEYCODE_KANA`)
    KanjiMode,
    /// The Katakana (Japanese Kana characters) key.
    Katakana,
    /// The Roman characters function key.
    Romaji,
    /// The Zenkaku (Full-Width) Characters key.
    Zenkaku,
    /// The Zenkaku/Hankaku (full-width/half-width) toggle key. (`KEYCODE_ZENKAKU_HANKAKU`)
    ZenkakuHankaku,
    /// General purpose virtual function key, as index 1.
    Soft1,
    /// General purpose virtual function key, as index 2.
    Soft2,
    /// General purpose virtual function key, as index 3.
    Soft3,
    /// General purpose virtual function key, as index 4.
    Soft4,
    /// Select next (numerically or logically) lower channel. (`APPCOMMAND_MEDIA_CHANNEL_DOWN`,
    /// `KEYCODE_CHANNEL_DOWN`)
    ChannelDown,
    /// Select next (numerically or logically) higher channel. (`APPCOMMAND_MEDIA_CHANNEL_UP`,
    /// `KEYCODE_CHANNEL_UP`)
    ChannelUp,
    /// Close the current document or message (Note: This doesn’t close the application).
    /// (`APPCOMMAND_CLOSE`)
    Close,
    /// Open an editor to forward the current message. (`APPCOMMAND_FORWARD_MAIL`)
    MailForward,
    /// Open an editor to reply to the current message. (`APPCOMMAND_REPLY_TO_MAIL`)
    MailReply,
    /// Send the current message. (`APPCOMMAND_SEND_MAIL`)
    MailSend,
    /// Close the current media, for example to close a CD or DVD tray. (`KEYCODE_MEDIA_CLOSE`)
    MediaClose,
    /// Initiate or continue forward playback at faster than normal speed, or increase speed if
    /// already fast forwarding. (`APPCOMMAND_MEDIA_FAST_FORWARD`, `KEYCODE_MEDIA_FAST_FORWARD`)
    MediaFastForward,
    /// Pause the currently playing media. (`APPCOMMAND_MEDIA_PAUSE`, `KEYCODE_MEDIA_PAUSE`)
    ///
    /// Note: Media controller devices should use this value rather than `"Pause"` for their pause
    /// keys.
    MediaPause,
    /// Initiate or continue media playback at normal speed, if not currently playing at normal
    /// speed. (`APPCOMMAND_MEDIA_PLAY`, `KEYCODE_MEDIA_PLAY`)
    MediaPlay,
    /// Toggle media between play and pause states. (`APPCOMMAND_MEDIA_PLAY_PAUSE`,
    /// `KEYCODE_MEDIA_PLAY_PAUSE`)
    MediaPlayPause,
    /// Initiate or resume recording of currently selected media. (`APPCOMMAND_MEDIA_RECORD`,
    /// `KEYCODE_MEDIA_RECORD`)
    MediaRecord,
    /// Initiate or continue reverse playback at faster than normal speed, or increase speed if
    /// already rewinding. (`APPCOMMAND_MEDIA_REWIND`, `KEYCODE_MEDIA_REWIND`)
    MediaRewind,
    /// Stop media playing, pausing, forwarding, rewinding, or recording, if not already stopped.
    /// (`APPCOMMAND_MEDIA_STOP`, `KEYCODE_MEDIA_STOP`)
    MediaStop,
    /// Seek to next media or program track. (`APPCOMMAND_MEDIA_NEXTTRACK`, `KEYCODE_MEDIA_NEXT`)
    MediaTrackNext,
    /// Seek to previous media or program track. (`APPCOMMAND_MEDIA_PREVIOUSTRACK`,
    /// `KEYCODE_MEDIA_PREVIOUS`)
    MediaTrackPrevious,
    /// Open a new document or message. (`APPCOMMAND_NEW`)
    New,
    /// Open an existing document or message. (`APPCOMMAND_OPEN`)
    Open,
    /// Print the current document or message. (`APPCOMMAND_PRINT`)
    Print,
    /// Save the current document or message. (`APPCOMMAND_SAVE`)
    Save,
    /// Spellcheck the current document or selection. (`APPCOMMAND_SPELL_CHECK`)
    SpellCheck,
    /// The `11` key found on media numpads that
    /// have buttons from `1` ... `12`.
    Key11,
    /// The `12` key found on media numpads that
    /// have buttons from `1` ... `12`.
    Key12,
    /// Adjust audio balance leftward. (`VK_AUDIO_BALANCE_LEFT`)
    AudioBalanceLeft,
    /// Adjust audio balance rightward. (`VK_AUDIO_BALANCE_RIGHT`)
    AudioBalanceRight,
    /// Decrease audio bass boost or cycle down through bass boost states. (`APPCOMMAND_BASS_DOWN`,
    /// `VK_BASS_BOOST_DOWN`)
    AudioBassBoostDown,
    /// Toggle bass boost on/off. (`APPCOMMAND_BASS_BOOST`)
    AudioBassBoostToggle,
    /// Increase audio bass boost or cycle up through bass boost states. (`APPCOMMAND_BASS_UP`,
    /// `VK_BASS_BOOST_UP`)
    AudioBassBoostUp,
    /// Adjust audio fader towards front. (`VK_FADER_FRONT`)
    AudioFaderFront,
    /// Adjust audio fader towards rear. (`VK_FADER_REAR`)
    AudioFaderRear,
    /// Advance surround audio mode to next available mode. (`VK_SURROUND_MODE_NEXT`)
    AudioSurroundModeNext,
    /// Decrease treble. (`APPCOMMAND_TREBLE_DOWN`)
    AudioTrebleDown,
    /// Increase treble. (`APPCOMMAND_TREBLE_UP`)
    AudioTrebleUp,
    /// Decrease audio volume. (`APPCOMMAND_VOLUME_DOWN`, `KEYCODE_VOLUME_DOWN`)
    AudioVolumeDown,
    /// Increase audio volume. (`APPCOMMAND_VOLUME_UP`, `KEYCODE_VOLUME_UP`)
    AudioVolumeUp,
    /// Toggle between muted state and prior volume level. (`APPCOMMAND_VOLUME_MUTE`,
    /// `KEYCODE_VOLUME_MUTE`)
    AudioVolumeMute,
    /// Toggle the microphone on/off. (`APPCOMMAND_MIC_ON_OFF_TOGGLE`)
    MicrophoneToggle,
    /// Decrease microphone volume. (`APPCOMMAND_MICROPHONE_VOLUME_DOWN`)
    MicrophoneVolumeDown,
    /// Increase microphone volume. (`APPCOMMAND_MICROPHONE_VOLUME_UP`)
    MicrophoneVolumeUp,
    /// Mute the microphone. (`APPCOMMAND_MICROPHONE_VOLUME_MUTE`, `KEYCODE_MUTE`)
    MicrophoneVolumeMute,
    /// Show correction list when a word is incorrectly identified. (`APPCOMMAND_CORRECTION_LIST`)
    SpeechCorrectionList,
    /// Toggle between dictation mode and command/control mode.
    /// (`APPCOMMAND_DICTATE_OR_COMMAND_CONTROL_TOGGLE`)
    SpeechInputToggle,
    /// The first generic "LaunchApplication" key. This is commonly associated with launching "My
    /// Computer", and may have a computer symbol on the key. (`APPCOMMAND_LAUNCH_APP1`)
    LaunchApplication1,
    /// The second generic "LaunchApplication" key. This is commonly associated with launching
    /// "Calculator", and may have a calculator symbol on the key. (`APPCOMMAND_LAUNCH_APP2`,
    /// `KEYCODE_CALCULATOR`)
    LaunchApplication2,
    /// The "Calendar" key. (`KEYCODE_CALENDAR`)
    LaunchCalendar,
    /// The "Contacts" key. (`KEYCODE_CONTACTS`)
    LaunchContacts,
    /// The "Mail" key. (`APPCOMMAND_LAUNCH_MAIL`)
    LaunchMail,
    /// The "Media Player" key. (`APPCOMMAND_LAUNCH_MEDIA_SELECT`)
    LaunchMediaPlayer,
    LaunchMusicPlayer,
    LaunchPhone,
    LaunchScreenSaver,
    LaunchSpreadsheet,
    LaunchWebBrowser,
    LaunchWebCam,
    LaunchWordProcessor,
    /// Navigate to previous content or page in current history. (`APPCOMMAND_BROWSER_BACKWARD`)
    BrowserBack,
    /// Open the list of browser favorites. (`APPCOMMAND_BROWSER_FAVORITES`)
    BrowserFavorites,
    /// Navigate to next content or page in current history. (`APPCOMMAND_BROWSER_FORWARD`)
    BrowserForward,
    /// Go to the user’s preferred home page. (`APPCOMMAND_BROWSER_HOME`)
    BrowserHome,
    /// Refresh the current page or content. (`APPCOMMAND_BROWSER_REFRESH`)
    BrowserRefresh,
    /// Call up the user’s preferred search page. (`APPCOMMAND_BROWSER_SEARCH`)
    BrowserSearch,
    /// Stop loading the current page or content. (`APPCOMMAND_BROWSER_STOP`)
    BrowserStop,
    /// The Application switch key, which provides a list of recent apps to switch between.
    /// (`KEYCODE_APP_SWITCH`)
    AppSwitch,
    /// The Call key. (`KEYCODE_CALL`)
    Call,
    /// The Camera key. (`KEYCODE_CAMERA`)
    Camera,
    /// The Camera focus key. (`KEYCODE_FOCUS`)
    CameraFocus,
    /// The End Call key. (`KEYCODE_ENDCALL`)
    EndCall,
    /// The Back key. (`KEYCODE_BACK`)
    GoBack,
    /// The Home key, which goes to the phone’s main screen. (`KEYCODE_HOME`)
    GoHome,
    /// The Headset Hook key. (`KEYCODE_HEADSETHOOK`)
    HeadsetHook,
    LastNumberRedial,
    /// The Notification key. (`KEYCODE_NOTIFICATION`)
    Notification,
    /// Toggle between manner mode state: silent, vibrate, ring, ... (`KEYCODE_MANNER_MODE`)
    MannerMode,
    VoiceDial,
    /// Switch to viewing TV. (`KEYCODE_TV`)
    TV,
    /// TV 3D Mode. (`KEYCODE_3D_MODE`)
    TV3DMode,
    /// Toggle between antenna and cable input. (`KEYCODE_TV_ANTENNA_CABLE`)
    TVAntennaCable,
    /// Audio description. (`KEYCODE_TV_AUDIO_DESCRIPTION`)
    TVAudioDescription,
    /// Audio description mixing volume down. (`KEYCODE_TV_AUDIO_DESCRIPTION_MIX_DOWN`)
    TVAudioDescriptionMixDown,
    /// Audio description mixing volume up. (`KEYCODE_TV_AUDIO_DESCRIPTION_MIX_UP`)
    TVAudioDescriptionMixUp,
    /// Contents menu. (`KEYCODE_TV_CONTENTS_MENU`)
    TVContentsMenu,
    /// Contents menu. (`KEYCODE_TV_DATA_SERVICE`)
    TVDataService,
    /// Switch the input mode on an external TV. (`KEYCODE_TV_INPUT`)
    TVInput,
    /// Switch to component input #1. (`KEYCODE_TV_INPUT_COMPONENT_1`)
    TVInputComponent1,
    /// Switch to component input #2. (`KEYCODE_TV_INPUT_COMPONENT_2`)
    TVInputComponent2,
    /// Switch to composite input #1. (`KEYCODE_TV_INPUT_COMPOSITE_1`)
    TVInputComposite1,
    /// Switch to composite input #2. (`KEYCODE_TV_INPUT_COMPOSITE_2`)
    TVInputComposite2,
    /// Switch to HDMI input #1. (`KEYCODE_TV_INPUT_HDMI_1`)
    TVInputHDMI1,
    /// Switch to HDMI input #2. (`KEYCODE_TV_INPUT_HDMI_2`)
    TVInputHDMI2,
    /// Switch to HDMI input #3. (`KEYCODE_TV_INPUT_HDMI_3`)
    TVInputHDMI3,
    /// Switch to HDMI input #4. (`KEYCODE_TV_INPUT_HDMI_4`)
    TVInputHDMI4,
    /// Switch to VGA input #1. (`KEYCODE_TV_INPUT_VGA_1`)
    TVInputVGA1,
    /// Media context menu. (`KEYCODE_TV_MEDIA_CONTEXT_MENU`)
    TVMediaContext,
    /// Toggle network. (`KEYCODE_TV_NETWORK`)
    TVNetwork,
    /// Number entry. (`KEYCODE_TV_NUMBER_ENTRY`)
    TVNumberEntry,
    /// Toggle the power on an external TV. (`KEYCODE_TV_POWER`)
    TVPower,
    /// Radio. (`KEYCODE_TV_RADIO_SERVICE`)
    TVRadioService,
    /// Satellite. (`KEYCODE_TV_SATELLITE`)
    TVSatellite,
    /// Broadcast Satellite. (`KEYCODE_TV_SATELLITE_BS`)
    TVSatelliteBS,
    /// Communication Satellite. (`KEYCODE_TV_SATELLITE_CS`)
    TVSatelliteCS,
    /// Toggle between available satellites. (`KEYCODE_TV_SATELLITE_SERVICE`)
    TVSatelliteToggle,
    /// Analog Terrestrial. (`KEYCODE_TV_TERRESTRIAL_ANALOG`)
    TVTerrestrialAnalog,
    /// Digital Terrestrial. (`KEYCODE_TV_TERRESTRIAL_DIGITAL`)
    TVTerrestrialDigital,
    /// Timer programming. (`KEYCODE_TV_TIMER_PROGRAMMING`)
    TVTimer,
    /// Switch the input mode on an external AVR (audio/video receiver). (`KEYCODE_AVR_INPUT`)
    AVRInput,
    /// Toggle the power on an external AVR (audio/video receiver). (`KEYCODE_AVR_POWER`)
    AVRPower,
    /// General purpose color-coded media function key, as index 0 (red). (`VK_COLORED_KEY_0`,
    /// `KEYCODE_PROG_RED`)
    ColorF0Red,
    /// General purpose color-coded media function key, as index 1 (green). (`VK_COLORED_KEY_1`,
    /// `KEYCODE_PROG_GREEN`)
    ColorF1Green,
    /// General purpose color-coded media function key, as index 2 (yellow). (`VK_COLORED_KEY_2`,
    /// `KEYCODE_PROG_YELLOW`)
    ColorF2Yellow,
    /// General purpose color-coded media function key, as index 3 (blue). (`VK_COLORED_KEY_3`,
    /// `KEYCODE_PROG_BLUE`)
    ColorF3Blue,
    /// General purpose color-coded media function key, as index 4 (grey). (`VK_COLORED_KEY_4`)
    ColorF4Grey,
    /// General purpose color-coded media function key, as index 5 (brown). (`VK_COLORED_KEY_5`)
    ColorF5Brown,
    /// Toggle the display of Closed Captions. (`VK_CC`, `KEYCODE_CAPTIONS`)
    ClosedCaptionToggle,
    /// Adjust brightness of device, by toggling between or cycling through states. (`VK_DIMMER`)
    Dimmer,
    /// Swap video sources. (`VK_DISPLAY_SWAP`)
    DisplaySwap,
    /// Select Digital Video Rrecorder. (`KEYCODE_DVR`)
    DVR,
    /// Exit the current application. (`VK_EXIT`)
    Exit,
    /// Clear program or content stored as favorite 0. (`VK_CLEAR_FAVORITE_0`)
    FavoriteClear0,
    /// Clear program or content stored as favorite 1. (`VK_CLEAR_FAVORITE_1`)
    FavoriteClear1,
    /// Clear program or content stored as favorite 2. (`VK_CLEAR_FAVORITE_2`)
    FavoriteClear2,
    /// Clear program or content stored as favorite 3. (`VK_CLEAR_FAVORITE_3`)
    FavoriteClear3,
    /// Select (recall) program or content stored as favorite 0. (`VK_RECALL_FAVORITE_0`)
    FavoriteRecall0,
    /// Select (recall) program or content stored as favorite 1. (`VK_RECALL_FAVORITE_1`)
    FavoriteRecall1,
    /// Select (recall) program or content stored as favorite 2. (`VK_RECALL_FAVORITE_2`)
    FavoriteRecall2,
    /// Select (recall) program or content stored as favorite 3. (`VK_RECALL_FAVORITE_3`)
    FavoriteRecall3,
    /// Store current program or content as favorite 0. (`VK_STORE_FAVORITE_0`)
    FavoriteStore0,
    /// Store current program or content as favorite 1. (`VK_STORE_FAVORITE_1`)
    FavoriteStore1,
    /// Store current program or content as favorite 2. (`VK_STORE_FAVORITE_2`)
    FavoriteStore2,
    /// Store current program or content as favorite 3. (`VK_STORE_FAVORITE_3`)
    FavoriteStore3,
    /// Toggle display of program or content guide. (`VK_GUIDE`, `KEYCODE_GUIDE`)
    Guide,
    /// If guide is active and displayed, then display next day’s content. (`VK_NEXT_DAY`)
    GuideNextDay,
    /// If guide is active and displayed, then display previous day’s content. (`VK_PREV_DAY`)
    GuidePreviousDay,
    /// Toggle display of information about currently selected context or media. (`VK_INFO`,
    /// `KEYCODE_INFO`)
    Info,
    /// Toggle instant replay. (`VK_INSTANT_REPLAY`)
    InstantReplay,
    /// Launch linked content, if available and appropriate. (`VK_LINK`)
    Link,
    /// List the current program. (`VK_LIST`)
    ListProgram,
    /// Toggle display listing of currently available live content or programs. (`VK_LIVE`)
    LiveContent,
    /// Lock or unlock current content or program. (`VK_LOCK`)
    Lock,
    /// Show a list of media applications: audio/video players and image viewers. (`VK_APPS`)
    ///
    /// Note: Do not confuse this key value with the Windows' `VK_APPS` / `VK_CONTEXT_MENU` key,
    /// which is encoded as `"ContextMenu"`.
    MediaApps,
    /// Audio track key. (`KEYCODE_MEDIA_AUDIO_TRACK`)
    MediaAudioTrack,
    /// Select previously selected channel or media. (`VK_LAST`, `KEYCODE_LAST_CHANNEL`)
    MediaLast,
    /// Skip backward to next content or program. (`KEYCODE_MEDIA_SKIP_BACKWARD`)
    MediaSkipBackward,
    /// Skip forward to next content or program. (`VK_SKIP`, `KEYCODE_MEDIA_SKIP_FORWARD`)
    MediaSkipForward,
    /// Step backward to next content or program. (`KEYCODE_MEDIA_STEP_BACKWARD`)
    MediaStepBackward,
    /// Step forward to next content or program. (`KEYCODE_MEDIA_STEP_FORWARD`)
    MediaStepForward,
    /// Media top menu. (`KEYCODE_MEDIA_TOP_MENU`)
    MediaTopMenu,
    /// Navigate in. (`KEYCODE_NAVIGATE_IN`)
    NavigateIn,
    /// Navigate to next key. (`KEYCODE_NAVIGATE_NEXT`)
    NavigateNext,
    /// Navigate out. (`KEYCODE_NAVIGATE_OUT`)
    NavigateOut,
    /// Navigate to previous key. (`KEYCODE_NAVIGATE_PREVIOUS`)
    NavigatePrevious,
    /// Cycle to next favorite channel (in favorites list). (`VK_NEXT_FAVORITE_CHANNEL`)
    NextFavoriteChannel,
    /// Cycle to next user profile (if there are multiple user profiles). (`VK_USER`)
    NextUserProfile,
    /// Access on-demand content or programs. (`VK_ON_DEMAND`)
    OnDemand,
    /// Pairing key to pair devices. (`KEYCODE_PAIRING`)
    Pairing,
    /// Move picture-in-picture window down. (`VK_PINP_DOWN`)
    PinPDown,
    /// Move picture-in-picture window. (`VK_PINP_MOVE`)
    PinPMove,
    /// Toggle display of picture-in-picture window. (`VK_PINP_TOGGLE`)
    PinPToggle,
    /// Move picture-in-picture window up. (`VK_PINP_UP`)
    PinPUp,
    /// Decrease media playback speed. (`VK_PLAY_SPEED_DOWN`)
    PlaySpeedDown,
    /// Reset playback to normal speed. (`VK_PLAY_SPEED_RESET`)
    PlaySpeedReset,
    /// Increase media playback speed. (`VK_PLAY_SPEED_UP`)
    PlaySpeedUp,
    /// Toggle random media or content shuffle mode. (`VK_RANDOM_TOGGLE`)
    RandomToggle,
    /// Not a physical key, but this key code is sent when the remote control battery is low.
    /// (`VK_RC_LOW_BATTERY`)
    RcLowBattery,
    /// Toggle or cycle between media recording speeds. (`VK_RECORD_SPEED_NEXT`)
    RecordSpeedNext,
    /// Toggle RF (radio frequency) input bypass mode (pass RF input directly to the RF output).
    /// (`VK_RF_BYPASS`)
    RfBypass,
    /// Toggle scan channels mode. (`VK_SCAN_CHANNELS_TOGGLE`)
    ScanChannelsToggle,
    /// Advance display screen mode to next available mode. (`VK_SCREEN_MODE_NEXT`)
    ScreenModeNext,
    /// Toggle display of device settings screen. (`VK_SETTINGS`, `KEYCODE_SETTINGS`)
    Settings,
    /// Toggle split screen mode. (`VK_SPLIT_SCREEN_TOGGLE`)
    SplitScreenToggle,
    /// Switch the input mode on an external STB (set top box). (`KEYCODE_STB_INPUT`)
    STBInput,
    /// Toggle the power on an external STB (set top box). (`KEYCODE_STB_POWER`)
    STBPower,
    /// Toggle display of subtitles, if available. (`VK_SUBTITLE`)
    Subtitle,
    /// Toggle display of teletext, if available (`VK_TELETEXT`, `KEYCODE_TV_TELETEXT`).
    Teletext,
    /// Advance video mode to next available mode. (`VK_VIDEO_MODE_NEXT`)
    VideoModeNext,
    /// Cause device to identify itself in some manner, e.g., audibly or visibly. (`VK_WINK`)
    Wink,
    /// Toggle between full-screen and scaled content, or alter magnification level. (`VK_ZOOM`,
    /// `KEYCODE_TV_ZOOM_MODE`)
    ZoomToggle,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F1,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F2,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F3,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F4,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F5,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F6,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F7,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F8,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F9,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F10,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F11,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F12,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F13,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F14,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F15,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F16,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F17,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F18,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F19,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F20,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F21,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F22,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F23,
    /// General-purpose function key.
    /// Usually found at the top of the keyboard.
    F24,
    /// General-purpose function key.
    F25,
    /// General-purpose function key.
    F26,
    /// General-purpose function key.
    F27,
    /// General-purpose function key.
    F28,
    /// General-purpose function key.
    F29,
    /// General-purpose function key.
    F30,
    /// General-purpose function key.
    F31,
    /// General-purpose function key.
    F32,
    /// General-purpose function key.
    F33,
    /// General-purpose function key.
    F34,
    /// General-purpose function key.
    F35,
}

impl<'a> Key<'a> {
    pub fn from_key_attribute_value(kav: &'a str) -> Self {
        match kav {
            // TODO: Report this in a better way.
            "Unidentified" => Key::Unidentified(NativeKeyCode::Web("Unidentified")),
            "Dead" => Key::Dead(None),
            "Alt" => Key::Alt,
            "AltGraph" => Key::AltGraph,
            "CapsLock" => Key::CapsLock,
            "Control" => Key::Control,
            "Fn" => Key::Fn,
            "FnLock" => Key::FnLock,
            "NumLock" => Key::NumLock,
            "ScrollLock" => Key::ScrollLock,
            "Shift" => Key::Shift,
            "Symbol" => Key::Symbol,
            "SymbolLock" => Key::SymbolLock,
            "Hyper" => Key::Hyper,
            "Meta" => Key::Super,
            "Enter" => Key::Enter,
            "Tab" => Key::Tab,
            "Space" => Key::Space,
            "ArrowDown" => Key::ArrowDown,
            "ArrowLeft" => Key::ArrowLeft,
            "ArrowRight" => Key::ArrowRight,
            "ArrowUp" => Key::ArrowUp,
            "End" => Key::End,
            "Home" => Key::Home,
            "PageDown" => Key::PageDown,
            "PageUp" => Key::PageUp,
            "Backspace" => Key::Backspace,
            "Clear" => Key::Clear,
            "Copy" => Key::Copy,
            "CrSel" => Key::CrSel,
            "Cut" => Key::Cut,
            "Delete" => Key::Delete,
            "EraseEof" => Key::EraseEof,
            "ExSel" => Key::ExSel,
            "Insert" => Key::Insert,
            "Paste" => Key::Paste,
            "Redo" => Key::Redo,
            "Undo" => Key::Undo,
            "Accept" => Key::Accept,
            "Again" => Key::Again,
            "Attn" => Key::Attn,
            "Cancel" => Key::Cancel,
            "ContextMenu" => Key::ContextMenu,
            "Escape" => Key::Escape,
            "Execute" => Key::Execute,
            "Find" => Key::Find,
            "Help" => Key::Help,
            "Pause" => Key::Pause,
            "Play" => Key::Play,
            "Props" => Key::Props,
            "Select" => Key::Select,
            "ZoomIn" => Key::ZoomIn,
            "ZoomOut" => Key::ZoomOut,
            "BrightnessDown" => Key::BrightnessDown,
            "BrightnessUp" => Key::BrightnessUp,
            "Eject" => Key::Eject,
            "LogOff" => Key::LogOff,
            "Power" => Key::Power,
            "PowerOff" => Key::PowerOff,
            "PrintScreen" => Key::PrintScreen,
            "Hibernate" => Key::Hibernate,
            "Standby" => Key::Standby,
            "WakeUp" => Key::WakeUp,
            "AllCandidates" => Key::AllCandidates,
            "Alphanumeric" => Key::Alphanumeric,
            "CodeInput" => Key::CodeInput,
            "Compose" => Key::Compose,
            "Convert" => Key::Convert,
            "FinalMode" => Key::FinalMode,
            "GroupFirst" => Key::GroupFirst,
            "GroupLast" => Key::GroupLast,
            "GroupNext" => Key::GroupNext,
            "GroupPrevious" => Key::GroupPrevious,
            "ModeChange" => Key::ModeChange,
            "NextCandidate" => Key::NextCandidate,
            "NonConvert" => Key::NonConvert,
            "PreviousCandidate" => Key::PreviousCandidate,
            "Process" => Key::Process,
            "SingleCandidate" => Key::SingleCandidate,
            "HangulMode" => Key::HangulMode,
            "HanjaMode" => Key::HanjaMode,
            "JunjaMode" => Key::JunjaMode,
            "Eisu" => Key::Eisu,
            "Hankaku" => Key::Hankaku,
            "Hiragana" => Key::Hiragana,
            "HiraganaKatakana" => Key::HiraganaKatakana,
            "KanaMode" => Key::KanaMode,
            "KanjiMode" => Key::KanjiMode,
            "Katakana" => Key::Katakana,
            "Romaji" => Key::Romaji,
            "Zenkaku" => Key::Zenkaku,
            "ZenkakuHankaku" => Key::ZenkakuHankaku,
            "Soft1" => Key::Soft1,
            "Soft2" => Key::Soft2,
            "Soft3" => Key::Soft3,
            "Soft4" => Key::Soft4,
            "ChannelDown" => Key::ChannelDown,
            "ChannelUp" => Key::ChannelUp,
            "Close" => Key::Close,
            "MailForward" => Key::MailForward,
            "MailReply" => Key::MailReply,
            "MailSend" => Key::MailSend,
            "MediaClose" => Key::MediaClose,
            "MediaFastForward" => Key::MediaFastForward,
            "MediaPause" => Key::MediaPause,
            "MediaPlay" => Key::MediaPlay,
            "MediaPlayPause" => Key::MediaPlayPause,
            "MediaRecord" => Key::MediaRecord,
            "MediaRewind" => Key::MediaRewind,
            "MediaStop" => Key::MediaStop,
            "MediaTrackNext" => Key::MediaTrackNext,
            "MediaTrackPrevious" => Key::MediaTrackPrevious,
            "New" => Key::New,
            "Open" => Key::Open,
            "Print" => Key::Print,
            "Save" => Key::Save,
            "SpellCheck" => Key::SpellCheck,
            "Key11" => Key::Key11,
            "Key12" => Key::Key12,
            "AudioBalanceLeft" => Key::AudioBalanceLeft,
            "AudioBalanceRight" => Key::AudioBalanceRight,
            "AudioBassBoostDown" => Key::AudioBassBoostDown,
            "AudioBassBoostToggle" => Key::AudioBassBoostToggle,
            "AudioBassBoostUp" => Key::AudioBassBoostUp,
            "AudioFaderFront" => Key::AudioFaderFront,
            "AudioFaderRear" => Key::AudioFaderRear,
            "AudioSurroundModeNext" => Key::AudioSurroundModeNext,
            "AudioTrebleDown" => Key::AudioTrebleDown,
            "AudioTrebleUp" => Key::AudioTrebleUp,
            "AudioVolumeDown" => Key::AudioVolumeDown,
            "AudioVolumeUp" => Key::AudioVolumeUp,
            "AudioVolumeMute" => Key::AudioVolumeMute,
            "MicrophoneToggle" => Key::MicrophoneToggle,
            "MicrophoneVolumeDown" => Key::MicrophoneVolumeDown,
            "MicrophoneVolumeUp" => Key::MicrophoneVolumeUp,
            "MicrophoneVolumeMute" => Key::MicrophoneVolumeMute,
            "SpeechCorrectionList" => Key::SpeechCorrectionList,
            "SpeechInputToggle" => Key::SpeechInputToggle,
            "LaunchApplication1" => Key::LaunchApplication1,
            "LaunchApplication2" => Key::LaunchApplication2,
            "LaunchCalendar" => Key::LaunchCalendar,
            "LaunchContacts" => Key::LaunchContacts,
            "LaunchMail" => Key::LaunchMail,
            "LaunchMediaPlayer" => Key::LaunchMediaPlayer,
            "LaunchMusicPlayer" => Key::LaunchMusicPlayer,
            "LaunchPhone" => Key::LaunchPhone,
            "LaunchScreenSaver" => Key::LaunchScreenSaver,
            "LaunchSpreadsheet" => Key::LaunchSpreadsheet,
            "LaunchWebBrowser" => Key::LaunchWebBrowser,
            "LaunchWebCam" => Key::LaunchWebCam,
            "LaunchWordProcessor" => Key::LaunchWordProcessor,
            "BrowserBack" => Key::BrowserBack,
            "BrowserFavorites" => Key::BrowserFavorites,
            "BrowserForward" => Key::BrowserForward,
            "BrowserHome" => Key::BrowserHome,
            "BrowserRefresh" => Key::BrowserRefresh,
            "BrowserSearch" => Key::BrowserSearch,
            "BrowserStop" => Key::BrowserStop,
            "AppSwitch" => Key::AppSwitch,
            "Call" => Key::Call,
            "Camera" => Key::Camera,
            "CameraFocus" => Key::CameraFocus,
            "EndCall" => Key::EndCall,
            "GoBack" => Key::GoBack,
            "GoHome" => Key::GoHome,
            "HeadsetHook" => Key::HeadsetHook,
            "LastNumberRedial" => Key::LastNumberRedial,
            "Notification" => Key::Notification,
            "MannerMode" => Key::MannerMode,
            "VoiceDial" => Key::VoiceDial,
            "TV" => Key::TV,
            "TV3DMode" => Key::TV3DMode,
            "TVAntennaCable" => Key::TVAntennaCable,
            "TVAudioDescription" => Key::TVAudioDescription,
            "TVAudioDescriptionMixDown" => Key::TVAudioDescriptionMixDown,
            "TVAudioDescriptionMixUp" => Key::TVAudioDescriptionMixUp,
            "TVContentsMenu" => Key::TVContentsMenu,
            "TVDataService" => Key::TVDataService,
            "TVInput" => Key::TVInput,
            "TVInputComponent1" => Key::TVInputComponent1,
            "TVInputComponent2" => Key::TVInputComponent2,
            "TVInputComposite1" => Key::TVInputComposite1,
            "TVInputComposite2" => Key::TVInputComposite2,
            "TVInputHDMI1" => Key::TVInputHDMI1,
            "TVInputHDMI2" => Key::TVInputHDMI2,
            "TVInputHDMI3" => Key::TVInputHDMI3,
            "TVInputHDMI4" => Key::TVInputHDMI4,
            "TVInputVGA1" => Key::TVInputVGA1,
            "TVMediaContext" => Key::TVMediaContext,
            "TVNetwork" => Key::TVNetwork,
            "TVNumberEntry" => Key::TVNumberEntry,
            "TVPower" => Key::TVPower,
            "TVRadioService" => Key::TVRadioService,
            "TVSatellite" => Key::TVSatellite,
            "TVSatelliteBS" => Key::TVSatelliteBS,
            "TVSatelliteCS" => Key::TVSatelliteCS,
            "TVSatelliteToggle" => Key::TVSatelliteToggle,
            "TVTerrestrialAnalog" => Key::TVTerrestrialAnalog,
            "TVTerrestrialDigital" => Key::TVTerrestrialDigital,
            "TVTimer" => Key::TVTimer,
            "AVRInput" => Key::AVRInput,
            "AVRPower" => Key::AVRPower,
            "ColorF0Red" => Key::ColorF0Red,
            "ColorF1Green" => Key::ColorF1Green,
            "ColorF2Yellow" => Key::ColorF2Yellow,
            "ColorF3Blue" => Key::ColorF3Blue,
            "ColorF4Grey" => Key::ColorF4Grey,
            "ColorF5Brown" => Key::ColorF5Brown,
            "ClosedCaptionToggle" => Key::ClosedCaptionToggle,
            "Dimmer" => Key::Dimmer,
            "DisplaySwap" => Key::DisplaySwap,
            "DVR" => Key::DVR,
            "Exit" => Key::Exit,
            "FavoriteClear0" => Key::FavoriteClear0,
            "FavoriteClear1" => Key::FavoriteClear1,
            "FavoriteClear2" => Key::FavoriteClear2,
            "FavoriteClear3" => Key::FavoriteClear3,
            "FavoriteRecall0" => Key::FavoriteRecall0,
            "FavoriteRecall1" => Key::FavoriteRecall1,
            "FavoriteRecall2" => Key::FavoriteRecall2,
            "FavoriteRecall3" => Key::FavoriteRecall3,
            "FavoriteStore0" => Key::FavoriteStore0,
            "FavoriteStore1" => Key::FavoriteStore1,
            "FavoriteStore2" => Key::FavoriteStore2,
            "FavoriteStore3" => Key::FavoriteStore3,
            "Guide" => Key::Guide,
            "GuideNextDay" => Key::GuideNextDay,
            "GuidePreviousDay" => Key::GuidePreviousDay,
            "Info" => Key::Info,
            "InstantReplay" => Key::InstantReplay,
            "Link" => Key::Link,
            "ListProgram" => Key::ListProgram,
            "LiveContent" => Key::LiveContent,
            "Lock" => Key::Lock,
            "MediaApps" => Key::MediaApps,
            "MediaAudioTrack" => Key::MediaAudioTrack,
            "MediaLast" => Key::MediaLast,
            "MediaSkipBackward" => Key::MediaSkipBackward,
            "MediaSkipForward" => Key::MediaSkipForward,
            "MediaStepBackward" => Key::MediaStepBackward,
            "MediaStepForward" => Key::MediaStepForward,
            "MediaTopMenu" => Key::MediaTopMenu,
            "NavigateIn" => Key::NavigateIn,
            "NavigateNext" => Key::NavigateNext,
            "NavigateOut" => Key::NavigateOut,
            "NavigatePrevious" => Key::NavigatePrevious,
            "NextFavoriteChannel" => Key::NextFavoriteChannel,
            "NextUserProfile" => Key::NextUserProfile,
            "OnDemand" => Key::OnDemand,
            "Pairing" => Key::Pairing,
            "PinPDown" => Key::PinPDown,
            "PinPMove" => Key::PinPMove,
            "PinPToggle" => Key::PinPToggle,
            "PinPUp" => Key::PinPUp,
            "PlaySpeedDown" => Key::PlaySpeedDown,
            "PlaySpeedReset" => Key::PlaySpeedReset,
            "PlaySpeedUp" => Key::PlaySpeedUp,
            "RandomToggle" => Key::RandomToggle,
            "RcLowBattery" => Key::RcLowBattery,
            "RecordSpeedNext" => Key::RecordSpeedNext,
            "RfBypass" => Key::RfBypass,
            "ScanChannelsToggle" => Key::ScanChannelsToggle,
            "ScreenModeNext" => Key::ScreenModeNext,
            "Settings" => Key::Settings,
            "SplitScreenToggle" => Key::SplitScreenToggle,
            "STBInput" => Key::STBInput,
            "STBPower" => Key::STBPower,
            "Subtitle" => Key::Subtitle,
            "Teletext" => Key::Teletext,
            "VideoModeNext" => Key::VideoModeNext,
            "Wink" => Key::Wink,
            "ZoomToggle" => Key::ZoomToggle,
            "F1" => Key::F1,
            "F2" => Key::F2,
            "F3" => Key::F3,
            "F4" => Key::F4,
            "F5" => Key::F5,
            "F6" => Key::F6,
            "F7" => Key::F7,
            "F8" => Key::F8,
            "F9" => Key::F9,
            "F10" => Key::F10,
            "F11" => Key::F11,
            "F12" => Key::F12,
            "F13" => Key::F13,
            "F14" => Key::F14,
            "F15" => Key::F15,
            "F16" => Key::F16,
            "F17" => Key::F17,
            "F18" => Key::F18,
            "F19" => Key::F19,
            "F20" => Key::F20,
            "F21" => Key::F21,
            "F22" => Key::F22,
            "F23" => Key::F23,
            "F24" => Key::F24,
            "F25" => Key::F25,
            "F26" => Key::F26,
            "F27" => Key::F27,
            "F28" => Key::F28,
            "F29" => Key::F29,
            "F30" => Key::F30,
            "F31" => Key::F31,
            "F32" => Key::F32,
            "F33" => Key::F33,
            "F34" => Key::F34,
            "F35" => Key::F35,
            string @ _ => Key::Character(string),
        }
    }

    pub fn as_key_code_attribute_value(&self) -> &str {
        match self {
            Key::Character(character) => character,
            Key::Unidentified(_) => "Unidentified",
            Key::Dead(_) => "Dead",
            Key::Alt => "Alt",
            Key::AltGraph => "AltGraph",
            Key::CapsLock => "CapsLock",
            Key::Control => "Control",
            Key::Fn => "Fn",
            Key::FnLock => "FnLock",
            Key::NumLock => "NumLock",
            Key::ScrollLock => "ScrollLock",
            Key::Shift => "Shift",
            Key::Symbol => "Symbol",
            Key::SymbolLock => "SymbolLock",
            Key::Hyper => "Hyper",
            Key::Super => "Meta",
            Key::Enter => "Enter",
            Key::Tab => "Tab",
            Key::Space => "Space",
            Key::ArrowDown => "ArrowDown",
            Key::ArrowLeft => "ArrowLeft",
            Key::ArrowRight => "ArrowRight",
            Key::ArrowUp => "ArrowUp",
            Key::End => "End",
            Key::Home => "Home",
            Key::PageDown => "PageDown",
            Key::PageUp => "PageUp",
            Key::Backspace => "Backspace",
            Key::Clear => "Clear",
            Key::Copy => "Copy",
            Key::CrSel => "CrSel",
            Key::Cut => "Cut",
            Key::Delete => "Delete",
            Key::EraseEof => "EraseEof",
            Key::ExSel => "ExSel",
            Key::Insert => "Insert",
            Key::Paste => "Paste",
            Key::Redo => "Redo",
            Key::Undo => "Undo",
            Key::Accept => "Accept",
            Key::Again => "Again",
            Key::Attn => "Attn",
            Key::Cancel => "Cancel",
            Key::ContextMenu => "ContextMenu",
            Key::Escape => "Escape",
            Key::Execute => "Execute",
            Key::Find => "Find",
            Key::Help => "Help",
            Key::Pause => "Pause",
            Key::Play => "Play",
            Key::Props => "Props",
            Key::Select => "Select",
            Key::ZoomIn => "ZoomIn",
            Key::ZoomOut => "ZoomOut",
            Key::BrightnessDown => "BrightnessDown",
            Key::BrightnessUp => "BrightnessUp",
            Key::Eject => "Eject",
            Key::LogOff => "LogOff",
            Key::Power => "Power",
            Key::PowerOff => "PowerOff",
            Key::PrintScreen => "PrintScreen",
            Key::Hibernate => "Hibernate",
            Key::Standby => "Standby",
            Key::WakeUp => "WakeUp",
            Key::AllCandidates => "AllCandidates",
            Key::Alphanumeric => "Alphanumeric",
            Key::CodeInput => "CodeInput",
            Key::Compose => "Compose",
            Key::Convert => "Convert",
            Key::FinalMode => "FinalMode",
            Key::GroupFirst => "GroupFirst",
            Key::GroupLast => "GroupLast",
            Key::GroupNext => "GroupNext",
            Key::GroupPrevious => "GroupPrevious",
            Key::ModeChange => "ModeChange",
            Key::NextCandidate => "NextCandidate",
            Key::NonConvert => "NonConvert",
            Key::PreviousCandidate => "PreviousCandidate",
            Key::Process => "Process",
            Key::SingleCandidate => "SingleCandidate",
            Key::HangulMode => "HangulMode",
            Key::HanjaMode => "HanjaMode",
            Key::JunjaMode => "JunjaMode",
            Key::Eisu => "Eisu",
            Key::Hankaku => "Hankaku",
            Key::Hiragana => "Hiragana",
            Key::HiraganaKatakana => "HiraganaKatakana",
            Key::KanaMode => "KanaMode",
            Key::KanjiMode => "KanjiMode",
            Key::Katakana => "Katakana",
            Key::Romaji => "Romaji",
            Key::Zenkaku => "Zenkaku",
            Key::ZenkakuHankaku => "ZenkakuHankaku",
            Key::Soft1 => "Soft1",
            Key::Soft2 => "Soft2",
            Key::Soft3 => "Soft3",
            Key::Soft4 => "Soft4",
            Key::ChannelDown => "ChannelDown",
            Key::ChannelUp => "ChannelUp",
            Key::Close => "Close",
            Key::MailForward => "MailForward",
            Key::MailReply => "MailReply",
            Key::MailSend => "MailSend",
            Key::MediaClose => "MediaClose",
            Key::MediaFastForward => "MediaFastForward",
            Key::MediaPause => "MediaPause",
            Key::MediaPlay => "MediaPlay",
            Key::MediaPlayPause => "MediaPlayPause",
            Key::MediaRecord => "MediaRecord",
            Key::MediaRewind => "MediaRewind",
            Key::MediaStop => "MediaStop",
            Key::MediaTrackNext => "MediaTrackNext",
            Key::MediaTrackPrevious => "MediaTrackPrevious",
            Key::New => "New",
            Key::Open => "Open",
            Key::Print => "Print",
            Key::Save => "Save",
            Key::SpellCheck => "SpellCheck",
            Key::Key11 => "Key11",
            Key::Key12 => "Key12",
            Key::AudioBalanceLeft => "AudioBalanceLeft",
            Key::AudioBalanceRight => "AudioBalanceRight",
            Key::AudioBassBoostDown => "AudioBassBoostDown",
            Key::AudioBassBoostToggle => "AudioBassBoostToggle",
            Key::AudioBassBoostUp => "AudioBassBoostUp",
            Key::AudioFaderFront => "AudioFaderFront",
            Key::AudioFaderRear => "AudioFaderRear",
            Key::AudioSurroundModeNext => "AudioSurroundModeNext",
            Key::AudioTrebleDown => "AudioTrebleDown",
            Key::AudioTrebleUp => "AudioTrebleUp",
            Key::AudioVolumeDown => "AudioVolumeDown",
            Key::AudioVolumeUp => "AudioVolumeUp",
            Key::AudioVolumeMute => "AudioVolumeMute",
            Key::MicrophoneToggle => "MicrophoneToggle",
            Key::MicrophoneVolumeDown => "MicrophoneVolumeDown",
            Key::MicrophoneVolumeUp => "MicrophoneVolumeUp",
            Key::MicrophoneVolumeMute => "MicrophoneVolumeMute",
            Key::SpeechCorrectionList => "SpeechCorrectionList",
            Key::SpeechInputToggle => "SpeechInputToggle",
            Key::LaunchApplication1 => "LaunchApplication1",
            Key::LaunchApplication2 => "LaunchApplication2",
            Key::LaunchCalendar => "LaunchCalendar",
            Key::LaunchContacts => "LaunchContacts",
            Key::LaunchMail => "LaunchMail",
            Key::LaunchMediaPlayer => "LaunchMediaPlayer",
            Key::LaunchMusicPlayer => "LaunchMusicPlayer",
            Key::LaunchPhone => "LaunchPhone",
            Key::LaunchScreenSaver => "LaunchScreenSaver",
            Key::LaunchSpreadsheet => "LaunchSpreadsheet",
            Key::LaunchWebBrowser => "LaunchWebBrowser",
            Key::LaunchWebCam => "LaunchWebCam",
            Key::LaunchWordProcessor => "LaunchWordProcessor",
            Key::BrowserBack => "BrowserBack",
            Key::BrowserFavorites => "BrowserFavorites",
            Key::BrowserForward => "BrowserForward",
            Key::BrowserHome => "BrowserHome",
            Key::BrowserRefresh => "BrowserRefresh",
            Key::BrowserSearch => "BrowserSearch",
            Key::BrowserStop => "BrowserStop",
            Key::AppSwitch => "AppSwitch",
            Key::Call => "Call",
            Key::Camera => "Camera",
            Key::CameraFocus => "CameraFocus",
            Key::EndCall => "EndCall",
            Key::GoBack => "GoBack",
            Key::GoHome => "GoHome",
            Key::HeadsetHook => "HeadsetHook",
            Key::LastNumberRedial => "LastNumberRedial",
            Key::Notification => "Notification",
            Key::MannerMode => "MannerMode",
            Key::VoiceDial => "VoiceDial",
            Key::TV => "TV",
            Key::TV3DMode => "TV3DMode",
            Key::TVAntennaCable => "TVAntennaCable",
            Key::TVAudioDescription => "TVAudioDescription",
            Key::TVAudioDescriptionMixDown => "TVAudioDescriptionMixDown",
            Key::TVAudioDescriptionMixUp => "TVAudioDescriptionMixUp",
            Key::TVContentsMenu => "TVContentsMenu",
            Key::TVDataService => "TVDataService",
            Key::TVInput => "TVInput",
            Key::TVInputComponent1 => "TVInputComponent1",
            Key::TVInputComponent2 => "TVInputComponent2",
            Key::TVInputComposite1 => "TVInputComposite1",
            Key::TVInputComposite2 => "TVInputComposite2",
            Key::TVInputHDMI1 => "TVInputHDMI1",
            Key::TVInputHDMI2 => "TVInputHDMI2",
            Key::TVInputHDMI3 => "TVInputHDMI3",
            Key::TVInputHDMI4 => "TVInputHDMI4",
            Key::TVInputVGA1 => "TVInputVGA1",
            Key::TVMediaContext => "TVMediaContext",
            Key::TVNetwork => "TVNetwork",
            Key::TVNumberEntry => "TVNumberEntry",
            Key::TVPower => "TVPower",
            Key::TVRadioService => "TVRadioService",
            Key::TVSatellite => "TVSatellite",
            Key::TVSatelliteBS => "TVSatelliteBS",
            Key::TVSatelliteCS => "TVSatelliteCS",
            Key::TVSatelliteToggle => "TVSatelliteToggle",
            Key::TVTerrestrialAnalog => "TVTerrestrialAnalog",
            Key::TVTerrestrialDigital => "TVTerrestrialDigital",
            Key::TVTimer => "TVTimer",
            Key::AVRInput => "AVRInput",
            Key::AVRPower => "AVRPower",
            Key::ColorF0Red => "ColorF0Red",
            Key::ColorF1Green => "ColorF1Green",
            Key::ColorF2Yellow => "ColorF2Yellow",
            Key::ColorF3Blue => "ColorF3Blue",
            Key::ColorF4Grey => "ColorF4Grey",
            Key::ColorF5Brown => "ColorF5Brown",
            Key::ClosedCaptionToggle => "ClosedCaptionToggle",
            Key::Dimmer => "Dimmer",
            Key::DisplaySwap => "DisplaySwap",
            Key::DVR => "DVR",
            Key::Exit => "Exit",
            Key::FavoriteClear0 => "FavoriteClear0",
            Key::FavoriteClear1 => "FavoriteClear1",
            Key::FavoriteClear2 => "FavoriteClear2",
            Key::FavoriteClear3 => "FavoriteClear3",
            Key::FavoriteRecall0 => "FavoriteRecall0",
            Key::FavoriteRecall1 => "FavoriteRecall1",
            Key::FavoriteRecall2 => "FavoriteRecall2",
            Key::FavoriteRecall3 => "FavoriteRecall3",
            Key::FavoriteStore0 => "FavoriteStore0",
            Key::FavoriteStore1 => "FavoriteStore1",
            Key::FavoriteStore2 => "FavoriteStore2",
            Key::FavoriteStore3 => "FavoriteStore3",
            Key::Guide => "Guide",
            Key::GuideNextDay => "GuideNextDay",
            Key::GuidePreviousDay => "GuidePreviousDay",
            Key::Info => "Info",
            Key::InstantReplay => "InstantReplay",
            Key::Link => "Link",
            Key::ListProgram => "ListProgram",
            Key::LiveContent => "LiveContent",
            Key::Lock => "Lock",
            Key::MediaApps => "MediaApps",
            Key::MediaAudioTrack => "MediaAudioTrack",
            Key::MediaLast => "MediaLast",
            Key::MediaSkipBackward => "MediaSkipBackward",
            Key::MediaSkipForward => "MediaSkipForward",
            Key::MediaStepBackward => "MediaStepBackward",
            Key::MediaStepForward => "MediaStepForward",
            Key::MediaTopMenu => "MediaTopMenu",
            Key::NavigateIn => "NavigateIn",
            Key::NavigateNext => "NavigateNext",
            Key::NavigateOut => "NavigateOut",
            Key::NavigatePrevious => "NavigatePrevious",
            Key::NextFavoriteChannel => "NextFavoriteChannel",
            Key::NextUserProfile => "NextUserProfile",
            Key::OnDemand => "OnDemand",
            Key::Pairing => "Pairing",
            Key::PinPDown => "PinPDown",
            Key::PinPMove => "PinPMove",
            Key::PinPToggle => "PinPToggle",
            Key::PinPUp => "PinPUp",
            Key::PlaySpeedDown => "PlaySpeedDown",
            Key::PlaySpeedReset => "PlaySpeedReset",
            Key::PlaySpeedUp => "PlaySpeedUp",
            Key::RandomToggle => "RandomToggle",
            Key::RcLowBattery => "RcLowBattery",
            Key::RecordSpeedNext => "RecordSpeedNext",
            Key::RfBypass => "RfBypass",
            Key::ScanChannelsToggle => "ScanChannelsToggle",
            Key::ScreenModeNext => "ScreenModeNext",
            Key::Settings => "Settings",
            Key::SplitScreenToggle => "SplitScreenToggle",
            Key::STBInput => "STBInput",
            Key::STBPower => "STBPower",
            Key::Subtitle => "Subtitle",
            Key::Teletext => "Teletext",
            Key::VideoModeNext => "VideoModeNext",
            Key::Wink => "Wink",
            Key::ZoomToggle => "ZoomToggle",
            Key::F1 => "F1",
            Key::F2 => "F2",
            Key::F3 => "F3",
            Key::F4 => "F4",
            Key::F5 => "F5",
            Key::F6 => "F6",
            Key::F7 => "F7",
            Key::F8 => "F8",
            Key::F9 => "F9",
            Key::F10 => "F10",
            Key::F11 => "F11",
            Key::F12 => "F12",
            Key::F13 => "F13",
            Key::F14 => "F14",
            Key::F15 => "F15",
            Key::F16 => "F16",
            Key::F17 => "F17",
            Key::F18 => "F18",
            Key::F19 => "F19",
            Key::F20 => "F20",
            Key::F21 => "F21",
            Key::F22 => "F22",
            Key::F23 => "F23",
            Key::F24 => "F24",
            Key::F25 => "F25",
            Key::F26 => "F26",
            Key::F27 => "F27",
            Key::F28 => "F28",
            Key::F29 => "F29",
            Key::F30 => "F30",
            Key::F31 => "F31",
            Key::F32 => "F32",
            Key::F33 => "F33",
            Key::F34 => "F34",
            Key::F35 => "F35",
        }
    }
}

impl<'a> Key<'a> {
    pub fn to_text(&self) -> Option<&'a str> {
        match self {
            Key::Character(ch) => Some(*ch),
            Key::Enter => Some("\r"),
            Key::Backspace => Some("\x08"),
            Key::Tab => Some("\t"),
            Key::Space => Some(" "),
            Key::Escape => Some("\x1b"),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum KeyLocation {
    Standard,
    Left,
    Right,
    Numpad,
}
