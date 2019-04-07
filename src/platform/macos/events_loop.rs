use {ControlFlow, EventsLoopClosed};
use cocoa::{self, appkit, foundation};
use cocoa::appkit::{NSApplication, NSEvent, NSEventMask, NSEventModifierFlags, NSEventPhase, NSView, NSWindow};
use events::{self, ElementState, Event, TouchPhase, WindowEvent, DeviceEvent, ModifiersState, KeyboardInput};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, Weak};
use super::window::Window2;
use std;
use std::os::raw::*;
use super::DeviceId;

pub struct EventsLoop {
    modifiers: Modifiers,
    pub shared: Arc<Shared>,
}

// State shared between the `EventsLoop` and its registered windows.
pub struct Shared {
    pub windows: Mutex<Vec<Weak<Window2>>>,
    pub pending_events: Mutex<VecDeque<Event>>,
    // The user event callback given via either of the `poll_events` or `run_forever` methods.
    //
    // We store the user's callback here so that it may be accessed by each of the window delegate
    // callbacks (e.g. resize, close, etc) for the duration of a call to either of the
    // `poll_events` or `run_forever` methods.
    //
    // This is *only* `Some` for the duration of a call to either of these methods and will be
    // `None` otherwise.
    user_callback: UserCallback,
}

#[derive(Clone)]
pub struct Proxy {}

struct Modifiers {
    shift_pressed: bool,
    ctrl_pressed: bool,
    win_pressed: bool,
    alt_pressed: bool,
}

// Wrapping the user callback in a type allows us to:
//
// - ensure the callback pointer is never accidentally cloned
// - ensure that only the `EventsLoop` can `store` and `drop` the callback pointer
// - Share access to the user callback with the NSWindow callbacks.
pub struct UserCallback {
    mutex: Mutex<Option<*mut FnMut(Event)>>,
}


impl Shared {

    pub fn new() -> Self {
        Shared {
            windows: Mutex::new(Vec::new()),
            pending_events: Mutex::new(VecDeque::new()),
            user_callback: UserCallback { mutex: Mutex::new(None) },
        }
    }

    fn call_user_callback_with_pending_events(&self) {
        loop {
            let event = match self.pending_events.lock().unwrap().pop_front() {
                Some(event) => event,
                None => return,
            };
            unsafe {
                self.user_callback.call_with_event(event);
            }
        }
    }

    // Calls the user callback if one exists.
    //
    // Otherwise, stores the event in the `pending_events` queue.
    //
    // This is necessary for the case when `WindowDelegate` callbacks are triggered during a call
    // to the user's callback.
    pub fn call_user_callback_with_event_or_store_in_pending(&self, event: Event) {
        if self.user_callback.mutex.lock().unwrap().is_some() {
            unsafe {
                self.user_callback.call_with_event(event);
            }
        } else {
            self.pending_events.lock().unwrap().push_back(event);
        }
    }

