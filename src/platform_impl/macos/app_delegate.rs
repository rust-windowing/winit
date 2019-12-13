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
use std::{
    os::raw::c_void,
    sync::atomic::{AtomicBool, Ordering},
};

pub struct AppDelegateClass(pub *const Class);
unsafe impl Send for AppDelegateClass {}
unsafe impl Sync for AppDelegateClass {}

lazy_static! {
    pub static ref APP_DELEGATE_CLASS: AppDelegateClass = unsafe {
        let superclass = class!(NSResponder);
        let mut decl = ClassDecl::new("WinitAppDelegate", superclass).unwrap();

        decl.add_class_method(
            sel!(new),
            new as extern "C" fn(&Class, Sel) -> id,
        );
        decl.add_method(sel!(dealloc), dealloc as extern "C" fn(&Object, Sel));
        decl.add_method(
            sel!(applicationDidFinishLaunching:),
            did_finish_launching as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(applicationDidResignActive:),
            did_resign_active as extern "C" fn(&mut Object, Sel, id),
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
        // and be in a grayed-out uninteractable state. Switching to another
        // app and back is the only way to get the winit window into a normal
        // state. None of this happens if the app is bundled, i.e. when running
        // via Xcode.
        //
        // To workaround this, we just switch focus to the Dock and then switch
        // back to our app. We only do this for unbundled apps, and only when
        // they fail to become active on their own.
        //
        // This solution was derived from this Godot PR:
        // https://github.com/godotengine/godot/pull/17187
        // (which appears to be based on https://stackoverflow.com/a/7602677)
        //
        // We omit the 2nd step of the solution used in Godot, since it appears
        // to have no effect - I speculate that it's just technical debt picked
        // up from the SO answer; the API used is fairly exotic, and was
        // historically used for very old versions of macOS that didn't support
        // `activateIgnoringOtherApps`, i.e. in previous versions of SDL:
        // https://hg.libsdl.org/SDL/file/c0bcc39a3491/src/video/cocoa/SDL_cocoaevents.m#l322
        //
        // The `performSelector` delays in the Godot solution are used for
        // sequencing, since refocusing the app will fail if the call is made
        // before it finishes unfocusing. The delays used there are much
        // smaller than the ones in the original SO answer, presumably because
        // they found the fastest delay that works reliably through trial and
        // error. Instead of using delays, we just handle
        // `applicationDidResignActive`; despite the app not activating
        // reliably, that still triggers when we switch focus to the Dock.
        //
        // Fun fact: this issue is still present in GLFW
        // (https://github.com/glfw/glfw/issues/1515)
        //
        // A similar issue was found in SDL, but the resolution doesn't seem to
        // work for us: https://bugzilla.libsdl.org/show_bug.cgi?id=3051
        decl.add_ivar::<*mut c_void>(ACTIVATION_HACK_FLAG);
        decl.add_method(
            sel!(activationHackUnfocus:),
            activation_hack_unfocus as extern "C" fn(&mut Object, Sel, id),
        );

        AppDelegateClass(decl.register())
    };
}

extern "C" fn new(class: &Class, _: Sel) -> id {
    unsafe {
        let this: id = msg_send![class, alloc];
        let this: id = msg_send![this, init];
        let flag = Box::new(AtomicBool::default());
        (*this).set_ivar(ACTIVATION_HACK_FLAG, Box::into_raw(flag) as *mut c_void);
        this
    }
}

extern "C" fn dealloc(this: &Object, _sel: Sel) {
    unsafe {
        Box::from_raw(activation_hack_flag_ptr(this));
    }
}

extern "C" fn did_finish_launching(this: &Object, _: Sel, _: id) {
    trace!("Triggered `did_finish_launching`");
    unsafe {
        if let None = util::app_name() {
            // This app is unbundled, so we need to do some shenanigans for the
            // app to reliably activate correctly.
            //
            // While it would be nice to just call our method directly instead
            // of using `performSelector` here, `NSApp isActive` always returns
            // `NO` if we do that. We defer this until after activeness has
            // been determined using the tried-and-true method of adding a
            // delay... finding a more correct solution is hard, since despite
            // `applicationDidResignActive` always firing when we switch focus
            // to the Dock, we don't get a preceding
            // `applicationDidBecomeActive` unless the app activated normally.
            let () = msg_send![
                this,
                performSelector: sel!(activationHackUnfocus:)
                withObject: nil
                afterDelay: 0.02
            ];
        }
    }
    AppState::launched();
    trace!("Completed `did_finish_launching`");
}

extern "C" fn did_resign_active(this: &mut Object, _: Sel, _: id) {
    trace!("Triggered `did_resign_active`");
    activation_hack_refocus(this);
    trace!("Completed `did_resign_active`");
}

static ACTIVATION_HACK_FLAG: &'static str = "activationHackFlag";

unsafe fn activation_hack_flag_ptr(this: &Object) -> *mut AtomicBool {
    let flag: *mut c_void = *(*this).get_ivar(ACTIVATION_HACK_FLAG);
    assert!(!flag.is_null(), "`activationHackFlag` pointer was null");
    flag as *mut AtomicBool
}

unsafe fn set_activation_hack_flag(this: &Object, value: bool) {
    let flag = activation_hack_flag_ptr(this);
    (*flag).store(value, Ordering::Release);
}

unsafe fn get_activation_hack_flag(this: &Object) -> bool {
    let flag = activation_hack_flag_ptr(this);
    (*flag).load(Ordering::Acquire)
}

// First, we switch focus to the dock.
extern "C" fn activation_hack_unfocus(this: &mut Object, _: Sel, _: id) {
    trace!("Triggered `activation_hack_unfocus`");
    unsafe {
        // We only perform the hack if the app failed to activate, since
        // otherwise, there'd be a gross (but fast) flicker as it unfocused and
        // then refocused.
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
                set_activation_hack_flag(this, true);
                let dock: id = msg_send![dock_array, objectAtIndex: 0];
                // This will trigger `did_resign_active`, which will call
                // `activation_hack_refocus`.
                let status: BOOL = msg_send![
                    dock,
                    activateWithOptions: NSApplicationActivateIgnoringOtherApps
                ];
                if status == NO {
                    error!("Failed to switch focus to Dock");
                }
            }
        }
    }
    trace!("Completed `activation_hack_unfocus`");
}

// Then, we switch focus back to our app, and the user rejoices!
extern "C" fn activation_hack_refocus(this: &mut Object) {
    trace!("Triggered `activation_hack_refocus`");
    unsafe {
        if get_activation_hack_flag(this) {
            set_activation_hack_flag(this, false);
            let app: id = msg_send![class!(NSRunningApplication), currentApplication];
            // Simply calling `NSApp activateIgnoringOtherApps` doesn't work.
            // The nuanced difference isn't clear to me, but hey, I tried.
            let success: BOOL = msg_send![
                app,
                activateWithOptions: NSApplicationActivateIgnoringOtherApps
            ];
            if success == NO {
                error!("Failed to refocus app");
            }
        }
    }
    trace!("Completed `activation_hack_refocus`");
}
