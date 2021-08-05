use std::{slice, str};

use super::*;
use crate::keyboard::ModifiersState;

pub const VIRTUAL_CORE_POINTER: c_int = 2;
pub const VIRTUAL_CORE_KEYBOARD: c_int = 3;

// A base buffer size of 1kB uses a negligible amount of RAM while preventing us from having to
// re-allocate (and make another round-trip) in the *vast* majority of cases.
// To test if `lookup_utf8` works correctly, set this to 1.
const TEXT_BUFFER_SIZE: usize = 1024;

impl ModifiersState {
    pub(crate) fn from_x11(state: &ffi::XIModifierState) -> Self {
        ModifiersState::from_x11_mask(state.effective as c_uint)
    }

    pub(crate) fn from_x11_mask(mask: c_uint) -> Self {
        let mut m = ModifiersState::empty();
        m.set(ModifiersState::ALT, mask & ffi::Mod1Mask != 0);
        m.set(ModifiersState::SHIFT, mask & ffi::ShiftMask != 0);
        m.set(ModifiersState::CONTROL, mask & ffi::ControlMask != 0);
        m.set(ModifiersState::SUPER, mask & ffi::Mod4Mask != 0);
        m
    }
}

// NOTE: Some of these fields are not used, but may be of use in the future.
pub struct PointerState<'a> {
    xconn: &'a XConnection,
    pub root: ffi::Window,
    pub child: ffi::Window,
    pub root_x: c_double,
    pub root_y: c_double,
    pub win_x: c_double,
    pub win_y: c_double,
    buttons: ffi::XIButtonState,
    modifiers: ffi::XIModifierState,
    pub group: ffi::XIGroupState,
    pub relative_to_window: bool,
}

impl<'a> PointerState<'a> {
    pub fn get_modifier_state(&self) -> ModifiersState {
        ModifiersState::from_x11(&self.modifiers)
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
    pub fn select_xinput_events(
        &self,
        window: c_ulong,
        device_id: c_int,
        mask: i32,
    ) -> Flusher<'_> {
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

    pub fn select_xkb_events(&self, device_id: c_uint, mask: c_ulong) -> Option<Flusher<'_>> {
        let status = unsafe { (self.xlib.XkbSelectEvents)(self.display, device_id, mask, mask) };
        if status == ffi::True {
            Some(Flusher::new(self))
        } else {
            error!("Could not select XKB events: The XKB extension is not initialized!");
            None
        }
    }

    pub fn query_pointer(
        &self,
        window: ffi::Window,
        device_id: c_int,
    ) -> Result<PointerState<'_>, XError> {
        unsafe {
            let mut root = 0;
            let mut child = 0;
            let mut root_x = 0.0;
            let mut root_y = 0.0;
            let mut win_x = 0.0;
            let mut win_y = 0.0;
            let mut buttons = Default::default();
            let mut modifiers = Default::default();
            let mut group = Default::default();

            let relative_to_window = (self.xinput2.XIQueryPointer)(
                self.display,
                device_id,
                window,
                &mut root,
                &mut child,
                &mut root_x,
                &mut root_y,
                &mut win_x,
                &mut win_y,
                &mut buttons,
                &mut modifiers,
                &mut group,
            ) == ffi::True;

            self.check_errors()?;

            Ok(PointerState {
                xconn: self,
                root,
                child,
                root_x,
                root_y,
                win_x,
                win_y,
                buttons,
                modifiers,
                group,
                relative_to_window,
            })
        }
    }

    fn lookup_utf8_inner(
        &self,
        ic: ffi::XIC,
        key_event: &mut ffi::XKeyEvent,
        buffer: *mut u8,
        size: usize,
    ) -> (ffi::KeySym, ffi::Status, c_int) {
        let mut keysym: ffi::KeySym = 0;
        let mut status: ffi::Status = 0;
        let count = unsafe {
            (self.xlib.Xutf8LookupString)(
                ic,
                key_event,
                buffer as *mut c_char,
                size as c_int,
                &mut keysym,
                &mut status,
            )
        };
        (keysym, status, count)
    }

    pub fn lookup_utf8(&self, ic: ffi::XIC, key_event: &mut ffi::XKeyEvent) -> String {
        // `assume_init` is safe here because the array consists of `MaybeUninit` values,
        // which do not require initialization.
        let mut buffer: [MaybeUninit<u8>; TEXT_BUFFER_SIZE] =
            unsafe { MaybeUninit::uninit().assume_init() };
        // If the buffer overflows, we'll make a new one on the heap.
        let mut vec;

        let (_, status, count) =
            self.lookup_utf8_inner(ic, key_event, buffer.as_mut_ptr() as *mut u8, buffer.len());

        let bytes = if status == ffi::XBufferOverflow {
            vec = Vec::with_capacity(count as usize);
            let (_, _, new_count) =
                self.lookup_utf8_inner(ic, key_event, vec.as_mut_ptr(), vec.capacity());
            debug_assert_eq!(count, new_count);

            unsafe { vec.set_len(count as usize) };
            &vec[..count as usize]
        } else {
            unsafe { slice::from_raw_parts(buffer.as_ptr() as *const u8, count as usize) }
        };

        str::from_utf8(bytes).unwrap_or("").to_string()
    }
}
