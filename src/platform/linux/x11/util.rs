use std::mem;
use std::ptr;
use std::sync::Arc;

use libc::{c_char, c_int, c_long, c_short, c_uchar, c_ulong};

use super::{ffi, XConnection, XError};

pub unsafe fn send_client_msg(
    xconn: &Arc<XConnection>,
    target_window: c_ulong,
    message_type: ffi::Atom,
    data: (c_long, c_long, c_long, c_long, c_long),
) {
    let mut event: ffi::XClientMessageEvent = mem::uninitialized();
    event.type_ = ffi::ClientMessage;
    event.display = xconn.display;
    event.window = target_window;
    event.message_type = message_type;
    event.format = 32;
    event.data = ffi::ClientMessageData::new();
    event.data.set_long(0, data.0);
    event.data.set_long(1, data.1);
    event.data.set_long(2, data.2);
    event.data.set_long(3, data.3);
    event.data.set_long(4, data.4);

    (xconn.xlib.XSendEvent)(
        xconn.display,
        target_window,
        ffi::False,
        ffi::NoEventMask,
        &mut event.into(),
    );
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
