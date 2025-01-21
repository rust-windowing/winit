//! XKB state.

use std::sync::Arc;

use kbvm::lookup::LookupTable;
use kbvm::{Components, GroupDelta, GroupIndex, Keycode, Keysym, ModifierMask};
use smol_str::SmolStr;
#[cfg(x11_platform)]
use {kbvm::xkb::x11::KbvmX11Ext, x11rb::xcb_ffi::XCBConnection};

use crate::platform_impl::common::xkb::keymap::XkbKeymap;

#[derive(Debug)]
pub struct XkbState {
    modifiers: ModifiersState,
    keymap: Arc<LookupTable>,
    components: Components,
}

impl XkbState {
    #[cfg(wayland_platform)]
    pub fn new_wayland(keymap: &XkbKeymap) -> Self {
        Self {
            modifiers: Default::default(),
            keymap: keymap.keymap.clone(),
            components: Default::default(),
        }
    }

    #[cfg(x11_platform)]
    pub fn new_x11(xcb: &XCBConnection, keymap: &XkbKeymap) -> Option<Self> {
        let components = xcb.get_xkb_components(keymap._core_keyboard_id).ok()?;
        let mut state =
            Self { modifiers: Default::default(), keymap: keymap.keymap.clone(), components };
        state.reload_modifiers();
        Some(state)
    }

    pub fn get_one_sym_raw(&mut self, keycode: Keycode) -> Keysym {
        self.keymap
            .lookup(self.components.group, self.components.mods, keycode)
            .into_iter()
            .next()
            .map(|p| p.keysym())
            .unwrap_or_default()
    }

    pub fn layout(&mut self, key: Keycode) -> u32 {
        self.keymap.effective_group(self.components.group, key).map(|g| g.0).unwrap_or(!0)
    }

    #[cfg(x11_platform)]
    pub fn depressed_modifiers(&mut self) -> u32 {
        self.components.mods_pressed.0
    }

    #[cfg(x11_platform)]
    pub fn latched_modifiers(&mut self) -> u32 {
        self.components.mods_latched.0
    }

    #[cfg(x11_platform)]
    pub fn locked_modifiers(&mut self) -> u32 {
        self.components.mods_locked.0
    }

    pub fn get_utf8_raw(
        &mut self,
        keycode: Keycode,
        scratch_buffer: &mut String,
    ) -> Option<SmolStr> {
        scratch_buffer.clear();
        for p in self.keymap.lookup(self.components.group, self.components.mods, keycode) {
            if let Some(c) = p.char() {
                scratch_buffer.push(c);
            }
        }
        Some(SmolStr::new(scratch_buffer))
    }

    pub fn modifiers(&self) -> ModifiersState {
        self.modifiers
    }

    pub fn update_modifiers(
        &mut self,
        mods_depressed: u32,
        mods_latched: u32,
        mods_locked: u32,
        depressed_group: u32,
        latched_group: u32,
        locked_group: u32,
    ) {
        let old_mods = self.components.mods;
        self.components.group_pressed = GroupDelta(depressed_group);
        self.components.group_latched = GroupDelta(latched_group);
        self.components.group_locked = GroupIndex(locked_group);
        self.components.mods_pressed = ModifierMask(mods_depressed);
        self.components.mods_latched = ModifierMask(mods_latched);
        self.components.mods_locked = ModifierMask(mods_locked);
        self.components.update_effective();
        if old_mods != self.components.mods {
            self.reload_modifiers();
        }
    }

    /// Reload the modifiers.
    fn reload_modifiers(&mut self) {
        let mods = self.components.mods;
        self.modifiers.ctrl = mods.contains(ModifierMask::CONTROL);
        self.modifiers.alt = mods.contains(ModifierMask::ALT);
        self.modifiers.shift = mods.contains(ModifierMask::SHIFT);
        self.modifiers.caps_lock = mods.contains(ModifierMask::LOCK);
        self.modifiers.logo = mods.contains(ModifierMask::SUPER);
        self.modifiers.num_lock = mods.contains(ModifierMask::NUM_LOCK);
    }
}

/// Represents the current state of the keyboard modifiers
///
/// Each field of this struct represents a modifier and is `true` if this modifier is active.
///
/// For some modifiers, this means that the key is currently pressed, others are toggled
/// (like caps lock).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct ModifiersState {
    /// The "control" key
    pub ctrl: bool,
    /// The "alt" key
    pub alt: bool,
    /// The "shift" key
    pub shift: bool,
    /// The "Caps lock" key
    pub caps_lock: bool,
    /// The "logo" key
    ///
    /// Also known as the "windows" key on most keyboards
    pub logo: bool,
    /// The "Num lock" key
    pub num_lock: bool,
}

impl From<ModifiersState> for crate::keyboard::ModifiersState {
    fn from(mods: ModifiersState) -> crate::keyboard::ModifiersState {
        let mut to_mods = crate::keyboard::ModifiersState::empty();
        to_mods.set(crate::keyboard::ModifiersState::SHIFT, mods.shift);
        to_mods.set(crate::keyboard::ModifiersState::CONTROL, mods.ctrl);
        to_mods.set(crate::keyboard::ModifiersState::ALT, mods.alt);
        to_mods.set(crate::keyboard::ModifiersState::SUPER, mods.logo);
        to_mods
    }
}
