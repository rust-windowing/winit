use super::*;

pub type Cardinal = c_long;
pub const CARDINAL_SIZE: usize = mem::size_of::<c_long>();

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
// To test if `get_property` works correctly, set this to 1.
const PROPERTY_BUFFER_SIZE: c_long = 1024; // 4k of RAM ought to be enough for anyone!

#[derive(Debug)]
#[allow(dead_code)]
pub enum PropMode {
    Replace = ffi::PropModeReplace as isize,
    Prepend = ffi::PropModePrepend as isize,
    Append = ffi::PropModeAppend as isize,
}

impl XConnection {
    pub fn get_property<T: Formattable>(
        &self,
        window: c_ulong,
        property: ffi::Atom,
        property_type: ffi::Atom,
    ) -> Result<Vec<T>, GetPropertyError> {
        let mut data = Vec::new();
        let mut offset = 0;

        let mut done = false;
        let mut actual_type = 0;
        let mut actual_format = 0;
        let mut quantity_returned = 0;
        let mut bytes_after = 0;
        let mut buf: *mut c_uchar = ptr::null_mut();

        while !done {
            unsafe {
                (self.xlib.XGetWindowProperty)(
                    self.display,
                    window,
                    property,
                    // This offset is in terms of 32-bit chunks.
                    offset,
                    // This is the quantity of 32-bit chunks to receive at once.
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

                if let Err(e) = self.check_errors() {
                    return Err(GetPropertyError::XError(e));
                }

                if actual_type != property_type {
                    return Err(GetPropertyError::TypeMismatch(actual_type));
                }

                let format_mismatch = Format::from_format(actual_format as _) != Some(T::FORMAT);
                if format_mismatch {
                    return Err(GetPropertyError::FormatMismatch(actual_format));
                }

                if !buf.is_null() {
                    offset += PROPERTY_BUFFER_SIZE;
                    let new_data =
                        std::slice::from_raw_parts(buf as *mut T, quantity_returned as usize);
                    /*println!(
                        "XGetWindowProperty prop:{:?} fmt:{:02} len:{:02} off:{:02} out:{:02}, buf:{:?}",
                        property,
                        mem::size_of::<T>() * 8,
                        data.len(),
                        offset,
                        quantity_returned,
                        new_data,
                    );*/
                    data.extend_from_slice(new_data);
                    // Fun fact: XGetWindowProperty allocates one extra byte at the end.
                    (self.xlib.XFree)(buf as _); // Don't try to access new_data after this.
                } else {
                    return Err(GetPropertyError::NothingAllocated);
                }

                done = bytes_after == 0;
            }
        }

        Ok(data)
    }

    pub fn change_property<'a, T: Formattable>(
        &'a self,
        window: c_ulong,
        property: ffi::Atom,
        property_type: ffi::Atom,
        mode: PropMode,
        new_value: &[T],
    ) -> Flusher<'a> {
        debug_assert_eq!(mem::size_of::<T>(), T::FORMAT.get_actual_size());
        unsafe {
            (self.xlib.XChangeProperty)(
                self.display,
                window,
                property,
                property_type,
                T::FORMAT as c_int,
                mode as c_int,
                new_value.as_ptr() as *const c_uchar,
                new_value.len() as c_int,
            );
        }
        /*println!(
            "XChangeProperty prop:{:?} val:{:?}",
            property,
            new_value,
        );*/
        Flusher::new(self)
    }
}
