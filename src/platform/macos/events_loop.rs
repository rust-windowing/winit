use cocoa::{self, appkit, foundation};
use cocoa::appkit::{NSApplication, NSEvent, NSView, NSWindow};
use events::{self, ElementState, Event, MouseButton, TouchPhase, WindowEvent};
use super::window::Window;
use std;


pub struct EventsLoop {
    pub windows: std::sync::Mutex<Vec<std::sync::Arc<Window>>>,
    pub pending_events: std::sync::Mutex<std::collections::VecDeque<Event>>,
    modifiers: std::sync::Mutex<Modifiers>,
    interrupted: std::sync::atomic::AtomicBool,

    /// The user's event callback given via either the `poll_events` or `run_forever` method. This
    /// will only be `Some` for the duration of whichever of these methods has been called and will
    /// always be `None` otherwise.
    pub callback: std::sync::Mutex<Option<Box<FnMut(Event)>>>
}

struct Modifiers {
    shift_pressed: bool,
    ctrl_pressed: bool,
    win_pressed: bool,
    alt_pressed: bool,
}


impl EventsLoop {

    pub fn new() -> Self {
        let modifiers = Modifiers {
            shift_pressed: false,
            ctrl_pressed: false,
            win_pressed: false,
            alt_pressed: false,
        };
        EventsLoop {
            windows: std::sync::Mutex::new(Vec::new()),
            pending_events: std::sync::Mutex::new(std::collections::VecDeque::new()),
            modifiers: std::sync::Mutex::new(modifiers),
            interrupted: std::sync::atomic::AtomicBool::new(false),
            callback: std::sync::Mutex::new(None),
        }
    }

    pub fn poll_events<F>(&self, callback: F)
        where F: FnMut(Event),
    {
        unsafe {
            if !msg_send![cocoa::base::class("NSThread"), isMainThread] {
                panic!("Events can only be polled from the main thread on macOS");
            }

            self.store_callback(callback);
        }

        // Loop as long as we have pending events to return.
        loop {
            // First, yield all pending events.
            while let Some(event) = self.pending_events.lock().unwrap().pop_front() {
                if let Ok(mut callback) = self.callback.lock() {
                    callback.as_mut().unwrap()(event);
                }
            }

            unsafe {
                let pool = foundation::NSAutoreleasePool::new(cocoa::base::nil);

                // Poll for the next event, returning `nil` if there are none.
                let ns_event = appkit::NSApp().nextEventMatchingMask_untilDate_inMode_dequeue_(
                    appkit::NSAnyEventMask.bits() | appkit::NSEventMaskPressure.bits(),
                    foundation::NSDate::distantPast(cocoa::base::nil),
                    foundation::NSDefaultRunLoopMode,
                    cocoa::base::YES);

                let event = self.ns_event_to_event(ns_event);

                let _: () = msg_send![pool, release];

                match event {
                    // Call the user's callback.
                    Some(event) => if let Ok(mut callback) = self.callback.lock() {
                        callback.as_mut().unwrap()(event);
                    },
                    None => break,
                }
            }
        }

        // Drop the callback to enforce our guarantee that it will never live longer than the
        // duration of this method.
        self.callback.lock().unwrap().take();
    }

    pub fn run_forever<F>(&self, callback: F)
        where F: FnMut(Event)
    {
        self.interrupted.store(false, std::sync::atomic::Ordering::Relaxed);

        unsafe {
            if !msg_send![cocoa::base::class("NSThread"), isMainThread] {
                panic!("Events can only be polled from the main thread on macOS");
            }

            self.store_callback(callback);
        }

        loop {
            // First, yield all pending events.
            while let Some(event) = self.pending_events.lock().unwrap().pop_front() {
                if let Ok(mut callback) = self.callback.lock() {
                    callback.as_mut().unwrap()(event);
                }
            }

            unsafe {
                let pool = foundation::NSAutoreleasePool::new(cocoa::base::nil);

                // Wait for the next event. Note that this function blocks during resize.
                let ns_event = appkit::NSApp().nextEventMatchingMask_untilDate_inMode_dequeue_(
                    appkit::NSAnyEventMask.bits() | appkit::NSEventMaskPressure.bits(),
                    foundation::NSDate::distantFuture(cocoa::base::nil),
                    foundation::NSDefaultRunLoopMode,
                    cocoa::base::YES);

                if let Some(event) = self.ns_event_to_event(ns_event) {
                    if let Ok(mut callback) = self.callback.lock() {
                        callback.as_mut().unwrap()(event);
                    }
                }

                let _: () = msg_send![pool, release];
            }

            if self.interrupted.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
        }

        // Drop the callback to enforce our guarantee that it will never live longer than the
        // duration of this method.
        self.callback.lock().unwrap().take();
    }

