//! iOS support
//!
//! # Building app
//! To build ios app you will need rustc built for this targets:
//!
//!  - armv7-apple-ios
//!  - armv7s-apple-ios
//!  - i386-apple-ios
//!  - aarch64-apple-ios
//!  - x86_64-apple-ios
//!
//! Then
//!
//! ```
//! cargo build --target=...
//! ```
//! The simplest way to integrate your app into xcode environment is to build it
//! as a static library. Wrap your main function and export it.
//!
//! ```rust, ignore
//! #[no_mangle]
//! pub extern fn start_winit_app() {
//!     start_inner()
//! }
//!
//! fn start_inner() {
//!    ...
//! }
//!
//! ```
//!
//! Compile project and then drag resulting .a into Xcode project. Add winit.h to xcode.
//!
//! ```ignore
//! void start_winit_app();
//! ```
//!
//! Use start_winit_app inside your xcode's main function.
//!
//!
//! # App lifecycle and events
//!
//! iOS environment is very different from other platforms and you must be very
//! careful with it's events. Familiarize yourself with
//! [app lifecycle](https://developer.apple.com/library/ios/documentation/UIKit/Reference/UIApplicationDelegate_Protocol/).
//!
//!
//! This is how those event are represented in winit:
//!
//!  - applicationDidBecomeActive is Suspended(false)
//!  - applicationWillResignActive is Suspended(true)
//!  - applicationWillTerminate is LoopDestroyed
//!
//! Keep in mind that after LoopDestroyed event is received every attempt to draw with
//! opengl will result in segfault.
//!
//! Also note that app may not receive the LoopDestroyed event if suspended; it might be SIGKILL'ed.

#![cfg(target_os = "ios")]

use std::{fmt, mem, ptr};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::os::raw::*;
use std::sync::Arc;

use objc::declare::ClassDecl;
use objc::runtime::{BOOL, Class, Object, Sel, YES};

use {
    CreationError,
    Event,
    LogicalPosition,
    LogicalSize,
    MouseCursor,
    PhysicalPosition,
    PhysicalSize,
    WindowAttributes,
    WindowEvent,
    WindowId as RootEventId,
};
use error::{ExternalError, NotSupportedError};
use events::{Touch, TouchPhase};
use window::MonitorHandle as RootMonitorHandle;

mod ffi;
use self::ffi::{
    CFTimeInterval,
    CFRunLoopRunInMode,
    CGFloat,
    CGPoint,
    CGRect,
    id,
    JBLEN,
    JmpBuf,
    kCFRunLoopDefaultMode,
    kCFRunLoopRunHandledSource,
    longjmp,
    nil,
    NSString,
    setjmp,
    UIApplicationMain,
 };

static mut JMPBUF: Option<Box<JmpBuf>> = None;

pub type OsError = std::io::Error;

pub struct Window {
    _events_queue: Arc<RefCell<VecDeque<Event>>>,
    delegate_state: Box<DelegateState>,
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

#[derive(Debug)]
struct DelegateState {
    window: id,
    controller: id,
    view: id,
    size: LogicalSize,
    scale: f64,
}

impl DelegateState {
    fn new(window: id, controller: id, view: id, size: LogicalSize, scale: f64) -> DelegateState {
        DelegateState {
            window,
            controller,
            view,
            size,
            scale,
        }
    }
}

impl Drop for DelegateState {
    fn drop(&mut self) {
        unsafe {
            let _: () = msg_send![self.window, release];
            let _: () = msg_send![self.controller, release];
            let _: () = msg_send![self.view, release];
        }
    }
}

#[derive(Clone)]
pub struct MonitorHandle;

impl fmt::Debug for MonitorHandle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        #[derive(Debug)]
        struct MonitorHandle {
            name: Option<String>,
            dimensions: PhysicalSize,
            position: PhysicalPosition,
            hidpi_factor: f64,
        }

        let monitor_id_proxy = MonitorHandle {
            name: self.name(),
            dimensions: self.dimensions(),
            position: self.outer_position(),
            hidpi_factor: self.hidpi_factor(),
        };

        monitor_id_proxy.fmt(f)
    }
}

impl MonitorHandle {
    #[inline]
    pub fn uiscreen(&self) -> id {
        let class = class!(UIScreen);
        unsafe { msg_send![class, mainScreen] }
    }

    #[inline]
    pub fn name(&self) -> Option<String> {
        Some("Primary".to_string())
    }

