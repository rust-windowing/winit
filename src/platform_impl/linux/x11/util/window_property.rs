use super::*;
use xcb_dl_util::format::XcbDataType;
use xcb_dl_util::property::XcbGetPropertyError;

pub type Cardinal = u32;
pub const CARDINAL_SIZE: usize = mem::size_of::<Cardinal>();

#[derive(Debug)]
#[allow(dead_code)]
#[repr(u32)]
pub enum PropMode {
    Replace = ffi::XCB_PROP_MODE_REPLACE,
    Prepend = ffi::XCB_PROP_MODE_PREPEND,
    Append = ffi::XCB_PROP_MODE_APPEND,
}

impl XConnection {
    pub fn get_property<T: XcbDataType>(
        &self,
        window: ffi::xcb_window_t,
        property: ffi::xcb_atom_t,
        property_type: ffi::xcb_atom_t,
    ) -> Result<Vec<T>, XcbGetPropertyError> {
        const STEP: u32 = 256 * 1024;
        unsafe {
            xcb_dl_util::property::get_property(
                &self.xcb,
                &self.errors,
                window,
                property,
                property_type,
                false,
                STEP,
            )
        }
    }

    pub fn change_property<T: XcbDataType>(
        &self,
        window: ffi::xcb_window_t,
        property: ffi::xcb_atom_t,
        property_type: ffi::xcb_atom_t,
        mode: PropMode,
        new_value: &[T],
    ) -> XcbPendingCommand {
        unsafe {
            self.xcb
                .xcb_change_property_checked(
                    self.c,
                    mode as u8,
                    window,
                    property,
                    property_type,
                    T::XCB_BITS,
                    new_value.len() as _,
                    new_value.as_ptr() as _,
                )
                .into()
        }
    }
}
