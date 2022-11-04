use std::mem;
use std::ops::Deref;
use std::sync::{Mutex, Weak};

use dispatch::Queue;
use objc2::foundation::{is_main_thread, CGFloat, NSPoint, NSSize, NSString};
use objc2::rc::{autoreleasepool, Id, Shared};

use crate::platform_impl::platform::window_delegate::WinitWindowDelegate;
use crate::{
    dpi::LogicalSize,
    platform_impl::platform::{
        appkit::{NSScreen, NSWindow, NSWindowLevel, NSWindowStyleMask},
        ffi,
        window::{SharedState, SharedStateMutexGuard},
    },
};

// Unsafe wrapper type that allows us to dispatch things that aren't Send.
// This should *only* be used to dispatch to the main queue.
// While it is indeed not guaranteed that these types can safely be sent to
// other threads, we know that they're safe to use on the main thread.
struct MainThreadSafe<T>(T);

unsafe impl<T> Send for MainThreadSafe<T> {}

impl<T> Deref for MainThreadSafe<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

fn set_style_mask(window: &NSWindow, mask: NSWindowStyleMask) {
    window.setStyleMask(mask);
    // If we don't do this, key handling will break
    // (at least until the window is clicked again/etc.)
    let _ = window.makeFirstResponder(Some(&window.contentView()));
}

