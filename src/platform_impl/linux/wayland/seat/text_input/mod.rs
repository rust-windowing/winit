use sctk::reexports::client::protocol::wl_seat::WlSeat;
use sctk::reexports::client::Attached;
use sctk::reexports::protocols::unstable::text_input::v3::client::zwp_text_input_manager_v3::ZwpTextInputManagerV3;
use sctk::reexports::protocols::unstable::text_input::v3::client::zwp_text_input_v3::{
    ContentHint, ContentPurpose, ZwpTextInputV3,
};

use crate::platform_impl::wayland::event_loop::WinitState;
use crate::platform_impl::wayland::WindowId;
use crate::window::ImePurpose;

mod handlers;

/// A handler for text input that we're advertising for `WindowHandle`.
#[derive(Eq, PartialEq)]
pub struct TextInputHandler {
    text_input: ZwpTextInputV3,
}

trait ZwpTextInputV3Ext {
    fn set_content_type_by_purpose(&self, purpose: ImePurpose);
}

impl ZwpTextInputV3Ext for ZwpTextInputV3 {
    fn set_content_type_by_purpose(&self, purpose: ImePurpose) {
        let (hint, purpose) = match purpose {
            ImePurpose::Normal => (ContentHint::None, ContentPurpose::Normal),
            ImePurpose::Password => (ContentHint::SensitiveData, ContentPurpose::Password),
            ImePurpose::Terminal => (ContentHint::None, ContentPurpose::Terminal),
        };
        self.set_content_type(hint, purpose);
    }
}

impl TextInputHandler {
    #[inline]
    pub fn set_ime_position(&self, x: i32, y: i32) {
        self.text_input.set_cursor_rectangle(x, y, 0, 0);
        self.text_input.commit();
    }

    #[inline]
    pub fn set_content_type_by_purpose(&self, purpose: ImePurpose) {
        self.text_input.set_content_type_by_purpose(purpose);
        self.text_input.commit();
    }

    #[inline]
    pub fn set_input_allowed(&self, allowed: Option<ImePurpose>) {
        if let Some(purpose) = allowed {
            self.text_input.set_content_type_by_purpose(purpose);
            self.text_input.enable();
        } else {
            self.text_input.disable();
        }

        self.text_input.commit();
    }
}

/// A wrapper around text input to automatically destroy the object on `Drop`.
pub struct TextInput {
    text_input: Attached<ZwpTextInputV3>,
}

impl TextInput {
    pub fn new(seat: &Attached<WlSeat>, text_input_manager: &ZwpTextInputManagerV3) -> Self {
        let text_input = text_input_manager.get_text_input(seat);
        let mut text_input_inner = TextInputInner::new();
        text_input.quick_assign(move |text_input, event, mut dispatch_data| {
            let winit_state = dispatch_data.get::<WinitState>().unwrap();
            handlers::handle_text_input(text_input, &mut text_input_inner, event, winit_state);
        });

        let text_input: Attached<ZwpTextInputV3> = text_input.into();

        Self { text_input }
    }
}

impl Drop for TextInput {
    fn drop(&mut self) {
        self.text_input.destroy();
    }
}

struct TextInputInner {
    /// Currently focused surface.
    target_window_id: Option<WindowId>,

    /// Pending commit event which will be dispatched on `text_input_v3::Done`.
    pending_commit: Option<String>,

    /// Pending preedit event which will be dispatched on `text_input_v3::Done`.
    pending_preedit: Option<Preedit>,
}

struct Preedit {
    text: String,
    cursor_begin: Option<usize>,
    cursor_end: Option<usize>,
}

impl TextInputInner {
    fn new() -> Self {
        Self {
            target_window_id: None,
            pending_commit: None,
            pending_preedit: None,
        }
    }
}
