use std::{
    os::raw::c_void,
    sync::{Mutex, Weak},
};

use cocoa::{
    appkit::{CGFloat, NSScreen, NSWindow, NSWindowStyleMask},
    base::{id, nil},
    foundation::{NSAutoreleasePool, NSPoint, NSSize, NSString},
};
use dispatch::ffi::{dispatch_async_f, dispatch_get_main_queue, dispatch_sync_f};

use crate::{
    dpi::LogicalSize,
    platform_impl::platform::{ffi, util::IdRef, window::SharedState},
};

unsafe fn set_style_mask(ns_window: id, ns_view: id, mask: NSWindowStyleMask) {
    ns_window.setStyleMask_(mask);
    // If we don't do this, key handling will break
    // (at least until the window is clicked again/etc.)
    ns_window.makeFirstResponder_(ns_view);
}

struct SetStyleMaskData {
    ns_window: id,
    ns_view: id,
    mask: NSWindowStyleMask,
}
impl SetStyleMaskData {
    fn new_ptr(ns_window: id, ns_view: id, mask: NSWindowStyleMask) -> *mut Self {
        Box::into_raw(Box::new(SetStyleMaskData {
            ns_window,
            ns_view,
            mask,
        }))
    }
}
extern "C" fn set_style_mask_callback(context: *mut c_void) {
    unsafe {
        let context_ptr = context as *mut SetStyleMaskData;
        {
            let context = &*context_ptr;
            set_style_mask(context.ns_window, context.ns_view, context.mask);
        }
        Box::from_raw(context_ptr);
    }
}
// Always use this function instead of trying to modify `styleMask` directly!
// `setStyleMask:` isn't thread-safe, so we have to use Grand Central Dispatch.
// Otherwise, this would vomit out errors about not being on the main thread
// and fail to do anything.
pub unsafe fn set_style_mask_async(ns_window: id, ns_view: id, mask: NSWindowStyleMask) {
    let context = SetStyleMaskData::new_ptr(ns_window, ns_view, mask);
    dispatch_async_f(
        dispatch_get_main_queue(),
        context as *mut _,
        set_style_mask_callback,
    );
}
pub unsafe fn set_style_mask_sync(ns_window: id, ns_view: id, mask: NSWindowStyleMask) {
    let context = SetStyleMaskData::new_ptr(ns_window, ns_view, mask);
    dispatch_sync_f(
        dispatch_get_main_queue(),
        context as *mut _,
        set_style_mask_callback,
    );
}

struct SetContentSizeData {
    ns_window: id,
    size: LogicalSize,
}
impl SetContentSizeData {
    fn new_ptr(ns_window: id, size: LogicalSize) -> *mut Self {
        Box::into_raw(Box::new(SetContentSizeData { ns_window, size }))
    }
}
extern "C" fn set_content_size_callback(context: *mut c_void) {
    unsafe {
        let context_ptr = context as *mut SetContentSizeData;
        {
            let context = &*context_ptr;
            NSWindow::setContentSize_(
                context.ns_window,
                NSSize::new(
                    context.size.width as CGFloat,
                    context.size.height as CGFloat,
                ),
            );
        }
        Box::from_raw(context_ptr);
    }
}
// `setContentSize:` isn't thread-safe either, though it doesn't log any errors
// and just fails silently. Anyway, GCD to the rescue!
pub unsafe fn set_content_size_async(ns_window: id, size: LogicalSize) {
    let context = SetContentSizeData::new_ptr(ns_window, size);
    dispatch_async_f(
        dispatch_get_main_queue(),
        context as *mut _,
        set_content_size_callback,
    );
}

struct SetFrameTopLeftPointData {
    ns_window: id,
    point: NSPoint,
}
impl SetFrameTopLeftPointData {
    fn new_ptr(ns_window: id, point: NSPoint) -> *mut Self {
        Box::into_raw(Box::new(SetFrameTopLeftPointData { ns_window, point }))
    }
}
extern "C" fn set_frame_top_left_point_callback(context: *mut c_void) {
    unsafe {
        let context_ptr = context as *mut SetFrameTopLeftPointData;
        {
            let context = &*context_ptr;
            NSWindow::setFrameTopLeftPoint_(context.ns_window, context.point);
        }
        Box::from_raw(context_ptr);
    }
}
// `setFrameTopLeftPoint:` isn't thread-safe, but fortunately has the courtesy
// to log errors.
pub unsafe fn set_frame_top_left_point_async(ns_window: id, point: NSPoint) {
    let context = SetFrameTopLeftPointData::new_ptr(ns_window, point);
    dispatch_async_f(
        dispatch_get_main_queue(),
        context as *mut _,
        set_frame_top_left_point_callback,
    );
}

