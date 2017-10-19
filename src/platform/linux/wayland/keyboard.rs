use std::sync::{Arc, Mutex};

use {VirtualKeyCode, ElementState, WindowEvent as Event, KeyboardInput, ModifiersState};

use super::{EventsLoopSink, WindowId, make_wid, DeviceId};
use super::wayland_kbd::{MappedKeyboardImplementation, register_kbd};
use wayland_client::protocol::wl_keyboard;
use wayland_client::EventQueueHandle;

pub fn init_keyboard(evq: &mut EventQueueHandle, keyboard: &wl_keyboard::WlKeyboard, sink: &Arc<Mutex<EventsLoopSink>>) {
    let idata = KeyboardIData {
        sink: sink.clone(),
        target: None
    };

    if register_kbd(evq, keyboard, mapped_keyboard_impl(), idata).is_err() {
        // initializing libxkbcommon failed :(
        // fallback implementation
        let idata = KeyboardIData {
            sink: sink.clone(),
            target: None
        };
        evq.register(keyboard, raw_keyboard_impl(), idata);
    }
}

struct KeyboardIData {
    sink: Arc<Mutex<EventsLoopSink>>,
    target: Option<WindowId>
}

fn mapped_keyboard_impl() -> MappedKeyboardImplementation<KeyboardIData> {
    MappedKeyboardImplementation {
        enter: |_, idata, _, _, surface, _, _, _| {
            let wid = make_wid(surface);
            idata.sink.lock().unwrap().send_event(Event::Focused(true), wid);
            idata.target = Some(wid);
        },
        leave: |_, idata, _, _, surface| {
            let wid = make_wid(surface);
            idata.sink.lock().unwrap().send_event(Event::Focused(false), wid);
            idata.target = None;
        },
        key: |_, idata, _, _, _, mods, rawkey, keysym, state, utf8| {
            if let Some(wid) = idata.target {
                let state = match state {
                    wl_keyboard::KeyState::Pressed => ElementState::Pressed,
                    wl_keyboard::KeyState::Released => ElementState::Released,
                };
                let vkcode = key_to_vkey(rawkey, keysym);
                let mut guard = idata.sink.lock().unwrap();
                guard.send_event(
                    Event::KeyboardInput {
                        device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                        input: KeyboardInput {
                            state: state,
                            scancode: rawkey,
                            virtual_keycode: vkcode,
                            modifiers: ModifiersState {
                                shift: mods.shift,
                                ctrl: mods.ctrl,
                                alt: mods.alt,
                                logo: mods.logo
                            },
                        },
                    },
                    wid
                );
                // send char event only on key press, not release
                if let ElementState::Released = state { return }
                if let Some(txt) = utf8 {
                    for chr in txt.chars() {
                        guard.send_event(Event::ReceivedCharacter(chr), wid);
                    }
                }
            }
        },
        repeat_info: |_, _idata, _, _rate, _delay| {
            // TODO: handle repeat info
        }
    }
}


// This is fallback impl if libxkbcommon was not available
// This case should probably never happen, as most wayland
// compositors _need_ libxkbcommon anyway...
//
// In this case, we don't have the keymap information (it is
// supposed to be serialized by the compositor using libxkbcommon)
fn raw_keyboard_impl() -> wl_keyboard::Implementation<KeyboardIData> {
    wl_keyboard::Implementation {
        enter: |_, idata, _, _, surface, _| {
            let wid = make_wid(surface);
            idata.sink.lock().unwrap().send_event(Event::Focused(true), wid);
            idata.target = Some(wid);
        },
        leave: |_, idata, _, _, surface| {
            let wid = make_wid(surface);
            idata.sink.lock().unwrap().send_event(Event::Focused(false), wid);
            idata.target = None;
        },
        key: |_, idata, _, _, _, key, state| {
            if let Some(wid) = idata.target {
                let state = match state {
                    wl_keyboard::KeyState::Pressed => ElementState::Pressed,
                    wl_keyboard::KeyState::Released => ElementState::Released,
                };
                idata.sink.lock().unwrap().send_event(
                    Event::KeyboardInput {
                        device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                        input: KeyboardInput {
                            state: state,
                            scancode: key,
                            virtual_keycode: None,
                            modifiers: ModifiersState::default(),
                        },
                    },
                    wid
                );
            }
        },
        repeat_info: |_, _idata, _, _rate, _delay| {},
        keymap: |_, _, _, _, _, _| {},
        modifiers: |_, _, _, _, _, _, _, _| {}
    }
}

fn key_to_vkey(rawkey: u32, keysym: u32) -> Option<VirtualKeyCode> {
    match rawkey {
         1 => Some(VirtualKeyCode::Escape),
         2 => Some(VirtualKeyCode::Key1),
         3 => Some(VirtualKeyCode::Key2),
         4 => Some(VirtualKeyCode::Key3),
         5 => Some(VirtualKeyCode::Key4),
         6 => Some(VirtualKeyCode::Key5),
         7 => Some(VirtualKeyCode::Key6),
         8 => Some(VirtualKeyCode::Key7),
         9 => Some(VirtualKeyCode::Key8),
        10 => Some(VirtualKeyCode::Key9),
        11 => Some(VirtualKeyCode::Key0),
        _  => keysym_to_vkey(keysym)
    }
}