    #[inline]
    pub fn dimensions(&self) -> PhysicalSize {
        let bounds: CGRect = unsafe { msg_send![self.uiscreen(), nativeBounds] };
        (bounds.size.width as f64, bounds.size.height as f64).into()
    }

    #[inline]
    pub fn outer_position(&self) -> PhysicalPosition {
        // iOS assumes single screen
        (0, 0).into()
    }

    #[inline]
    pub fn hidpi_factor(&self) -> f64 {
        let scale: CGFloat = unsafe { msg_send![self.uiscreen(), nativeScale] };
        scale as f64
    }
}

pub struct EventLoop {
    events_queue: Arc<RefCell<VecDeque<Event>>>,
}

#[derive(Clone)]
pub struct EventLoopProxy;

impl EventLoop {
    pub fn new() -> EventLoop {
        unsafe {
            if !msg_send![class!(NSThread), isMainThread] {
                panic!("`EventLoop` can only be created on the main thread on iOS");
            }
        }
        EventLoop { events_queue: Default::default() }
    }

    #[inline]
    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        let mut rb = VecDeque::with_capacity(1);
        rb.push_back(MonitorHandle);
        rb
    }

    #[inline]
    pub fn primary_monitor(&self) -> MonitorHandle {
        MonitorHandle
    }

    pub fn poll_events<F>(&mut self, mut callback: F)
        where F: FnMut(::Event)
    {
        if let Some(event) = self.events_queue.borrow_mut().pop_front() {
            callback(event);
            return;
        }

        unsafe {
            // jump hack, so we won't quit on willTerminate event before processing it
            assert!(JMPBUF.is_some(), "`EventLoop::poll_events` must be called after window creation on iOS");
            if setjmp(mem::transmute_copy(&mut JMPBUF)) != 0 {
                if let Some(event) = self.events_queue.borrow_mut().pop_front() {
                    callback(event);
                    return;
                }
            }
        }
    };
}

mod app_state;
mod event_loop;
mod ffi;
mod monitor;
mod view;
mod window;

