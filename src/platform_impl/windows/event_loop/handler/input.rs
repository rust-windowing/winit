//! Common input handling for Windows.

use super::super::Force;
use super::prelude::*;
use std::sync::MutexGuard;

/// Emit a `ModifiersChanged` event whenever modifiers have changed.
pub(super) fn update_modifiers(window: HWND, userdata: &dyn GenericWindowData) {
    use crate::event::WindowEvent::ModifiersChanged;

    let modifiers = event::get_key_mods();
    let mut window_state = userdata.window_state_lock();
    if window_state.modifiers_state != modifiers {
        window_state.modifiers_state = modifiers;

        // Drop lock
        drop(window_state);

        unsafe {
            userdata.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: ModifiersChanged(modifiers),
            });
        }
    }
}

/// Capture mouse input, allowing `window` to receive mouse events when the cursor is outside of
/// the window.
pub(super) unsafe fn capture_mouse(window: HWND, window_state: &mut WindowState) {
    window_state.mouse.capture_count += 1;
    SetCapture(window);
}

/// Release mouse input, stopping windows on this thread from receiving mouse input when the cursor
/// is outside the window.
pub(super) unsafe fn release_mouse(mut window_state: MutexGuard<'_, WindowState>) {
    window_state.mouse.capture_count = window_state.mouse.capture_count.saturating_sub(1);
    if window_state.mouse.capture_count == 0 {
        // ReleaseCapture() causes a WM_CAPTURECHANGED where we lock the window_state.
        drop(window_state);
        ReleaseCapture();
    }
}

pub(super) fn normalize_pointer_pressure(pressure: u32) -> Option<Force> {
    match pressure {
        1..=1024 => Some(Force::Normalized(pressure as f64 / 1024.0)),
        _ => None,
    }
}

pub(super) unsafe fn gain_active_focus(window: HWND, userdata: &dyn GenericWindowData) {
    use crate::event::{ElementState::Released, WindowEvent::Focused};
    for windows_keycode in event::get_pressed_keys() {
        let scancode = MapVirtualKeyA(windows_keycode as u32, MAPVK_VK_TO_VSC);
        let virtual_keycode = event::vkey_to_winit_vkey(windows_keycode);

        update_modifiers(window, userdata);

        #[allow(deprecated)]
        userdata.send_event(Event::WindowEvent {
            window_id: RootWindowId(WindowId(window)),
            event: WindowEvent::KeyboardInput {
                device_id: DEVICE_ID,
                input: KeyboardInput {
                    scancode,
                    virtual_keycode,
                    state: Released,
                    modifiers: event::get_key_mods(),
                },
                is_synthetic: true,
            },
        })
    }

    userdata.send_event(Event::WindowEvent {
        window_id: RootWindowId(WindowId(window)),
        event: Focused(true),
    });
}

pub(super) unsafe fn lose_active_focus(window: HWND, userdata: &dyn GenericWindowData) {
    use crate::event::{
        ElementState::Released,
        ModifiersState,
        WindowEvent::{Focused, ModifiersChanged},
    };
    for windows_keycode in event::get_pressed_keys() {
        let scancode = MapVirtualKeyA(windows_keycode as u32, MAPVK_VK_TO_VSC);
        let virtual_keycode = event::vkey_to_winit_vkey(windows_keycode);

        #[allow(deprecated)]
        userdata.send_event(Event::WindowEvent {
            window_id: RootWindowId(WindowId(window)),
            event: WindowEvent::KeyboardInput {
                device_id: DEVICE_ID,
                input: KeyboardInput {
                    scancode,
                    virtual_keycode,
                    state: Released,
                    modifiers: event::get_key_mods(),
                },
                is_synthetic: true,
            },
        })
    }

    userdata.window_state_lock().modifiers_state = ModifiersState::empty();
    userdata.send_event(Event::WindowEvent {
        window_id: RootWindowId(WindowId(window)),
        event: ModifiersChanged(ModifiersState::empty()),
    });

    userdata.send_event(Event::WindowEvent {
        window_id: RootWindowId(WindowId(window)),
        event: Focused(false),
    });
}
