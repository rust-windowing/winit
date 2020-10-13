use sctk::reexports::client::protocol::wl_seat::WlSeat;
use sctk::reexports::client::Attached;
use sctk::reexports::protocols::unstable::text_input::v3::client::zwp_text_input_manager_v3::ZwpTextInputManagerV3;
use sctk::reexports::protocols::unstable::text_input::v3::client::zwp_text_input_v3::ZwpTextInputV3;

use crate::platform_impl::wayland::event_loop::WinitState;
use crate::platform_impl::wayland::WindowId;

mod handlers;

/// A handler for text input that we're advertising for `WindowHandle`.
#[derive(Eq, PartialEq)]
pub struct TextInputHandler {
    text_input: ZwpTextInputV3,
}

impl TextInputHandler {
    #[inline]
    pub fn set_ime_position(&self, x: i32, y: i32) {
        self.text_input.set_cursor_rectangle(x, y, 0, 0);
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

    /// Pending string to commit.
    commit_string: Option<String>,
}

impl TextInputInner {
    fn new() -> Self {
        Self {
            target_window_id: None,
            commit_string: None,
        }
    }
}
