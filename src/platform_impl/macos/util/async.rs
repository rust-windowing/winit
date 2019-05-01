use std::{os::raw::c_void, sync::{Mutex, Weak}};

use cocoa::{
    appkit::{CGFloat, NSWindow, NSWindowStyleMask},
    base::{id, nil},
    foundation::{NSAutoreleasePool, NSPoint, NSSize},
};
use dispatch::ffi::{dispatch_async_f, dispatch_get_main_queue, dispatch_sync_f};

use dpi::LogicalSize;
use platform_impl::platform::{ffi, window::SharedState};

unsafe fn set_style_mask(nswindow: id, nsview: id, mask: NSWindowStyleMask) {
    nswindow.setStyleMask_(mask);
    // If we don't do this, key handling will break
    // (at least until the window is clicked again/etc.)
    nswindow.makeFirstResponder_(nsview);
}

struct SetStyleMaskData {
    nswindow: id,
    nsview: id,
    mask: NSWindowStyleMask,
}
impl SetStyleMaskData {
    fn new_ptr(
        nswindow: id,
        nsview: id,
        mask: NSWindowStyleMask,
    ) -> *mut Self {
        Box::into_raw(Box::new(SetStyleMaskData { nswindow, nsview, mask }))
    }
}
extern fn set_style_mask_callback(context: *mut c_void) {
    unsafe {
        let context_ptr = context as *mut SetStyleMaskData;
        {
            let context = &*context_ptr;
            set_style_mask(context.nswindow, context.nsview, context.mask);
        }
        Box::from_raw(context_ptr);
    }
}
// Always use this function instead of trying to modify `styleMask` directly!
// `setStyleMask:` isn't thread-safe, so we have to use Grand Central Dispatch.
// Otherwise, this would vomit out errors about not being on the main thread
// and fail to do anything.
pub unsafe fn set_style_mask_async(nswindow: id, nsview: id, mask: NSWindowStyleMask) {
    let context = SetStyleMaskData::new_ptr(nswindow, nsview, mask);
    dispatch_async_f(
        dispatch_get_main_queue(),
        context as *mut _,
        set_style_mask_callback,
    );
}
pub unsafe fn set_style_mask_sync(nswindow: id, nsview: id, mask: NSWindowStyleMask) {
    let context = SetStyleMaskData::new_ptr(nswindow, nsview, mask);
    dispatch_sync_f(
        dispatch_get_main_queue(),
        context as *mut _,
        set_style_mask_callback,
    );
}