struct SetLevelData {
    ns_window: id,
    level: ffi::NSWindowLevel,
}
impl SetLevelData {
    fn new_ptr(ns_window: id, level: ffi::NSWindowLevel) -> *mut Self {
        Box::into_raw(Box::new(SetLevelData { ns_window, level }))
    }
}
extern "C" fn set_level_callback(context: *mut c_void) {
    unsafe {
        let context_ptr = context as *mut SetLevelData;
        {
            let context = &*context_ptr;
            context.ns_window.setLevel_(context.level as _);
        }
        Box::from_raw(context_ptr);
    }
}
// `setFrameTopLeftPoint:` isn't thread-safe, and fails silently.
pub unsafe fn set_level_async(ns_window: id, level: ffi::NSWindowLevel) {
    let context = SetLevelData::new_ptr(ns_window, level);
    dispatch_async_f(
        dispatch_get_main_queue(),
        context as *mut _,
        set_level_callback,
    );
}

struct ToggleFullScreenData {
    ns_window: id,
    ns_view: id,
    not_fullscreen: bool,
    shared_state: Weak<Mutex<SharedState>>,
}
impl ToggleFullScreenData {
    fn new_ptr(
        ns_window: id,
        ns_view: id,
        not_fullscreen: bool,
        shared_state: Weak<Mutex<SharedState>>,
    ) -> *mut Self {
        Box::into_raw(Box::new(ToggleFullScreenData {
            ns_window,
            ns_view,
            not_fullscreen,
            shared_state,
        }))
    }
}
extern "C" fn toggle_full_screen_callback(context: *mut c_void) {
    unsafe {
        let context_ptr = context as *mut ToggleFullScreenData;
        {
            let context = &*context_ptr;

            // `toggleFullScreen` doesn't work if the `StyleMask` is none, so we
            // set a normal style temporarily. The previous state will be
            // restored in `WindowDelegate::window_did_exit_fullscreen`.
            if context.not_fullscreen {
                let curr_mask = context.ns_window.styleMask();
                let required = NSWindowStyleMask::NSTitledWindowMask
                    | NSWindowStyleMask::NSResizableWindowMask;
                if !curr_mask.contains(required) {
                    set_style_mask(context.ns_window, context.ns_view, required);
                    if let Some(shared_state) = context.shared_state.upgrade() {
                        trace!("Locked shared state in `toggle_full_screen_callback`");
                        let mut shared_state_lock = shared_state.lock().unwrap();
                        (*shared_state_lock).saved_style = Some(curr_mask);
                        trace!("Unlocked shared state in `toggle_full_screen_callback`");
                    }
                }
            }
            // Window level must be restored from `CGShieldingWindowLevel()
            // + 1` back to normal in order for `toggleFullScreen` to do
            // anything
            context.ns_window.setLevel_(0);
            context.ns_window.toggleFullScreen_(nil);
        }
        Box::from_raw(context_ptr);
    }
}
// `toggleFullScreen` is thread-safe, but our additional logic to account for
// window styles isn't.
pub unsafe fn toggle_full_screen_async(
    ns_window: id,
    ns_view: id,
    not_fullscreen: bool,
    shared_state: Weak<Mutex<SharedState>>,
) {
    let context = ToggleFullScreenData::new_ptr(ns_window, ns_view, not_fullscreen, shared_state);
    dispatch_async_f(
        dispatch_get_main_queue(),
        context as *mut _,
        toggle_full_screen_callback,
    );
}

extern "C" fn restore_display_mode_callback(screen: *mut c_void) {
    unsafe {
        let screen = Box::from_raw(screen as *mut u32);
        ffi::CGRestorePermanentDisplayConfiguration();
        assert_eq!(ffi::CGDisplayRelease(*screen), ffi::kCGErrorSuccess);
    }
}
pub unsafe fn restore_display_mode_async(ns_screen: u32) {
    dispatch_async_f(
        dispatch_get_main_queue(),
        Box::into_raw(Box::new(ns_screen)) as *mut _,
        restore_display_mode_callback,
    );
}