    // Removes the window with the given `Id` from the `windows` list.
    //
    // This is called in response to `windowWillClose`.
    pub fn find_and_remove_window(&self, id: super::window::Id) {
        if let Ok(mut windows) = self.windows.lock() {
            windows.retain(|w| match w.upgrade() {
                Some(w) => w.id() != id,
                None => false,
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


impl UserCallback {

    // Here we store user's `callback` behind the mutex so that they may be safely shared between
    // each of the window delegates.
    //
    // In order to make sure that the pointer is always valid, we must manually guarantee that it
    // is dropped before the callback itself is dropped. Thus, this should *only* be called at the
    // beginning of a call to `poll_events` and `run_forever`, both of which *must* drop the
    // callback at the end of their scope using the `drop` method.
    fn store<F>(&self, callback: &mut F)
        where F: FnMut(Event)
    {
        let trait_object = callback as &mut FnMut(Event);
        let trait_object_ptr = trait_object as *const FnMut(Event) as *mut FnMut(Event);
        *self.mutex.lock().unwrap() = Some(trait_object_ptr);
    }

    // Emits the given event via the user-given callback.
    //
    // This is unsafe as it requires dereferencing the pointer to the user-given callback. We
    // guarantee this is safe by ensuring the `UserCallback` never lives longer than the user-given
    // callback.
    //
    // Note that the callback may not always be `Some`. This is because some `NSWindowDelegate`
    // callbacks can be triggered by means other than `NSApp().sendEvent`. For example, if a window
    // is destroyed or created during a call to the user's callback, the `WindowDelegate` methods
    // may be called with `windowShouldClose` or `windowDidResignKey`.
    unsafe fn call_with_event(&self, event: Event) {
        let callback = match self.mutex.lock().unwrap().take() {
            Some(callback) => callback,
            None => return,
        };
        (*callback)(event);
        *self.mutex.lock().unwrap() = Some(callback);
    }

    // Used to drop the user callback pointer at the end of the `poll_events` and `run_forever`
    // methods. This is done to enforce our guarantee that the top callback will never live longer
    // than the call to either `poll_events` or `run_forever` to which it was given.
    fn drop(&self) {
        self.mutex.lock().unwrap().take();
    }

}


impl EventsLoop {

    pub fn new() -> Self {
        // Mark this thread as the main thread of the Cocoa event system.
        //
        // This must be done before any worker threads get a chance to call it
        // (e.g., via `EventsLoopProxy::wakeup()`), causing a wrong thread to be
        // marked as the main thread.
        unsafe { appkit::NSApp(); }

        EventsLoop {
            shared: Arc::new(Shared::new()),
            modifiers: Modifiers::new(),
        }
    }

    pub fn poll_events<F>(&mut self, mut callback: F)
        where F: FnMut(Event),
    {
        unsafe {
            if !msg_send![class!(NSThread), isMainThread] {
                panic!("Events can only be polled from the main thread on macOS");
            }
        }

        self.shared.user_callback.store(&mut callback);

        // Loop as long as we have pending events to return.
        loop {
            unsafe {
                // First, yield all pending events.
                self.shared.call_user_callback_with_pending_events();

                let pool = foundation::NSAutoreleasePool::new(cocoa::base::nil);

                // Poll for the next event, returning `nil` if there are none.
                let ns_event = appkit::NSApp().nextEventMatchingMask_untilDate_inMode_dequeue_(
                    NSEventMask::NSAnyEventMask.bits() | NSEventMask::NSEventMaskPressure.bits(),
                    foundation::NSDate::distantPast(cocoa::base::nil),
                    foundation::NSDefaultRunLoopMode,
                    cocoa::base::YES);

                let event = self.ns_event_to_event(ns_event);

                let _: () = msg_send![pool, release];

                match event {
                    // Call the user's callback.
                    Some(event) => self.shared.user_callback.call_with_event(event),
                    None => break,
                }
            }
        }

        self.shared.user_callback.drop();
    }

    pub fn run_forever<F>(&mut self, mut callback: F)
        where F: FnMut(Event) -> ControlFlow
    {
        unsafe {
            if !msg_send![class!(NSThread), isMainThread] {
                panic!("Events can only be polled from the main thread on macOS");
            }
        }

        // Track whether or not control flow has changed.
        let control_flow = std::cell::Cell::new(ControlFlow::Continue);

        let mut callback = |event| {
            if let ControlFlow::Break = callback(event) {
                control_flow.set(ControlFlow::Break);
            }
        };

        self.shared.user_callback.store(&mut callback);

        loop {
            unsafe {
                // First, yield all pending events.
                self.shared.call_user_callback_with_pending_events();
                if let ControlFlow::Break = control_flow.get() {
                    break;
                }

                let pool = foundation::NSAutoreleasePool::new(cocoa::base::nil);

                // Wait for the next event. Note that this function blocks during resize.
                let ns_event = appkit::NSApp().nextEventMatchingMask_untilDate_inMode_dequeue_(
                    NSEventMask::NSAnyEventMask.bits() | NSEventMask::NSEventMaskPressure.bits(),
                    foundation::NSDate::distantFuture(cocoa::base::nil),
                    foundation::NSDefaultRunLoopMode,
                    cocoa::base::YES);

                let maybe_event = self.ns_event_to_event(ns_event);

                // Release the pool before calling the top callback in case the user calls either
                // `run_forever` or `poll_events` within the callback.
                let _: () = msg_send![pool, release];

                if let Some(event) = maybe_event {
                    self.shared.user_callback.call_with_event(event);
                    if let ControlFlow::Break = control_flow.get() {
                        break;
                    }
                }
            }
        }

        self.shared.user_callback.drop();
    }

    // Convert some given `NSEvent` into a winit `Event`.
    unsafe fn ns_event_to_event(&mut self, ns_event: cocoa::base::id) -> Option<Event> {
        if ns_event == cocoa::base::nil {
            return None;
        }

        // FIXME: Despite not being documented anywhere, an `NSEvent` is produced when a user opens
        // Spotlight while the NSApplication is in focus. This `NSEvent` produces a `NSEventType`
        // with value `21`. This causes a SEGFAULT as soon as we try to match on the `NSEventType`
        // enum as there is no variant associated with the value. Thus, we return early if this
        // sneaky event occurs. If someone does find some documentation on this, please fix this by
        // adding an appropriate variant to the `NSEventType` enum in the cocoa-rs crate.
        if ns_event.eventType() as u64 == 21 {
            return None;
        }

        let event_type = ns_event.eventType();
        let ns_window = ns_event.window();
        let window_id = super::window::get_window_id(ns_window);

        // FIXME: Document this. Why do we do this? Seems like it passes on events to window/app.
        // If we don't do this, window does not become main for some reason.
        appkit::NSApp().sendEvent_(ns_event);

        let windows = self.shared.windows.lock().unwrap();
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
            // https://github.com/glfw/glfw/blob/50eccd298a2bbc272b4977bd162d3e4b55f15394/src/cocoa_window.m#L881
            appkit::NSKeyUp  => {
                if let Some(key_window) = maybe_key_window() {
                    if event_mods(ns_event).logo {
                        let _: () = msg_send![*key_window.window, sendEvent:ns_event];
                    }
                }
                None
            },
            // similar to above, but for `<Cmd-.>`, the keyDown is suppressed instead of the
            // KeyUp, and the above trick does not appear to work.
            appkit::NSKeyDown => {
                let modifiers = event_mods(ns_event);
                let keycode = NSEvent::keyCode(ns_event);
                if modifiers.logo && keycode == 47 {
                    modifier_event(ns_event, NSEventModifierFlags::NSCommandKeyMask, false)
                        .map(into_event)
                } else {
                    None
                }
            },
            appkit::NSFlagsChanged => {
                let mut events = std::collections::VecDeque::new();

                if let Some(window_event) = modifier_event(
                    ns_event,
                    NSEventModifierFlags::NSShiftKeyMask,
                    self.modifiers.shift_pressed,
                ) {
                    self.modifiers.shift_pressed = !self.modifiers.shift_pressed;
                    events.push_back(into_event(window_event));
                }

                if let Some(window_event) = modifier_event(
                    ns_event,
                    NSEventModifierFlags::NSControlKeyMask,
                    self.modifiers.ctrl_pressed,
                ) {
                    self.modifiers.ctrl_pressed = !self.modifiers.ctrl_pressed;
                    events.push_back(into_event(window_event));
                }

                if let Some(window_event) = modifier_event(
                    ns_event,
                    NSEventModifierFlags::NSCommandKeyMask,
                    self.modifiers.win_pressed,
                ) {
                    self.modifiers.win_pressed = !self.modifiers.win_pressed;
                    events.push_back(into_event(window_event));
                }

                if let Some(window_event) = modifier_event(
                    ns_event,
                    NSEventModifierFlags::NSAlternateKeyMask,
                    self.modifiers.alt_pressed,
                ) {
                    self.modifiers.alt_pressed = !self.modifiers.alt_pressed;
                    events.push_back(into_event(window_event));
                }

                let event = events.pop_front();
                self.shared.pending_events
                    .lock()
                    .unwrap()
                    .extend(events.into_iter());
                event
            },

            appkit::NSMouseEntered => {
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
                let x = view_point.x as f64;
                let y = (view_rect.size.height - view_point.y) as f64;
                let window_event = WindowEvent::CursorMoved {
                    device_id: DEVICE_ID,
                    position: (x, y).into(),
                    modifiers: event_mods(ns_event),
                };
                let event = Event::WindowEvent { window_id: ::WindowId(window.id()), event: window_event };
                self.shared.pending_events.lock().unwrap().push_back(event);
                Some(into_event(WindowEvent::CursorEntered { device_id: DEVICE_ID }))
            },
            appkit::NSMouseExited => { Some(into_event(WindowEvent::CursorLeft { device_id: DEVICE_ID })) },

            appkit::NSMouseMoved |
            appkit::NSLeftMouseDragged |
            appkit::NSOtherMouseDragged |
            appkit::NSRightMouseDragged => {
                // If the mouse movement was on one of our windows, use it.
                // Otherwise, if one of our windows is the key window (receiving input), use it.
                // Otherwise, return `None`.
                match maybe_window.or_else(maybe_key_window) {
                    Some(_window) => (),
                    None => return None,
                }

                let mut events = std::collections::VecDeque::with_capacity(3);

                let delta_x = ns_event.deltaX() as f64;
                if delta_x != 0.0 {
                    let motion_event = DeviceEvent::Motion { axis: 0, value: delta_x };
                    let event = Event::DeviceEvent { device_id: DEVICE_ID, event: motion_event };
                    events.push_back(event);
                }

                let delta_y = ns_event.deltaY() as f64;
                if delta_y != 0.0 {
                    let motion_event = DeviceEvent::Motion { axis: 1, value: delta_y };
                    let event = Event::DeviceEvent { device_id: DEVICE_ID, event: motion_event };
                    events.push_back(event);
                }

                if delta_x != 0.0 || delta_y != 0.0 {
                    let motion_event = DeviceEvent::MouseMotion { delta: (delta_x, delta_y) };
                    let event = Event::DeviceEvent { device_id: DEVICE_ID, event: motion_event };
                    events.push_back(event);
                }

                let event = events.pop_front();
                self.shared.pending_events.lock().unwrap().extend(events.into_iter());
                event
            },

            appkit::NSScrollWheel => {
                // If none of the windows received the scroll, return `None`.
                if maybe_window.is_none() {
                    return None;
                }

                use events::MouseScrollDelta::{LineDelta, PixelDelta};
                let delta = if ns_event.hasPreciseScrollingDeltas() == cocoa::base::YES {
                    PixelDelta((
                        ns_event.scrollingDeltaX() as f64,
                        ns_event.scrollingDeltaY() as f64,
                    ).into())
                } else {
                    // TODO: This is probably wrong
                    LineDelta(
                        ns_event.scrollingDeltaX() as f32,
                        ns_event.scrollingDeltaY() as f32,
                    )
                };
                let phase = match ns_event.phase() {
                    NSEventPhase::NSEventPhaseMayBegin | NSEventPhase::NSEventPhaseBegan => TouchPhase::Started,
                    NSEventPhase::NSEventPhaseEnded => TouchPhase::Ended,
                    _ => TouchPhase::Moved,
                };
                self.shared.pending_events.lock().unwrap().push_back(Event::DeviceEvent {
                    device_id: DEVICE_ID,
                    event: DeviceEvent::MouseWheel {
                        delta: if ns_event.hasPreciseScrollingDeltas() == cocoa::base::YES {
                            PixelDelta((
                                ns_event.scrollingDeltaX() as f64,
                                ns_event.scrollingDeltaY() as f64,
                            ).into())
                        } else {
                            LineDelta(
                                ns_event.scrollingDeltaX() as f32,
                                ns_event.scrollingDeltaY() as f32,
                            )
                        },
                    }
                });
                let window_event = WindowEvent::MouseWheel { device_id: DEVICE_ID, delta: delta, phase: phase, modifiers: event_mods(ns_event) };
                Some(into_event(window_event))
            },

            appkit::NSEventTypePressure => {
                let pressure = ns_event.pressure();
                let stage = ns_event.stage();
                let window_event = WindowEvent::TouchpadPressure { device_id: DEVICE_ID, pressure: pressure, stage: stage };
                Some(into_event(window_event))
            },

            appkit::NSApplicationDefined => match ns_event.subtype() {
                appkit::NSEventSubtype::NSApplicationActivatedEventType => {
                    Some(Event::Awakened)
                },
                _ => None,
            },

            _  => None,
        }
    }

    pub fn create_proxy(&self) -> Proxy {
        Proxy {}
    }

}

impl Proxy {
    pub fn wakeup(&self) -> Result<(), EventsLoopClosed> {
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
        Ok(())
    }
}

pub fn char_to_keycode(c: char) -> Option<events::VirtualKeyCode> {
    // We only translate keys that are affected by keyboard layout.
    //
    // Note that since keys are translated in a somewhat "dumb" way (reading character)
    // there is a concern that some combination, i.e. Cmd+char, causes the wrong
    // letter to be received, and so we receive the wrong key.
    //
    // Implementation reference: https://github.com/WebKit/webkit/blob/82bae82cf0f329dbe21059ef0986c4e92fea4ba6/Source/WebCore/platform/cocoa/KeyEventCocoa.mm#L626
    Some(match c {
        'a' | 'A' => events::VirtualKeyCode::A,
        'b' | 'B' => events::VirtualKeyCode::B,
        'c' | 'C' => events::VirtualKeyCode::C,
        'd' | 'D' => events::VirtualKeyCode::D,
        'e' | 'E' => events::VirtualKeyCode::E,
        'f' | 'F' => events::VirtualKeyCode::F,
        'g' | 'G' => events::VirtualKeyCode::G,
        'h' | 'H' => events::VirtualKeyCode::H,
        'i' | 'I' => events::VirtualKeyCode::I,
        'j' | 'J' => events::VirtualKeyCode::J,
        'k' | 'K' => events::VirtualKeyCode::K,
        'l' | 'L' => events::VirtualKeyCode::L,
        'm' | 'M' => events::VirtualKeyCode::M,
        'n' | 'N' => events::VirtualKeyCode::N,
        'o' | 'O' => events::VirtualKeyCode::O,
        'p' | 'P' => events::VirtualKeyCode::P,
        'q' | 'Q' => events::VirtualKeyCode::Q,
        'r' | 'R' => events::VirtualKeyCode::R,
        's' | 'S' => events::VirtualKeyCode::S,
        't' | 'T' => events::VirtualKeyCode::T,
        'u' | 'U' => events::VirtualKeyCode::U,
        'v' | 'V' => events::VirtualKeyCode::V,
        'w' | 'W' => events::VirtualKeyCode::W,
        'x' | 'X' => events::VirtualKeyCode::X,
        'y' | 'Y' => events::VirtualKeyCode::Y,
        'z' | 'Z' => events::VirtualKeyCode::Z,
        '1' | '!' => events::VirtualKeyCode::Key1,
        '2' | '@' => events::VirtualKeyCode::Key2,
        '3' | '#' => events::VirtualKeyCode::Key3,
        '4' | '$' => events::VirtualKeyCode::Key4,
        '5' | '%' => events::VirtualKeyCode::Key5,
        '6' | '^' => events::VirtualKeyCode::Key6,
        '7' | '&' => events::VirtualKeyCode::Key7,
        '8' | '*' => events::VirtualKeyCode::Key8,
        '9' | '(' => events::VirtualKeyCode::Key9,
        '0' | ')' => events::VirtualKeyCode::Key0,
        '=' | '+' => events::VirtualKeyCode::Equals,
        '-' | '_' => events::VirtualKeyCode::Minus,
        ']' | '}' => events::VirtualKeyCode::RBracket,
        '[' | '{' => events::VirtualKeyCode::LBracket,
        '\''| '"' => events::VirtualKeyCode::Apostrophe,
        ';' | ':' => events::VirtualKeyCode::Semicolon,
        '\\'| '|' => events::VirtualKeyCode::Backslash,
        ',' | '<' => events::VirtualKeyCode::Comma,
        '/' | '?' => events::VirtualKeyCode::Slash,
        '.' | '>' => events::VirtualKeyCode::Period,
        '`' | '~' => events::VirtualKeyCode::Grave,
        _ => return None,
    })
}

pub fn scancode_to_keycode(code: c_ushort) -> Option<events::VirtualKeyCode> {
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
        0x3a => events::VirtualKeyCode::LAlt,
        0x3b => events::VirtualKeyCode::LControl,
        0x3c => events::VirtualKeyCode::RShift,
        0x3d => events::VirtualKeyCode::RAlt,
        0x3e => events::VirtualKeyCode::RControl,
        //0x3f => Fn key,
        0x40 => events::VirtualKeyCode::F17,
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
        0x4e => events::VirtualKeyCode::Subtract,
        //0x4d => unkown,
        0x4e => events::VirtualKeyCode::Subtract,
        0x4f => events::VirtualKeyCode::F18,
        0x50 => events::VirtualKeyCode::F19,
        0x51 => events::VirtualKeyCode::NumpadEquals,
        0x52 => events::VirtualKeyCode::Numpad0,
        0x53 => events::VirtualKeyCode::Numpad1,
        0x54 => events::VirtualKeyCode::Numpad2,
        0x55 => events::VirtualKeyCode::Numpad3,
        0x56 => events::VirtualKeyCode::Numpad4,
        0x57 => events::VirtualKeyCode::Numpad5,
        0x58 => events::VirtualKeyCode::Numpad6,
        0x59 => events::VirtualKeyCode::Numpad7,
        0x5a => events::VirtualKeyCode::F20,
        0x5b => events::VirtualKeyCode::Numpad8,
        0x5c => events::VirtualKeyCode::Numpad9,
        0x5d => events::VirtualKeyCode::Yen,
        //0x5e => JIS Ro,
        //0x5f => unkown,
        0x60 => events::VirtualKeyCode::F5,
        0x61 => events::VirtualKeyCode::F6,
        0x62 => events::VirtualKeyCode::F7,
        0x63 => events::VirtualKeyCode::F3,
        0x64 => events::VirtualKeyCode::F8,
        0x65 => events::VirtualKeyCode::F9,
        //0x66 => JIS Eisuu (macOS),
        0x67 => events::VirtualKeyCode::F11,
        //0x68 => JIS Kana (macOS),
        0x69 => events::VirtualKeyCode::F13,
        0x6a => events::VirtualKeyCode::F16,
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

        0xa => events::VirtualKeyCode::Caret,
        _ => return None,
    })
}

pub fn check_function_keys(
    s: &String
) -> Option<events::VirtualKeyCode> {
    if let Some(ch) = s.encode_utf16().next() {
        return Some(match ch {
            0xf718 => events::VirtualKeyCode::F21,
            0xf719 => events::VirtualKeyCode::F22,
            0xf71a => events::VirtualKeyCode::F23,
            0xf71b => events::VirtualKeyCode::F24,
            _ => return None,
        })
    }

    None
}

pub fn event_mods(event: cocoa::base::id) -> ModifiersState {
    let flags = unsafe {
        NSEvent::modifierFlags(event)
    };
    ModifiersState {
        shift: flags.contains(NSEventModifierFlags::NSShiftKeyMask),
        ctrl: flags.contains(NSEventModifierFlags::NSControlKeyMask),
        alt: flags.contains(NSEventModifierFlags::NSAlternateKeyMask),
        logo: flags.contains(NSEventModifierFlags::NSCommandKeyMask),
    }
}

pub fn get_scancode(event: cocoa::base::id) -> c_ushort {
    // In AppKit, `keyCode` refers to the position (scancode) of a key rather than its character,
    // and there is no easy way to navtively retrieve the layout-dependent character.
    // In winit, we use keycode to refer to the key's character, and so this function aligns
    // AppKit's terminology with ours.
    unsafe {
        msg_send![event, keyCode]
    }
}

unsafe fn modifier_event(
    ns_event: cocoa::base::id,
    keymask: NSEventModifierFlags,
    was_key_pressed: bool,
) -> Option<WindowEvent> {
    if !was_key_pressed && NSEvent::modifierFlags(ns_event).contains(keymask)
    || was_key_pressed && !NSEvent::modifierFlags(ns_event).contains(keymask) {
        let state = if was_key_pressed {
            ElementState::Released
        } else {
            ElementState::Pressed
        };

        let scancode = get_scancode(ns_event);
        let virtual_keycode = scancode_to_keycode(scancode);
        Some(WindowEvent::KeyboardInput {
            device_id: DEVICE_ID,
            input: KeyboardInput {
                state,
                scancode: scancode as u32,
                virtual_keycode,
                modifiers: event_mods(ns_event),
            },
        })
    } else {
        None
    }
}

// Constant device ID, to be removed when this backend is updated to report real device IDs.
pub const DEVICE_ID: ::DeviceId = ::DeviceId(DeviceId);
