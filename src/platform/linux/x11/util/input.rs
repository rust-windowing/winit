use super::*;
use events::ModifiersState;

pub unsafe fn select_xinput_events(
    xconn: &Arc<XConnection>,
    window: c_ulong,
    device_id: c_int,
    mask: i32,
) -> Flusher {
    let mut event_mask = ffi::XIEventMask {
        deviceid: device_id,
        mask: &mask as *const _ as *mut c_uchar,
        mask_len: mem::size_of_val(&mask) as c_int,
    };
    (xconn.xinput2.XISelectEvents)(
        xconn.display,
        window,
        &mut event_mask as *mut ffi::XIEventMask,
        1, // number of masks to read from pointer above
    );
    Flusher::new(xconn)
}

#[allow(dead_code)]
pub unsafe fn select_xkb_events(
    xconn: &Arc<XConnection>,
    device_id: c_uint,
    mask: c_ulong,
) -> Option<Flusher> {
    let status = (xconn.xlib.XkbSelectEvents)(
        xconn.display,
        device_id,
        mask,
        mask,
    );
    if status == ffi::True {
        Some(Flusher::new(xconn))
    } else {
        None
    }
}

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
    xconn: &'a Arc<XConnection>,
    _root: ffi::Window,
    _child: ffi::Window,
    _root_x: c_double,
    _root_y: c_double,
    _win_x: c_double,
    _win_y: c_double,
    _buttons: ffi::XIButtonState,
    modifiers: ffi::XIModifierState,
    _group: ffi::XIGroupState,
    _relative_to_window: bool,
}

impl<'a> PointerState<'a> {
    pub fn get_modifier_state(&self) -> ModifiersState {
        self.modifiers.into()
    }
}

impl<'a> Drop for PointerState<'a> {
    fn drop(&mut self) {
        unsafe {
            // This is why you need to read the docs carefully...
            (self.xconn.xlib.XFree)(self._buttons.mask as _);
        }
    }
}

pub unsafe fn query_pointer(
    xconn: &Arc<XConnection>,
    window: ffi::Window,
    device_id: c_int,
) -> Result<PointerState, XError> {
    let mut root_return = mem::uninitialized();
    let mut child_return = mem::uninitialized();
    let mut root_x_return = mem::uninitialized();
    let mut root_y_return = mem::uninitialized();
    let mut win_x_return = mem::uninitialized();
    let mut win_y_return = mem::uninitialized();
    let mut buttons_return = mem::uninitialized();
    let mut modifiers_return = mem::uninitialized();
    let mut group_return = mem::uninitialized();

    let relative_to_window = (xconn.xinput2.XIQueryPointer)(
        xconn.display,
        device_id,
        window,
        &mut root_return,
        &mut child_return,
        &mut root_x_return,
        &mut root_y_return,
        &mut win_x_return,
        &mut win_y_return,
        &mut buttons_return,
        &mut modifiers_return,
        &mut group_return,
    ) == ffi::True;

    xconn.check_errors()?;

    Ok(PointerState {
        xconn,
        _root: root_return,
        _child: child_return,
        _root_x: root_x_return,
        _root_y: root_y_return,
        _win_x: win_x_return,
        _win_y: win_y_return,
        _buttons: buttons_return,
        modifiers: modifiers_return,
        _group: group_return,
        _relative_to_window: relative_to_window,
    })
}

unsafe fn lookup_utf8_inner(
    xconn: &Arc<XConnection>,
    ic: ffi::XIC,
    key_event: &mut ffi::XKeyEvent,
    buffer: &mut [u8],
) -> (ffi::KeySym, ffi::Status, c_int) {
    let mut keysym: ffi::KeySym = 0;
    let mut status: ffi::Status = 0;
    let count = (xconn.xlib.Xutf8LookupString)(
        ic,
        key_event,
        buffer.as_mut_ptr() as *mut c_char,
        buffer.len() as c_int,
        &mut keysym,
        &mut status,
    );
    (keysym, status, count)
}

// A base buffer size of 1kB uses a negligible amount of RAM while preventing us from having to
// re-allocate (and make another round-trip) in the *vast* majority of cases.
// To test if lookup_utf8 works correctly, set this to 1.
const TEXT_BUFFER_SIZE: usize = 1024;

pub unsafe fn lookup_utf8(
    xconn: &Arc<XConnection>,
    ic: ffi::XIC,
    key_event: &mut ffi::XKeyEvent,
) -> String {
    let mut buffer: [u8; TEXT_BUFFER_SIZE] = mem::uninitialized();
    let (_, status, count) = lookup_utf8_inner(
        xconn,
        ic,
        key_event,
        &mut buffer,
    );

    // The buffer overflowed, so we'll make a new one on the heap.
    if status == ffi::XBufferOverflow {
        let mut buffer = Vec::with_capacity(count as usize);
        buffer.set_len(count as usize);
        let (_, _, new_count) = lookup_utf8_inner(
            xconn,
            ic,
            key_event,
            &mut buffer,
        );
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
