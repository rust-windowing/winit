// Normally when you run or distribute a macOS app, it's bundled: it's in one
// of those fun little folders that you have to right click "Show Package
// Contents" on, and usually contains myriad delights including, but not
// limited to, plists, icons, and of course, your beloved executable. However,
// when you use `cargo run`, your app is unbundled - it's just a lonely, bare
// executable.
//
// Apple isn't especially fond of unbundled apps, which is to say, they seem to
// barely be supported. If you move the mouse while opening a winit window from
// an unbundled app, the window will fail to activate and be in a grayed-out
// uninteractable state. Switching to another app and back is the only way to
// get the winit window into a normal state. None of this happens if the app is
// bundled, i.e. when running via Xcode.
//
// To workaround this, we just switch focus to the Dock and then switch back to
// our app. We only do this for unbundled apps, and only when they fail to
// become active on their own.
//
// This solution was derived from this Godot PR:
// https://github.com/godotengine/godot/pull/17187
// (which appears to be based on https://stackoverflow.com/a/7602677)
// The curious specialness of mouse motions is touched upon here:
// https://github.com/godotengine/godot/issues/8653#issuecomment-358130512
//
// We omit the 2nd step of the solution used in Godot, since it appears to have
// no effect - I speculate that it's just technical debt picked up from the SO
// answer; the API used is fairly exotic, and was historically used for very
// old versions of macOS that didn't support `activateIgnoringOtherApps`, i.e.
// in previous versions of SDL:
// https://hg.libsdl.org/SDL/file/c0bcc39a3491/src/video/cocoa/SDL_cocoaevents.m#l322
//
// The `performSelector` delays in the Godot solution are used for sequencing,
// since refocusing the app will fail if the call is made before it finishes
// unfocusing. The delays used there are much smaller than the ones in the
// original SO answer, presumably because they found the fastest delay that
// works reliably through trial and error. Instead of using delays, we just
// handle `applicationDidResignActive`; despite the app not activating reliably,
// that still triggers when we switch focus to the Dock.
//
// The Godot solution doesn't appear to skip the hack when an unbundled app
// activates normally. Checking for this is difficult, since if you call
// `isActive` too early, it will always be `NO`. Even though we receive
// `applicationDidResignActive` when switching focus to the Dock, we never
// receive a preceding `applicationDidBecomeActive` if the app fails to
// activate normally. I wasn't able to find a proper point in time to perform
// the `isActive` check, so we instead check for the cause of the quirk: if
// any mouse motion occurs prior to us receiving `applicationDidResignActive`,
// we assume the app failed to become active.
//
// Fun fact: this issue is still present in GLFW
// (https://github.com/glfw/glfw/issues/1515)
//
// A similar issue was found in SDL, but the resolution doesn't seem to work
// for us: https://bugzilla.libsdl.org/show_bug.cgi?id=3051

use super::util;
use cocoa::{
    appkit::{NSApp, NSApplicationActivateIgnoringOtherApps},
    base::id,
    foundation::NSUInteger,
};
use objc::runtime::{Object, Sel, BOOL, NO, YES};
use std::{
    os::raw::c_void,
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Debug, Default)]
pub struct State {
    // Indicates that the hack has either completed or been skipped.
    activated: AtomicBool,
    // Indicates that the mouse has moved at some point in time.
    mouse_moved: AtomicBool,
    // Indicates that the hack is in progress, and that we should refocus when
    // the app resigns active.
    needs_refocus: AtomicBool,
}

