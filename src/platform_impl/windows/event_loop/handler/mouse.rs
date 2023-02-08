//! Handles mouse events.

use super::prelude::*;

use std::mem;

fn handle_mousemove(
    window: HWND,
    _: u32,
    _: WPARAM,
    lparam: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    use crate::event::WindowEvent::{CursorEntered, CursorMoved};
    let mouse_was_outside_window = {
        let mut w = userdata.window_state_lock();

        let was_outside_window = !w.mouse.cursor_flags().contains(CursorFlags::IN_WINDOW);
        w.mouse
            .set_cursor_flags(window, |f| f.set(CursorFlags::IN_WINDOW, true))
            .ok();
        was_outside_window
    };

    if mouse_was_outside_window {
        unsafe {
            userdata.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: CursorEntered {
                    device_id: DEVICE_ID,
                },
            });

            // Calling TrackMouseEvent in order to receive mouse leave events.
            TrackMouseEvent(&mut TRACKMOUSEEVENT {
                cbSize: mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: window,
                dwHoverTime: HOVER_DEFAULT,
            });
        }
    }

    let x = get_x_lparam(lparam as u32) as f64;
    let y = get_y_lparam(lparam as u32) as f64;
    let position = PhysicalPosition::new(x, y);
    let cursor_moved;
    {
        // handle spurious WM_MOUSEMOVE messages
        // see https://devblogs.microsoft.com/oldnewthing/20031001-00/?p=42343
        // and http://debugandconquer.blogspot.com/2015/08/the-cause-of-spurious-mouse-move.html
        let mut w = userdata.window_state_lock();
        cursor_moved = w.mouse.last_position != Some(position);
        w.mouse.last_position = Some(position);
    }
    if cursor_moved {
        update_modifiers(window, userdata);

        unsafe {
            userdata.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: CursorMoved {
                    device_id: DEVICE_ID,
                    position,
                    modifiers: event::get_key_mods(),
                },
            });
        }
    }

    0
}

fn handle_mouseleave(
    window: HWND,
    _: u32,
    _: WPARAM,
    _: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    use crate::event::WindowEvent::CursorLeft;
    {
        let mut w = userdata.window_state_lock();
        w.mouse
            .set_cursor_flags(window, |f| f.set(CursorFlags::IN_WINDOW, false))
            .ok();
    }

    unsafe {
        userdata.send_event(Event::WindowEvent {
            window_id: RootWindowId(WindowId(window)),
            event: CursorLeft {
                device_id: DEVICE_ID,
            },
        });
    }

    0
}

fn handle_mousewheel(
    window: HWND,
    _: u32,
    wparam: WPARAM,
    _: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    use crate::event::MouseScrollDelta::LineDelta;

    let value = (wparam >> 16) as i16;
    let value = value as i32;
    let value = value as f32 / WHEEL_DELTA as f32;

    update_modifiers(window, userdata);

    unsafe {
        userdata.send_event(Event::WindowEvent {
            window_id: RootWindowId(WindowId(window)),
            event: WindowEvent::MouseWheel {
                device_id: DEVICE_ID,
                delta: LineDelta(0.0, value),
                phase: TouchPhase::Moved,
                modifiers: event::get_key_mods(),
            },
        });
    }

    0
}

fn handle_mousehwheel(
    window: HWND,
    _: u32,
    wparam: WPARAM,
    _: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    use crate::event::MouseScrollDelta::LineDelta;

    let value = (wparam >> 16) as i16;
    let value = value as i32;
    let value = -value as f32 / WHEEL_DELTA as f32; // NOTE: inverted! See https://github.com/rust-windowing/winit/pull/2105/

    update_modifiers(window, userdata);

    unsafe {
        userdata.send_event(Event::WindowEvent {
            window_id: RootWindowId(WindowId(window)),
            event: WindowEvent::MouseWheel {
                device_id: DEVICE_ID,
                delta: LineDelta(value, 0.0),
                phase: TouchPhase::Moved,
                modifiers: event::get_key_mods(),
            },
        });
    }

    0
}

fn handle_lbuttondown(
    window: HWND,
    _: u32,
    _: WPARAM,
    _: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    use crate::event::{ElementState::Pressed, MouseButton::Left, WindowEvent::MouseInput};

    unsafe {
        capture_mouse(window, &mut userdata.window_state_lock());
    }

    update_modifiers(window, userdata);

    unsafe {
        userdata.send_event(Event::WindowEvent {
            window_id: RootWindowId(WindowId(window)),
            event: MouseInput {
                device_id: DEVICE_ID,
                state: Pressed,
                button: Left,
                modifiers: event::get_key_mods(),
            },
        });
    }

    0
}

fn handle_lbuttonup(
    window: HWND,
    _: u32,
    _: WPARAM,
    _: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    use crate::event::{ElementState::Released, MouseButton::Left, WindowEvent::MouseInput};

    unsafe {
        release_mouse(userdata.window_state_lock());
    }

    update_modifiers(window, userdata);

    unsafe {
        userdata.send_event(Event::WindowEvent {
            window_id: RootWindowId(WindowId(window)),
            event: MouseInput {
                device_id: DEVICE_ID,
                state: Released,
                button: Left,
                modifiers: event::get_key_mods(),
            },
        });
    }

    0
}

