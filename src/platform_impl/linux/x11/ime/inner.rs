use std::{collections::HashMap, mem, ptr, sync::Arc};

use super::{ffi, XConnection, XError};

use super::{context::ImeContext, input_method::PotentialInputMethods};
use crate::platform_impl::platform::x11::ime::ImeEventSender;

pub unsafe fn close_im(xconn: &Arc<XConnection>, im: ffi::XIM) -> Result<(), XError> {
    (xconn.xlib.XCloseIM)(im);
    xconn.check_errors()
}

pub unsafe fn destroy_ic(xconn: &Arc<XConnection>, ic: ffi::XIC) -> Result<(), XError> {
    (xconn.xlib.XDestroyIC)(ic);
    xconn.check_errors()
}

pub struct ImeInner {
    pub xconn: Arc<XConnection>,
    // WARNING: this is initially null!
    pub im: ffi::XIM,
    pub potential_input_methods: PotentialInputMethods,
    pub contexts: HashMap<ffi::Window, Option<ImeContext>>,
    // WARNING: this is initially zeroed!
    pub destroy_callback: ffi::XIMCallback,
    pub event_sender: ImeEventSender,
    // Indicates whether or not the the input method was destroyed on the server end
    // (i.e. if ibus/fcitx/etc. was terminated/restarted)
    pub is_destroyed: bool,
    pub is_fallback: bool,
}

impl ImeInner {
    pub fn new(
        xconn: Arc<XConnection>,
        potential_input_methods: PotentialInputMethods,
        event_sender: ImeEventSender,
    ) -> Self {
        ImeInner {
            xconn,
            im: ptr::null_mut(),
            potential_input_methods,
            contexts: HashMap::new(),
            destroy_callback: unsafe { mem::zeroed() },
            event_sender,
            is_destroyed: false,
            is_fallback: false,
        }
    }

    pub unsafe fn close_im_if_necessary(&self) -> Result<bool, XError> {
        if !self.is_destroyed {
            close_im(&self.xconn, self.im).map(|_| true)
        } else {
            Ok(false)
        }
    }

    pub unsafe fn destroy_ic_if_necessary(&self, ic: ffi::XIC) -> Result<bool, XError> {
        if !self.is_destroyed {
            destroy_ic(&self.xconn, ic).map(|_| true)
        } else {
            Ok(false)
        }
    }

    pub unsafe fn destroy_all_contexts_if_necessary(&self) -> Result<bool, XError> {
        for context in self.contexts.values() {
            if let &Some(ref context) = context {
                self.destroy_ic_if_necessary(context.ic)?;
            }
        }
        Ok(!self.is_destroyed)
    }
}