impl State {
    pub fn name() -> &'static str {
        "activationHackState"
    }

    pub fn new() -> *mut c_void {
        let this = Box::new(Self::default());
        Box::into_raw(this) as *mut c_void
    }

    pub unsafe fn free(this: *mut Self) {
        Box::from_raw(this);
    }

    pub unsafe fn get_ptr(obj: &Object) -> *mut Self {
        let this: *mut c_void = *(*obj).get_ivar(Self::name());
        assert!(!this.is_null(), "`activationHackState` pointer was null");
        this as *mut Self
    }

    pub unsafe fn set_activated(obj: &Object, value: bool) {
        let this = Self::get_ptr(obj);
        (*this).activated.store(value, Ordering::Release);
    }

    unsafe fn get_activated(obj: &Object) -> bool {
        let this = Self::get_ptr(obj);
        (*this).activated.load(Ordering::Acquire)
    }

    pub unsafe fn set_mouse_moved(obj: &Object, value: bool) {
        let this = Self::get_ptr(obj);
        (*this).mouse_moved.store(value, Ordering::Release);
    }

    pub unsafe fn get_mouse_moved(obj: &Object) -> bool {
        let this = Self::get_ptr(obj);
        (*this).mouse_moved.load(Ordering::Acquire)
    }

    pub unsafe fn set_needs_refocus(obj: &Object, value: bool) {
        let this = Self::get_ptr(obj);
        (*this).needs_refocus.store(value, Ordering::Release);
    }

    unsafe fn get_needs_refocus(obj: &Object) -> bool {
        let this = Self::get_ptr(obj);
        (*this).needs_refocus.load(Ordering::Acquire)
    }
}

// This is the entry point for the hack - if the app is unbundled and a mouse
// movement occurs before the app activates, it will trigger the hack. Because
// mouse movements prior to activation are the cause of this quirk, they should
// be a reliable way to determine if the hack needs to be performed.
pub extern "C" fn mouse_moved(this: &Object, _: Sel, _: id) {
    trace!("Triggered `activationHackMouseMoved`");
    unsafe {
        if !State::get_activated(this) {
            // We check if `CFBundleName` is undefined to determine if the
            // app is unbundled.
            if let None = util::app_name() {
                info!("App detected as unbundled");
                unfocus(this);
            } else {
                info!("App detected as bundled");
            }
        }
    }
    trace!("Completed `activationHackMouseMoved`");
}

// Switch focus to the dock.
unsafe fn unfocus(this: &Object) {
    // We only perform the hack if the app failed to activate, since otherwise,
    // there'd be a gross (but fast) flicker as it unfocused and then refocused.
    // However, we only enter this function if we detect mouse movement prior
    // to activation, so this should always be `NO`.
    //
    // Note that this check isn't necessarily reliable in detecting a violation
    // of the invariant above, since it's not guaranteed that activation will
    // resolve before this point. In other words, it can spuriously return `NO`.
    // This is also why the mouse motion approach was chosen, since it's not
    // obvious how to sequence this check - if someone knows how to, then that
    // would almost surely be a cleaner approach.
    let active: BOOL = msg_send![NSApp(), isActive];
    if active == YES {
        error!("Unbundled app activation hack triggered on an app that's already active; this shouldn't happen!");
    } else {
        info!("Performing unbundled app activation hack");
        let dock_bundle_id = util::ns_string_id_ref("com.apple.dock");
        let dock_array: id = msg_send![
            class!(NSRunningApplication),
            runningApplicationsWithBundleIdentifier: *dock_bundle_id
        ];
        let dock_array_len: NSUInteger = msg_send![dock_array, count];
        if dock_array_len == 0 {
            error!("The Dock doesn't seem to be running, so switching focus to it is impossible");
        } else {
            State::set_needs_refocus(this, true);
            let dock: id = msg_send![dock_array, objectAtIndex: 0];
            // This will trigger `applicationDidResignActive`, which will in
            // turn call `refocus`.
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

// Switch focus back to our app, causing the user to rejoice!
pub unsafe fn refocus(this: &Object) {
    if State::get_needs_refocus(this) {
        State::set_needs_refocus(this, false);
        let app: id = msg_send![class!(NSRunningApplication), currentApplication];
        // Simply calling `NSApp activateIgnoringOtherApps` doesn't work. The
        // nuanced difference isn't clear to me, but hey, I tried.
        let success: BOOL = msg_send![
            app,
            activateWithOptions: NSApplicationActivateIgnoringOtherApps
        ];
        if success == NO {
            error!("Failed to refocus app");
        }
    }
}
