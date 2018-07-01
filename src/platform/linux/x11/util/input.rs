use std::str;

use super::*;
use events::ModifiersState;

pub const VIRTUAL_CORE_POINTER: c_int = 2;
pub const VIRTUAL_CORE_KEYBOARD: c_int = 3;

// A base buffer size of 1kB uses a negligible amount of RAM while preventing us from having to
// re-allocate (and make another round-trip) in the *vast* majority of cases.
// To test if `lookup_utf8` works correctly, set this to 1.
const TEXT_BUFFER_SIZE: usize = 1024;

impl From<ffi::XIModifierState> for ModifiersState {
    fn from(mods: ffi::XIModifierState) -> Self {
        let state = mods.effective as c_uint;
        ModifiersState {
            alt: state & ffi::Mod1Mask != 0,
            shift: state & ffi::ShiftMask != 0,
            ctrl: state & ffi::ControlMask != 0,
            logo: state & ffi::Mod4Mask != 0,
        }
    }
}

pub struct PointerState<'a> {
    xconn: &'a XConnection,
    root: ffi::Window,
    child: ffi::Window,
    pub root_x: c_double,
    pub root_y: c_double,
    win_x: c_double,
    win_y: c_double,
    buttons: ffi::XIButtonState,
    modifiers: ffi::XIModifierState,
    group: ffi::XIGroupState,
    relative_to_window: bool,
}

impl<'a> PointerState<'a> {
    pub fn get_modifier_state(&self) -> ModifiersState {
        self.modifiers.into()
    }
}

impl<'a> Drop for PointerState<'a> {
    fn drop(&mut self) {
        if !self.buttons.mask.is_null() {
            unsafe {
                // This is why you need to read the docs carefully...
                (self.xconn.xlib.XFree)(self.buttons.mask as _);
            }
        }
    }
}

impl XConnection {
    pub fn select_xinput_events(&self, window: c_ulong, device_id: c_int, mask: i32) -> Flusher {
        let mut event_mask = ffi::XIEventMask {
            deviceid: device_id,
            mask: &mask as *const _ as *mut c_uchar,
            mask_len: mem::size_of_val(&mask) as c_int,
        };
        unsafe {
            (self.xinput2.XISelectEvents)(
                self.display,
                window,
                &mut event_mask as *mut ffi::XIEventMask,
                1, // number of masks to read from pointer above
            );
        }
        Flusher::new(self)
    }

    #[allow(dead_code)]
    pub fn select_xkb_events(&self, device_id: c_uint, mask: c_ulong) -> Option<Flusher> {
        let status = unsafe {
            (self.xlib.XkbSelectEvents)(
                self.display,
                device_id,
                mask,
                mask,
            )
        };
        if status == ffi::True {
            Some(Flusher::new(self))
        } else {
            None
        }
    }

    pub fn query_pointer(&self, window: ffi::Window, device_id: c_int) -> Result<PointerState, XError> {
        unsafe {
            let mut pointer_state: PointerState = mem::uninitialized();
            pointer_state.xconn = self;
            pointer_state.relative_to_window = (self.xinput2.XIQueryPointer)(
                self.display,
                device_id,
                window,
                &mut pointer_state.root,
                &mut pointer_state.child,
                &mut pointer_state.root_x,
                &mut pointer_state.root_y,
                &mut pointer_state.win_x,
                &mut pointer_state.win_y,
                &mut pointer_state.buttons,
                &mut pointer_state.modifiers,
                &mut pointer_state.group,
            ) == ffi::True;
            if let Err(err) = self.check_errors() {
                // Running the destrutor would be bad news for us...
                mem::forget(pointer_state);
                Err(err)
            } else {
                Ok(pointer_state)
            }
        }
    }

    fn lookup_utf8_inner(
        &self,
        ic: ffi::XIC,
        key_event: &mut ffi::XKeyEvent,
        buffer: &mut [u8],
    ) -> (ffi::KeySym, ffi::Status, c_int) {
        let mut keysym: ffi::KeySym = 0;
        let mut status: ffi::Status = 0;
        let count = unsafe {
            (self.xlib.Xutf8LookupString)(
                ic,
                key_event,
                buffer.as_mut_ptr() as *mut c_char,
                buffer.len() as c_int,
                &mut keysym,
                &mut status,
            )
        };
        (keysym, status, count)
    }

    pub fn lookup_utf8(&self, ic: ffi::XIC, key_event: &mut ffi::XKeyEvent) -> String {
        let mut buffer: [u8; TEXT_BUFFER_SIZE] = unsafe { mem::uninitialized() };
        let (_, status, count) = self.lookup_utf8_inner(ic, key_event, &mut buffer);
        // The buffer overflowed, so we'll make a new one on the heap.
        if status == ffi::XBufferOverflow {
            let mut buffer = Vec::with_capacity(count as usize);
            unsafe { buffer.set_len(count as usize) };
            let (_, _, new_count) = self.lookup_utf8_inner(ic, key_event, &mut buffer);
            debug_assert_eq!(count, new_count);
            str::from_utf8(&buffer[..count as usize])
                .unwrap_or("")
                .to_string()
        } else {
            str::from_utf8(&buffer[..count as usize])
                .unwrap_or("")
                .to_string()
        }
    }
}
