use {ControlFlow, EventsLoopClosed};
use cocoa::{self, appkit, foundation};
use cocoa::appkit::{NSApplication, NSEvent, NSView, NSWindow};
use core_foundation::base::{CFRetain, CFRelease, CFTypeRef};
use core_foundation::runloop;
use events::{self, ElementState, Event, MouseButton, TouchPhase, WindowEvent, DeviceEvent, ModifiersState, KeyboardInput};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, Weak};
use super::window::Window;
use std;
use super::DeviceId;
use super::send_event::SendEvent;

pub struct EventsLoop {
    modifiers: Modifiers,
    pub shared: Arc<Shared>,
    current_cocoa_event: Option<CocoaEvent>,
}

// State shared between the `EventsLoop` and its registered windows.
pub struct Shared {
    pub windows: Mutex<Vec<Weak<Window>>>,

    // A queue of events that are pending delivery to the library user.
    pub pending_events: Mutex<VecDeque<Event>>,
}

pub struct Proxy {
    shared: Weak<Shared>,
}

struct Modifiers {
    shift_pressed: bool,
    ctrl_pressed: bool,
    win_pressed: bool,
    alt_pressed: bool,
}

impl Shared {

    pub fn new() -> Self {
        Shared {
            windows: Mutex::new(Vec::new()),
            pending_events: Mutex::new(VecDeque::new()),
        }
    }

    // Enqueues the event for prompt delivery to the application.
    pub fn enqueue_event(&self, event: Event) {
        self.pending_events.lock().unwrap().push_back(event);

        // attempt to wake the runloop
        unsafe {
            runloop::CFRunLoopWakeUp(runloop::CFRunLoopGetMain());
        }
    }

    // Dequeues the first event, if any, from the queue.
    fn dequeue_event(&self) -> Option<Event> {
        self.pending_events.lock().unwrap().pop_front()
    }

    // Removes the window with the given `Id` from the `windows` list.
    //
    // This is called when a window is either `Closed` or `Drop`ped.
    pub fn find_and_remove_window(&self, id: super::window::Id) {
        if let Ok(mut windows) = self.windows.lock() {
            windows.retain(|w| match w.upgrade() {
                Some(w) => w.id() != id,
                None => true,
            });
        }
    }

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

#[derive(Debug,Clone,Copy,Eq,PartialEq)]
enum Timeout {
    Now,
    Forever,
}

impl Timeout {
    fn is_elapsed(&self) -> bool {
        match self {
            &Timeout::Now => true,
            &Timeout::Forever => false,
        }
    }
}

impl EventsLoop {

    pub fn new() -> Self {
        EventsLoop {
            shared: Arc::new(Shared::new()),
            modifiers: Modifiers::new(),
            current_cocoa_event: None,
        }
    }

    // Attempt to get an Event by a specified timeout.
    fn get_event(&mut self, timeout: Timeout) -> Option<Event> {
        unsafe {
            if !msg_send![cocoa::base::class("NSThread"), isMainThread] {
                panic!("Events can only be polled from the main thread on macOS");
            }
        }

        loop {
            // Pop any queued events
            // This is immediate, so no need to consider a timeout
            if let Some(event) = self.shared.dequeue_event() {
                return Some(event);
            }

            // If we have no CocoaEvent, attempt to receive one
            // CocoaEvent::receive() respects the timeout
            if self.current_cocoa_event.is_none() {
                self.current_cocoa_event = CocoaEvent::receive(timeout);
            }

            // If we have a CocoaEvent, attempt to process it
            // TODO: plumb timeouts down to CocoaEvent::work()
            if let Some(mut current_event) = self.current_cocoa_event.take() {
                if current_event.work(self) == false {
                    // Event is not complete
                    // We must either process it further or store it again for later
                    if let Some(event) = self.shared.dequeue_event() {
                        // Another event landed while we were working this
                        // Store the CocoaEvent and return the Event from the queue
                        self.current_cocoa_event = Some(current_event);
                        return Some(event);

                    } else if timeout.is_elapsed() {
                        // Timeout is elapsed; we must return empty-handed
                        // Store the CocoaEvent and return nothing
                        self.current_cocoa_event = Some(current_event);
                        return None;

                    } else {
                        // We can repeat
                        continue;
                    }
                }

                // CocoaEvent processing is complete
                // Is it an event?
                if let CocoaEvent::Complete(Some(winit_event)) = current_event {
                    // Return it
                    return Some(winit_event);
                } else {
                    // CocoaEvent did not translate into an events::Event
                    // Loop around again
                }
            }
        }
    }

