use std::{slice, str};

use super::*;
use crate::event::ModifiersState;

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

    #[allow(dead_code)]
    pub fn select_xkb_events(&self, device_id: c_uint, mask: c_ulong) -> Option<Flusher<'_>> {
        let status = unsafe { (self.xlib.XkbSelectEvents)(self.display, device_id, mask, mask) };
        if status == ffi::True {
            Some(Flusher::new(self))
        } else {
            None
        }
    }

    pub fn query_pointer(
        &self,
        window: ffi::Window,
        device_id: c_int,
    ) -> Result<PointerState<'_>, XError> {
        unsafe {
            let mut root = MaybeUninit::uninit();
            let mut child = MaybeUninit::uninit();
            let mut root_x = MaybeUninit::uninit();
            let mut root_y = MaybeUninit::uninit();
            let mut win_x = MaybeUninit::uninit();
            let mut win_y = MaybeUninit::uninit();
            let mut buttons = MaybeUninit::uninit();
            let mut modifiers = MaybeUninit::uninit();
            let mut group = MaybeUninit::uninit();

            let relative_to_window = (self.xinput2.XIQueryPointer)(
                self.display,
                device_id,
                window,
                root.as_mut_ptr(),
                child.as_mut_ptr(),
                root_x.as_mut_ptr(),
                root_y.as_mut_ptr(),
                win_x.as_mut_ptr(),
                win_y.as_mut_ptr(),
                buttons.as_mut_ptr(),
                modifiers.as_mut_ptr(),
                group.as_mut_ptr(),
            ) == ffi::True;

            self.check_errors()?;

            Ok(PointerState {
                xconn: self,
                root: root.assume_init(),
                child: child.assume_init(),
                root_x: root_x.assume_init(),
                root_y: root_y.assume_init(),
                win_x: win_x.assume_init(),
                win_y: win_y.assume_init(),
                buttons: buttons.assume_init(),
                modifiers: modifiers.assume_init(),
                group: group.assume_init(),
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
