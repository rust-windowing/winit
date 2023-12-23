#![allow(clippy::unnecessary_cast)]

use icrate::AppKit::{
    NSEvent, NSEventModifierFlagCommand, NSEventTypeKeyUp, NSEventTypeLeftMouseDown,
    NSEventTypeLeftMouseDragged, NSEventTypeLeftMouseUp, NSEventTypeMouseMoved,
    NSEventTypeOtherMouseDown, NSEventTypeOtherMouseDragged, NSEventTypeOtherMouseUp,
    NSEventTypeRightMouseDown, NSEventTypeRightMouseDragged, NSEventTypeRightMouseUp,
};
use icrate::Foundation::NSObject;
use objc2::{declare_class, msg_send, mutability, ClassType, DeclaredClass};

use super::appkit::{NSApplication, NSResponder};
use super::event::flags_contains;
use super::{app_state::AppState, DEVICE_ID};
use crate::event::{DeviceEvent, ElementState, Event};

declare_class!(
    pub(super) struct WinitApplication;

    unsafe impl ClassType for WinitApplication {
        #[inherits(NSResponder, NSObject)]
        type Super = NSApplication;
        type Mutability = mutability::InteriorMutable;
        const NAME: &'static str = "WinitApplication";
    }

    impl DeclaredClass for WinitApplication {}

    unsafe impl WinitApplication {
        // Normally, holding Cmd + any key never sends us a `keyUp` event for that key.
        // Overriding `sendEvent:` like this fixes that. (https://stackoverflow.com/a/15294196)
        // Fun fact: Firefox still has this bug! (https://bugzilla.mozilla.org/show_bug.cgi?id=1299553)
        #[method(sendEvent:)]
        fn send_event(&self, event: &NSEvent) {
            // For posterity, there are some undocumented event types
            // (https://github.com/servo/cocoa-rs/issues/155)
            // but that doesn't really matter here.
            let event_type = unsafe { event.r#type() };
            let modifier_flags = unsafe { event.modifierFlags() };
            if event_type == NSEventTypeKeyUp
                && flags_contains(modifier_flags, NSEventModifierFlagCommand)
            {
                if let Some(key_window) = self.keyWindow() {
                    unsafe { key_window.sendEvent(event) };
                }
            } else {
                maybe_dispatch_device_event(event);
                unsafe { msg_send![super(self), sendEvent: event] }
            }
        }
    }
);

fn maybe_dispatch_device_event(event: &NSEvent) {
    let event_type = unsafe { event.r#type() };
    #[allow(non_upper_case_globals)]
    match event_type {
        NSEventTypeMouseMoved
        | NSEventTypeLeftMouseDragged
        | NSEventTypeOtherMouseDragged
        | NSEventTypeRightMouseDragged => {
            let delta_x = unsafe { event.deltaX() } as f64;
            let delta_y = unsafe { event.deltaY() } as f64;

            if delta_x != 0.0 {
                queue_device_event(DeviceEvent::Motion {
                    axis: 0,
                    value: delta_x,
                });
            }

            if delta_y != 0.0 {
                queue_device_event(DeviceEvent::Motion {
                    axis: 1,
                    value: delta_y,
                })
            }

            if delta_x != 0.0 || delta_y != 0.0 {
                queue_device_event(DeviceEvent::MouseMotion {
                    delta: (delta_x, delta_y),
                });
            }
        }
        NSEventTypeLeftMouseDown | NSEventTypeRightMouseDown | NSEventTypeOtherMouseDown => {
            queue_device_event(DeviceEvent::Button {
                button: unsafe { event.buttonNumber() } as u32,
                state: ElementState::Pressed,
            });
        }
        NSEventTypeLeftMouseUp | NSEventTypeRightMouseUp | NSEventTypeOtherMouseUp => {
            queue_device_event(DeviceEvent::Button {
                button: unsafe { event.buttonNumber() } as u32,
                state: ElementState::Released,
            });
        }
        _ => (),
    }
}

fn queue_device_event(event: DeviceEvent) {
    let event = Event::DeviceEvent {
        device_id: DEVICE_ID,
        event,
    };
    AppState::queue_event(event);
}
