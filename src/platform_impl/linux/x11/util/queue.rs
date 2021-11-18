use crate::platform_impl::x11::XConnection;
use std::sync::Arc;
use xcb_dl::ffi;
use xcb_dl_util::xcb_box::XcbBox;

pub struct EventQueue {
    xconn: Arc<XConnection>,
    pending: Option<XcbBox<ffi::xcb_generic_event_t>>,
}

impl EventQueue {
    pub fn new(xconn: &Arc<XConnection>) -> Self {
        Self {
            xconn: xconn.clone(),
            pending: None,
        }
    }

    pub fn has_pending_events(&mut self) -> bool {
        if let Some(event) = self.poll_for_event() {
            self.pending = Some(event);
        }
        self.pending.is_some()
    }

    pub fn poll_for_event(&mut self) -> Option<XcbBox<ffi::xcb_generic_event_t>> {
        if self.pending.is_some() {
            return self.pending.take();
        }
        unsafe {
            let event = self.xconn.xcb.xcb_poll_for_event(self.xconn.c);
            if event.is_null() {
                if let Err(e) = self.xconn.errors.check_connection(&self.xconn.xcb) {
                    panic!("The X connection is broken: {}", e);
                }
                None
            } else {
                Some(XcbBox::new(event))
            }
        }
    }
}
