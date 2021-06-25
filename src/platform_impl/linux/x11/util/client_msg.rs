use super::*;

pub type ClientMsgPayload = [c_long; 5];

impl XConnection {
    pub fn send_event<T: Into<ffi::XEvent>>(
        &self,
        target_window: c_ulong,
        event_mask: Option<c_long>,
        event: T,
    ) -> Flusher<'_> {
        let event_mask = event_mask.unwrap_or(ffi::NoEventMask);
        unsafe {
            (self.xlib.XSendEvent)(
                self.display,
                target_window,
                ffi::False,
                event_mask,
                &mut event.into(),
            );
        }
        Flusher::new(self)
    }

    pub fn send_client_msg(
        &self,
        window: c_ulong, // The window this is "about"; not necessarily this window
        target_window: c_ulong, // The window we're sending to
        message_type: ffi::Atom,
        event_mask: Option<c_long>,
        data: ClientMsgPayload,
    ) -> Flusher<'_> {
        let event = ffi::XClientMessageEvent {
            type_: ffi::ClientMessage,
            display: self.display,
            window,
            message_type,
            format: c_long::FORMAT as c_int,
            data: unsafe { mem::transmute(data) },
            // These fields are ignored by `XSendEvent`
            serial: 0,
            send_event: 0,
        };
        self.send_event(target_window, event_mask, event)
    }
}
