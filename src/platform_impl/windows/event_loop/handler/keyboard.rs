//! Handles keyboard events.

use super::prelude::*;

fn handle_nclbuttondown(
    window: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: &dyn GenericWindowData,
) -> LRESULT {
    unsafe {
        if wparam == HTCAPTION as _ {
            PostMessageW(window, WM_MOUSEMOVE, 0, lparam);
        }

        DefWindowProcW(window, msg, wparam, lparam)
    }
}

fn handle_char_or_syschar(
    window: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    use crate::event::WindowEvent::ReceivedCharacter;
    use std::char;
    let is_high_surrogate = (0xD800..=0xDBFF).contains(&wparam);
    let is_low_surrogate = (0xDC00..=0xDFFF).contains(&wparam);

    if is_high_surrogate {
        userdata.window_state_lock().high_surrogate = Some(wparam as u16);
    } else if is_low_surrogate {
        let high_surrogate = userdata.window_state_lock().high_surrogate.take();

        if let Some(high_surrogate) = high_surrogate {
            let pair = [high_surrogate, wparam as u16];
            if let Some(Ok(chr)) = char::decode_utf16(pair.iter().copied()).next() {
                unsafe {
                    userdata.send_event(Event::WindowEvent {
                        window_id: RootWindowId(WindowId(window)),
                        event: ReceivedCharacter(chr),
                    });
                }
            }
        }
    } else {
        userdata.window_state_lock().high_surrogate = None;

        if let Some(chr) = char::from_u32(wparam as u32) {
            unsafe {
                userdata.send_event(Event::WindowEvent {
                    window_id: RootWindowId(WindowId(window)),
                    event: ReceivedCharacter(chr),
                });
            }
        }
    }

    // todo(msiglreith):
    //   Ideally, `WM_SYSCHAR` shouldn't emit a `ReceivedChar` event
    //   indicating user text input. As we lack dedicated support
    //   accelerators/keybindings these events will be additionally
    //   emitted for downstream users.
    //   This means certain key combinations (ie Alt + Space) will
    //   trigger the default system behavior **and** emit a char event.
    if msg == WM_SYSCHAR {
        unsafe { DefWindowProcW(window, msg, wparam, lparam) }
    } else {
        0
    }
}

fn handle_menuchar(_: HWND, _: u32, _: WPARAM, _: LPARAM, _: &dyn GenericWindowData) -> LRESULT {
    (MNC_CLOSE << 16) as LRESULT
}

fn handle_ime_startcomposition(
    window: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    unsafe {
        let ime_allowed = userdata.window_state_lock().ime_allowed;
        if ime_allowed {
            userdata.window_state_lock().ime_state = ImeState::Enabled;

            userdata.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: WindowEvent::Ime(Ime::Enabled),
            });
        }

        DefWindowProcW(window, msg, wparam, lparam)
    }
}

fn handle_ime_composition(
    window: HWND,
    _: u32,
    _: WPARAM,
    lparam: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    let ime_allowed_and_composing = {
        let w = userdata.window_state_lock();
        w.ime_allowed && w.ime_state != ImeState::Disabled
    };
    // Windows Hangul IME sends WM_IME_COMPOSITION after WM_IME_ENDCOMPOSITION, so
    // check whether composing.
    if ime_allowed_and_composing {
        let ime_context = unsafe { ImeContext::current(window) };

        if lparam == 0 {
            unsafe {
                userdata.send_event(Event::WindowEvent {
                    window_id: RootWindowId(WindowId(window)),
                    event: WindowEvent::Ime(Ime::Preedit(String::new(), None)),
                });
            }
        }

        // Google Japanese Input and ATOK have both flags, so
        // first, receive composing result if exist.
        if (lparam as u32 & GCS_RESULTSTR) != 0 {
            if let Some(text) = unsafe { ime_context.get_composed_text() } {
                userdata.window_state_lock().ime_state = ImeState::Enabled;

                unsafe {
                    userdata.send_event(Event::WindowEvent {
                        window_id: RootWindowId(WindowId(window)),
                        event: WindowEvent::Ime(Ime::Preedit(String::new(), None)),
                    });
                    userdata.send_event(Event::WindowEvent {
                        window_id: RootWindowId(WindowId(window)),
                        event: WindowEvent::Ime(Ime::Commit(text)),
                    });
                }
            }
        }

        // Next, receive preedit range for next composing if exist.
        if (lparam as u32 & GCS_COMPSTR) != 0 {
            if let Some((text, first, last)) =
                unsafe { ime_context.get_composing_text_and_cursor() }
            {
                userdata.window_state_lock().ime_state = ImeState::Preedit;
                let cursor_range = first.map(|f| (f, last.unwrap_or(f)));

                unsafe {
                    userdata.send_event(Event::WindowEvent {
                        window_id: RootWindowId(WindowId(window)),
                        event: WindowEvent::Ime(Ime::Preedit(text, cursor_range)),
                    });
                }
            }
        }
    }

    // Not calling DefWindowProc to hide composing text drawn by IME.
    0
}

