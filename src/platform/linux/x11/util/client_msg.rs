use super::*;

pub type ClientMsgPayload = [c_long; 5];

impl XConnection {
    pub fn send_event<T: Into<ffi::XEvent>>(
        &self,
        target_window: c_ulong,
        event_mask: Option<c_long>,
        event: T,
    ) -> Flusher {
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
        window: c_ulong,        // The window this is "about"; not necessarily this window
        target_window: c_ulong, // The window we're sending to
        message_type: ffi::Atom,
        event_mask: Option<c_long>,
        data: ClientMsgPayload,
    ) -> Flusher {
        let mut event: ffi::XClientMessageEvent = unsafe { mem::uninitialized() };
        event.type_ = ffi::ClientMessage;
        event.display = self.display;
        event.window = window;
        event.message_type = message_type;
        event.format = c_long::FORMAT as c_int;
        event.data = unsafe { mem::transmute(data) };
        self.send_event(target_window, event_mask, event)
    }

    // Prepare yourself for the ultimate in unsafety!
    // You should favor `send_client_msg` whenever possible, but some protocols (i.e. startup notification) require you
    // to send more than one message worth of data.
    pub fn send_client_msg_multi<T: Formattable>(
        &self,
        window: c_ulong,        // The window this is "about"; not necessarily this window
        target_window: c_ulong, // The window we're sending to
        message_type: ffi::Atom,
        event_mask: Option<c_long>,
        data: &[T],
    ) -> Flusher {
        let format = T::FORMAT;
        let size_of_t = mem::size_of::<T>();
        debug_assert_eq!(size_of_t, format.get_actual_size());
        let mut event: ffi::XClientMessageEvent = unsafe { mem::uninitialized() };
        event.type_ = ffi::ClientMessage;
        event.display = self.display;
        event.window = window;
        event.message_type = message_type;
        event.format = format as c_int;

        let t_per_payload = format.get_payload_size() / size_of_t;
        assert!(t_per_payload > 0);
        let payload_count = data.len() / t_per_payload;
        let payload_remainder = data.len() % t_per_payload;
        let payload_ptr = data.as_ptr() as *const ClientMsgPayload;

        let mut payload_index = 0;
        while payload_index < payload_count {
            let payload = unsafe { payload_ptr.offset(payload_index as isize) };
            payload_index += 1;
            event.data = unsafe { mem::transmute(*payload) };
            self.send_event(target_window, event_mask, &event).queue();
        }

        if payload_remainder > 0 {
            let mut payload: ClientMsgPayload = [0; 5];
            let t_payload = payload.as_mut_ptr() as *mut T;
            let invalid_payload = unsafe { payload_ptr.offset(payload_index as isize) };
            let invalid_t_payload = invalid_payload as *const T;
            let mut t_index = 0;
            while t_index < payload_remainder {
                let valid_t = unsafe { invalid_t_payload.offset(t_index as isize) };
                unsafe { (*t_payload.offset(t_index as isize)) = (*valid_t).clone() };
                t_index += 1;
            }
            event.data = unsafe { mem::transmute(payload) };
            self.send_event(target_window, event_mask, &event).queue();
        }

        Flusher::new(self)
    }
}