    pub fn poll_events<F>(&mut self, mut callback: F)
        where F: FnMut(Event),
    {
        // Return as many events as we can without blocking
        while let Some(event) = self.get_event(Timeout::Now) {
            callback(event);
        }
    }

    pub fn run_forever<F>(&mut self, mut callback: F)
        where F: FnMut(Event) -> ControlFlow
    {
        // Get events until we're told to stop
        while let Some(event) = self.get_event(Timeout::Forever) {
            // Send to the app
            let control_flow = callback(event);

            // Do what it says
            match control_flow {
                ControlFlow::Break => break,
                ControlFlow::Continue => (),
            }
        }
    }

    pub fn create_proxy(&self) -> Proxy {
        Proxy { shared: Arc::downgrade(&self.shared) }
    }
}

impl Proxy {
    pub fn wakeup(&self) -> Result<(), EventsLoopClosed> {
        if let Some(shared) = self.shared.upgrade() {
            shared.enqueue_event(Event::Awakened);
            Ok(())
        } else {
            Err(EventsLoopClosed)
        }
    }
}

struct RetainedEvent(cocoa::base::id);
impl RetainedEvent {
    fn new(event: cocoa::base::id) -> RetainedEvent {
        unsafe { CFRetain(event as CFTypeRef); }
        RetainedEvent(event)
    }
    fn into_inner(self) -> cocoa::base::id {
        self.0
    }
}
impl Drop for RetainedEvent {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0 as CFTypeRef); }
    }
}

// Encapsulates the lifecycle of a Cocoa event
enum CocoaEvent {
    // We just received this event, and haven't processed it yet
    Received(RetainedEvent),

    // We're trying to sending to the windowing system and haven't completed it yet
    Sending(SendEvent, RetainedEvent),

    // We delivered the message to the windowing system and possibly got back an events::Event
    Complete(Option<Event>),
}

