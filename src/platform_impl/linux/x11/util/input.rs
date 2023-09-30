use x11rb::protocol::{
    xinput::{self, ConnectionExt as _},
    xkb,
};

use super::*;

pub const VIRTUAL_CORE_POINTER: u16 = 2;
pub const VIRTUAL_CORE_KEYBOARD: u16 = 3;

impl XConnection {
    pub fn select_xinput_events(
        &self,
        window: xproto::Window,
        device_id: u16,
        mask: xinput::XIEventMask,
    ) -> Result<VoidCookie<'_>, X11Error> {
        self.xcb_connection()
            .xinput_xi_select_events(
                window,
                &[xinput::EventMask {
                    deviceid: device_id,
                    mask: vec![mask],
                }],
            )
            .map_err(Into::into)
    }

    pub fn select_xkb_events(
        &self,
        device_id: xkb::DeviceSpec,
        mask: xkb::EventType,
    ) -> Result<bool, X11Error> {
        let mask = u16::from(mask) as _;
        let status =
            unsafe { (self.xlib.XkbSelectEvents)(self.display, device_id as _, mask, mask) };

        if status == ffi::True {
            self.flush_requests()?;
            Ok(true)
        } else {
            error!("Could not select XKB events: The XKB extension is not initialized!");
            Ok(false)
        }
    }

    pub fn query_pointer(
        &self,
        window: xproto::Window,
        device_id: u16,
    ) -> Result<xinput::XIQueryPointerReply, X11Error> {
        self.xcb_connection()
            .xinput_xi_query_pointer(window, device_id)?
            .reply()
            .map_err(Into::into)
    }
}
