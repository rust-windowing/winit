use super::wayland::core::{default_display, Display, Registry};
use super::wayland::core::compositor::{Compositor, SurfaceId, WSurface};
use super::wayland::core::output::Output;
use super::wayland::core::seat::{ButtonState, Seat, Pointer, Keyboard, KeyState};
use super::wayland::core::shell::Shell;
use super::wayland_kbd::MappedKeyboard;
use super::keyboard::keycode_to_vkey;


use std::collections::{VecDeque, HashMap};
use std::sync::{Arc, Mutex};

use Event;
use MouseButton;
use ElementState;

enum AnyKeyboard {
    RawKeyBoard(Keyboard),
    XKB(MappedKeyboard)
}

pub struct WaylandContext {
    pub display: Display,
    pub registry: Registry,
    pub compositor: Compositor,
    pub shell: Shell,
    pub seat: Seat,
    pointer: Option<Mutex<Pointer<WSurface>>>,
    keyboard: Option<AnyKeyboard>,
    windows_event_queues: Arc<Mutex<HashMap<SurfaceId, Arc<Mutex<VecDeque<Event>>>>>>,
    current_pointer_surface: Arc<Mutex<Option<SurfaceId>>>,
    current_keyboard_surface: Arc<Mutex<Option<SurfaceId>>>,
    pub outputs: Vec<Arc<Output>>
}

