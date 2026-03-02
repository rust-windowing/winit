#![allow(clippy::unnecessary_cast)]

use std::cell::Cell;
use std::rc::Rc;
use std::{mem, ptr};

use dispatch2::MainThreadBound;
use objc2::runtime::{Imp, Sel};
use objc2::sel;
use objc2_app_kit::{NSApplication, NSEvent, NSEventModifierFlags, NSEventType};
use objc2_foundation::MainThreadMarker;
use tracing::trace_span;
use winit_core::event::{DeviceEvent, ElementState};

use super::app_state::AppState;

type SendEvent = extern "C-unwind" fn(&NSApplication, Sel, &NSEvent);

static ORIGINAL: MainThreadBound<Cell<Option<SendEvent>>> = {
    // SAFETY: Creating in a `const` context, where there is no concept of the main thread.
    MainThreadBound::new(Cell::new(None), unsafe { MainThreadMarker::new_unchecked() })
};

extern "C-unwind" fn send_event(app: &NSApplication, sel: Sel, event: &NSEvent) {
    // Pass `RUST_LOG='trace,winit_appkit::app=warn'` if you want TRACE logs but not this.
    let _entered = trace_span!("sendEvent:", ?event).entered();
    let mtm = MainThreadMarker::from(app);

    // Normally, holding Cmd + any key never sends us a `keyUp` event for that key.
    // Overriding `sendEvent:` fixes that. (https://stackoverflow.com/a/15294196)
    // Fun fact: Firefox still has this bug! (https://bugzilla.mozilla.org/show_bug.cgi?id=1299553)
    //
    // For posterity, there are some undocumented event types
    // (https://github.com/servo/cocoa-rs/issues/155)
    // but that doesn't really matter here.
    let event_type = event.r#type();
    let modifier_flags = event.modifierFlags();
    if event_type == NSEventType::KeyUp && modifier_flags.contains(NSEventModifierFlags::Command) {
        if let Some(key_window) = app.keyWindow() {
            key_window.sendEvent(event);
        }
        return;
    }

    // Events are generally scoped to the window level, so the best way
    // to get device events is to listen for them on NSApplication.
    let app_state = AppState::get(mtm);
    maybe_dispatch_device_event(&app_state, event);

    let original = ORIGINAL.get(mtm).get().expect("no existing sendEvent: handler set");
    original(app, sel, event)
}

/// Intercept the [`sendEvent:`][NSApplication::sendEvent] method on the given application class.
///
/// The previous implementation created a subclass of [`NSApplication`], however we would like to
/// give the user full control over their `NSApplication`, so we override the method here using
/// method swizzling instead.
///
/// This _should_ also allow two versions of Winit to exist in the same application.
///
/// See the following links for more info on method swizzling:
/// - <https://nshipster.com/method-swizzling/>
/// - <https://spin.atomicobject.com/method-swizzling-objective-c/>
/// - <https://web.archive.org/web/20130308110627/http://cocoadev.com/wiki/MethodSwizzling>
///
/// NOTE: This function assumes that the passed in application object is the one returned from
/// [`NSApplication::sharedApplication`], i.e. the one and only global shared application object.
/// For testing though, we allow it to be a different object.
pub(crate) fn override_send_event(global_app: &NSApplication) {
    let mtm = MainThreadMarker::from(global_app);
    let class = global_app.class();

    let method =
        class.instance_method(sel!(sendEvent:)).expect("NSApplication must have sendEvent: method");

    // SAFETY: Converting our `sendEvent:` implementation to an IMP.
    let overridden = unsafe { mem::transmute::<SendEvent, Imp>(send_event) };

    // If we've already overridden the method, don't do anything.
    if ptr::fn_addr_eq(overridden, method.implementation()) {
        return;
    }

    // SAFETY: Our implementation has:
    // 1. The same signature as `sendEvent:`.
    // 2. Does not impose extra safety requirements on callers.
    let original = unsafe { method.set_implementation(overridden) };

    // SAFETY: This is the actual signature of `sendEvent:`.
    let original = unsafe { mem::transmute::<Imp, SendEvent>(original) };

    // NOTE: If NSApplication was safe to use from multiple threads, then this would potentially be
    // a (checked) race-condition, since one could call `sendEvent:` before the original had been
    // stored here.
    //
    // It is only usable from the main thread, however, so we're good!
    ORIGINAL.get(mtm).set(Some(original));
}

fn maybe_dispatch_device_event(app_state: &Rc<AppState>, event: &NSEvent) {
    let event_type = event.r#type();
    #[allow(non_upper_case_globals)]
    match event_type {
        NSEventType::MouseMoved
        | NSEventType::LeftMouseDragged
        | NSEventType::OtherMouseDragged
        | NSEventType::RightMouseDragged => {
            let delta_x = event.deltaX() as f64;
            let delta_y = event.deltaY() as f64;

            if delta_x != 0.0 || delta_y != 0.0 {
                app_state.maybe_queue_with_handler(move |app, event_loop| {
                    app.device_event(event_loop, None, DeviceEvent::PointerMotion {
                        delta: (delta_x, delta_y),
                    });
                });
            }
        },
        NSEventType::LeftMouseDown | NSEventType::RightMouseDown | NSEventType::OtherMouseDown => {
            let button = event.buttonNumber() as u32;
            app_state.maybe_queue_with_handler(move |app, event_loop| {
                app.device_event(event_loop, None, DeviceEvent::Button {
                    button,
                    state: ElementState::Pressed,
                });
            });
        },
        NSEventType::LeftMouseUp | NSEventType::RightMouseUp | NSEventType::OtherMouseUp => {
            let button = event.buttonNumber() as u32;
            app_state.maybe_queue_with_handler(move |app, event_loop| {
                app.device_event(event_loop, None, DeviceEvent::Button {
                    button,
                    state: ElementState::Released,
                });
            });
        },
        _ => (),
    }
}

#[cfg(test)]
mod tests {
    use objc2::rc::Retained;
    use objc2::{ClassType, define_class, msg_send};
    use objc2_app_kit::NSResponder;
    use objc2_foundation::NSObject;

    use super::*;

    #[test]
    fn test_override() {
        // FIXME(madsmtm): Ensure this always runs (maybe use cargo-nextest or `--test-threads=1`?)
        let Some(mtm) = MainThreadMarker::new() else { return };

        // Create a new application, without making it the shared application.
        let app = NSApplication::new(mtm);
        override_send_event(&app);
        // Test calling twice works.
        override_send_event(&app);

        // FIXME(madsmtm): Can't test this yet, need some way to mock AppState.
        // let event = super::super::event::dummy_event().unwrap();
        // app.sendEvent(&event)
    }

    #[test]
    fn test_custom_class() {
        let Some(_mtm) = MainThreadMarker::new() else { return };

        define_class!(
            #[unsafe(super(NSApplication, NSResponder, NSObject))]
            #[name = "TestApplication"]
            pub(super) struct TestApplication;

            impl TestApplication {
                #[unsafe(method(sendEvent:))]
                fn send_event(&self, _event: &NSEvent) {
                    todo!()
                }
            }
        );

        let app: Retained<TestApplication> = unsafe { msg_send![TestApplication::class(), new] };
        override_send_event(&app);
    }
}
