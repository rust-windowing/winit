use std;
use std::fmt::Debug;

use super::*;

#[derive(Debug, Clone)]
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

// Number of 32-bit chunks to retrieve per iteration of get_property's inner loop.
// To test if get_property works correctly, set this to 1.
const PROPERTY_BUFFER_SIZE: c_long = 1024; // 4k of RAM ought to be enough for anyone!

pub unsafe fn get_property<T: Debug + Clone>(
    xconn: &Arc<XConnection>,
    window: c_ulong,
    property: ffi::Atom,
    property_type: ffi::Atom,
) -> Result<Vec<T>, GetPropertyError> {
    let mut data = Vec::new();
    let mut offset = 0;

    let mut done = false;
    while !done {
        let mut actual_type: ffi::Atom = mem::uninitialized();
        let mut actual_format: c_int = mem::uninitialized();
        let mut quantity_returned: c_ulong = mem::uninitialized();
        let mut bytes_after: c_ulong = mem::uninitialized();
        let mut buf: *mut c_uchar = ptr::null_mut();
        (xconn.xlib.XGetWindowProperty)(
            xconn.display,
            window,
            property,
            // This offset is in terms of 32-bit chunks.
            offset,
            // This is the quanity of 32-bit chunks to receive at once.
            PROPERTY_BUFFER_SIZE,
            ffi::False,
            property_type,
            &mut actual_type,
            &mut actual_format,
            // This is the quantity of items we retrieved in our format, NOT of 32-bit chunks!
            &mut quantity_returned,
            // ...and this is a quantity of bytes. So, this function deals in 3 different units.
            &mut bytes_after,
            &mut buf,
        );

        if let Err(e) = xconn.check_errors() {
            return Err(GetPropertyError::XError(e));
        }

        if actual_type != property_type {
            return Err(GetPropertyError::TypeMismatch(actual_type));
        }

        let format_mismatch = Format::from_format(actual_format as _)
            .map(|actual_format| !actual_format.is_same_size_as::<T>())
            // This won't actually be reached; the XError condition above is triggered first.
            .unwrap_or(true);

        if format_mismatch {
            return Err(GetPropertyError::FormatMismatch(actual_format));
        }

        if !buf.is_null() {
            offset += PROPERTY_BUFFER_SIZE;
            let new_data = std::slice::from_raw_parts(
                buf as *mut T,
                quantity_returned as usize,
            );
            /*println!(
                "XGetWindowProperty prop:{:?} fmt:{:02} len:{:02} off:{:02} out:{:02}, buf:{:?}",
                property,
                mem::size_of::<T>() * 8,
                data.len(),
                offset,
                quantity_returned,
                new_data,
            );*/
            data.extend_from_slice(&new_data);
            // Fun fact: XGetWindowProperty allocates one extra byte at the end.
            (xconn.xlib.XFree)(buf as _); // Don't try to access new_data after this.
        } else {
            return Err(GetPropertyError::NothingAllocated);
        }

        done = bytes_after == 0;
    }

    Ok(data)
}

#[derive(Debug)]
pub enum PropMode {
    Replace = ffi::PropModeReplace as isize,
    _Prepend = ffi::PropModePrepend as isize,
    _Append = ffi::PropModeAppend as isize,
}

#[derive(Debug, Clone)]
pub struct InvalidFormat {
    format_used: Format,
    size_passed: usize,
    size_expected: usize,
}

pub unsafe fn change_property<'a, T: Debug>(
    xconn: &'a Arc<XConnection>,
    window: c_ulong,
    property: ffi::Atom,
    property_type: ffi::Atom,
    format: Format,
    mode: PropMode,
    new_value: &[T],
) -> Flusher<'a> {
    if !format.is_same_size_as::<T>() {
        panic!(format!(
            "[winit developer error] Incorrect usage of `util::change_property`: {:#?}",
            InvalidFormat {
                format_used: format,
                size_passed: mem::size_of::<T>() * 8,
                size_expected: format.get_actual_size() * 8,
            },
        ));
    }

    (xconn.xlib.XChangeProperty)(
        xconn.display,
        window,
        property,
        property_type,
        format as c_int,
        mode as c_int,
        new_value.as_ptr() as *const c_uchar,
        new_value.len() as c_int,
    );

    /*println!(
        "XChangeProperty prop:{:?} val:{:?}",
        property,
        new_value,
    );*/

    Flusher::new(xconn)
}
