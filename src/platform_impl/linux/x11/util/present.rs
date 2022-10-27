use std::os::raw::{c_int, c_uint};

use crate::error::{ExternalError, NotSupportedError};
use crate::platform_impl::OsError;

use super::ffi::{PresentCompleteNotifyMask, True, Window, XID};
use super::XConnection;

impl XConnection {
    pub fn xpresent_select_input(&self, window: Window) -> Result<(), ExternalError> {
        let xpresent = match self.xpresent.as_ref() {
            Some(xpresent) => xpresent,
            None => return Err(ExternalError::NotSupported(NotSupportedError::new())),
        };

        let mask = PresentCompleteNotifyMask as c_uint;
        unsafe {
            let _ = (xpresent.XPresentSelectInput)(self.display, window, mask);
        }

        if let Err(err) = self.check_errors() {
            Err(ExternalError::Os(os_error!(OsError::XError(err))))
        } else {
            Ok(())
        }
    }

    pub fn xpresent_free_input(
        &self,
        window: Window,
        event_id: XID,
    ) -> Result<(), NotSupportedError> {
        let xpresent = match self.xpresent.as_ref() {
            Some(xpresent) => xpresent,
            None => return Err(NotSupportedError::new()),
        };

        unsafe {
            (xpresent.XPresentFreeInput)(self.display, window, event_id);
        }

        // Drain errors.
        let _ = self.check_errors();

        Ok(())
    }

    pub fn xperest_event_offset(&self) -> Option<c_int> {
        let xpresent = self.xpresent.as_ref()?;
        unsafe {
            let mut major = 0;
            let mut minor = 0;
            if (xpresent.XPresentQueryVersion)(self.display, &mut major, &mut minor) != True as i32
            {
                return None;
            }
        }

        unsafe {
            let mut event_offset = 0;
            let mut error_offest = 0;
            let mut dummy = 0;

            // FIXME(kchibisov) the number of arguments is wrong, should fix upstream here.
            if (xpresent.XPresentQueryExtension)(
                self.display,
                &mut event_offset,
                &mut error_offest,
                &mut dummy,
            ) != True as i32
            {
                None
            } else {
                Some(event_offset)
            }
        }
    }
}
