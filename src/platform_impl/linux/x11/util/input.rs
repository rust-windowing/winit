use super::*;
use crate::event::ModifiersState;

use x11rb::protocol::xinput;

pub const VIRTUAL_CORE_POINTER: xinput::DeviceId = 2;
pub const VIRTUAL_CORE_KEYBOARD: xinput::DeviceId = 3;

impl ModifiersState {
    pub(crate) fn from_x11(state: &xinput::ModifierInfo) -> Self {
        ModifiersState::from_x11_mask(state.effective as c_uint)
    }

    pub(crate) fn from_x11_mask(mask: c_uint) -> Self {
        let mut m = ModifiersState::empty();
        m.set(ModifiersState::ALT, mask & ffi::Mod1Mask != 0);
        m.set(ModifiersState::SHIFT, mask & ffi::ShiftMask != 0);
        m.set(ModifiersState::CTRL, mask & ffi::ControlMask != 0);
        m.set(ModifiersState::LOGO, mask & ffi::Mod4Mask != 0);
        m
    }
}

impl XConnection {
    #[allow(dead_code)]
    pub fn select_xkb_events(&self, device_id: c_uint, mask: c_ulong) -> Option<Flusher<'_>> {
        let status =
            unsafe { (self.xlib.XkbSelectEvents)(self.display.as_ptr(), device_id, mask, mask) };
        if status == ffi::True {
            Some(Flusher::new(self))
        } else {
            None
        }
    }
}