fn keysym_to_vkey(keysym: u32) -> Option<VirtualKeyCode> {
    use super::wayland_kbd::keysyms;
    match keysym {
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
        // F--
        keysyms::XKB_KEY_F1  => Some(VirtualKeyCode::F1),
        keysyms::XKB_KEY_F2  => Some(VirtualKeyCode::F2),
        keysyms::XKB_KEY_F3  => Some(VirtualKeyCode::F3),
        keysyms::XKB_KEY_F4  => Some(VirtualKeyCode::F4),
        keysyms::XKB_KEY_F5  => Some(VirtualKeyCode::F5),
        keysyms::XKB_KEY_F6  => Some(VirtualKeyCode::F6),
        keysyms::XKB_KEY_F7  => Some(VirtualKeyCode::F7),
        keysyms::XKB_KEY_F8  => Some(VirtualKeyCode::F8),
        keysyms::XKB_KEY_F9  => Some(VirtualKeyCode::F9),
        keysyms::XKB_KEY_F10 => Some(VirtualKeyCode::F10),
        keysyms::XKB_KEY_F11 => Some(VirtualKeyCode::F11),
        keysyms::XKB_KEY_F12 => Some(VirtualKeyCode::F12),
        keysyms::XKB_KEY_F13 => Some(VirtualKeyCode::F13),
        keysyms::XKB_KEY_F14 => Some(VirtualKeyCode::F14),
        keysyms::XKB_KEY_F15 => Some(VirtualKeyCode::F15),
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
        //
        keysyms::XKB_KEY_BackSpace => Some(VirtualKeyCode::Back),
        keysyms::XKB_KEY_Return => Some(VirtualKeyCode::Return),
        keysyms::XKB_KEY_space => Some(VirtualKeyCode::Space),
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
        // => Some(VirtualKeyCode::At),
        // => Some(VirtualKeyCode::Ax),
        keysyms::XKB_KEY_backslash => Some(VirtualKeyCode::Backslash),
        // => Some(VirtualKeyCode::Calculator),
        // => Some(VirtualKeyCode::Capital),
        keysyms::XKB_KEY_colon => Some(VirtualKeyCode::Colon),
        keysyms::XKB_KEY_comma => Some(VirtualKeyCode::Comma),
        // => Some(VirtualKeyCode::Convert),
        // => Some(VirtualKeyCode::Decimal),
        // => Some(VirtualKeyCode::Divide),
        keysyms::XKB_KEY_equal => Some(VirtualKeyCode::Equals),
        // => Some(VirtualKeyCode::Grave),
        // => Some(VirtualKeyCode::Kana),
        // => Some(VirtualKeyCode::Kanji),
        keysyms::XKB_KEY_Alt_L => Some(VirtualKeyCode::LAlt),
        // => Some(VirtualKeyCode::LBracket),
        keysyms::XKB_KEY_Control_L => Some(VirtualKeyCode::LControl),
        // => Some(VirtualKeyCode::LMenu),
        keysyms::XKB_KEY_Shift_L => Some(VirtualKeyCode::LShift),
        // => Some(VirtualKeyCode::LWin),
        // => Some(VirtualKeyCode::Mail),
        // => Some(VirtualKeyCode::MediaSelect),
        // => Some(VirtualKeyCode::MediaStop),
        keysyms::XKB_KEY_minus => Some(VirtualKeyCode::Minus),
        keysyms::XKB_KEY_asterisk => Some(VirtualKeyCode::Multiply),
        // => Some(VirtualKeyCode::Mute),
        // => Some(VirtualKeyCode::MyComputer),
        // => Some(VirtualKeyCode::NextTrack),
        // => Some(VirtualKeyCode::NoConvert),
        keysyms::XKB_KEY_KP_Separator => Some(VirtualKeyCode::NumpadComma),
        keysyms::XKB_KEY_KP_Enter => Some(VirtualKeyCode::NumpadEnter),
        keysyms::XKB_KEY_KP_Equal => Some(VirtualKeyCode::NumpadEquals),
        // => Some(VirtualKeyCode::OEM102),
        // => Some(VirtualKeyCode::Period),
        // => Some(VirtualKeyCode::Playpause),
        // => Some(VirtualKeyCode::Power),
        // => Some(VirtualKeyCode::Prevtrack),
        keysyms::XKB_KEY_Alt_R => Some(VirtualKeyCode::RAlt),
        // => Some(VirtualKeyCode::RBracket),
        keysyms::XKB_KEY_Control_R => Some(VirtualKeyCode::RControl),
        // => Some(VirtualKeyCode::RMenu),
        keysyms::XKB_KEY_Shift_R => Some(VirtualKeyCode::RShift),
        // => Some(VirtualKeyCode::RWin),
        keysyms::XKB_KEY_semicolon => Some(VirtualKeyCode::Semicolon),
        keysyms::XKB_KEY_slash => Some(VirtualKeyCode::Slash),
        // => Some(VirtualKeyCode::Sleep),
        // => Some(VirtualKeyCode::Stop),
        // => Some(VirtualKeyCode::Subtract),
        // => Some(VirtualKeyCode::Sysrq),
        keysyms::XKB_KEY_Tab => Some(VirtualKeyCode::Tab),
        keysyms::XKB_KEY_ISO_Left_Tab => Some(VirtualKeyCode::Tab),
        // => Some(VirtualKeyCode::Underline),
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
        // => Some(VirtualKeyCode::Yen),
        // fallback
        _ => None
    }
}