fn handle_ime_endcomposition(
    window: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    let ime_allowed_or_composing = {
        let w = userdata.window_state_lock();
        w.ime_allowed || w.ime_state != ImeState::Disabled
    };
    if ime_allowed_or_composing {
        if userdata.window_state_lock().ime_state == ImeState::Preedit {
            // Windows Hangul IME sends WM_IME_COMPOSITION after WM_IME_ENDCOMPOSITION, so
            // trying receiving composing result and commit if exists.
            unsafe {
                let ime_context = ImeContext::current(window);
                if let Some(text) = ime_context.get_composed_text() {
                    userdata.send_event(Event::WindowEvent {
                        window_id: RootWindowId(WindowId(window)),
                        event: WindowEvent::Ime(Ime::Preedit(String::new(), None)),
                    });
                    userdata.send_event(Event::WindowEvent {
                        window_id: RootWindowId(WindowId(window)),
                        event: WindowEvent::Ime(Ime::Commit(text)),
                    });
                }
            }
        }

        userdata.window_state_lock().ime_state = ImeState::Disabled;

        unsafe {
            userdata.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: WindowEvent::Ime(Ime::Disabled),
            });
        }
    }

    unsafe { DefWindowProcW(window, msg, wparam, lparam) }
}

fn handle_ime_setcontext(
    window: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: &dyn GenericWindowData,
) -> LRESULT {
    let wparam = wparam & (!ISC_SHOWUICOMPOSITIONWINDOW as WPARAM);

    unsafe { DefWindowProcW(window, msg, wparam, lparam) }
}

fn handle_syscommand(
    window: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    if wparam == SC_RESTORE as usize {
        let mut w = userdata.window_state_lock();
        w.set_window_flags_in_place(|f| f.set(WindowFlags::MINIMIZED, false));
    }
    if wparam == SC_MINIMIZE as usize {
        let mut w = userdata.window_state_lock();
        w.set_window_flags_in_place(|f| f.set(WindowFlags::MINIMIZED, true));
    }
    // Send `WindowEvent::Minimized` here if we decide to implement one

    if wparam == SC_SCREENSAVE as usize {
        let window_state = userdata.window_state_lock();
        if window_state.fullscreen.is_some() {
            return 0;
        }
    }

    unsafe { DefWindowProcW(window, msg, wparam, lparam) }
}

fn handle_keydown_or_syskeydown(
    window: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    use crate::event::{ElementState::Pressed, VirtualKeyCode};
    if let Some((scancode, vkey)) = process_key_params(wparam, lparam) {
        update_modifiers(window, userdata);

        unsafe {
            #[allow(deprecated)]
            userdata.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: WindowEvent::KeyboardInput {
                    device_id: DEVICE_ID,
                    input: KeyboardInput {
                        state: Pressed,
                        scancode,
                        virtual_keycode: vkey,
                        modifiers: event::get_key_mods(),
                    },
                    is_synthetic: false,
                },
            });
        }
        // Windows doesn't emit a delete character by default, but in order to make it
        // consistent with the other platforms we'll emit a delete character here.
        if vkey == Some(VirtualKeyCode::Delete) {
            unsafe {
                userdata.send_event(Event::WindowEvent {
                    window_id: RootWindowId(WindowId(window)),
                    event: WindowEvent::ReceivedCharacter('\u{7F}'),
                });
            }
        }
    }

    if msg == WM_SYSKEYDOWN {
        unsafe { DefWindowProcW(window, msg, wparam, lparam) }
    } else {
        0
    }
}

fn handle_keyup_or_syskeyup(
    window: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    use crate::event::ElementState::Released;
    if let Some((scancode, vkey)) = process_key_params(wparam, lparam) {
        update_modifiers(window, userdata);

        unsafe {
            #[allow(deprecated)]
            userdata.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: WindowEvent::KeyboardInput {
                    device_id: DEVICE_ID,
                    input: KeyboardInput {
                        state: Released,
                        scancode,
                        virtual_keycode: vkey,
                        modifiers: event::get_key_mods(),
                    },
                    is_synthetic: false,
                },
            });
        }
    }

    unsafe {
        if msg == WM_SYSKEYUP && GetMenu(window) != 0 {
            // let Windows handle event if the window has a native menu, a modal event loop
            // is started here on Alt key up.
            DefWindowProcW(window, msg, wparam, lparam)
        } else {
            0
        }
    }
}

submit! {
    (WM_NCLBUTTONDOWN, handle_nclbuttondown),
    (WM_CHAR, handle_char_or_syschar),
    (WM_SYSCHAR, handle_char_or_syschar),
    (WM_MENUCHAR, handle_menuchar),
    (WM_IME_STARTCOMPOSITION, handle_ime_startcomposition),
    (WM_IME_COMPOSITION, handle_ime_composition),
    (WM_IME_ENDCOMPOSITION, handle_ime_endcomposition),
    (WM_IME_SETCONTEXT, handle_ime_setcontext),
    (WM_SYSCOMMAND, handle_syscommand),
    (WM_KEYDOWN, handle_keydown_or_syskeydown),
    (WM_SYSKEYDOWN, handle_keydown_or_syskeydown),
    (WM_KEYUP, handle_keyup_or_syskeyup),
    (WM_SYSKEYUP, handle_keyup_or_syskeyup),
}
