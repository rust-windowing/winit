//! Types related to the keyboard.

#[doc(inline)]
pub use winit_core::keyboard::*;

use bitflags::bitflags;

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
    pub(crate) struct ModifiersKeys: u8 {
        const LSHIFT   = 0b0000_0001;
        const RSHIFT   = 0b0000_0010;
        const LCONTROL = 0b0000_0100;
        const RCONTROL = 0b0000_1000;
        const LALT     = 0b0001_0000;
        const RALT     = 0b0010_0000;
        const LSUPER   = 0b0100_0000;
        const RSUPER   = 0b1000_0000;
    }
}
