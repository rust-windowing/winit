//! Handling of IME events.

use sctk::reexports::client::Main;
use sctk::reexports::protocols::unstable::text_input::v3::client::zwp_text_input_v3::{
    Event as TextInputEvent, ZwpTextInputV3,
};

use crate::event::{Ime, WindowEvent};
use crate::platform_impl::wayland;
use crate::platform_impl::wayland::event_loop::WinitState;

use super::{Preedit, TextInputHandler, TextInputInner, ZwpTextInputV3Ext};

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
            if window_handle.ime_allowed.get() {
                text_input.enable();
                text_input.set_content_type_by_purpose(window_handle.ime_purpose.get());
                text_input.commit();
                event_sink.push_window_event(WindowEvent::Ime(Ime::Enabled), window_id);
            }

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
            event_sink.push_window_event(WindowEvent::Ime(Ime::Disabled), window_id);
        }
        TextInputEvent::PreeditString {
            text,
            cursor_begin,
            cursor_end,
        } => {
            let text = text.unwrap_or_default();
            let cursor_begin = usize::try_from(cursor_begin)
                .ok()
                .and_then(|idx| text.is_char_boundary(idx).then(|| idx));
            let cursor_end = usize::try_from(cursor_end)
                .ok()
                .and_then(|idx| text.is_char_boundary(idx).then(|| idx));

            inner.pending_preedit = Some(Preedit {
                text,
                cursor_begin,
                cursor_end,
            });
        }
        TextInputEvent::CommitString { text } => {
            // Update currenly commited string and reset previous preedit.
            inner.pending_preedit = None;
            inner.pending_commit = Some(text.unwrap_or_default());
        }
        TextInputEvent::Done { .. } => {
            let window_id = match inner.target_window_id {
                Some(window_id) => window_id,
                _ => return,
            };

            // Clear preedit at the start of `Done`.
            event_sink.push_window_event(
                WindowEvent::Ime(Ime::Preedit(String::new(), None)),
                window_id,
            );

            // Send `Commit`.
            if let Some(text) = inner.pending_commit.take() {
                event_sink.push_window_event(WindowEvent::Ime(Ime::Commit(text)), window_id);
            }

            // Send preedit.
            if let Some(preedit) = inner.pending_preedit.take() {
                let cursor_range = preedit
                    .cursor_begin
                    .map(|b| (b, preedit.cursor_end.unwrap_or(b)));

                event_sink.push_window_event(
                    WindowEvent::Ime(Ime::Preedit(preedit.text, cursor_range)),
                    window_id,
                );
            }
        }
        _ => (),
    }
}
