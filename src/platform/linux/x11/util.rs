use std::mem;
use std::ptr;
use std::sync::Arc;
use std::ops::{Deref, DerefMut};
use std::os::raw::{c_char, c_double, c_int, c_long, c_short, c_uchar, c_uint, c_ulong};

use super::{ffi, XConnection, XError};
use events::ModifiersState;

pub struct XSmartPointer<'a, T> {
    xconn: &'a Arc<XConnection>,
    pub ptr: *mut T,
}

impl<'a, T> XSmartPointer<'a, T> {
    // You're responsible for only passing things to this that should be XFree'd.
    // Returns None if ptr is null.
    pub fn new(xconn: &'a Arc<XConnection>, ptr: *mut T) -> Option<Self> {
        if !ptr.is_null() {
            Some(XSmartPointer {
                xconn,
                ptr,
            })
        } else {
            None
        }
    }
}

impl<'a, T> Deref for XSmartPointer<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.ptr }
    }
}

impl<'a, T> DerefMut for XSmartPointer<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.ptr }
    }
}

impl<'a, T> Drop for XSmartPointer<'a, T> {
    fn drop(&mut self) {
        unsafe {
            (self.xconn.xlib.XFree)(self.ptr as *mut _);
        }
    }
}

pub unsafe fn get_atom(xconn: &Arc<XConnection>, name: &[u8]) -> Result<ffi::Atom, XError> {
    let atom_name: *const c_char = name.as_ptr() as _;
    let atom = (xconn.xlib.XInternAtom)(xconn.display, atom_name, ffi::False);
    xconn.check_errors().map(|_| atom)
}

pub unsafe fn send_client_msg(
    xconn: &Arc<XConnection>,
    window: c_ulong,        // the window this is "about"; not necessarily this window
    target_window: c_ulong, // the window we're sending to
    message_type: ffi::Atom,
    event_mask: Option<c_long>,
    data: (c_long, c_long, c_long, c_long, c_long),
) -> Result<(), XError> {
    let mut event: ffi::XClientMessageEvent = mem::uninitialized();
    event.type_ = ffi::ClientMessage;
    event.display = xconn.display;
    event.window = window;
    event.message_type = message_type;
    event.format = 32;
    event.data = ffi::ClientMessageData::new();
    event.data.set_long(0, data.0);
    event.data.set_long(1, data.1);
    event.data.set_long(2, data.2);
    event.data.set_long(3, data.3);
    event.data.set_long(4, data.4);

    let event_mask = event_mask.unwrap_or(ffi::NoEventMask);

    (xconn.xlib.XSendEvent)(
        xconn.display,
        target_window,
        ffi::False,
        event_mask,
        &mut event.into(),
    );

    xconn.check_errors().map(|_| ())
}

#[derive(Debug)]
pub enum GetPropertyError {
    XError(XError),
    TypeMismatch(ffi::Atom),
    FormatMismatch(c_int),
    NothingAllocated,
}

pub unsafe fn get_property<T>(
    xconn: &Arc<XConnection>,
    window: c_ulong,
    property: ffi::Atom,
    property_type: ffi::Atom,
) -> Result<Vec<T>, GetPropertyError> {
    let mut data = Vec::new();

    let mut done = false;
    while !done {
        let mut actual_type: ffi::Atom = mem::uninitialized();
        let mut actual_format: c_int = mem::uninitialized();
        let mut byte_count: c_ulong = mem::uninitialized();
        let mut bytes_after: c_ulong = mem::uninitialized();
        let mut buf: *mut c_uchar = ptr::null_mut();
        (xconn.xlib.XGetWindowProperty)(
            xconn.display,
            window,
            property,
            (data.len() / 4) as c_long,
            1024,
            ffi::False,
            property_type,
            &mut actual_type,
            &mut actual_format,
            &mut byte_count,
            &mut bytes_after,
            &mut buf,
        );

        if let Err(e) = xconn.check_errors() {
            return Err(GetPropertyError::XError(e));
        }

        if actual_type != property_type {
            return Err(GetPropertyError::TypeMismatch(actual_type));
        }

        // Fun fact: actual_format ISN'T the size of the type; it's more like a really bad enum
        let format_mismatch = match actual_format as usize {
            8 => mem::size_of::<T>() != mem::size_of::<c_char>(),
            16 => mem::size_of::<T>() != mem::size_of::<c_short>(),
            32 => mem::size_of::<T>() != mem::size_of::<c_long>(),
            _ => true, // this won't actually be reached; the XError condition above is triggered
        };

        if format_mismatch {
            return Err(GetPropertyError::FormatMismatch(actual_format));
        }

        if !buf.is_null() {
            let mut buf =
                Vec::from_raw_parts(buf as *mut T, byte_count as usize, byte_count as usize);
            data.append(&mut buf);
        } else {
            return Err(GetPropertyError::NothingAllocated);
        }

        done = bytes_after == 0;
    }

    Ok(data)
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

#[derive(Debug)]
pub struct PointerState {
    #[allow(dead_code)]
    root: ffi::Window,
    #[allow(dead_code)]
    child: ffi::Window,
    #[allow(dead_code)]
    root_x: c_double,
    #[allow(dead_code)]
    root_y: c_double,
    #[allow(dead_code)]
    win_x: c_double,
    #[allow(dead_code)]
    win_y: c_double,
    #[allow(dead_code)]
    buttons: ffi::XIButtonState,
    modifiers: ffi::XIModifierState,
    #[allow(dead_code)]
    group: ffi::XIGroupState,
    #[allow(dead_code)]
    relative_to_window: bool,
}

impl PointerState {
    pub fn get_modifier_state(&self) -> ModifiersState {
        self.modifiers.into()
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
        root: root_return,
        child: child_return,
        root_x: root_x_return,
        root_y: root_y_return,
        win_x: win_x_return,
        win_y: win_y_return,
        buttons: buttons_return,
        modifiers: modifiers_return,
        group: group_return,
        relative_to_window,
    })
}
