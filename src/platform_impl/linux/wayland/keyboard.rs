use std::sync::{Arc, Mutex};

use super::{event_loop::EventsSink, make_wid, DeviceId};
use smithay_client_toolkit::{
    keyboard::{
        self, map_keyboard_auto_with_repeat, Event as KbEvent, KeyRepeatEvent, KeyRepeatKind,
    },
    reexports::client::protocol::{wl_keyboard, wl_seat},
};

use crate::event::{ElementState, KeyboardInput, ModifiersState, VirtualKeyCode, WindowEvent};

pub fn init_keyboard(
    seat: &wl_seat::WlSeat,
    sink: EventsSink,
    modifiers_tracker: Arc<Mutex<ModifiersState>>,
) -> wl_keyboard::WlKeyboard {
    // { variables to be captured by the closures
    let target = Arc::new(Mutex::new(None));
    let my_sink = sink.clone();
    let repeat_sink = sink.clone();
    let repeat_target = target.clone();
    let my_modifiers = modifiers_tracker.clone();
    // }
    let ret = map_keyboard_auto_with_repeat(
        seat,
        KeyRepeatKind::System,
        move |evt: KbEvent<'_>, _| {
            match evt {
                KbEvent::Enter { surface, .. } => {
                    let wid = make_wid(&surface);
                    my_sink.send_window_event(WindowEvent::Focused(true), wid);
                    *target.lock().unwrap() = Some(wid);

                    let modifiers = *modifiers_tracker.lock().unwrap();

                    if !modifiers.is_empty() {
                        my_sink.send_window_event(WindowEvent::ModifiersChanged(modifiers), wid);
                    }
                }
                KbEvent::Leave { surface, .. } => {
                    let wid = make_wid(&surface);
                    let modifiers = *modifiers_tracker.lock().unwrap();

                    if !modifiers.is_empty() {
                        my_sink.send_window_event(
                            WindowEvent::ModifiersChanged(ModifiersState::empty()),
                            wid,
                        );
                    }

                    my_sink.send_window_event(WindowEvent::Focused(false), wid);
                    *target.lock().unwrap() = None;
                }
                KbEvent::Key {
                    rawkey,
                    keysym,
                    state,
                    utf8,
                    ..
                } => {
                    if let Some(wid) = *target.lock().unwrap() {
                        let state = match state {
                            wl_keyboard::KeyState::Pressed => ElementState::Pressed,
                            wl_keyboard::KeyState::Released => ElementState::Released,
                            _ => unreachable!(),
                        };
                        let vkcode = keysym_to_vkey(keysym);
                        my_sink.send_window_event(
                            #[allow(deprecated)]
                            WindowEvent::KeyboardInput {
                                device_id: crate::event::DeviceId(
                                    crate::platform_impl::DeviceId::Wayland(DeviceId),
                                ),
                                input: KeyboardInput {
                                    state,
                                    scancode: rawkey,
                                    virtual_keycode: vkcode,
                                    modifiers: modifiers_tracker.lock().unwrap().clone(),
                                },
                                is_synthetic: false,
                            },
                            wid,
                        );
                        // send char event only on key press, not release
                        if let ElementState::Released = state {
                            return;
                        }
                        if let Some(txt) = utf8 {
                            for chr in txt.chars() {
                                my_sink.send_window_event(WindowEvent::ReceivedCharacter(chr), wid);
                            }
                        }
                    }
                }
                KbEvent::RepeatInfo { .. } => { /* Handled by smithay client toolkit */ }
                KbEvent::Modifiers {
                    modifiers: event_modifiers,
                } => {
                    let modifiers = ModifiersState::from_wayland(event_modifiers);

                    *modifiers_tracker.lock().unwrap() = modifiers;

                    if let Some(wid) = *target.lock().unwrap() {
                        my_sink.send_window_event(WindowEvent::ModifiersChanged(modifiers), wid);
                    }
                }
            }
        },
        move |repeat_event: KeyRepeatEvent, _| {
            if let Some(wid) = *repeat_target.lock().unwrap() {
                let state = ElementState::Pressed;
                let vkcode = keysym_to_vkey(repeat_event.keysym);
                repeat_sink.send_window_event(
                    #[allow(deprecated)]
                    WindowEvent::KeyboardInput {
                        device_id: crate::event::DeviceId(crate::platform_impl::DeviceId::Wayland(
                            DeviceId,
                        )),
                        input: KeyboardInput {
                            state,
                            scancode: repeat_event.rawkey,
                            virtual_keycode: vkcode,
                            modifiers: my_modifiers.lock().unwrap().clone(),
                        },
                        is_synthetic: false,
                    },
                    wid,
                );
                if let Some(txt) = repeat_event.utf8 {
                    for chr in txt.chars() {
                        repeat_sink.send_window_event(WindowEvent::ReceivedCharacter(chr), wid);
                    }
                }
            }
        },
    );

    match ret {
        Ok(keyboard) => keyboard,
        Err(_) => {
            // This is a fallback impl if libxkbcommon was not available
            // This case should probably never happen, as most wayland
            // compositors _need_ libxkbcommon anyway...
            //
            // In this case, we don't have the keymap information (it is
            // supposed to be serialized by the compositor using libxkbcommon)

            seat.get_keyboard(|keyboard| {
                // { variables to be captured by the closure
                let mut target = None;
                let my_sink = sink;
                // }

                keyboard.implement_closure(
                    move |evt, _| match evt {
                        wl_keyboard::Event::Enter { surface, .. } => {
                            let wid = make_wid(&surface);
                            my_sink.send_window_event(WindowEvent::Focused(true), wid);
                            target = Some(wid);
                        }
                        wl_keyboard::Event::Leave { surface, .. } => {
                            let wid = make_wid(&surface);
                            my_sink.send_window_event(WindowEvent::Focused(false), wid);
                            target = None;
                        }
                        wl_keyboard::Event::Key { key, state, .. } => {
                            if let Some(wid) = target {
                                let state = match state {
                                    wl_keyboard::KeyState::Pressed => ElementState::Pressed,
                                    wl_keyboard::KeyState::Released => ElementState::Released,
                                    _ => unreachable!(),
                                };
                                my_sink.send_window_event(
                                    #[allow(deprecated)]
                                    WindowEvent::KeyboardInput {
                                        device_id: crate::event::DeviceId(
                                            crate::platform_impl::DeviceId::Wayland(DeviceId),
                                        ),
                                        input: KeyboardInput {
                                            state,
                                            scancode: key,
                                            virtual_keycode: None,
                                            modifiers: ModifiersState::default(),
                                        },
                                        is_synthetic: false,
                                    },
                                    wid,
                                );
                            }
                        }
                        _ => (),
                    },
                    (),
                )
            })
            .unwrap()
        }
    }
}