// Always use this function instead of trying to modify `styleMask` directly!
// `setStyleMask:` isn't thread-safe, so we have to use Grand Central Dispatch.
// Otherwise, this would vomit out errors about not being on the main thread
// and fail to do anything.
pub(crate) fn set_style_mask_async(window: &NSWindow, mask: NSWindowStyleMask) {
    // TODO(madsmtm): Remove this 'static hack!
    let window = unsafe { MainThreadSafe(mem::transmute::<&NSWindow, &'static NSWindow>(window)) };
    Queue::main().exec_async(move || {
        set_style_mask(*window, mask);
    });
}
pub(crate) fn set_style_mask_sync(window: &NSWindow, mask: NSWindowStyleMask) {
    if is_main_thread() {
        set_style_mask(window, mask);
    } else {
        let window = MainThreadSafe(window);
        Queue::main().exec_sync(move || {
            set_style_mask(*window, mask);
        })
    }
}

// `setContentSize:` isn't thread-safe either, though it doesn't log any errors
// and just fails silently. Anyway, GCD to the rescue!
pub(crate) fn set_content_size_async(window: &NSWindow, size: LogicalSize<f64>) {
    let window = unsafe { MainThreadSafe(mem::transmute::<&NSWindow, &'static NSWindow>(window)) };
    Queue::main().exec_async(move || {
        window.setContentSize(NSSize::new(size.width as CGFloat, size.height as CGFloat));
    });
}

// `setFrameTopLeftPoint:` isn't thread-safe, but fortunately has the courtesy
// to log errors.
pub(crate) fn set_frame_top_left_point_async(window: &NSWindow, point: NSPoint) {
    let window = unsafe { MainThreadSafe(mem::transmute::<&NSWindow, &'static NSWindow>(window)) };
    Queue::main().exec_async(move || {
        window.setFrameTopLeftPoint(point);
    });
}

// `setFrameTopLeftPoint:` isn't thread-safe, and fails silently.
pub(crate) fn set_level_async(window: &NSWindow, level: NSWindowLevel) {
    let window = unsafe { MainThreadSafe(mem::transmute::<&NSWindow, &'static NSWindow>(window)) };
    Queue::main().exec_async(move || {
        window.setLevel(level);
    });
}

// `setIgnoresMouseEvents_:` isn't thread-safe, and fails silently.
pub(crate) fn set_ignore_mouse_events(window: &NSWindow, ignore: bool) {
    let window = unsafe { MainThreadSafe(mem::transmute::<&NSWindow, &'static NSWindow>(window)) };
    Queue::main().exec_async(move || {
        window.setIgnoresMouseEvents(ignore);
    });
}

// `toggleFullScreen` is thread-safe, but our additional logic to account for
// window styles isn't.
pub(crate) fn toggle_full_screen_async(
    window: &NSWindow,
    not_fullscreen: bool,
    shared_state: Weak<Mutex<SharedState>>,
) {
    let window = unsafe { MainThreadSafe(mem::transmute::<&NSWindow, &'static NSWindow>(window)) };
    let shared_state = MainThreadSafe(shared_state);
    Queue::main().exec_async(move || {
        // `toggleFullScreen` doesn't work if the `StyleMask` is none, so we
        // set a normal style temporarily. The previous state will be
        // restored in `WindowDelegate::window_did_exit_fullscreen`.
        if not_fullscreen {
            let curr_mask = window.styleMask();
            let required =
                NSWindowStyleMask::NSTitledWindowMask | NSWindowStyleMask::NSResizableWindowMask;
            if !curr_mask.contains(required) {
                set_style_mask(*window, required);
                if let Some(shared_state) = shared_state.upgrade() {
                    let mut shared_state_lock = SharedStateMutexGuard::new(
                        shared_state.lock().unwrap(),
                        "toggle_full_screen_callback",
                    );
                    (*shared_state_lock).saved_style = Some(curr_mask);
                }
            }
        }
        // Window level must be restored from `CGShieldingWindowLevel()
        // + 1` back to normal in order for `toggleFullScreen` to do
        // anything
        window.setLevel(NSWindowLevel::Normal);
        window.toggleFullScreen(None);
    });
}

pub(crate) unsafe fn restore_display_mode_async(ns_screen: u32) {
    Queue::main().exec_async(move || {
        unsafe { ffi::CGRestorePermanentDisplayConfiguration() };
        assert_eq!(
            unsafe { ffi::CGDisplayRelease(ns_screen) },
            ffi::kCGErrorSuccess
        );
    });
}

// `setMaximized` is not thread-safe
pub(crate) fn set_maximized_async(
    window: &NSWindow,
    is_zoomed: bool,
    maximized: bool,
    shared_state: Weak<Mutex<SharedState>>,
) {
    let window = unsafe { MainThreadSafe(mem::transmute::<&NSWindow, &'static NSWindow>(window)) };
    let shared_state = MainThreadSafe(shared_state);
    Queue::main().exec_async(move || {
        if let Some(shared_state) = shared_state.upgrade() {
            let mut shared_state_lock =
                SharedStateMutexGuard::new(shared_state.lock().unwrap(), "set_maximized");

            // Save the standard frame sized if it is not zoomed
            if !is_zoomed {
                shared_state_lock.standard_frame = Some(window.frame());
            }

            shared_state_lock.maximized = maximized;

            if shared_state_lock.fullscreen.is_some() {
                // Handle it in window_did_exit_fullscreen
                return;
            }

            if window
                .styleMask()
                .contains(NSWindowStyleMask::NSResizableWindowMask)
            {
                // Just use the native zoom if resizable
                window.zoom(None);
            } else {
                // if it's not resizable, we set the frame directly
                let new_rect = if maximized {
                    let screen = NSScreen::main().expect("no screen found");
                    screen.visibleFrame()
                } else {
                    shared_state_lock.saved_standard_frame()
                };
                window.setFrame_display(new_rect, false);
            }
        }
    });
}

// `orderOut:` isn't thread-safe. Calling it from another thread actually works,
// but with an odd delay.
pub(crate) fn order_out_async(window: &NSWindow) {
    let window = unsafe { MainThreadSafe(mem::transmute::<&NSWindow, &'static NSWindow>(window)) };
    Queue::main().exec_async(move || {
        window.orderOut(None);
    });
}

// `makeKeyAndOrderFront:` isn't thread-safe. Calling it from another thread
// actually works, but with an odd delay.
pub(crate) fn make_key_and_order_front_async(window: &NSWindow) {
    let window = unsafe { MainThreadSafe(mem::transmute::<&NSWindow, &'static NSWindow>(window)) };
    Queue::main().exec_async(move || {
        window.makeKeyAndOrderFront(None);
    });
}

// `setTitle:` isn't thread-safe. Calling it from another thread invalidates the
// window drag regions, which throws an exception when not done in the main
// thread
pub(crate) fn set_title_async(window: &NSWindow, title: String) {
    let window = unsafe { MainThreadSafe(mem::transmute::<&NSWindow, &'static NSWindow>(window)) };
    Queue::main().exec_async(move || {
        window.setTitle(&NSString::from_str(&title));
    });
}

// `close:` is thread-safe, but we want the event to be triggered from the main
// thread. Though, it's a good idea to look into that more...
//
// ArturKovacs: It's important that this operation keeps the underlying window alive
// through the `Id` because otherwise it would dereference free'd memory
pub(crate) fn close_async(delegate: Id<WinitWindowDelegate, Shared>) {
    let delegate = MainThreadSafe(delegate);
    Queue::main().exec_async(move || {
        autoreleasepool(move |_| {
            delegate.window.close();
        });
    });
}