struct SetContentSizeData {
    nswindow: id,
    size: LogicalSize,
}
impl SetContentSizeData {
    fn new_ptr(
        nswindow: id,
        size: LogicalSize,
    ) -> *mut Self {
        Box::into_raw(Box::new(SetContentSizeData { nswindow, size }))
    }
}
extern fn set_content_size_callback(context: *mut c_void) {
    unsafe {
        let context_ptr = context as *mut SetContentSizeData;
        {
            let context = &*context_ptr;
            NSWindow::setContentSize_(
                context.nswindow,
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
pub unsafe fn set_content_size_async(nswindow: id, size: LogicalSize) {
    let context = SetContentSizeData::new_ptr(nswindow, size);
    dispatch_async_f(
        dispatch_get_main_queue(),
        context as *mut _,
        set_content_size_callback,
    );
}

struct SetFrameTopLeftPointData {
    nswindow: id,
    point: NSPoint,
}
impl SetFrameTopLeftPointData {
    fn new_ptr(
        nswindow: id,
        point: NSPoint,
    ) -> *mut Self {
        Box::into_raw(Box::new(SetFrameTopLeftPointData { nswindow, point }))
    }
}
extern fn set_frame_top_left_point_callback(context: *mut c_void) {
    unsafe {
        let context_ptr = context as *mut SetFrameTopLeftPointData;
        {
            let context = &*context_ptr;
            NSWindow::setFrameTopLeftPoint_(context.nswindow, context.point);
        }
        Box::from_raw(context_ptr);
    }
}
// `setFrameTopLeftPoint:` isn't thread-safe, but fortunately has the courtesy
// to log errors.
pub unsafe fn set_frame_top_left_point_async(nswindow: id, point: NSPoint) {
    let context = SetFrameTopLeftPointData::new_ptr(nswindow, point);
    dispatch_async_f(
        dispatch_get_main_queue(),
        context as *mut _,
        set_frame_top_left_point_callback,
    );
}

struct SetLevelData {
    nswindow: id,
    level: ffi::NSWindowLevel,
}
impl SetLevelData {
    fn new_ptr(
        nswindow: id,
        level: ffi::NSWindowLevel,
    ) -> *mut Self {
        Box::into_raw(Box::new(SetLevelData { nswindow, level }))
    }
}
extern fn set_level_callback(context: *mut c_void) {
    unsafe {
        let context_ptr = context as *mut SetLevelData;
        {
            let context = &*context_ptr;
            context.nswindow.setLevel_(context.level as _);
        }
        Box::from_raw(context_ptr);
    }
}
// `setFrameTopLeftPoint:` isn't thread-safe, and fails silently.
pub unsafe fn set_level_async(nswindow: id, level: ffi::NSWindowLevel) {
    let context = SetLevelData::new_ptr(nswindow, level);
    dispatch_async_f(
        dispatch_get_main_queue(),
        context as *mut _,
        set_level_callback,
    );
}

struct ToggleFullScreenData {
    nswindow: id,
    nsview: id,
    not_fullscreen: bool,
    shared_state: Weak<Mutex<SharedState>>,
}
impl ToggleFullScreenData {
    fn new_ptr(
        nswindow: id,
        nsview: id,
        not_fullscreen: bool,
        shared_state: Weak<Mutex<SharedState>>,
    ) -> *mut Self {
        Box::into_raw(Box::new(ToggleFullScreenData {
            nswindow,
            nsview,
            not_fullscreen,
            shared_state,
        }))
    }
}
extern fn toggle_full_screen_callback(context: *mut c_void) {
    unsafe {
        let context_ptr = context as *mut ToggleFullScreenData;
        {
            let context = &*context_ptr;

            // `toggleFullScreen` doesn't work if the `StyleMask` is none, so we
            // set a normal style temporarily. The previous state will be
            // restored in `WindowDelegate::window_did_exit_fullscreen`.
            if context.not_fullscreen {
                let curr_mask = context.nswindow.styleMask();
                let required = NSWindowStyleMask::NSTitledWindowMask
                    | NSWindowStyleMask::NSResizableWindowMask;
                if !curr_mask.contains(required) {
                    set_style_mask(context.nswindow, context.nsview, required);
                    if let Some(shared_state) = context.shared_state.upgrade() {
                        trace!("Locked shared state in `toggle_full_screen_callback`");
                        let mut shared_state_lock = shared_state.lock().unwrap();
                        (*shared_state_lock).saved_style = Some(curr_mask);
                        trace!("Unlocked shared state in `toggle_full_screen_callback`");
                    }
                }
            }

            context.nswindow.toggleFullScreen_(nil);
        }
        Box::from_raw(context_ptr);
    }
}
// `toggleFullScreen` is thread-safe, but our additional logic to account for
// window styles isn't.
pub unsafe fn toggle_full_screen_async(
    nswindow: id,
    nsview: id,
    not_fullscreen: bool,
    shared_state: Weak<Mutex<SharedState>>,
) {
    let context = ToggleFullScreenData::new_ptr(
        nswindow,
        nsview,
        not_fullscreen,
        shared_state,
    );
    dispatch_async_f(
        dispatch_get_main_queue(),
        context as *mut _,
        toggle_full_screen_callback,
    );
}

struct OrderOutData {
    nswindow: id,
}
impl OrderOutData {
    fn new_ptr(nswindow: id) -> *mut Self {
        Box::into_raw(Box::new(OrderOutData { nswindow }))
    }
}
extern fn order_out_callback(context: *mut c_void) {
    unsafe {
        let context_ptr = context as *mut OrderOutData;
        {
            let context = &*context_ptr;
            context.nswindow.orderOut_(nil);
        }
        Box::from_raw(context_ptr);
    }
}
// `orderOut:` isn't thread-safe. Calling it from another thread actually works,
// but with an odd delay.
pub unsafe fn order_out_async(nswindow: id) {
    let context = OrderOutData::new_ptr(nswindow);
    dispatch_async_f(
        dispatch_get_main_queue(),
        context as *mut _,
        order_out_callback,
    );
}

struct MakeKeyAndOrderFrontData {
    nswindow: id,
}
impl MakeKeyAndOrderFrontData {
    fn new_ptr(nswindow: id) -> *mut Self {
        Box::into_raw(Box::new(MakeKeyAndOrderFrontData { nswindow }))
    }
}
extern fn make_key_and_order_front_callback(context: *mut c_void) {
    unsafe {
        let context_ptr = context as *mut MakeKeyAndOrderFrontData;
        {
            let context = &*context_ptr;
            context.nswindow.makeKeyAndOrderFront_(nil);
        }
        Box::from_raw(context_ptr);
    }
}
// `makeKeyAndOrderFront:` isn't thread-safe. Calling it from another thread
// actually works, but with an odd delay.
pub unsafe fn make_key_and_order_front_async(nswindow: id) {
    let context = MakeKeyAndOrderFrontData::new_ptr(nswindow);
    dispatch_async_f(
        dispatch_get_main_queue(),
        context as *mut _,
        make_key_and_order_front_callback,
    );
}

struct CloseData {
    nswindow: id,
}
impl CloseData {
    fn new_ptr(nswindow: id) -> *mut Self {
        Box::into_raw(Box::new(CloseData { nswindow }))
    }
}
extern fn close_callback(context: *mut c_void) {
    unsafe {
        let context_ptr = context as *mut CloseData;
        {
            let context = &*context_ptr;
            let pool = NSAutoreleasePool::new(nil);
            context.nswindow.close();
            pool.drain();
        }
        Box::from_raw(context_ptr);
    }
}
// `close:` is thread-safe, but we want the event to be triggered from the main
// thread. Though, it's a good idea to look into that more...
pub unsafe fn close_async(nswindow: id) {
    let context = CloseData::new_ptr(nswindow);
    dispatch_async_f(
        dispatch_get_main_queue(),
        context as *mut _,
        close_callback,
    );
}
