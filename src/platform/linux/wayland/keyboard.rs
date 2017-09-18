use std::sync::{Arc, Mutex};

use WindowEvent as Event;

use keyboard_types::{Code, Key, KeyEvent, KeyState, Modifiers as KeyModifiers, Location as KeyLocation};

use super::{wayland_kbd, EventsLoopSink, WindowId, DeviceId};
use wayland_client::EventQueueHandle;
use wayland_client::protocol::wl_keyboard;

pub struct KbdHandler {
    sink: Arc<Mutex<EventsLoopSink>>,
    pub target: Option<WindowId>
}

impl KbdHandler {
    pub fn new(sink: Arc<Mutex<EventsLoopSink>>) -> KbdHandler {
        KbdHandler { sink: sink, target: None }
    }
}

impl wayland_kbd::Handler for KbdHandler {
    fn key(&mut self,
           _evqh: &mut EventQueueHandle,
           _proxy: &wl_keyboard::WlKeyboard,
           _serial: u32,
           _time: u32,
           _mods: &wayland_kbd::ModifiersState,
           _rawkey: u32,
           _keysym: u32,
           state: wl_keyboard::KeyState,
           utf8: Option<String>)
    {
        if let Some(wid) = self.target {
            let state = match state {
                wl_keyboard::KeyState::Pressed => KeyState::Down,
                wl_keyboard::KeyState::Released => KeyState::Up,
            };
            let mut guard = self.sink.lock().unwrap();
            guard.send_event(
                Event::KeyboardInput {
                    device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                    input: KeyEvent {
                        state: state,
                        key: Key::Unidentified,
                        code: Code::Unidentified,
                        location: KeyLocation::Standard,
                        modifiers: KeyModifiers::empty(),
                        repeat: false,
                        is_composing: false,
                    },
                    keycode: 0
                },
                wid
            );
            // send char event only on key press, not release
            if let KeyState::Up = state { return }
            if let Some(txt) = utf8 {
                for chr in txt.chars() {
                    guard.send_event(Event::ReceivedCharacter(chr), wid);
                }
            }
        }
    }
}
