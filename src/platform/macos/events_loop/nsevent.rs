use std;
use std::sync::Arc;
use cocoa;
use cocoa::appkit::{self, NSApplication, NSApp, NSEvent, NSView, NSWindow};
use cocoa::foundation;
use core_foundation::base::{CFRetain,CFRelease,CFTypeRef};

use super::Timeout;
use super::super::DeviceId;
use super::super::window::{self, Window};
use events::{self, ElementState, Event, MouseButton, TouchPhase, DeviceEvent, WindowEvent, ModifiersState, KeyboardInput};

// RetainedEvent wraps an `NSEvent`, incrementing its refcount on `new` and decrementing on `drop`.
pub struct RetainedEvent(cocoa::base::id);

impl RetainedEvent {
    pub fn new(event: cocoa::base::id) -> RetainedEvent {
        unsafe { CFRetain(event as CFTypeRef); }
        RetainedEvent(event)
    }
    pub fn into_inner(self) -> cocoa::base::id {
        self.0
    }
    pub fn id(&self) -> cocoa::base::id {
        self.0
    }
}

impl Drop for RetainedEvent {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0 as CFTypeRef); }
    }
}

/// Should this event be discarded immediately after receipt?
pub fn should_discard_event_early(event: &RetainedEvent) -> bool {
    // is this even an event?
    if event.0 == cocoa::base::nil {
        // discard
        return true;
    }

    // FIXME: Despite not being documented anywhere, an `NSEvent` is produced when a user opens
    // Spotlight while the NSApplication is in focus. This `NSEvent` produces a `NSEventType`
    // with value `21`. This causes a SEGFAULT as soon as we try to match on the `NSEventType`
    // enum as there is no variant associated with the value. Thus, we return early if this
    // sneaky event occurs. If someone does find some documentation on this, please fix this by
    // adding an appropriate variant to the `NSEventType` enum in the cocoa-rs crate.
    if unsafe { event.0.eventType() } as u64 == 21 {
        // discard
        return true;
    }

    return false;
}

/// Should this event be forwarded back to the windowing system?
pub fn should_forward_event(event: &RetainedEvent) -> bool {
    // Determine if we need to send this event
    // FIXME: Document this. Why do we do this? Seems like it passes on events to window/app.
    // If we don't do this, window does not become main for some reason.
    match unsafe { event.0.eventType() } {
        appkit::NSKeyDown => false,
        _ => true,
    }
}

/// Processing of events sometimes requires persisting state from one event to another, e.g.
/// "shift key down", "A key down" => "shift A". The state required for that is stored here.
pub struct PersistentState {
    modifiers: Modifiers,
}

impl PersistentState {
    pub fn new() -> PersistentState {
        PersistentState {
            modifiers: Modifiers::new(),
        }
    }
}

pub trait WindowFinder {
    fn find_window_by_id(&self, id: window::Id) -> Option<Arc<Window>>;

    fn find_key_window(&self) -> Option<Arc<Window>> {
        unsafe {
            let cocoa_id = msg_send![NSApp(), keyWindow];
            if cocoa_id == cocoa::base::nil {
                None
            } else {
                self.find_window_by_id(window::get_window_id(cocoa_id))
            }
        }
    }
}

struct Modifiers {
    shift_pressed: bool,
    ctrl_pressed: bool,
    win_pressed: bool,
    alt_pressed: bool,
}

impl Modifiers {
    pub fn new() -> Self {
        Modifiers {
            shift_pressed: false,
            ctrl_pressed: false,
            win_pressed: false,
            alt_pressed: false,
        }
    }
}

pub fn receive_event_from_cocoa(timeout: Timeout) -> Option<RetainedEvent> {
    unsafe {
        let pool = foundation::NSAutoreleasePool::new(cocoa::base::nil);

        // Pick a timeout
        let timeout = match timeout {
            Timeout::Now => foundation::NSDate::distantPast(cocoa::base::nil),
            Timeout::Forever => foundation::NSDate::distantFuture(cocoa::base::nil),
        };

        // Poll for the next event
        let ns_event = appkit::NSApp().nextEventMatchingMask_untilDate_inMode_dequeue_(
            appkit::NSAnyEventMask.bits() | appkit::NSEventMaskPressure.bits(),
            timeout,
            foundation::NSDefaultRunLoopMode,
            cocoa::base::YES);

        // Wrap the event, if any, in a RetainedEvent
        let event = if ns_event == cocoa::base::nil {
            None
        } else {
            Some(RetainedEvent::new(ns_event))
        };

        let _: () = msg_send![pool, release];

        return event
    }
}