struct SetMaximizedData {
    ns_window: id,
    is_zoomed: bool,
    maximized: bool,
    shared_state: Weak<Mutex<SharedState>>,
}
impl SetMaximizedData {
    fn new_ptr(
        ns_window: id,
        is_zoomed: bool,
        maximized: bool,
        shared_state: Weak<Mutex<SharedState>>,
    ) -> *mut Self {
        Box::into_raw(Box::new(SetMaximizedData {
            ns_window,
            is_zoomed,
            maximized,
            shared_state,
        }))
    }
}
extern "C" fn set_maximized_callback(context: *mut c_void) {
    unsafe {
        let context_ptr = context as *mut SetMaximizedData;
        {
            let context = &*context_ptr;

            if let Some(shared_state) = context.shared_state.upgrade() {
                trace!("Locked shared state in `set_maximized`");
                let mut shared_state_lock = shared_state.lock().unwrap();

                // Save the standard frame sized if it is not zoomed
                if !context.is_zoomed {
                    shared_state_lock.standard_frame = Some(NSWindow::frame(context.ns_window));
                }

                shared_state_lock.maximized = context.maximized;

                let curr_mask = context.ns_window.styleMask();
                if shared_state_lock.fullscreen.is_some() {
                    // Handle it in window_did_exit_fullscreen
                    return;
                } else if curr_mask.contains(NSWindowStyleMask::NSResizableWindowMask) {
                    // Just use the native zoom if resizable
                    context.ns_window.zoom_(nil);
                } else {
                    // if it's not resizable, we set the frame directly
                    let new_rect = if context.maximized {
                        let screen = NSScreen::mainScreen(nil);
                        NSScreen::visibleFrame(screen)
                    } else {
                        shared_state_lock.saved_standard_frame()
                    };
                    context.ns_window.setFrame_display_(new_rect, 0);
                }

                trace!("Unlocked shared state in `set_maximized`");
            }
        }
        Box::from_raw(context_ptr);
    }
}
// `setMaximized` is not thread-safe
pub unsafe fn set_maximized_async(
    ns_window: id,
    is_zoomed: bool,
    maximized: bool,
    shared_state: Weak<Mutex<SharedState>>,
) {
    let context = SetMaximizedData::new_ptr(ns_window, is_zoomed, maximized, shared_state);
    dispatch_async_f(
        dispatch_get_main_queue(),
        context as *mut _,
        set_maximized_callback,
    );
}

struct OrderOutData {
    ns_window: id,
}
impl OrderOutData {
    fn new_ptr(ns_window: id) -> *mut Self {
        Box::into_raw(Box::new(OrderOutData { ns_window }))
    }
}
extern "C" fn order_out_callback(context: *mut c_void) {
    unsafe {
        let context_ptr = context as *mut OrderOutData;
        {
            let context = &*context_ptr;
            context.ns_window.orderOut_(nil);
        }
        Box::from_raw(context_ptr);
    }
}
// `orderOut:` isn't thread-safe. Calling it from another thread actually works,
// but with an odd delay.
pub unsafe fn order_out_async(ns_window: id) {
    let context = OrderOutData::new_ptr(ns_window);
    dispatch_async_f(
        dispatch_get_main_queue(),
        context as *mut _,
        order_out_callback,
    );
}

struct MakeKeyAndOrderFrontData {
    ns_window: id,
}
impl MakeKeyAndOrderFrontData {
    fn new_ptr(ns_window: id) -> *mut Self {
        Box::into_raw(Box::new(MakeKeyAndOrderFrontData { ns_window }))
    }
}
extern "C" fn make_key_and_order_front_callback(context: *mut c_void) {
    unsafe {
        let context_ptr = context as *mut MakeKeyAndOrderFrontData;
        {
            let context = &*context_ptr;
            context.ns_window.makeKeyAndOrderFront_(nil);
        }
        Box::from_raw(context_ptr);
    }
}
// `makeKeyAndOrderFront:` isn't thread-safe. Calling it from another thread
// actually works, but with an odd delay.
pub unsafe fn make_key_and_order_front_async(ns_window: id) {
    let context = MakeKeyAndOrderFrontData::new_ptr(ns_window);
    dispatch_async_f(
        dispatch_get_main_queue(),
        context as *mut _,
        make_key_and_order_front_callback,
    );
}

struct SetTitleData {
    ns_window: id,
    title: String,
}
impl SetTitleData {
    fn new_ptr(ns_window: id, title: String) -> *mut Self {
        Box::into_raw(Box::new(SetTitleData { ns_window, title }))
    }
}
extern "C" fn set_title_callback(context: *mut c_void) {
    unsafe {
        let context_ptr = context as *mut SetTitleData;
        {
            let context = &*context_ptr;
            let title = IdRef::new(NSString::alloc(nil).init_str(&context.title));
            context.ns_window.setTitle_(*title);
        }
        Box::from_raw(context_ptr);
    }
}
// `setTitle:` isn't thread-safe. Calling it from another thread invalidates the
// window drag regions, which throws an exception when not done in the main
// thread
pub unsafe fn set_title_async(ns_window: id, title: String) {
    let context = SetTitleData::new_ptr(ns_window, title);
    dispatch_async_f(
        dispatch_get_main_queue(),
        context as *mut _,
        set_title_callback,
    );
}

struct CloseData {
    ns_window: id,
}
impl CloseData {
    fn new_ptr(ns_window: id) -> *mut Self {
        Box::into_raw(Box::new(CloseData { ns_window }))
    }
}
extern "C" fn close_callback(context: *mut c_void) {
    unsafe {
        let context_ptr = context as *mut CloseData;
        {
            let context = &*context_ptr;
            let pool = NSAutoreleasePool::new(nil);
            context.ns_window.close();
            pool.drain();
        }
        Box::from_raw(context_ptr);
    }
}
// `close:` is thread-safe, but we want the event to be triggered from the main
// thread. Though, it's a good idea to look into that more...
pub unsafe fn close_async(ns_window: id) {
    let context = CloseData::new_ptr(ns_window);
    dispatch_async_f(dispatch_get_main_queue(), context as *mut _, close_callback);
}