impl WaylandContext {
    pub fn new() -> Option<WaylandContext> {
        let display = match default_display() {
            Some(d) => d,
            None => return None,
        };
        let registry = display.get_registry();
        // let the registry get its events
        display.sync_roundtrip();
        let compositor = match registry.get_compositor() {
            Some(c) => c,
            None => return None,
        };
        let shell = match registry.get_shell() {
            Some(s) => s,
            None => return None,
        };
        let seat = match registry.get_seats().into_iter().next() {
            Some(s) => s,
            None => return None,
        };
        let outputs = registry.get_outputs().into_iter().map(Arc::new).collect::<Vec<_>>();
        // let the other globals get their events
        display.sync_roundtrip();

        let current_pointer_surface = Arc::new(Mutex::new(None));

        // rustc has trouble finding the correct type here, so we explicit it.
        let windows_event_queues = Arc::new(Mutex::new(
            HashMap::<SurfaceId, Arc<Mutex<VecDeque<Event>>>>::new()
        ));

        // handle pointer inputs
        let mut pointer = seat.get_pointer();
        if let Some(ref mut p) = pointer {
            // set the enter/leave callbacks
            let current_surface = current_pointer_surface.clone();
            p.set_enter_action(move |_, _, sid, x, y| {
                *current_surface.lock().unwrap() = Some(sid);
            });
            let current_surface = current_pointer_surface.clone();
            p.set_leave_action(move |_, _, sid| {
                *current_surface.lock().unwrap() = None;
            });
            // set the events callbacks
            let current_surface = current_pointer_surface.clone();
            let event_queues = windows_event_queues.clone();
            p.set_motion_action(move |_, _, x, y| {
                // dispatch to the appropriate queue
                let sid = *current_surface.lock().unwrap();
                if let Some(sid) = sid {
                    let map = event_queues.lock().unwrap();
                    if let Some(queue) = map.get(&sid) {
                        queue.lock().unwrap().push_back(Event::MouseMoved((x as i32,y as i32)))
                    }
                }
            });
            let current_surface = current_pointer_surface.clone();
            let event_queues = windows_event_queues.clone();
            p.set_button_action(move |_, _, sid, b, s| {
                let button = match b {
                    0x110 => MouseButton::Left,
                    0x111 => MouseButton::Right,
                    0x112 => MouseButton::Middle,
                    _ => return
                };
                let state = match s {
                    ButtonState::Released => ElementState::Released,
                    ButtonState::Pressed => ElementState::Pressed
                };
                // dispatch to the appropriate queue
                let sid = *current_surface.lock().unwrap();
                if let Some(sid) = sid {
                    let map = event_queues.lock().unwrap();
                    if let Some(queue) = map.get(&sid) {
                        queue.lock().unwrap().push_back(Event::MouseInput(state, button))
                    }
                }
            });
        }

        // handle keyboard inputs
        let mut keyboard = None;
        let current_keyboard_surface = Arc::new(Mutex::new(None));
        if let Some(mut wkbd) = seat.get_keyboard() {
            display.sync_roundtrip();

            let current_surface = current_keyboard_surface.clone();
            wkbd.set_enter_action(move |_, _, sid, _| {
                *current_surface.lock().unwrap() = Some(sid);
            });
            let current_surface = current_keyboard_surface.clone();
            wkbd.set_leave_action(move |_, _, sid| {
                *current_surface.lock().unwrap() = None;
            });

            let kbd = match MappedKeyboard::new(wkbd) {
                Ok(mkbd) => {
                    // We managed to load a keymap
                    let current_surface = current_keyboard_surface.clone();
                    let event_queues = windows_event_queues.clone();
                    mkbd.set_key_action(move |state, _, _, _, keycode, keystate| {
                        let kstate = match keystate {
                            KeyState::Released => ElementState::Released,
                            KeyState::Pressed => ElementState::Pressed
                        };
                        let mut events = Vec::new();
                        // key event
                        events.push(Event::KeyboardInput(
                            kstate,
                            (keycode & 0xff) as u8,
                            keycode_to_vkey(state, keycode)
                        ));
                        // utf8 events
                        if kstate == ElementState::Pressed {
                            if let Some(txt) = state.get_utf8(keycode) {
                                events.extend(
                                    txt.chars().map(Event::ReceivedCharacter)
                                );
                            }
                        }
                        // dispatch to the appropriate queue
                        let sid = *current_surface.lock().unwrap();
                        if let Some(sid) = sid {
                            let map = event_queues.lock().unwrap();
                            if let Some(queue) = map.get(&sid) {
                                queue.lock().unwrap().extend(events.into_iter());
                            }
                        }
                    });
                    AnyKeyboard::XKB(mkbd)
                },
                Err(mut rkbd) => {
                    // fallback to raw inputs, no virtual keycodes
                    let current_surface = current_keyboard_surface.clone();
                    let event_queues = windows_event_queues.clone();
                    rkbd.set_key_action(move |_, _, _, keycode, keystate| {
                        let kstate = match keystate {
                            KeyState::Released => ElementState::Released,
                            KeyState::Pressed => ElementState::Pressed
                        };
                        let event = Event::KeyboardInput(kstate, (keycode & 0xff) as u8, None);
                        // dispatch to the appropriate queue
                        let sid = *current_surface.lock().unwrap();
                        if let Some(sid) = sid {
                            let map = event_queues.lock().unwrap();
                            if let Some(queue) = map.get(&sid) {
                                queue.lock().unwrap().push_back(event);
                            }
                        }
                    });
                    AnyKeyboard::RawKeyBoard(rkbd)
                }
            };
            keyboard = Some(kbd);
        }

        Some(WaylandContext {
            display: display,
            registry: registry,
            compositor: compositor,
            shell: shell,
            seat: seat,
            pointer: pointer.map(|p| Mutex::new(p)),
            keyboard: keyboard,
            windows_event_queues: windows_event_queues,
            current_pointer_surface: current_pointer_surface,
            current_keyboard_surface: current_keyboard_surface,
            outputs: outputs
        })
    }

    pub fn register_surface(&self, sid: SurfaceId, queue: Arc<Mutex<VecDeque<Event>>>) {
        self.windows_event_queues.lock().unwrap().insert(sid, queue);
        if let Some(ref p) = self.pointer {
            p.lock().unwrap().add_handled_surface(sid);
        }
    }

    pub fn deregister_surface(&self, sid: SurfaceId) {
        self.windows_event_queues.lock().unwrap().remove(&sid);
        if let Some(ref p) = self.pointer {
            p.lock().unwrap().remove_handled_surface(sid);
        }
    }

    pub fn push_event_for(&self, sid: SurfaceId, evt: Event) {
        let mut guard = self.windows_event_queues.lock().unwrap();
        if let Some(queue) = guard.get(&sid) {
            queue.lock().unwrap().push_back(evt);
        }
    }
}