pub fn forward_event_to_cocoa(event: &RetainedEvent) {
    unsafe {
        NSApp().sendEvent_(event.id());
    }
}

// Attempt to translate an `NSEvent` into zero or more `Event`s.
pub fn to_events<WF>(event: &RetainedEvent, state: &mut PersistentState, window_finder: &WF) -> Vec<Event>
    where WF: WindowFinder
{
    let ns_event = event.0;
    let mut events: Vec<Event> = Vec::new();

    unsafe {
        let event_type = ns_event.eventType();
        let ns_window = ns_event.window();
        let window_id = window::get_window_id(ns_window);

        let maybe_window = window_finder.find_window_by_id(window_id);

        let into_event = |window_event| Event::WindowEvent {
            window_id: ::WindowId(window_id),
            event: window_event,
        };

        match event_type {
            appkit::NSKeyDown => {
                let received_c_str = foundation::NSString::UTF8String(ns_event.characters());
                let received_str = std::ffi::CStr::from_ptr(received_c_str);

                let vkey = to_virtual_key_code(NSEvent::keyCode(ns_event));
                let state = ElementState::Pressed;
                let code = NSEvent::keyCode(ns_event) as u32;
                let window_event = WindowEvent::KeyboardInput {
                    device_id: DEVICE_ID,
                    input: KeyboardInput {
                        state: state,
                        scancode: code,
                        virtual_keycode: vkey,
                        modifiers: event_mods(ns_event),
                    },
                };

                events.push(into_event(window_event));

                for received_char in std::str::from_utf8(received_str.to_bytes()).unwrap().chars() {
                    let window_event = WindowEvent::ReceivedCharacter(received_char);
                    events.push(into_event(window_event));
                }
            },

            appkit::NSKeyUp => {
                let vkey = to_virtual_key_code(NSEvent::keyCode(ns_event));

                let state = ElementState::Released;
                let code = NSEvent::keyCode(ns_event) as u32;
                let window_event = WindowEvent::KeyboardInput {
                    device_id: DEVICE_ID,
                    input: KeyboardInput {
                        state: state,
                        scancode: code,
                        virtual_keycode: vkey,
                        modifiers: event_mods(ns_event),
                    },
                };

                events.push(into_event(window_event));
            },

            appkit::NSFlagsChanged => {
                unsafe fn modifier_event(event: cocoa::base::id,
                                         keymask: appkit::NSEventModifierFlags,
                                         key: events::VirtualKeyCode,
                                         key_pressed: bool) -> Option<WindowEvent>
                {
                    if !key_pressed && NSEvent::modifierFlags(event).contains(keymask) {
                        let state = ElementState::Pressed;
                        let code = NSEvent::keyCode(event) as u32;
                        let window_event = WindowEvent::KeyboardInput {
                            device_id: DEVICE_ID,
                            input: KeyboardInput {
                                state: state,
                                scancode: code,
                                virtual_keycode: Some(key),
                                modifiers: event_mods(event),
                            },
                        };
                        Some(window_event)

                    } else if key_pressed && !NSEvent::modifierFlags(event).contains(keymask) {
                        let state = ElementState::Released;
                        let code = NSEvent::keyCode(event) as u32;
                        let window_event = WindowEvent::KeyboardInput {
                            device_id: DEVICE_ID,
                            input: KeyboardInput {
                                state: state,
                                scancode: code,
                                virtual_keycode: Some(key),
                                modifiers: event_mods(event),
                            },
                        };
                        Some(window_event)

                    } else {
                        None
                    }
                }

                if let Some(window_event) = modifier_event(ns_event,
                                                           appkit::NSShiftKeyMask,
                                                           events::VirtualKeyCode::LShift,
                                                           state.modifiers.shift_pressed)
                    {
                        state.modifiers.shift_pressed = !state.modifiers.shift_pressed;
                        events.push(into_event(window_event));
                    }

                if let Some(window_event) = modifier_event(ns_event,
                                                           appkit::NSControlKeyMask,
                                                           events::VirtualKeyCode::LControl,
                                                           state.modifiers.ctrl_pressed)
                    {
                        state.modifiers.ctrl_pressed = !state.modifiers.ctrl_pressed;
                        events.push(into_event(window_event));
                    }

                if let Some(window_event) = modifier_event(ns_event,
                                                           appkit::NSCommandKeyMask,
                                                           events::VirtualKeyCode::LWin,
                                                           state.modifiers.win_pressed)
                    {
                        state.modifiers.win_pressed = !state.modifiers.win_pressed;
                        events.push(into_event(window_event));
                    }

                if let Some(window_event) = modifier_event(ns_event,
                                                           appkit::NSAlternateKeyMask,
                                                           events::VirtualKeyCode::LAlt,
                                                           state.modifiers.alt_pressed)
                    {
                        state.modifiers.alt_pressed = !state.modifiers.alt_pressed;
                        events.push(into_event(window_event));
                    }
            },

            appkit::NSLeftMouseDown => { events.push(into_event(WindowEvent::MouseInput { device_id: DEVICE_ID, state: ElementState::Pressed, button: MouseButton::Left })); },
            appkit::NSLeftMouseUp => { events.push(into_event(WindowEvent::MouseInput { device_id: DEVICE_ID, state: ElementState::Released, button: MouseButton::Left })); },
            appkit::NSRightMouseDown => { events.push(into_event(WindowEvent::MouseInput { device_id: DEVICE_ID, state: ElementState::Pressed, button: MouseButton::Right })); },
            appkit::NSRightMouseUp => { events.push(into_event(WindowEvent::MouseInput { device_id: DEVICE_ID, state: ElementState::Released, button: MouseButton::Right })); },
            appkit::NSOtherMouseDown => { events.push(into_event(WindowEvent::MouseInput { device_id: DEVICE_ID, state: ElementState::Pressed, button: MouseButton::Middle })); },
            appkit::NSOtherMouseUp => { events.push(into_event(WindowEvent::MouseInput { device_id: DEVICE_ID, state: ElementState::Released, button: MouseButton::Middle })); },

            appkit::NSMouseEntered => { events.push(into_event(WindowEvent::MouseEntered { device_id: DEVICE_ID })); },
            appkit::NSMouseExited => { events.push(into_event(WindowEvent::MouseLeft { device_id: DEVICE_ID })); },

            appkit::NSMouseMoved |
            appkit::NSLeftMouseDragged |
            appkit::NSOtherMouseDragged |
            appkit::NSRightMouseDragged => {
                // If the mouse movement was on one of our windows, use it.
                // Otherwise, if one of our windows is the key window (receiving input), use it.
                // Otherwise, exit early.
                let window = match maybe_window.or_else(|| window_finder.find_key_window()) {
                    Some(window) => window,
                    None => return events,
                };

                let window_point = ns_event.locationInWindow();
                let view_point = if ns_window == cocoa::base::nil {
                    let ns_size = foundation::NSSize::new(0.0, 0.0);
                    let ns_rect = foundation::NSRect::new(window_point, ns_size);
                    let window_rect = window.window.convertRectFromScreen_(ns_rect);
                    window.view.convertPoint_fromView_(window_rect.origin, cocoa::base::nil)
                } else {
                    window.view.convertPoint_fromView_(window_point, cocoa::base::nil)
                };
                let view_rect = NSView::frame(*window.view);
                let scale_factor = window.hidpi_factor();

                {
                    let x = (scale_factor * view_point.x as f32) as f64;
                    let y = (scale_factor * (view_rect.size.height - view_point.y) as f32) as f64;
                    let window_event = WindowEvent::MouseMoved { device_id: DEVICE_ID, position: (x, y) };
                    let event = Event::WindowEvent { window_id: ::WindowId(window.id()), event: window_event };
                    events.push(event);
                }

                let delta_x = (scale_factor * ns_event.deltaX() as f32) as f64;
                if delta_x != 0.0 {
                    let motion_event = DeviceEvent::Motion { axis: 0, value: delta_x };
                    let event = Event::DeviceEvent{ device_id: DEVICE_ID, event: motion_event };
                    events.push(event);
                }

                let delta_y = (scale_factor * ns_event.deltaY() as f32) as f64;
                if delta_y != 0.0 {
                    let motion_event = DeviceEvent::Motion { axis: 1, value: delta_y };
                    let event = Event::DeviceEvent{ device_id: DEVICE_ID, event: motion_event };
                    events.push(event);
                }
            },

            appkit::NSScrollWheel => {
                // If none of the windows received the scroll, return early.
                let window = match maybe_window {
                    Some(window) => window,
                    None => return events,
                };

                use events::MouseScrollDelta::{LineDelta, PixelDelta};
                let scale_factor = window.hidpi_factor();
                let delta = if ns_event.hasPreciseScrollingDeltas() == cocoa::base::YES {
                    PixelDelta(scale_factor * ns_event.scrollingDeltaX() as f32,
                               scale_factor * ns_event.scrollingDeltaY() as f32)
                } else {
                    LineDelta(scale_factor * ns_event.scrollingDeltaX() as f32,
                              scale_factor * ns_event.scrollingDeltaY() as f32)
                };
                let phase = match ns_event.phase() {
                    appkit::NSEventPhaseMayBegin | appkit::NSEventPhaseBegan => TouchPhase::Started,
                    appkit::NSEventPhaseEnded => TouchPhase::Ended,
                    _ => TouchPhase::Moved,
                };
                let window_event = WindowEvent::MouseWheel { device_id: DEVICE_ID, delta: delta, phase: phase };
                events.push(into_event(window_event));
            },

            appkit::NSEventTypePressure => {
                let pressure = ns_event.pressure();
                let stage = ns_event.stage();
                let window_event = WindowEvent::TouchpadPressure { device_id: DEVICE_ID, pressure: pressure, stage: stage };
                events.push(into_event(window_event));
            },

            _ => (),
        }
    }

    events
}

