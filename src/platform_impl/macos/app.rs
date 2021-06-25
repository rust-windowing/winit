use std::collections::VecDeque;

use cocoa::{
    appkit::{self, NSEvent},
    base::id,
};
use objc::{
    declare::ClassDecl,
    runtime::{Class, Object, Sel},
};

use super::{app_state::AppState, event::EventWrapper, util, DEVICE_ID};
use crate::event::{DeviceEvent, ElementState, Event};

pub struct AppClass(pub *const Class);
unsafe impl Send for AppClass {}
unsafe impl Sync for AppClass {}

lazy_static! {
    pub static ref APP_CLASS: AppClass = unsafe {
        let superclass = class!(NSApplication);
        let mut decl = ClassDecl::new("WinitApp", superclass).unwrap();

        decl.add_method(
            sel!(sendEvent:),
            send_event as extern "C" fn(&Object, Sel, id),
        );

        AppClass(decl.register())
    };
}

// Normally, holding Cmd + any key never sends us a `keyUp` event for that key.
// Overriding `sendEvent:` like this fixes that. (https://stackoverflow.com/a/15294196)
// Fun fact: Firefox still has this bug! (https://bugzilla.mozilla.org/show_bug.cgi?id=1299553)
extern "C" fn send_event(this: &Object, _sel: Sel, event: id) {
    unsafe {
        // For posterity, there are some undocumented event types
        // (https://github.com/servo/cocoa-rs/issues/155)
        // but that doesn't really matter here.
        let event_type = event.eventType();
        let modifier_flags = event.modifierFlags();
        if event_type == appkit::NSKeyUp
            && util::has_flag(
                modifier_flags,
                appkit::NSEventModifierFlags::NSCommandKeyMask,
            )
        {
            let key_window: id = msg_send![this, keyWindow];
            let _: () = msg_send![key_window, sendEvent: event];
        } else {
            maybe_dispatch_device_event(event);
            let superclass = util::superclass(this);
            let _: () = msg_send![super(this, superclass), sendEvent: event];
        }
    }
}

unsafe fn maybe_dispatch_device_event(event: id) {
    let event_type = event.eventType();
    match event_type {
        appkit::NSMouseMoved
        | appkit::NSLeftMouseDragged
        | appkit::NSOtherMouseDragged
        | appkit::NSRightMouseDragged => {
            let mut events = VecDeque::with_capacity(3);

            let delta_x = event.deltaX() as f64;
            let delta_y = event.deltaY() as f64;

            if delta_x != 0.0 {
                events.push_back(EventWrapper::StaticEvent(Event::DeviceEvent {
                    device_id: DEVICE_ID,
                    event: DeviceEvent::Motion {
                        axis: 0,
                        value: delta_x,
                    },
                }));
            }

            if delta_y != 0.0 {
                events.push_back(EventWrapper::StaticEvent(Event::DeviceEvent {
                    device_id: DEVICE_ID,
                    event: DeviceEvent::Motion {
                        axis: 1,
                        value: delta_y,
                    },
                }));
            }

            if delta_x != 0.0 || delta_y != 0.0 {
                events.push_back(EventWrapper::StaticEvent(Event::DeviceEvent {
                    device_id: DEVICE_ID,
                    event: DeviceEvent::MouseMotion {
                        delta: (delta_x, delta_y),
                    },
                }));
            }

            AppState::queue_events(events);
        }
        appkit::NSLeftMouseDown | appkit::NSRightMouseDown | appkit::NSOtherMouseDown => {
            let mut events = VecDeque::with_capacity(1);

            events.push_back(EventWrapper::StaticEvent(Event::DeviceEvent {
                device_id: DEVICE_ID,
                event: DeviceEvent::Button {
                    button: event.buttonNumber() as u32,
                    state: ElementState::Pressed,
                },
            }));

            AppState::queue_events(events);
        }
        appkit::NSLeftMouseUp | appkit::NSRightMouseUp | appkit::NSOtherMouseUp => {
            let mut events = VecDeque::with_capacity(1);

            events.push_back(EventWrapper::StaticEvent(Event::DeviceEvent {
                device_id: DEVICE_ID,
                event: DeviceEvent::Button {
                    button: event.buttonNumber() as u32,
                    state: ElementState::Released,
                },
            }));

            AppState::queue_events(events);
        }
        _ => (),
    }
}