pub use self::event_loop::{EventLoop, EventLoopProxy, EventLoopWindowTarget};
pub use self::monitor::MonitorHandle;
pub use self::window::{
    PlatformSpecificWindowBuilderAttributes,
    Window,
    WindowId,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId {
    uiscreen: ffi::id,
}

impl DeviceId {
    pub unsafe fn dummy() -> Self {
        DeviceId
    }
}

#[derive(Clone)]
pub struct PlatformSpecificWindowBuilderAttributes {
    pub root_view_class: &'static Class,
}

impl Default for PlatformSpecificWindowBuilderAttributes {
    fn default() -> Self {
        PlatformSpecificWindowBuilderAttributes {
            root_view_class: class!(UIView),
        }
    }
}

// TODO: AFAIK transparency is enabled by default on iOS,
// so to be consistent with other platforms we have to change that.
impl Window {
    pub fn new(
        ev: &EventLoop,
        _attributes: WindowAttributes,
        pl_attributes: PlatformSpecificWindowBuilderAttributes,
    ) -> Result<Window, CreationError> {
        unsafe {
            debug_assert!(mem::size_of_val(&JMPBUF) == mem::size_of::<Box<JmpBuf>>());
            assert!(mem::replace(&mut JMPBUF, Some(Box::new([0; JBLEN]))).is_none(), "Only one `Window` is supported on iOS");
        }

        unsafe {
            if setjmp(mem::transmute_copy(&mut JMPBUF)) != 0 {
                let app_class = class!(UIApplication);
                let app: id = msg_send![app_class, sharedApplication];
                let delegate: id = msg_send![app, delegate];
                let state: *mut c_void = *(&*delegate).get_ivar("winitState");
                let mut delegate_state = Box::from_raw(state as *mut DelegateState);
                let events_queue = &*ev.events_queue;
                (&mut *delegate).set_ivar("eventsQueue", mem::transmute::<_, *mut c_void>(events_queue));

                // easiest? way to get access to PlatformSpecificWindowBuilderAttributes to configure the view
                let rect: CGRect = msg_send![MonitorHandle.uiscreen(), bounds];

                let uiview_class = class!(UIView);
                let root_view_class = pl_attributes.root_view_class;
                let is_uiview: BOOL = msg_send![root_view_class, isSubclassOfClass:uiview_class];
                assert!(is_uiview == YES, "`root_view_class` must inherit from `UIView`");

                delegate_state.view = msg_send![root_view_class, alloc];
                assert!(!delegate_state.view.is_null(), "Failed to create `UIView` instance");
                delegate_state.view = msg_send![delegate_state.view, initWithFrame:rect];
                assert!(!delegate_state.view.is_null(), "Failed to initialize `UIView` instance");

                let _: () = msg_send![delegate_state.controller, setView:delegate_state.view];
                let _: () = msg_send![delegate_state.window, makeKeyAndVisible];

                return Ok(Window {
                    _events_queue: ev.events_queue.clone(),
                    delegate_state,
                });
            }
        }

        create_delegate_class();
        start_app();

        panic!("Couldn't create `UIApplication`!")
    }

    #[inline]
    pub fn uiwindow(&self) -> id {
        self.delegate_state.window
    }

    #[inline]
    pub fn uiview(&self) -> id {
        self.delegate_state.view
    }

    #[inline]
    pub fn set_title(&self, _title: &str) {
        // N/A
    }

    pub fn set_visible(&self, _visible: bool) {
        // N/A
    }

    #[inline]
    pub fn outer_position(&self) -> Result<LogicalPosition, NotSupportedError> {
        Err(NotSupportedError::new())
    }

    #[inline]
    pub fn inner_position(&self) -> Result<LogicalPosition, NotSupportedError> {
        Err(NotSupportedError::new())
    }

    #[inline]
    pub fn set_outer_position(&self, _position: LogicalPosition) {
        // N/A
    }

    #[inline]
    pub fn inner_size(&self) -> LogicalSize {
        self.delegate_state.size
    }

    #[inline]
    pub fn outer_size(&self) -> LogicalSize {
        self.inner_size()
    }

    #[inline]
    pub fn set_inner_size(&self, _size: LogicalSize) {
        // N/A
    }

    #[inline]
    pub fn set_min_inner_size(&self, _dimensions: Option<LogicalSize>) {
        // N/A
    }

    #[inline]
    pub fn set_max_inner_size(&self, _dimensions: Option<LogicalSize>) {
        // N/A
    }

    #[inline]
    pub fn set_resizable(&self, _resizable: bool) {
        // N/A
    }

    #[inline]
    pub fn set_cursor(&self, _cursor: MouseCursor) {
        // N/A
    }

    #[inline]
    pub fn set_cursor_grab(&self, _grab: bool) -> Result<(), ExternalError> {
        Err(NotSupportedError::new())
    }

    #[inline]
    pub fn set_cursor_visible(&self, _visible: bool) {
        // N/A
    }

    #[inline]
    pub fn hidpi_factor(&self) -> f64 {
        self.delegate_state.scale
    }

    #[inline]
    pub fn set_cursor_position(&self, _position: LogicalPosition) -> Result<(), ExternalError> {
        Err(NotSupportedError::new())
    }

    #[inline]
    pub fn set_maximized(&self, _maximized: bool) {
        // N/A
        // iOS has single screen maximized apps so nothing to do
    }

    #[inline]
    pub fn fullscreen(&self) -> Option<RootMonitorHandle> {
        // N/A
        // iOS has single screen maximized apps so nothing to do
        None
    }

    #[inline]
    pub fn set_fullscreen(&self, _monitor: Option<RootMonitorHandle>) {
        // N/A
        // iOS has single screen maximized apps so nothing to do
    }

    #[inline]
    pub fn set_decorations(&self, _decorations: bool) {
        // N/A
    }

    #[inline]
    pub fn set_always_on_top(&self, _always_on_top: bool) {
        // N/A
    }

    #[inline]
    pub fn set_window_icon(&self, _icon: Option<::Icon>) {
        // N/A
    }

    #[inline]
    pub fn set_ime_spot(&self, _logical_spot: LogicalPosition) {
        // N/A
    }

    #[inline]
    pub fn current_monitor(&self) -> RootMonitorHandle {
        RootMonitorHandle { inner: MonitorHandle }
    }

    #[inline]
    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        let mut rb = VecDeque::with_capacity(1);
        rb.push_back(MonitorHandle);
        rb
    }

    #[inline]
    pub fn primary_monitor(&self) -> MonitorHandle {
        MonitorHandle
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        WindowId
    }
}