fn to_virtual_key_code(code: u16) -> Option<events::VirtualKeyCode> {
    Some(match code {
        0x00 => events::VirtualKeyCode::A,
        0x01 => events::VirtualKeyCode::S,
        0x02 => events::VirtualKeyCode::D,
        0x03 => events::VirtualKeyCode::F,
        0x04 => events::VirtualKeyCode::H,
        0x05 => events::VirtualKeyCode::G,
        0x06 => events::VirtualKeyCode::Z,
        0x07 => events::VirtualKeyCode::X,
        0x08 => events::VirtualKeyCode::C,
        0x09 => events::VirtualKeyCode::V,
        //0x0a => World 1,
        0x0b => events::VirtualKeyCode::B,
        0x0c => events::VirtualKeyCode::Q,
        0x0d => events::VirtualKeyCode::W,
        0x0e => events::VirtualKeyCode::E,
        0x0f => events::VirtualKeyCode::R,
        0x10 => events::VirtualKeyCode::Y,
        0x11 => events::VirtualKeyCode::T,
        0x12 => events::VirtualKeyCode::Key1,
        0x13 => events::VirtualKeyCode::Key2,
        0x14 => events::VirtualKeyCode::Key3,
        0x15 => events::VirtualKeyCode::Key4,
        0x16 => events::VirtualKeyCode::Key6,
        0x17 => events::VirtualKeyCode::Key5,
        0x18 => events::VirtualKeyCode::Equals,
        0x19 => events::VirtualKeyCode::Key9,
        0x1a => events::VirtualKeyCode::Key7,
        0x1b => events::VirtualKeyCode::Minus,
        0x1c => events::VirtualKeyCode::Key8,
        0x1d => events::VirtualKeyCode::Key0,
        0x1e => events::VirtualKeyCode::RBracket,
        0x1f => events::VirtualKeyCode::O,
        0x20 => events::VirtualKeyCode::U,
        0x21 => events::VirtualKeyCode::LBracket,
        0x22 => events::VirtualKeyCode::I,
        0x23 => events::VirtualKeyCode::P,
        0x24 => events::VirtualKeyCode::Return,
        0x25 => events::VirtualKeyCode::L,
        0x26 => events::VirtualKeyCode::J,
        0x27 => events::VirtualKeyCode::Apostrophe,
        0x28 => events::VirtualKeyCode::K,
        0x29 => events::VirtualKeyCode::Semicolon,
        0x2a => events::VirtualKeyCode::Backslash,
        0x2b => events::VirtualKeyCode::Comma,
        0x2c => events::VirtualKeyCode::Slash,
        0x2d => events::VirtualKeyCode::N,
        0x2e => events::VirtualKeyCode::M,
        0x2f => events::VirtualKeyCode::Period,
        0x30 => events::VirtualKeyCode::Tab,
        0x31 => events::VirtualKeyCode::Space,
        0x32 => events::VirtualKeyCode::Grave,
        0x33 => events::VirtualKeyCode::Back,
        //0x34 => unkown,
        0x35 => events::VirtualKeyCode::Escape,
        0x36 => events::VirtualKeyCode::RWin,
        0x37 => events::VirtualKeyCode::LWin,
        0x38 => events::VirtualKeyCode::LShift,
        //0x39 => Caps lock,
        //0x3a => Left alt,
        0x3b => events::VirtualKeyCode::LControl,
        0x3c => events::VirtualKeyCode::RShift,
        //0x3d => Right alt,
        0x3e => events::VirtualKeyCode::RControl,
        //0x3f => Fn key,
        //0x40 => F17 Key,
        0x41 => events::VirtualKeyCode::Decimal,
        //0x42 -> unkown,
        0x43 => events::VirtualKeyCode::Multiply,
        //0x44 => unkown,
        0x45 => events::VirtualKeyCode::Add,
        //0x46 => unkown,
        0x47 => events::VirtualKeyCode::Numlock,
        //0x48 => KeypadClear,
        0x49 => events::VirtualKeyCode::VolumeUp,
        0x4a => events::VirtualKeyCode::VolumeDown,
        0x4b => events::VirtualKeyCode::Divide,
        0x4c => events::VirtualKeyCode::NumpadEnter,
        //0x4d => unkown,
        0x4e => events::VirtualKeyCode::Subtract,
        //0x4f => F18 key,
        //0x50 => F19 Key,
        0x51 => events::VirtualKeyCode::NumpadEquals,
        0x52 => events::VirtualKeyCode::Numpad0,
        0x53 => events::VirtualKeyCode::Numpad1,
        0x54 => events::VirtualKeyCode::Numpad2,
        0x55 => events::VirtualKeyCode::Numpad3,
        0x56 => events::VirtualKeyCode::Numpad4,
        0x57 => events::VirtualKeyCode::Numpad5,
        0x58 => events::VirtualKeyCode::Numpad6,
        0x59 => events::VirtualKeyCode::Numpad7,
        //0x5a => F20 Key,
        0x5b => events::VirtualKeyCode::Numpad8,
        0x5c => events::VirtualKeyCode::Numpad9,
        //0x5d => unkown,
        //0x5e => unkown,
        //0x5f => unkown,
        0x60 => events::VirtualKeyCode::F5,
        0x61 => events::VirtualKeyCode::F6,
        0x62 => events::VirtualKeyCode::F7,
        0x63 => events::VirtualKeyCode::F3,
        0x64 => events::VirtualKeyCode::F8,
        0x65 => events::VirtualKeyCode::F9,
        //0x66 => unkown,
        0x67 => events::VirtualKeyCode::F11,
        //0x68 => unkown,
        0x69 => events::VirtualKeyCode::F13,
        //0x6a => F16 Key,
        0x6b => events::VirtualKeyCode::F14,
        //0x6c => unkown,
        0x6d => events::VirtualKeyCode::F10,
        //0x6e => unkown,
        0x6f => events::VirtualKeyCode::F12,
        //0x70 => unkown,
        0x71 => events::VirtualKeyCode::F15,
        0x72 => events::VirtualKeyCode::Insert,
        0x73 => events::VirtualKeyCode::Home,
        0x74 => events::VirtualKeyCode::PageUp,
        0x75 => events::VirtualKeyCode::Delete,
        0x76 => events::VirtualKeyCode::F4,
        0x77 => events::VirtualKeyCode::End,
        0x78 => events::VirtualKeyCode::F2,
        0x79 => events::VirtualKeyCode::PageDown,
        0x7a => events::VirtualKeyCode::F1,
        0x7b => events::VirtualKeyCode::Left,
        0x7c => events::VirtualKeyCode::Right,
        0x7d => events::VirtualKeyCode::Down,
        0x7e => events::VirtualKeyCode::Up,
        //0x7f =>  unkown,

        _ => return None,
    })
}

fn event_mods(event: cocoa::base::id) -> ModifiersState {
    let flags = unsafe {
        NSEvent::modifierFlags(event)
    };
    ModifiersState {
        shift: flags.contains(appkit::NSShiftKeyMask),
        ctrl: flags.contains(appkit::NSControlKeyMask),
        alt: flags.contains(appkit::NSAlternateKeyMask),
        logo: flags.contains(appkit::NSCommandKeyMask),
    }
}

// Constant device ID, to be removed when this backend is updated to report real device IDs.
const DEVICE_ID: ::DeviceId = ::DeviceId(DeviceId);
