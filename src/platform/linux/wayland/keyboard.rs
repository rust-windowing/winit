use futures::{ self, Future, Sink, Stream };
use futures::sync::mpsc;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;

use {VirtualKeyCode, ElementState, WindowEvent as Event, KeyboardInput, ModifiersState};

use super::{EventsLoopSink, EventsLoopProxy, WindowId, make_wid, DeviceId, streams};
use super::wayland_kbd::{MappedKeyboardImplementation, register_kbd};
use tokio_core::reactor::Core;
use tokio_timer;
use wayland_client::protocol::wl_keyboard;
use wayland_client::EventQueueHandle;

pub fn init_keyboard(evq: &mut EventQueueHandle, proxy: EventsLoopProxy, keyboard: &wl_keyboard::WlKeyboard, sink: &Arc<Mutex<EventsLoopSink>>) {
    let idata = KeyboardIData::new(sink.clone(), proxy.clone());

    if register_kbd(evq, keyboard, mapped_keyboard_impl(), idata).is_err() {
        // initializing libxkbcommon failed :(
        // fallback implementation
        let idata = KeyboardIData::new(sink.clone(), proxy);
        evq.register(keyboard, raw_keyboard_impl(), idata);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct RepeatInfo {
    pub delay: Duration,
    pub frequency: u64,
}

impl RepeatInfo {
    fn from_wayland(delay: i32, rate: i32) -> RepeatInfo {
        RepeatInfo {
            delay: Duration::from_millis(delay as u64),
            frequency: rate as u64,
        }
    }

    fn interval(&self) -> Duration {
        Duration::from_millis(1000u64 / self.frequency as u64)
    }
}

impl Default for RepeatInfo {
    fn default() -> RepeatInfo {
        // These are just sensible defaults for type purposes. We are actually guaranteed to get
        // the information from wayland before any key events, so this is largely unnecessary, but
        // having defaults lets us avoid using Option<RepeatInfo> and upwrap()'ing everwhere
        RepeatInfo {
            delay: Duration::from_millis(500),
            frequency: 20,
        }
    }
}

struct KeyboardIData {
    sink: Arc<Mutex<EventsLoopSink>>,
    target: Option<WindowId>,
    repeat_info: Arc<RwLock<RepeatInfo>>,
    sender: mpsc::Sender<(Event, WindowId, Option<String>)>,
}

trait EventsLoopSinkExt {
    fn send_event_with_characters<It: Iterator<Item=char>>(&mut self, evt: Event, wid: WindowId, chars: It);
}

impl EventsLoopSinkExt for EventsLoopSink {
    fn send_event_with_characters<It: Iterator<Item=char>>(&mut self, evt: Event, wid: WindowId, chars: It) {
        self.send_event(evt, wid);
        let char_events = chars.map(|c| Event::ReceivedCharacter(c));
        for evt in char_events {
            self.send_event(evt, wid);
        }
    }
}

#[inline]
fn make_keyboard_event(input: KeyboardInput) -> Event {
    Event::KeyboardInput {
        device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
        input: input,
    }
}

trait FilterOpt<T> {
    fn filter_opt<F: FnOnce(&T) -> bool>(self, f: F) -> Option<T>;
}

impl<T> FilterOpt<T> for Option<T> {
    #[inline]
    fn filter_opt<F: FnOnce(&T) -> bool>(self, f: F) -> Option<T> {
        self.and_then(|v| if f(&v) {
            Some(v)
        } else {
            None
        })
    }
}

const REPEAT_INTERNAL_CHANNEL_SIZE: usize = 512;

impl KeyboardIData {
    fn new(sink: Arc<Mutex<EventsLoopSink>>, proxy: EventsLoopProxy) -> KeyboardIData {
        let (send, recv) = mpsc::channel(REPEAT_INTERNAL_CHANNEL_SIZE);
        let ret = KeyboardIData {
            sink: sink,
            target: None,
            repeat_info: Arc::new(RwLock::new(Default::default())),
            sender: send,
        };
        let repeat_info = ret.repeat_info.clone();
        let sink = ret.sink.clone();
        thread::spawn(move || {
            let proxy = proxy;
            let sink = sink;
            let timer = tokio_timer::wheel()
                .tick_duration(Duration::from_millis(10))
                .thread_name("keyboard-timer")
                .build();
            let timer = Rc::new(timer);
            let mut core = Core::new().unwrap();
            let handle = core.handle();
            let pressed_key: RefCell<Option<(KeyboardInput, Rc<RefCell<mpsc::Receiver<(KeyboardInput, WindowId)>>>)>> = RefCell::new(None);
            let events = recv
                .filter_map(|(evt, wid, utf8)| match evt {
                    Event::KeyboardInput { input, .. } => Some((input, wid, utf8)),
                    Event::Focused(_) => {
                        let mut pressed_key = pressed_key.borrow_mut();
                        pressed_key.take();
                        None
                    },
                    _ => None
                })
                .filter_map(|(input, wid, utf8)| {
                    let mut pressed_key = pressed_key.borrow_mut();
                    match input.state {
                        ElementState::Pressed => {
                            pressed_key.take();
                            Some((input, wid, utf8))
                        },
                        ElementState::Released => {
                            *pressed_key = pressed_key
                                .take()
                                .filter_opt(|&(pressed_input, _)| input.scancode != pressed_input.scancode);
                            None
                        },
                    }
                })
                .for_each(|(input, wid, utf8)| {
                    // These all need to be cloned at this point because they may out-live this
                    // function (once we call handle.spawn(...) on the generated future
                    let proxy = proxy.clone();
                    let sink = sink.clone();
                    let timer_ref = timer.clone();
                    let interval_handle = handle.clone();
                    let repeat_info: RepeatInfo = {
                        let repeat_info = repeat_info.read().unwrap();
                        *repeat_info
                    };
                    let (send, recv) = mpsc::channel(REPEAT_INTERNAL_CHANNEL_SIZE);
                    // Build the receiver stream
                    let recv = {
                        let mut pressed_key = pressed_key.borrow_mut();
                        let recv = Rc::new(RefCell::new(recv));
                        let ret = Rc::downgrade(&recv);
                        *pressed_key = Some((input, recv));
                        // This stream will end once ret no longer exists
                        streams::WhileExists(ret)
                            .map(|(input, wid)| (make_keyboard_event(input), wid))
                            .map(move |(evt, wid)| {
                                let mut sink = sink.lock().unwrap();
                                // We add in the utf8 in the receiver stream to avoid cloning it
                                // for every event
                                if let Some(chars) = utf8.as_ref().map(|s| s.chars()) {
                                    sink.send_event_with_characters(evt, wid, chars);
                                } else {
                                    sink.send_event(evt, wid);
                                }
                            })
                            .take_while(move |_| Ok(proxy.wakeup().is_ok()))
                    };
                    // Build the sink to which we send events
                    let send = send.sink_map_err(|_| ()); // Sender errors just mean that we're done
                    let f = timer.sleep(repeat_info.delay)
                        .map_err(|timer_error| panic!(timer_error))
                        .and_then(move |_| {
                            let f = timer_ref.interval(repeat_info.interval())
                                .map_err(|timer_error| panic!(timer_error))
                                .map(move |_| (input, wid))
                                .forward(send)
                                .map(|_| ());
                            // Run the interval timer, until it encounters an error,
                            // which will happen once there is a change in pressed_key
                            interval_handle.spawn(f);
                            futures::future::ok(())
                        });
                    // Run the initial timer delay
                    handle.spawn(f);
                    // Run the receiver stream handling key presses for this key press
                    handle.spawn(recv.for_each(Ok));
                    futures::future::ok(())
                });
            core.run(events).unwrap();
        });
        ret
    }

    fn update_repeat_info(&mut self, delay: i32, rate: i32) {
        *self.repeat_info.write().unwrap() = RepeatInfo::from_wayland(delay, rate);
    }
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
                let state: ElementState = state.into();
                let vkcode = key_to_vkey(rawkey, keysym);
                let mut guard = idata.sink.lock().unwrap();
                let evt = make_keyboard_event(KeyboardInput {
                    state: state,
                    scancode: rawkey,
                    virtual_keycode: vkcode,
                    modifiers: ModifiersState {
                        shift: mods.shift,
                        ctrl: mods.ctrl,
                        alt: mods.alt,
                        logo: mods.logo
                    },
                });
                let evt2 = evt.clone();
                // send char event only on key press, not release
                if let (ElementState::Pressed, Some(txt)) = (state, utf8.as_ref()) {
                    guard.send_event_with_characters(evt, wid, txt.chars());
                } else {
                    guard.send_event(evt, wid);
                }
                // The asynchronous repeat handler wants to hear about every event
                idata.sender.try_send((evt2, wid, utf8)).unwrap();
            }
        },
        repeat_info: |_, idata, _, rate, delay| idata.update_repeat_info(delay, rate),
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
                let state: ElementState = state.into();
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
        repeat_info: |_, idata, _, rate, delay| idata.update_repeat_info(delay, rate),
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
