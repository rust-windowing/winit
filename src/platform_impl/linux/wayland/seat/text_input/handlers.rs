//! Handling of IME events.

use sctk::reexports::client::Main;
use sctk::reexports::protocols::unstable::text_input::v3::client::zwp_text_input_v3::{
    Event as TextInputEvent, ZwpTextInputV3,
};

use crate::event::WindowEvent;
use crate::platform_impl::wayland;
use crate::platform_impl::wayland::event_loop::WinitState;

use super::{TextInputHandler, TextInputInner};

#[inline]
pub(super) fn handle_text_input(
    text_input: Main<ZwpTextInputV3>,
    inner: &mut TextInputInner,
    event: TextInputEvent,
    winit_state: &mut WinitState,
) {
    let event_sink = &mut winit_state.event_sink;
    match event {
        TextInputEvent::Enter { surface } => {
            let window_id = wayland::make_wid(&surface);

            let window_handle = match winit_state.window_map.get_mut(&window_id) {
                Some(window_handle) => window_handle,
                None => return,
            };
            inner.target_window_id = Some(window_id);

            // Enable text input on that surface.
            text_input.enable();
            text_input.commit();

            // Notify a window we're currently over about text input handler.
            let text_input_handler = TextInputHandler {
                text_input: text_input.detach(),
            };
            window_handle.text_input_entered(text_input_handler);
        }
        TextInputEvent::Leave { surface } => {
            // Always issue a disable.
            text_input.disable();
            text_input.commit();

            let window_id = wayland::make_wid(&surface);

            let window_handle = match winit_state.window_map.get_mut(&window_id) {
                Some(window_handle) => window_handle,
                None => return,
            };

            inner.target_window_id = None;

            // Remove text input handler from the window we're leaving.
            let text_input_handler = TextInputHandler {
                text_input: text_input.detach(),
            };
            window_handle.text_input_left(text_input_handler);
        }
        TextInputEvent::CommitString { text } => {
            // Update currenly commited string.
            inner.commit_string = text;
        }
        TextInputEvent::Done { .. } => {
            let (window_id, text) = match (inner.target_window_id, inner.commit_string.take()) {
                (Some(window_id), Some(text)) => (window_id, text),
                _ => return,
            };

            for ch in text.chars() {
                // event_sink.push_window_event(WindowEvent::ReceivedCharacter(ch), window_id);
            }
        }
        _ => (),
    }
}