fn keysym_to_vkey(keysym: u32) -> Option<VirtualKeyCode> {
    use smithay_client_toolkit::keyboard::keysyms;
    match keysym {
        // numbers
        keysyms::XKB_KEY_1 => Some(VirtualKeyCode::Key1),
        keysyms::XKB_KEY_2 => Some(VirtualKeyCode::Key2),
        keysyms::XKB_KEY_3 => Some(VirtualKeyCode::Key3),
        keysyms::XKB_KEY_4 => Some(VirtualKeyCode::Key4),
        keysyms::XKB_KEY_5 => Some(VirtualKeyCode::Key4),
        keysyms::XKB_KEY_6 => Some(VirtualKeyCode::Key5),
        keysyms::XKB_KEY_7 => Some(VirtualKeyCode::Key6),
        keysyms::XKB_KEY_8 => Some(VirtualKeyCode::Key7),
        keysyms::XKB_KEY_9 => Some(VirtualKeyCode::Key8),
        keysyms::XKB_KEY_0 => Some(VirtualKeyCode::Key9),
        // letters
        keysyms::XKB_KEY_A | keysyms::XKB_KEY_a => Some(VirtualKeyCode::A),
        keysyms::XKB_KEY_B | keysyms::XKB_KEY_b => Some(VirtualKeyCode::B),
        keysyms::XKB_KEY_C | keysyms::XKB_KEY_c => Some(VirtualKeyCode::C),
        keysyms::XKB_KEY_D | keysyms::XKB_KEY_d => Some(VirtualKeyCode::D),
        keysyms::XKB_KEY_E | keysyms::XKB_KEY_e => Some(VirtualKeyCode::E),
        keysyms::XKB_KEY_F | keysyms::XKB_KEY_f => Some(VirtualKeyCode::F),
        keysyms::XKB_KEY_G | keysyms::XKB_KEY_g => Some(VirtualKeyCode::G),
        keysyms::XKB_KEY_H | keysyms::XKB_KEY_h => Some(VirtualKeyCode::H),
        keysyms::XKB_KEY_I | keysyms::XKB_KEY_i => Some(VirtualKeyCode::I),
        keysyms::XKB_KEY_J | keysyms::XKB_KEY_j => Some(VirtualKeyCode::J),
        keysyms::XKB_KEY_K | keysyms::XKB_KEY_k => Some(VirtualKeyCode::K),
        keysyms::XKB_KEY_L | keysyms::XKB_KEY_l => Some(VirtualKeyCode::L),
        keysyms::XKB_KEY_M | keysyms::XKB_KEY_m => Some(VirtualKeyCode::M),
        keysyms::XKB_KEY_N | keysyms::XKB_KEY_n => Some(VirtualKeyCode::N),
        keysyms::XKB_KEY_O | keysyms::XKB_KEY_o => Some(VirtualKeyCode::O),
        keysyms::XKB_KEY_P | keysyms::XKB_KEY_p => Some(VirtualKeyCode::P),
        keysyms::XKB_KEY_Q | keysyms::XKB_KEY_q => Some(VirtualKeyCode::Q),
        keysyms::XKB_KEY_R | keysyms::XKB_KEY_r => Some(VirtualKeyCode::R),
        keysyms::XKB_KEY_S | keysyms::XKB_KEY_s => Some(VirtualKeyCode::S),
        keysyms::XKB_KEY_T | keysyms::XKB_KEY_t => Some(VirtualKeyCode::T),
        keysyms::XKB_KEY_U | keysyms::XKB_KEY_u => Some(VirtualKeyCode::U),
        keysyms::XKB_KEY_V | keysyms::XKB_KEY_v => Some(VirtualKeyCode::V),
        keysyms::XKB_KEY_W | keysyms::XKB_KEY_w => Some(VirtualKeyCode::W),
        keysyms::XKB_KEY_X | keysyms::XKB_KEY_x => Some(VirtualKeyCode::X),
        keysyms::XKB_KEY_Y | keysyms::XKB_KEY_y => Some(VirtualKeyCode::Y),
        keysyms::XKB_KEY_Z | keysyms::XKB_KEY_z => Some(VirtualKeyCode::Z),
        // Escape
        keysyms::XKB_KEY_Escape => Some(VirtualKeyCode::Escape),
        // F--
        keysyms::XKB_KEY_F1 => Some(VirtualKeyCode::F1),
        keysyms::XKB_KEY_F2 => Some(VirtualKeyCode::F2),
        keysyms::XKB_KEY_F3 => Some(VirtualKeyCode::F3),
        keysyms::XKB_KEY_F4 => Some(VirtualKeyCode::F4),
        keysyms::XKB_KEY_F5 => Some(VirtualKeyCode::F5),
        keysyms::XKB_KEY_F6 => Some(VirtualKeyCode::F6),
        keysyms::XKB_KEY_F7 => Some(VirtualKeyCode::F7),
        keysyms::XKB_KEY_F8 => Some(VirtualKeyCode::F8),
        keysyms::XKB_KEY_F9 => Some(VirtualKeyCode::F9),
        keysyms::XKB_KEY_F10 => Some(VirtualKeyCode::F10),
        keysyms::XKB_KEY_F11 => Some(VirtualKeyCode::F11),
        keysyms::XKB_KEY_F12 => Some(VirtualKeyCode::F12),
        keysyms::XKB_KEY_F13 => Some(VirtualKeyCode::F13),
        keysyms::XKB_KEY_F14 => Some(VirtualKeyCode::F14),
        keysyms::XKB_KEY_F15 => Some(VirtualKeyCode::F15),
        keysyms::XKB_KEY_F16 => Some(VirtualKeyCode::F16),
        keysyms::XKB_KEY_F17 => Some(VirtualKeyCode::F17),
        keysyms::XKB_KEY_F18 => Some(VirtualKeyCode::F18),
        keysyms::XKB_KEY_F19 => Some(VirtualKeyCode::F19),
        keysyms::XKB_KEY_F20 => Some(VirtualKeyCode::F20),
        keysyms::XKB_KEY_F21 => Some(VirtualKeyCode::F21),
        keysyms::XKB_KEY_F22 => Some(VirtualKeyCode::F22),
        keysyms::XKB_KEY_F23 => Some(VirtualKeyCode::F23),
        keysyms::XKB_KEY_F24 => Some(VirtualKeyCode::F24),
        // flow control
        keysyms::XKB_KEY_Print => Some(VirtualKeyCode::Snapshot),
        keysyms::XKB_KEY_Scroll_Lock => Some(VirtualKeyCode::Scroll),
        keysyms::XKB_KEY_Pause => Some(VirtualKeyCode::Pause),
        keysyms::XKB_KEY_Insert => Some(VirtualKeyCode::Insert),
        keysyms::XKB_KEY_Home => Some(VirtualKeyCode::Home),
        keysyms::XKB_KEY_Delete => Some(VirtualKeyCode::Delete),
        keysyms::XKB_KEY_End => Some(VirtualKeyCode::End),
        keysyms::XKB_KEY_Page_Down => Some(VirtualKeyCode::PageDown),
        keysyms::XKB_KEY_Page_Up => Some(VirtualKeyCode::PageUp),
        // arrows
        keysyms::XKB_KEY_Left => Some(VirtualKeyCode::Left),
        keysyms::XKB_KEY_Up => Some(VirtualKeyCode::Up),
        keysyms::XKB_KEY_Right => Some(VirtualKeyCode::Right),
        keysyms::XKB_KEY_Down => Some(VirtualKeyCode::Down),

        keysyms::XKB_KEY_BackSpace => Some(VirtualKeyCode::Back),
        keysyms::XKB_KEY_Return => Some(VirtualKeyCode::Return),
        keysyms::XKB_KEY_space => Some(VirtualKeyCode::Space),

        keysyms::XKB_KEY_Multi_key => Some(VirtualKeyCode::Compose),
        keysyms::XKB_KEY_caret => Some(VirtualKeyCode::Caret),

        // keypad
        keysyms::XKB_KEY_Num_Lock => Some(VirtualKeyCode::Numlock),
        keysyms::XKB_KEY_KP_0 => Some(VirtualKeyCode::Numpad0),
        keysyms::XKB_KEY_KP_1 => Some(VirtualKeyCode::Numpad1),
        keysyms::XKB_KEY_KP_2 => Some(VirtualKeyCode::Numpad2),
        keysyms::XKB_KEY_KP_3 => Some(VirtualKeyCode::Numpad3),
        keysyms::XKB_KEY_KP_4 => Some(VirtualKeyCode::Numpad4),
        keysyms::XKB_KEY_KP_5 => Some(VirtualKeyCode::Numpad5),
        keysyms::XKB_KEY_KP_6 => Some(VirtualKeyCode::Numpad6),
        keysyms::XKB_KEY_KP_7 => Some(VirtualKeyCode::Numpad7),
        keysyms::XKB_KEY_KP_8 => Some(VirtualKeyCode::Numpad8),
        keysyms::XKB_KEY_KP_9 => Some(VirtualKeyCode::Numpad9),
        // misc
        // => Some(VirtualKeyCode::AbntC1),
        // => Some(VirtualKeyCode::AbntC2),
        keysyms::XKB_KEY_plus => Some(VirtualKeyCode::Add),
        keysyms::XKB_KEY_apostrophe => Some(VirtualKeyCode::Apostrophe),
        // => Some(VirtualKeyCode::Apps),
        keysyms::XKB_KEY_at => Some(VirtualKeyCode::At),
        // => Some(VirtualKeyCode::Ax),
        keysyms::XKB_KEY_backslash => Some(VirtualKeyCode::Backslash),
        keysyms::XKB_KEY_XF86Calculator => Some(VirtualKeyCode::Calculator),
        // => Some(VirtualKeyCode::Capital),
        keysyms::XKB_KEY_colon => Some(VirtualKeyCode::Colon),
        keysyms::XKB_KEY_comma => Some(VirtualKeyCode::Comma),
        // => Some(VirtualKeyCode::Convert),
        keysyms::XKB_KEY_KP_Decimal => Some(VirtualKeyCode::Decimal),
        keysyms::XKB_KEY_equal => Some(VirtualKeyCode::Equals),
        keysyms::XKB_KEY_grave => Some(VirtualKeyCode::Grave),
        // => Some(VirtualKeyCode::Kana),
        keysyms::XKB_KEY_Kanji => Some(VirtualKeyCode::Kanji),
        keysyms::XKB_KEY_Alt_L => Some(VirtualKeyCode::LAlt),
        keysyms::XKB_KEY_bracketleft => Some(VirtualKeyCode::LBracket),
        keysyms::XKB_KEY_Control_L => Some(VirtualKeyCode::LControl),
        keysyms::XKB_KEY_Shift_L => Some(VirtualKeyCode::LShift),
        keysyms::XKB_KEY_Super_L => Some(VirtualKeyCode::LWin),
        keysyms::XKB_KEY_XF86Mail => Some(VirtualKeyCode::Mail),
        // => Some(VirtualKeyCode::MediaSelect),
        // => Some(VirtualKeyCode::MediaStop),
        keysyms::XKB_KEY_minus => Some(VirtualKeyCode::Minus),
        keysyms::XKB_KEY_asterisk => Some(VirtualKeyCode::Multiply),
        keysyms::XKB_KEY_XF86AudioMute => Some(VirtualKeyCode::Mute),
        // => Some(VirtualKeyCode::MyComputer),
        keysyms::XKB_KEY_XF86AudioNext => Some(VirtualKeyCode::NextTrack),
        // => Some(VirtualKeyCode::NoConvert),
        keysyms::XKB_KEY_KP_Separator => Some(VirtualKeyCode::NumpadComma),
        keysyms::XKB_KEY_KP_Enter => Some(VirtualKeyCode::NumpadEnter),
        keysyms::XKB_KEY_KP_Equal => Some(VirtualKeyCode::NumpadEquals),
        keysyms::XKB_KEY_KP_Add => Some(VirtualKeyCode::Add),
        keysyms::XKB_KEY_KP_Subtract => Some(VirtualKeyCode::Subtract),
        keysyms::XKB_KEY_KP_Divide => Some(VirtualKeyCode::Divide),
        keysyms::XKB_KEY_KP_Page_Up => Some(VirtualKeyCode::PageUp),
        keysyms::XKB_KEY_KP_Page_Down => Some(VirtualKeyCode::PageDown),
        keysyms::XKB_KEY_KP_Home => Some(VirtualKeyCode::Home),
        keysyms::XKB_KEY_KP_End => Some(VirtualKeyCode::End),
        // => Some(VirtualKeyCode::OEM102),
        keysyms::XKB_KEY_period => Some(VirtualKeyCode::Period),
        // => Some(VirtualKeyCode::Playpause),
        keysyms::XKB_KEY_XF86PowerOff => Some(VirtualKeyCode::Power),
        keysyms::XKB_KEY_XF86AudioPrev => Some(VirtualKeyCode::PrevTrack),
        keysyms::XKB_KEY_Alt_R => Some(VirtualKeyCode::RAlt),
        keysyms::XKB_KEY_bracketright => Some(VirtualKeyCode::RBracket),
        keysyms::XKB_KEY_Control_R => Some(VirtualKeyCode::RControl),
        keysyms::XKB_KEY_Shift_R => Some(VirtualKeyCode::RShift),
        keysyms::XKB_KEY_Super_R => Some(VirtualKeyCode::RWin),
        keysyms::XKB_KEY_semicolon => Some(VirtualKeyCode::Semicolon),
        keysyms::XKB_KEY_slash => Some(VirtualKeyCode::Slash),
        keysyms::XKB_KEY_XF86Sleep => Some(VirtualKeyCode::Sleep),
        // => Some(VirtualKeyCode::Stop),
        // => Some(VirtualKeyCode::Sysrq),
        keysyms::XKB_KEY_Tab => Some(VirtualKeyCode::Tab),
        keysyms::XKB_KEY_ISO_Left_Tab => Some(VirtualKeyCode::Tab),
        keysyms::XKB_KEY_underscore => Some(VirtualKeyCode::Underline),
        // => Some(VirtualKeyCode::Unlabeled),
        keysyms::XKB_KEY_XF86AudioLowerVolume => Some(VirtualKeyCode::VolumeDown),
        keysyms::XKB_KEY_XF86AudioRaiseVolume => Some(VirtualKeyCode::VolumeUp),
        // => Some(VirtualKeyCode::Wake),
        // => Some(VirtualKeyCode::Webback),
        // => Some(VirtualKeyCode::WebFavorites),
        // => Some(VirtualKeyCode::WebForward),
        // => Some(VirtualKeyCode::WebHome),
        // => Some(VirtualKeyCode::WebRefresh),
        // => Some(VirtualKeyCode::WebSearch),
        // => Some(VirtualKeyCode::WebStop),
        keysyms::XKB_KEY_yen => Some(VirtualKeyCode::Yen),
        keysyms::XKB_KEY_XF86Copy => Some(VirtualKeyCode::Copy),
        keysyms::XKB_KEY_XF86Paste => Some(VirtualKeyCode::Paste),
        keysyms::XKB_KEY_XF86Cut => Some(VirtualKeyCode::Cut),
        // fallback
        _ => None,
    }
}

impl ModifiersState {
    pub(crate) fn from_wayland(mods: keyboard::ModifiersState) -> ModifiersState {
        let mut m = ModifiersState::empty();
        m.set(ModifiersState::SHIFT, mods.shift);
        m.set(ModifiersState::CTRL, mods.ctrl);
        m.set(ModifiersState::ALT, mods.alt);
        m.set(ModifiersState::LOGO, mods.logo);
        m
    }
}