    pub fn interrupt(&self) {
        self.interrupted.store(true, std::sync::atomic::Ordering::Relaxed);

        // Awaken the event loop by triggering `NSApplicationActivatedEventType`.
        unsafe {
            let pool = foundation::NSAutoreleasePool::new(cocoa::base::nil);
            let event =
                NSEvent::otherEventWithType_location_modifierFlags_timestamp_windowNumber_context_subtype_data1_data2_(
                    cocoa::base::nil,
                    appkit::NSApplicationDefined,
                    foundation::NSPoint::new(0.0, 0.0),
                    appkit::NSEventModifierFlags::empty(),
                    0.0,
                    0,
                    cocoa::base::nil,
                    appkit::NSEventSubtype::NSApplicationActivatedEventType,
                    0,
                    0);
            appkit::NSApp().postEvent_atStart_(event, cocoa::base::NO);
            foundation::NSAutoreleasePool::drain(pool);
        }
    }

    // Here we store user's `callback` behind the `EventsLoop`'s mutex so that it may be safely
    // shared between each of the window delegates.
    //
    // In order to store the `callback` within the `Eventsloop` as a trait object, we must
    // `Box` the callback. Normally this would require that `F: 'static`, however we know that
    // the callback cannot live longer than the lifetime of this method. Thus, we use `unsafe`
    // to work around this requirement and enforce this guarantee ourselves.
    //
    // This should *only* be called at the beginning of `poll_events` and `run_forever`, both of
    // which *must* drop the callback at the end of their scope.
    unsafe fn store_callback<F>(&self, callback: F)
        where F: FnMut(Event)
    {
        let boxed: Box<F> = Box::new(callback);
        let boxed: Box<FnMut(Event)> = std::mem::transmute(boxed as Box<FnMut(Event)>);
        *self.callback.lock().unwrap() = Some(boxed);
    }