impl CocoaEvent {
    fn receive(timeout: Timeout) -> Option<CocoaEvent> {
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

            // Wrap the result in a CocoaEvent
            let event = Self::new(ns_event);

            let _: () = msg_send![pool, release];

            return event
        }
    }

    fn new(ns_event: cocoa::base::id) -> Option<CocoaEvent> {
        // we (possibly) received `ns_event` from the windowing subsystem
        // is this an event?
        if ns_event == cocoa::base::nil {
            return None;
        }

        // FIXME: Despite not being documented anywhere, an `NSEvent` is produced when a user opens
        // Spotlight while the NSApplication is in focus. This `NSEvent` produces a `NSEventType`
        // with value `21`. This causes a SEGFAULT as soon as we try to match on the `NSEventType`
        // enum as there is no variant associated with the value. Thus, we return early if this
        // sneaky event occurs. If someone does find some documentation on this, please fix this by
        // adding an appropriate variant to the `NSEventType` enum in the cocoa-rs crate.
        if unsafe { ns_event.eventType() } as u64 == 21 {
            return None;
        }

        // good enough, let's dispatch
        Some(CocoaEvent::Received(RetainedEvent::new(ns_event)))
    }

    // Attempt to push a CocoaEvent towards a winit Event
    // Returns true on completion
    fn work(&mut self, events_loop: &mut EventsLoop) -> bool {
        // take ourselves and match on it
        let (new_event, is_complete) = match std::mem::replace(self, CocoaEvent::Complete(None)) {
            CocoaEvent::Received(retained_event) => {
                // Determine if we need to send this event
                // FIXME: Document this. Why do we do this? Seems like it passes on events to window/app.
                // If we don't do this, window does not become main for some reason.
                let needs_send = match unsafe { retained_event.0.eventType() } {
                    appkit::NSKeyDown => false,
                    _ => true,
                };

                if needs_send {
                    (CocoaEvent::Sending(SendEvent::new(retained_event.0), retained_event), false)
                } else {
                    (CocoaEvent::Complete(Self::to_event(events_loop, retained_event.into_inner())), true)
                }
            }

            CocoaEvent::Sending(send_event, retained_event) => {
                // Try to advance send event
                if let Some(new_send_event) = send_event.work() {
                    // Needs more time
                    (CocoaEvent::Sending(new_send_event, retained_event), false)
                } else {
                    // Done
                    (CocoaEvent::Complete(Self::to_event(events_loop, retained_event.into_inner())), true)
                }
            }

            CocoaEvent::Complete(event) => {
                // nothing to do
                (CocoaEvent::Complete(event), true)
            }
        };

        // replace ourselves with the result of the match
        std::mem::replace(self, new_event);

        // return the completion flag
        is_complete
    }

    fn to_event(events_loop: &mut EventsLoop, ns_event: cocoa::base::id) -> Option<Event> {
        unsafe {
            let event_type = ns_event.eventType();
            let ns_window = ns_event.window();
            let window_id = super::window::get_window_id(ns_window);

            let windows = events_loop.shared.windows.lock().unwrap();
            let maybe_window = windows.iter()
                .filter_map(Weak::upgrade)
                .find(|window| window_id == window.id());

            let into_event = |window_event| Event::WindowEvent {
                window_id: ::WindowId(window_id),
                event: window_event,
            };

            // Returns `Some` window if one of our windows is the key window.
            let maybe_key_window = || windows.iter()
                .filter_map(Weak::upgrade)
                .find(|window| {
                    let is_key_window: cocoa::base::BOOL = msg_send![*window.window, isKeyWindow];
                    is_key_window == cocoa::base::YES
                });

            match event_type {
                appkit::NSKeyDown => {
                    let mut events = std::collections::VecDeque::new();
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
                    for received_char in std::str::from_utf8(received_str.to_bytes()).unwrap().chars() {
                        let window_event = WindowEvent::ReceivedCharacter(received_char);
                        events.push_back(into_event(window_event));
                    }
                    events_loop.shared.pending_events.lock().unwrap().extend(events.into_iter());
                    Some(into_event(window_event))
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
                    Some(into_event(window_event))
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

                    let mut events = std::collections::VecDeque::new();
                    if let Some(window_event) = modifier_event(ns_event,
                                                               appkit::NSShiftKeyMask,
                                                               events::VirtualKeyCode::LShift,
                                                               events_loop.modifiers.shift_pressed)
                        {
                            events_loop.modifiers.shift_pressed = !events_loop.modifiers.shift_pressed;
                            events.push_back(into_event(window_event));
                        }

                    if let Some(window_event) = modifier_event(ns_event,
                                                               appkit::NSControlKeyMask,
                                                               events::VirtualKeyCode::LControl,
                                                               events_loop.modifiers.ctrl_pressed)
                        {
                            events_loop.modifiers.ctrl_pressed = !events_loop.modifiers.ctrl_pressed;
                            events.push_back(into_event(window_event));
                        }

                    if let Some(window_event) = modifier_event(ns_event,
                                                               appkit::NSCommandKeyMask,
                                                               events::VirtualKeyCode::LWin,
                                                               events_loop.modifiers.win_pressed)
                        {
                            events_loop.modifiers.win_pressed = !events_loop.modifiers.win_pressed;
                            events.push_back(into_event(window_event));
                        }

                    if let Some(window_event) = modifier_event(ns_event,
                                                               appkit::NSAlternateKeyMask,
                                                               events::VirtualKeyCode::LAlt,
                                                               events_loop.modifiers.alt_pressed)
                        {
                            events_loop.modifiers.alt_pressed = !events_loop.modifiers.alt_pressed;
                            events.push_back(into_event(window_event));
                        }

                    let event = events.pop_front();
                    events_loop.shared.pending_events.lock().unwrap().extend(events.into_iter());
                    event
                },

                appkit::NSLeftMouseDown => { Some(into_event(WindowEvent::MouseInput { device_id: DEVICE_ID, state: ElementState::Pressed, button: MouseButton::Left })) },
                appkit::NSLeftMouseUp => { Some(into_event(WindowEvent::MouseInput { device_id: DEVICE_ID, state: ElementState::Released, button: MouseButton::Left })) },
                appkit::NSRightMouseDown => { Some(into_event(WindowEvent::MouseInput { device_id: DEVICE_ID, state: ElementState::Pressed, button: MouseButton::Right })) },
                appkit::NSRightMouseUp => { Some(into_event(WindowEvent::MouseInput { device_id: DEVICE_ID, state: ElementState::Released, button: MouseButton::Right })) },
                appkit::NSOtherMouseDown => { Some(into_event(WindowEvent::MouseInput { device_id: DEVICE_ID, state: ElementState::Pressed, button: MouseButton::Middle })) },
                appkit::NSOtherMouseUp => { Some(into_event(WindowEvent::MouseInput { device_id: DEVICE_ID, state: ElementState::Released, button: MouseButton::Middle })) },

                appkit::NSMouseEntered => { Some(into_event(WindowEvent::MouseEntered { device_id: DEVICE_ID })) },
                appkit::NSMouseExited => { Some(into_event(WindowEvent::MouseLeft { device_id: DEVICE_ID })) },

                appkit::NSMouseMoved |
                appkit::NSLeftMouseDragged |
                appkit::NSOtherMouseDragged |
                appkit::NSRightMouseDragged => {
                    // If the mouse movement was on one of our windows, use it.
                    // Otherwise, if one of our windows is the key window (receiving input), use it.
                    // Otherwise, return `None`.
                    let window = match maybe_window.or_else(maybe_key_window) {
                        Some(window) => window,
                        None => return None,
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

                    let mut events = std::collections::VecDeque::new();

                    {
                        let x = (scale_factor * view_point.x as f32) as f64;
                        let y = (scale_factor * (view_rect.size.height - view_point.y) as f32) as f64;
                        let window_event = WindowEvent::MouseMoved { device_id: DEVICE_ID, position: (x, y) };
                        let event = Event::WindowEvent { window_id: ::WindowId(window.id()), event: window_event };
                        events.push_back(event);
                    }

                    let delta_x = (scale_factor * ns_event.deltaX() as f32) as f64;
                    if delta_x != 0.0 {
                        let motion_event = DeviceEvent::Motion { axis: 0, value: delta_x };
                        let event = Event::DeviceEvent{ device_id: DEVICE_ID, event: motion_event };
                        events.push_back(event);
                    }

                    let delta_y = (scale_factor * ns_event.deltaY() as f32) as f64;
                    if delta_y != 0.0 {
                        let motion_event = DeviceEvent::Motion { axis: 1, value: delta_y };
                        let event = Event::DeviceEvent{ device_id: DEVICE_ID, event: motion_event };
                        events.push_back(event);
                    }

                    let event = events.pop_front();
                    events.shared.pending_events.lock().unwrap().extend(events.into_iter());
                    event
                },

                appkit::NSScrollWheel => {
                    // If none of the windows received the scroll, return `None`.
                    let window = match maybe_window {
                        Some(window) => window,
                        None => return None,
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
                    Some(into_event(window_event))
                },

                appkit::NSEventTypePressure => {
                    let pressure = ns_event.pressure();
                    let stage = ns_event.stage();
                    let window_event = WindowEvent::TouchpadPressure { device_id: DEVICE_ID, pressure: pressure, stage: stage };
                    Some(into_event(window_event))
                },

                _ => None,
            }
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
