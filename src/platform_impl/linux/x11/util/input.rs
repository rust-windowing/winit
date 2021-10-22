use super::*;

pub const VIRTUAL_CORE_POINTER: c_int = 2;

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
    pub relative_to_window: bool,
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

    pub fn select_xkb_event_details(
        &self,
        device_id: c_uint,
        event: c_uint,
        mask: c_ulong,
    ) -> Option<Flusher<'_>> {
        let status = unsafe {
            (self.xlib.XkbSelectEventDetails)(self.display, device_id, event, mask, mask)
        };
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
                relative_to_window,
            })
        }
    }
}