fn create_delegate_class() {
    extern fn did_finish_launching(this: &mut Object, _: Sel, _: id, _: id) -> BOOL {
        let screen_class = class!(UIScreen);
        let window_class = class!(UIWindow);
        let controller_class = class!(UIViewController);
        unsafe {
            let main_screen: id = msg_send![screen_class, mainScreen];
            let bounds: CGRect = msg_send![main_screen, bounds];
            let scale: CGFloat = msg_send![main_screen, nativeScale];

            let window: id = msg_send![window_class, alloc];
            let window: id = msg_send![window, initWithFrame:bounds.clone()];

            let size = (bounds.size.width as f64, bounds.size.height as f64).into();

            let view_controller: id = msg_send![controller_class, alloc];
            let view_controller: id = msg_send![view_controller, init];

            let _: () = msg_send![window, setRootViewController:view_controller];

            let state = Box::new(DelegateState::new(window, view_controller, ptr::null_mut(), size, scale as f64));
            let state_ptr: *mut DelegateState = mem::transmute(state);
            this.set_ivar("winitState", state_ptr as *mut c_void);

            // The `UIView` is setup in `Window::new` which gets `longjmp`'ed to here.
            // This makes it easier to configure the specific `UIView` type.
            let _: () = msg_send![this, performSelector:sel!(postLaunch:) withObject:nil afterDelay:0.0];
        }
        YES
    }

    extern fn post_launch(_: &Object, _: Sel, _: id) {
        unsafe { longjmp(mem::transmute_copy(&mut JMPBUF), 1); }
    }

    extern fn did_become_active(this: &Object, _: Sel, _: id) {
        unsafe {
            let events_queue: *mut c_void = *this.get_ivar("eventsQueue");
            let events_queue = &*(events_queue as *const RefCell<VecDeque<Event>>);
            events_queue.borrow_mut().push_back(Event::WindowEvent {
                window_id: RootEventId(WindowId),
                event: WindowEvent::Focused(true),
            });
        }
    }

    extern fn will_resign_active(this: &Object, _: Sel, _: id) {
        unsafe {
            let events_queue: *mut c_void = *this.get_ivar("eventsQueue");
            let events_queue = &*(events_queue as *const RefCell<VecDeque<Event>>);
            events_queue.borrow_mut().push_back(Event::WindowEvent {
                window_id: RootEventId(WindowId),
                event: WindowEvent::Focused(false),
            });
        }
    }

    extern fn will_enter_foreground(this: &Object, _: Sel, _: id) {
        unsafe {
            let events_queue: *mut c_void = *this.get_ivar("eventsQueue");
            let events_queue = &*(events_queue as *const RefCell<VecDeque<Event>>);
            events_queue.borrow_mut().push_back(Event::Suspended(false));
        }
    }

    extern fn did_enter_background(this: &Object, _: Sel, _: id) {
        unsafe {
            let events_queue: *mut c_void = *this.get_ivar("eventsQueue");
            let events_queue = &*(events_queue as *const RefCell<VecDeque<Event>>);
            events_queue.borrow_mut().push_back(Event::Suspended(true));
        }
    }

    extern fn will_terminate(this: &Object, _: Sel, _: id) {
        unsafe {
            let events_queue: *mut c_void = *this.get_ivar("eventsQueue");
            let events_queue = &*(events_queue as *const RefCell<VecDeque<Event>>);
            // push event to the front to garantee that we'll process it
            // immidiatly after jump
            events_queue.borrow_mut().push_front(Event::WindowEvent {
                window_id: RootEventId(WindowId),
                event: WindowEvent::Destroyed,
            });
            longjmp(mem::transmute_copy(&mut JMPBUF), 1);
        }
    }

    extern fn handle_touches(this: &Object, _: Sel, touches: id, _:id) {
        unsafe {
            let events_queue: *mut c_void = *this.get_ivar("eventsQueue");
            let events_queue = &*(events_queue as *const RefCell<VecDeque<Event>>);

            let touches_enum: id = msg_send![touches, objectEnumerator];

            loop {
                let touch: id = msg_send![touches_enum, nextObject];
                if touch == nil {
                    break
                }
                let location: CGPoint = msg_send![touch, locationInView:nil];
                let touch_id = touch as u64;
                let phase: i32 = msg_send![touch, phase];

                events_queue.borrow_mut().push_back(Event::WindowEvent {
                    window_id: RootEventId(WindowId),
                    event: WindowEvent::Touch(Touch {
                        device_id: DEVICE_ID,
                        id: touch_id,
                        location: (location.x as f64, location.y as f64).into(),
                        phase: match phase {
                            0 => TouchPhase::Started,
                            1 => TouchPhase::Moved,
                            // 2 is UITouchPhaseStationary and is not expected here
                            3 => TouchPhase::Ended,
                            4 => TouchPhase::Cancelled,
                            _ => panic!("unexpected touch phase: {:?}", phase)
                        }
                    }),
                });
            }
        }
    }
}

unsafe impl Send for DeviceId {}
unsafe impl Sync for DeviceId {}
