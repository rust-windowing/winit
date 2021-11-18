use super::*;
use xcb_dl_util::error::XcbError;
use xcb_dl_util::xcb_box::XcbBox;

pub const VIRTUAL_CORE_POINTER: ffi::xcb_input_device_id_t = 2;

impl XConnection {
    pub fn select_xinput_events(
        &self,
        window: ffi::xcb_window_t,
        device_id: ffi::xcb_input_device_id_t,
        mask: u32,
    ) -> XcbPendingCommand {
        unsafe {
            xcb_dl_util::input::select_events_checked(
                &self.xinput,
                self.c,
                window,
                device_id,
                [mask],
            )
            .into()
        }
    }

    pub fn select_xkb_events(
        &self,
        device_id: ffi::xcb_input_device_id_t,
        events: ffi::xcb_xkb_event_type_t,
    ) -> XcbPendingCommand {
        unsafe {
            self.xkb
                .xcb_xkb_select_events_checked(
                    self.c,
                    device_id,
                    events as _,
                    0,
                    events as _,
                    !0,
                    !0,
                    ptr::null(),
                )
                .into()
        }
    }

    pub fn select_xkb_event_details(
        &self,
        device_id: ffi::xcb_input_device_id_t,
        events: ffi::xcb_xkb_event_type_t,
        details: &ffi::xcb_xkb_select_events_details_t,
    ) -> XcbPendingCommand {
        unsafe {
            self.xkb
                .xcb_xkb_select_events_aux_checked(
                    self.c,
                    device_id,
                    events as _,
                    0,
                    0,
                    !0,
                    !0,
                    details,
                )
                .into()
        }
    }

    pub fn query_pointer(
        &self,
        window: ffi::xcb_window_t,
        device_id: ffi::xcb_input_device_id_t,
    ) -> Result<XcbBox<ffi::xcb_input_xi_query_pointer_reply_t>, XcbError> {
        unsafe {
            let mut err = ptr::null_mut();
            let reply = self.xinput.xcb_input_xi_query_pointer_reply(
                self.c,
                self.xinput
                    .xcb_input_xi_query_pointer(self.c, window, device_id),
                &mut err,
            );
            self.check(reply, err)
        }
    }

    pub fn make_auto_repeat_detectable(
        &self,
        device_id: ffi::xcb_input_device_id_t,
    ) -> Result<bool, XcbError> {
        unsafe {
            let cookie = self.xkb.xcb_xkb_per_client_flags(
                self.c,
                device_id,
                ffi::XCB_XKB_PER_CLIENT_FLAG_DETECTABLE_AUTO_REPEAT,
                ffi::XCB_XKB_PER_CLIENT_FLAG_DETECTABLE_AUTO_REPEAT,
                0,
                0,
                0,
            );
            let mut err = ptr::null_mut();
            let reply = self
                .xkb
                .xcb_xkb_per_client_flags_reply(self.c, cookie, &mut err);
            let reply = self.check(reply, err)?;
            Ok(reply.supported & ffi::XCB_XKB_PER_CLIENT_FLAG_DETECTABLE_AUTO_REPEAT != 0)
        }
    }
}
