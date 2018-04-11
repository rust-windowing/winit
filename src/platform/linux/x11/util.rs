use std::mem;
use std::ptr;
use std::str;
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

impl GetPropertyError {
    pub fn is_actual_property_type(&self, t: ffi::Atom) -> bool {
        if let GetPropertyError::TypeMismatch(actual_type) = *self {
            actual_type == t
        } else {
            false
        }
    }
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

pub unsafe fn lookup_utf8(
    xconn: &Arc<XConnection>,
    ic: ffi::XIC,
    key_event: &mut ffi::XKeyEvent,
) -> String {
    const INIT_BUFF_SIZE: usize = 16;

    // Buffer allocated on heap instead of stack, due to the possible reallocation
    let mut buffer: Vec<u8> = vec![mem::uninitialized(); INIT_BUFF_SIZE];
    let (_, status, mut count) = lookup_utf8_inner(
        xconn,
        ic,
        key_event,
        &mut buffer,
    );

    // Buffer overflowed, dynamically reallocate
    if status == ffi::XBufferOverflow {
        buffer = vec![mem::uninitialized(); count as usize];
        let (_, _, new_count) = lookup_utf8_inner(
            xconn,
            ic,
            key_event,
            &mut buffer,
        );
        count = new_count;
    }

    str::from_utf8(&buffer[..count as usize]).unwrap_or("").to_string()
}


#[derive(Debug)]
pub struct FrameExtents {
    pub left: c_ulong,
    pub right: c_ulong,
    pub top: c_ulong,
    pub bottom: c_ulong,
}

impl FrameExtents {
    pub fn new(left: c_ulong, right: c_ulong, top: c_ulong, bottom: c_ulong) -> Self {
        FrameExtents { left, right, top, bottom }
    }

    pub fn from_border(border: c_ulong) -> Self {
        Self::new(border, border, border, border)
    }
}

#[derive(Debug)]
pub struct WindowGeometry {
    pub x: c_int,
    pub y: c_int,
    pub width: c_uint,
    pub height: c_uint,
    pub frame: FrameExtents,
}

impl WindowGeometry {
    pub fn get_position(&self) -> (i32, i32) {
        (self.x as _, self.y as _)
    }

    pub fn get_inner_position(&self) -> (i32, i32) {
        (
            self.x.saturating_add(self.frame.left as c_int) as _,
            self.y.saturating_add(self.frame.top as c_int) as _,
        )
    }

    pub fn get_inner_size(&self) -> (u32, u32) {
        (self.width as _, self.height as _)
    }

    pub fn get_outer_size(&self) -> (u32, u32) {
        (
            self.width.saturating_add(
                self.frame.left.saturating_add(self.frame.right) as c_uint
            ) as _,
            self.height.saturating_add(
                self.frame.top.saturating_add(self.frame.bottom) as c_uint
            ) as _,
        )
    }
}

// Important: all XIM calls need to happen from the same thread!
pub struct Ime {
    xconn: Arc<XConnection>,
    pub im: ffi::XIM,
    pub ic: ffi::XIC,
    ic_spot: ffi::XPoint,
}

impl Ime {
    pub fn new(xconn: Arc<XConnection>, window: ffi::Window) -> Option<Self> {
        let im = unsafe {
            let mut im: ffi::XIM = ptr::null_mut();

            // Setting an empty string as the locale modifier results in the user's XMODIFIERS
            // environment variable being read, which should result in the user's configured input
            // method (ibus, fcitx, etc.) being used. If that fails, we fall back to internal
            // input methods which should always be available, though only support compose keys.
            for modifiers in &[b"\0" as &[u8], b"@im=local\0", b"@im=\0"] {
                if !im.is_null() {
                    break;
                }

                (xconn.xlib.XSetLocaleModifiers)(modifiers.as_ptr() as *const _);
                im = (xconn.xlib.XOpenIM)(
                    xconn.display,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                );
            }

            if im.is_null() {
                return None;
            }

            im
        };

        let ic = unsafe {
            let ic = (xconn.xlib.XCreateIC)(
                im,
                b"inputStyle\0".as_ptr() as *const _,
                ffi::XIMPreeditNothing | ffi::XIMStatusNothing,
                b"clientWindow\0".as_ptr() as *const _,
                window,
                ptr::null::<()>(),
            );
            if ic.is_null() {
                return None;
            }
            (xconn.xlib.XSetICFocus)(ic);
            xconn.check_errors().expect("Failed to call XSetICFocus");
            ic
        };

        Some(Ime {
            xconn,
            im,
            ic,
            ic_spot: ffi::XPoint { x: 0, y: 0 },
        })
    }

    pub fn focus(&self) -> Result<(), XError> {
        unsafe {
            (self.xconn.xlib.XSetICFocus)(self.ic);
        }
        self.xconn.check_errors()
    }

    pub fn unfocus(&self) -> Result<(), XError> {
        unsafe {
            (self.xconn.xlib.XUnsetICFocus)(self.ic);
        }
        self.xconn.check_errors()
    }

    pub fn send_xim_spot(&mut self, x: i16, y: i16) {
        let nspot = ffi::XPoint { x: x as _, y: y as _ };
        if self.ic_spot.x == x && self.ic_spot.y == y {
            return;
        }
        self.ic_spot = nspot;
        unsafe {
            let preedit_attr = (self.xconn.xlib.XVaCreateNestedList)(
                0,
                b"spotLocation\0",
                &nspot,
                ptr::null::<()>(),
            );
            (self.xconn.xlib.XSetICValues)(
                self.ic,
                b"preeditAttributes\0",
                preedit_attr,
                ptr::null::<()>(),
            );
            (self.xconn.xlib.XFree)(preedit_attr);
        }
    }
}

impl Drop for Ime {
    fn drop(&mut self) {
        unsafe {
            (self.xconn.xlib.XDestroyIC)(self.ic);
            (self.xconn.xlib.XCloseIM)(self.im);
        }
        self.xconn.check_errors().expect("Failed to close input method");
    }
}
