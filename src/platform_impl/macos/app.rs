#![allow(clippy::unnecessary_cast)]
#![allow(unknown_lints)] // New lint below
#![allow(static_mut_refs)] // Uses `MainThreadBound` in new version.

use std::cell::Cell;
use std::mem;

use objc2::runtime::{Imp, Sel};
use objc2::sel;
use objc2_app_kit::{NSApplication, NSEvent, NSEventModifierFlags, NSEventType};
use objc2_foundation::MainThreadMarker;

use super::app_state::ApplicationDelegate;
use crate::event::{DeviceEvent, ElementState};

type SendEvent = extern "C" fn(&NSApplication, Sel, &NSEvent);

// NOTE: Only used on the main thread. Ideally, we'd use `MainThreadBound`, but that isn't
// constructible from `const` with this `objc2` version.
static mut ORIGINAL: Cell<Option<SendEvent>> = Cell::new(None);

extern "C" fn send_event(app: &NSApplication, sel: Sel, event: &NSEvent) {
    let mtm = MainThreadMarker::from(app);

    // Normally, holding Cmd + any key never sends us a `keyUp` event for that key.
    // Overriding `sendEvent:` fixes that. (https://stackoverflow.com/a/15294196)
    // Fun fact: Firefox still has this bug! (https://bugzilla.mozilla.org/show_bug.cgi?id=1299553)
    //
    // For posterity, there are some undocumented event types
    // (https://github.com/servo/cocoa-rs/issues/155)
    // but that doesn't really matter here.
    let event_type = unsafe { event.r#type() };
    let modifier_flags = unsafe { event.modifierFlags() };
    if event_type == NSEventType::KeyUp
        && modifier_flags.contains(NSEventModifierFlags::NSEventModifierFlagCommand)
    {
        if let Some(key_window) = app.keyWindow() {
            key_window.sendEvent(event);
        }
        return;
    }

    // Events are generally scoped to the window level, so the best way
    // to get device events is to listen for them on NSApplication.
    let delegate = ApplicationDelegate::get(mtm);
    maybe_dispatch_device_event(&delegate, event);

    let _ = mtm;
    let original = unsafe { ORIGINAL.get().expect("no existing sendEvent: handler set") };
    original(app, sel, event)
}

/// Override the [`sendEvent:`][NSApplication::sendEvent] method on the given application class.
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
    // FIXME(madsmtm): Use `std::ptr::fn_addr_eq` (Rust 1.85) once available in MSRV.
    #[allow(unknown_lints, unpredictable_function_pointer_comparisons)]
    if overridden == method.implementation() {
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
    let _ = mtm;
    unsafe { ORIGINAL.set(Some(original)) };
}

fn maybe_dispatch_device_event(delegate: &ApplicationDelegate, event: &NSEvent) {
    let event_type = unsafe { event.r#type() };
    #[allow(non_upper_case_globals)]
    match event_type {
        NSEventType::MouseMoved
        | NSEventType::LeftMouseDragged
        | NSEventType::OtherMouseDragged
        | NSEventType::RightMouseDragged => {
            let delta_x = unsafe { event.deltaX() } as f64;
            let delta_y = unsafe { event.deltaY() } as f64;

            if delta_x != 0.0 {
                delegate.maybe_queue_device_event(DeviceEvent::Motion { axis: 0, value: delta_x });
            }

            if delta_y != 0.0 {
                delegate.maybe_queue_device_event(DeviceEvent::Motion { axis: 1, value: delta_y })
            }

            if delta_x != 0.0 || delta_y != 0.0 {
                delegate.maybe_queue_device_event(DeviceEvent::MouseMotion {
                    delta: (delta_x, delta_y),
                });
            }
        },
        NSEventType::LeftMouseDown | NSEventType::RightMouseDown | NSEventType::OtherMouseDown => {
            delegate.maybe_queue_device_event(DeviceEvent::Button {
                button: unsafe { event.buttonNumber() } as u32,
                state: ElementState::Pressed,
            });
        },
        NSEventType::LeftMouseUp | NSEventType::RightMouseUp | NSEventType::OtherMouseUp => {
            delegate.maybe_queue_device_event(DeviceEvent::Button {
                button: unsafe { event.buttonNumber() } as u32,
                state: ElementState::Released,
            });
        },
        _ => (),
    }
}

#[cfg(test)]
mod tests {
    use objc2::rc::Retained;
    use objc2::{declare_class, msg_send_id, mutability, ClassType, DeclaredClass};

    use super::*;

    #[test]
    fn test_override() {
        // FIXME(madsmtm): Ensure this always runs (maybe use cargo-nextest or `--test-threads=1`?)
        let Some(mtm) = MainThreadMarker::new() else { return };

        // Create a new application, without making it the shared application.
        let app = unsafe { NSApplication::new(mtm) };
        override_send_event(&app);
        // Test calling twice works.
        override_send_event(&app);

        // FIXME(madsmtm): Can't test this yet, need some way to mock AppState.
        // unsafe {
        //     let event = super::super::event::dummy_event().unwrap();
        //     app.sendEvent(&event)
        // }
    }

    #[test]
    fn test_custom_class() {
        let Some(_mtm) = MainThreadMarker::new() else { return };

        declare_class!(
            struct TestApplication;

            unsafe impl ClassType for TestApplication {
                type Super = NSApplication;
                type Mutability = mutability::MainThreadOnly;
                const NAME: &'static str = "TestApplication";
            }

            impl DeclaredClass for TestApplication {}

            unsafe impl TestApplication {
                #[method(sendEvent:)]
                fn send_event(&self, _event: &NSEvent) {
                    todo!()
                }
            }
        );

        let app: Retained<TestApplication> = unsafe { msg_send_id![TestApplication::class(), new] };
        override_send_event(&app);
    }
}
