use super::{app_state::AppState, util};

use cocoa::{
    appkit::{NSApp, NSApplicationActivateIgnoringOtherApps},
    base::{id, nil},
    foundation::NSUInteger,
};
use objc::{
    declare::ClassDecl,
    runtime::{Class, Object, Sel, BOOL, NO, YES},
};

pub struct AppDelegateClass(pub *const Class);
unsafe impl Send for AppDelegateClass {}
unsafe impl Sync for AppDelegateClass {}

lazy_static! {
    pub static ref APP_DELEGATE_CLASS: AppDelegateClass = unsafe {
        let superclass = class!(NSResponder);
        let mut decl = ClassDecl::new("WinitAppDelegate", superclass).unwrap();

        decl.add_method(
            sel!(applicationDidFinishLaunching:),
            did_finish_launching as extern "C" fn(&Object, Sel, id) -> BOOL,
        );
        decl.add_method(
            sel!(applicationDidBecomeActive:),
            did_become_active as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(applicationWillResignActive:),
            will_resign_active as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(applicationWillEnterForeground:),
            will_enter_foreground as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(applicationDidEnterBackground:),
            did_enter_background as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(applicationWillTerminate:),
            will_terminate as extern "C" fn(&Object, Sel, id),
        );

        // Normally when you run or distribute a macOS app, it's bundled:
        // it's in one of those fun little folders that you have to right click
        // "Show Package Contents" on, and usually contains myriad delights
        // including, but not limited to, plists, icons, and of course, your
        // beloved executable. However, when you use `cargo run`, your app is
        // unbundled - it's just a lonely, bare executable.
        //
        // Apple isn't especially fond of unbundled apps, which is to say, they
        // seem to barely be supported. If you move the mouse while opening a
        // winit window from an unbundled app, the window will fail to activate
        // and be in a grayed-out uninteractable state. Switching from another
        // window and back to the winit window is the only way to get the winit
        // window into a normal state. None of this happens if the app is
        // bundled, i.e. when running via Xcode.
        //
        // Solution courtesy of https://github.com/godotengine/godot/pull/17187
        // (which appears to be based on https://stackoverflow.com/a/7602677)
        // To emphasize the curious specialness of mouse move events, the same
        // repro was found here:
        // https://github.com/godotengine/godot/issues/8653#issuecomment-358130512
        //
        // To fix this, we just switch focus to the Dock and then switch back
        // to our app. We only do this for unbundled apps. We omit the 2nd step
        // of the solution used in Godot, since it appears to have no effect -
        // I speculate that it's just technical debt picked up from the SO
        // answer; the API used is fairly exotic, and was historically used
        // (i.e. in SDL) in very old versions of macOS that didn't support
        // `activateIgnoringOtherApps`.
        //
        // Fun fact: this issue is still present in GLFW (https://github.com/glfw/glfw/issues/1515)
        //
        // A similar issue was found in SDL, but the resolution doesn't seem to
        // work for us: https://bugzilla.libsdl.org/show_bug.cgi?id=3051
        decl.add_method(
            sel!(unbundledAppActivationHackUnfocus:),
            unbundled_app_activation_hack_unfocus as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(unbundledAppActivationHackRefocus:),
            unbundled_app_activation_hack_refocus as extern "C" fn(&Object, Sel, id),
        );

        AppDelegateClass(decl.register())
    };
}

extern "C" fn did_finish_launching(this: &Object, _: Sel, _: id) -> BOOL {
    trace!("Triggered `didFinishLaunching`");
    unsafe {
        if let None = util::app_name() {
            // This app is unbundled, so we need to do some shenanigans for the
            // window to reliably activate correctly.
            let () = msg_send![
                this,
                performSelector: sel!(unbundledAppActivationHackUnfocus:)
                withObject: nil
                afterDelay: UNBUNDLED_APP_ACTIVATION_HACK_DELAY
            ];
        }
    }
    AppState::launched();
    trace!("Completed `didFinishLaunching`");
    YES
}

extern "C" fn did_become_active(_: &Object, _: Sel, _: id) {
    trace!("Triggered `didBecomeActive`");
    /*unsafe {
        HANDLER.lock().unwrap().handle_nonuser_event(Event::Resumed)
    }*/
    trace!("Completed `didBecomeActive`");
}

extern "C" fn will_resign_active(_: &Object, _: Sel, _: id) {
    trace!("Triggered `willResignActive`");
    /*unsafe {
        HANDLER.lock().unwrap().handle_nonuser_event(Event::Suspended)
    }*/
    trace!("Completed `willResignActive`");
}