    // Convert some given `NSEvent` into a winit `Event`.
    unsafe fn ns_event_to_event(&self, ns_event: cocoa::base::id) -> Option<Event> {
        if ns_event == cocoa::base::nil {
            return None;
        }

        let event_type = ns_event.eventType();
        let window_id = super::window::get_window_id(ns_event.window());
        let windows = self.windows.lock().unwrap();
        let maybe_window = windows.iter().find(|window| window_id == window.id());

        let window = match maybe_window {
            Some(window) => window,
            None => return None,
        };

        // FIXME: Document this. Why do we do this?
        match event_type {
            appkit::NSKeyDown => (),
            _ => appkit::NSApp().sendEvent_(ns_event),
        }

        let into_event = |window_event| Event::WindowEvent {
            window_id: ::WindowId(window_id),
            event: window_event,
        };

        match event_type {

            appkit::NSKeyDown => {
                let mut events = std::collections::VecDeque::new();
                let received_c_str = foundation::NSString::UTF8String(ns_event.characters());
                let received_str = std::ffi::CStr::from_ptr(received_c_str);
                for received_char in std::str::from_utf8(received_str.to_bytes()).unwrap().chars() {
                    let window_event = WindowEvent::ReceivedCharacter(received_char);
                    events.push_back(into_event(window_event));
                }

                let vkey =  to_virtual_key_code(NSEvent::keyCode(ns_event));
                let state = ElementState::Pressed;
                let code = NSEvent::keyCode(ns_event) as u8;
                let window_event = WindowEvent::KeyboardInput(state, code, vkey);
                events.push_back(into_event(window_event));
                let event = events.pop_front();
                self.pending_events.lock().unwrap().extend(events.into_iter());
                event
            },

            appkit::NSKeyUp => {
                let vkey =  to_virtual_key_code(NSEvent::keyCode(ns_event));

                let state = ElementState::Released;
                let code = NSEvent::keyCode(ns_event) as u8;
                let window_event = WindowEvent::KeyboardInput(state, code, vkey);
                Some(into_event(window_event))
            },

            appkit::NSFlagsChanged => {
                let mut modifiers = self.modifiers.lock().unwrap();

                unsafe fn modifier_event(event: cocoa::base::id,
                                         keymask: appkit::NSEventModifierFlags,
                                         key: events::VirtualKeyCode,
                                         key_pressed: bool) -> Option<WindowEvent>
                {
                    if !key_pressed && NSEvent::modifierFlags(event).contains(keymask) {
                        let state = ElementState::Pressed;
                        let code = NSEvent::keyCode(event) as u8;
                        let window_event = WindowEvent::KeyboardInput(state, code, Some(key));
                        Some(window_event)

                    } else if key_pressed && !NSEvent::modifierFlags(event).contains(keymask) {
                        let state = ElementState::Released;
                        let code = NSEvent::keyCode(event) as u8;
                        let window_event = WindowEvent::KeyboardInput(state, code, Some(key));
                        Some(window_event)

                    } else {
                        None
                    }
                }

                let mut events = std::collections::VecDeque::new();
                if let Some(window_event) = modifier_event(ns_event,
                                                           appkit::NSShiftKeyMask,
                                                           events::VirtualKeyCode::LShift,
                                                           modifiers.shift_pressed)
                {
                    modifiers.shift_pressed = !modifiers.shift_pressed;
                    events.push_back(into_event(window_event));
                }

                if let Some(window_event) = modifier_event(ns_event,
                                                           appkit::NSControlKeyMask,
                                                           events::VirtualKeyCode::LControl,
                                                           modifiers.ctrl_pressed)
                {
                    modifiers.ctrl_pressed = !modifiers.ctrl_pressed;
                    events.push_back(into_event(window_event));
                }

                if let Some(window_event) = modifier_event(ns_event,
                                                           appkit::NSCommandKeyMask,
                                                           events::VirtualKeyCode::LWin,
                                                           modifiers.win_pressed)
                {
                    modifiers.win_pressed = !modifiers.win_pressed;
                    events.push_back(into_event(window_event));
                }

                if let Some(window_event) = modifier_event(ns_event,
                                                           appkit::NSAlternateKeyMask,
                                                           events::VirtualKeyCode::LAlt,
                                                           modifiers.alt_pressed)
                {
                    modifiers.alt_pressed = !modifiers.alt_pressed;
                    events.push_back(into_event(window_event));
                }

                let event = events.pop_front();
                self.pending_events.lock().unwrap().extend(events.into_iter());
                event
            },

            appkit::NSLeftMouseDown => { Some(into_event(WindowEvent::MouseInput(ElementState::Pressed, MouseButton::Left))) },
            appkit::NSLeftMouseUp => { Some(into_event(WindowEvent::MouseInput(ElementState::Released, MouseButton::Left))) },
            appkit::NSRightMouseDown => { Some(into_event(WindowEvent::MouseInput(ElementState::Pressed, MouseButton::Right))) },
            appkit::NSRightMouseUp => { Some(into_event(WindowEvent::MouseInput(ElementState::Released, MouseButton::Right))) },
            appkit::NSOtherMouseDown => { Some(into_event(WindowEvent::MouseInput(ElementState::Pressed, MouseButton::Middle))) },
            appkit::NSOtherMouseUp => { Some(into_event(WindowEvent::MouseInput(ElementState::Released, MouseButton::Middle))) },
            appkit::NSMouseEntered => { Some(into_event(WindowEvent::MouseEntered)) },
            appkit::NSMouseExited => { Some(into_event(WindowEvent::MouseLeft)) },
            appkit::NSMouseMoved |
            appkit::NSLeftMouseDragged |
            appkit::NSOtherMouseDragged |
            appkit::NSRightMouseDragged => {
                let window_point = ns_event.locationInWindow();
                let ns_window: cocoa::base::id = msg_send![ns_event, window];
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

                let x = (scale_factor * view_point.x as f32) as i32;
                let y = (scale_factor * (view_rect.size.height - view_point.y) as f32) as i32;
                let window_event = WindowEvent::MouseMoved(x, y);
                Some(into_event(window_event))
            },

            appkit::NSScrollWheel => {
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
                let window_event = WindowEvent::MouseWheel(delta, phase);
                Some(into_event(window_event))
            },

            appkit::NSEventTypePressure => {
                let pressure = ns_event.pressure();
                let stage = ns_event.stage();
                let window_event = WindowEvent::TouchpadPressure(pressure, stage);
                Some(into_event(window_event))
            },

            appkit::NSApplicationDefined => match ns_event.subtype() {
                appkit::NSEventSubtype::NSApplicationActivatedEventType => {
                    Some(into_event(WindowEvent::Awakened))
                },
                _ => None,
            },

            _  => None,
        }
    }

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