fn handle_rbuttondown(
    window: HWND,
    _: u32,
    _: WPARAM,
    _: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    use crate::event::{ElementState::Pressed, MouseButton::Right, WindowEvent::MouseInput};

    unsafe {
        capture_mouse(window, &mut userdata.window_state_lock());
    }

    update_modifiers(window, userdata);

    unsafe {
        userdata.send_event(Event::WindowEvent {
            window_id: RootWindowId(WindowId(window)),
            event: MouseInput {
                device_id: DEVICE_ID,
                state: Pressed,
                button: Right,
                modifiers: event::get_key_mods(),
            },
        });
    }

    0
}

fn handle_rbuttonup(
    window: HWND,
    _: u32,
    _: WPARAM,
    _: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    use crate::event::{ElementState::Released, MouseButton::Right, WindowEvent::MouseInput};

    unsafe {
        release_mouse(userdata.window_state_lock());
    }

    update_modifiers(window, userdata);

    unsafe {
        userdata.send_event(Event::WindowEvent {
            window_id: RootWindowId(WindowId(window)),
            event: MouseInput {
                device_id: DEVICE_ID,
                state: Released,
                button: Right,
                modifiers: event::get_key_mods(),
            },
        });
    }

    0
}

fn handle_mbuttondown(
    window: HWND,
    _: u32,
    _: WPARAM,
    _: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    use crate::event::{ElementState::Pressed, MouseButton::Middle, WindowEvent::MouseInput};

    unsafe {
        capture_mouse(window, &mut userdata.window_state_lock());
    }

    update_modifiers(window, userdata);

    unsafe {
        userdata.send_event(Event::WindowEvent {
            window_id: RootWindowId(WindowId(window)),
            event: MouseInput {
                device_id: DEVICE_ID,
                state: Pressed,
                button: Middle,
                modifiers: event::get_key_mods(),
            },
        });
    }

    0
}

fn handle_mbuttonup(
    window: HWND,
    _: u32,
    _: WPARAM,
    _: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    use crate::event::{ElementState::Released, MouseButton::Middle, WindowEvent::MouseInput};

    unsafe {
        release_mouse(userdata.window_state_lock());
    }

    update_modifiers(window, userdata);

    unsafe {
        userdata.send_event(Event::WindowEvent {
            window_id: RootWindowId(WindowId(window)),
            event: MouseInput {
                device_id: DEVICE_ID,
                state: Released,
                button: Middle,
                modifiers: event::get_key_mods(),
            },
        });
    }

    0
}

fn handle_xbuttondown(
    window: HWND,
    _: u32,
    wparam: WPARAM,
    _: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    use crate::event::{ElementState::Pressed, MouseButton::Other, WindowEvent::MouseInput};
    let xbutton = get_xbutton_wparam(wparam as u32);

    unsafe {
        capture_mouse(window, &mut userdata.window_state_lock());
    }

    update_modifiers(window, userdata);

    unsafe {
        userdata.send_event(Event::WindowEvent {
            window_id: RootWindowId(WindowId(window)),
            event: MouseInput {
                device_id: DEVICE_ID,
                state: Pressed,
                button: Other(xbutton),
                modifiers: event::get_key_mods(),
            },
        });
    }

    0
}

fn handle_xbuttonup(
    window: HWND,
    _: u32,
    wparam: WPARAM,
    _: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    use crate::event::{ElementState::Released, MouseButton::Other, WindowEvent::MouseInput};
    let xbutton = get_xbutton_wparam(wparam as u32);

    unsafe {
        release_mouse(userdata.window_state_lock());
    }

    update_modifiers(window, userdata);

    unsafe {
        userdata.send_event(Event::WindowEvent {
            window_id: RootWindowId(WindowId(window)),
            event: MouseInput {
                device_id: DEVICE_ID,
                state: Released,
                button: Other(xbutton),
                modifiers: event::get_key_mods(),
            },
        });
    }

    0
}

fn handle_capturechanged(
    window: HWND,
    _: u32,
    _: WPARAM,
    lparam: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    // lparam here is a handle to the window which is gaining mouse capture.
    // If it is the same as our window, then we're essentially retaining the capture. This
    // can happen if `SetCapture` is called on our window when it already has the mouse
    // capture.
    if lparam != window {
        userdata.window_state_lock().mouse.capture_count = 0;
    }
    0
}

submit! {
    (WM_MOUSEMOVE, handle_mousemove),
    (WM_MOUSELEAVE, handle_mouseleave),
    (WM_MOUSEWHEEL, handle_mousewheel),
    (WM_MOUSEHWHEEL, handle_mousehwheel),
    (WM_LBUTTONDOWN, handle_lbuttondown),
    (WM_LBUTTONUP, handle_lbuttonup),
    (WM_RBUTTONDOWN, handle_rbuttondown),
    (WM_RBUTTONUP, handle_rbuttonup),
    (WM_MBUTTONDOWN, handle_mbuttondown),
    (WM_MBUTTONUP, handle_mbuttonup),
    (WM_XBUTTONDOWN, handle_xbuttondown),
    (WM_XBUTTONUP, handle_xbuttonup),
    (WM_CAPTURECHANGED, handle_capturechanged),
}