extern "C" fn will_enter_foreground(_: &Object, _: Sel, _: id) {
    trace!("Triggered `willEnterForeground`");
    trace!("Completed `willEnterForeground`");
}

extern "C" fn did_enter_background(_: &Object, _: Sel, _: id) {
    trace!("Triggered `didEnterBackground`");
    trace!("Completed `didEnterBackground`");
}

extern "C" fn will_terminate(_: &Object, _: Sel, _: id) {
    trace!("Triggered `willTerminate`");
    /*unsafe {
        let app: id = msg_send![class!(UIApplication), sharedApplication];
        let windows: id = msg_send![app, windows];
        let windows_enum: id = msg_send![windows, objectEnumerator];
        let mut events = Vec::new();
        loop {
            let window: id = msg_send![windows_enum, nextObject];
            if window == nil {
                break
            }
            let is_winit_window: BOOL = msg_send![window, isKindOfClass:class!(WinitUIWindow)];
            if is_winit_window == YES {
                events.push(Event::WindowEvent {
                    window_id: RootWindowId(window.into()),
                    event: WindowEvent::Destroyed,
                });
            }
        }
        HANDLER.lock().unwrap().handle_nonuser_events(events);
        HANDLER.lock().unwrap().terminated();
    }*/
    trace!("Completed `willTerminate`");
}

// The Godot implementation doesn't explain why this delay was chosen, which is
// notably different from the (also unexplained) choice of 0.1 in the
// stackoverflow answer it's based on. However, from some cursory testing, this
// delay seems to be the fastest one that works reliably... when using 0.0, it
// would sometimes fail. Tragically, this is also why we have 2 separate
// methods, since the delay between them is necessary for the racey garbage
// we're doing.
const UNBUNDLED_APP_ACTIVATION_HACK_DELAY: std::os::raw::c_double = 0.02;

// First, we switch focus to the dock.
extern "C" fn unbundled_app_activation_hack_unfocus(this: &Object, _: Sel, _: id) {
    trace!("Triggered `unbundledAppActivationHackUnfocus`");
    unsafe {
        // We only perform the hack if the app failed to activate, since
        // otherwise, there'd be a gross (but fast) flicker as it unfocused and
        // then refocused. While you'd think it would be nicer to do this check
        // in `did_finish_launching`, it mysteriously doesn't work correctly if
        // you do.
        let active: BOOL = msg_send![NSApp(), isActive];
        info!(
            "Unbundled app detected as {}",
            if active == YES {
                "active; skipping activation hack"
            } else {
                "inactive; performing activation hack"
            }
        );
        if active == NO {
            let dock_bundle_id = util::ns_string_id_ref("com.apple.dock");
            let dock_array: id = msg_send![
                class!(NSRunningApplication),
                runningApplicationsWithBundleIdentifier: *dock_bundle_id
            ];
            let dock_array_len: NSUInteger = msg_send![dock_array, count];
            if dock_array_len == 0 {
                error!(
                    "The Dock doesn't seem to be running, so switching focus to it is impossible"
                );
            } else {
                let dock: id = msg_send![dock_array, objectAtIndex: 0];
                let status: BOOL = msg_send![
                    dock,
                    activateWithOptions: NSApplicationActivateIgnoringOtherApps
                ];
                if status == NO {
                    error!("Failed to switch focus to Dock");
                }
                let () = msg_send![
                    this,
                    performSelector: sel!(unbundledAppActivationHackRefocus:)
                    withObject: nil
                    afterDelay: UNBUNDLED_APP_ACTIVATION_HACK_DELAY
                ];
            }
        }
    }
    trace!("Completed `unbundledAppActivationHackUnfocus`");
}

// Then, we switch focus back to our window, and the user rejoices!
extern "C" fn unbundled_app_activation_hack_refocus(_: &Object, _: Sel, _: id) {
    trace!("Triggered `unbundledAppActivationHackRefocus`");
    unsafe {
        let app: id = msg_send![class!(NSRunningApplication), currentApplication];
        // Simply calling `activateIgnoringOtherApps` on `NSApp` doesn't work.
        // The nuanced difference isn't clear to me, but hey, at least I tried.
        let success: BOOL = msg_send![
            app,
            activateWithOptions: NSApplicationActivateIgnoringOtherApps
        ];
        if success == NO {
            error!("Failed to refocus app");
        }
    }
    trace!("Completed `unbundledAppActivationHackRefocus`");
}
