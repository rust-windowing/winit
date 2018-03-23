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
//! pub extern fn start_glutin_app() {
//!     start_inner()
//! }
//!
//! fn start_inner() {
//!    ...
//! }
//!
//! ```
//!
//! Compile project and then drag resulting .a into Xcode project. Add glutin.h to xcode.
//!
//! ```ignore
//! void start_glutin_app();
//! ```
//!
//! Use start_glutin_app inside your xcode's main function.
//!
//!
//! # App lifecycle and events
//!
//! iOS environment is very different from other platforms and you must be very
//! careful with it's events. Familiarize yourself with
//! [app lifecycle](https://developer.apple.com/library/ios/documentation/UIKit/Reference/UIApplicationDelegate_Protocol/).
//!
//!
//! This is how those event are represented in glutin:
//!
//!  - applicationDidBecomeActive is Focused(true)
//!  - applicationWillResignActive is Focused(false)
//!  - applicationDidEnterBackground is Suspended(true)
//!  - applicationWillEnterForeground is Suspended(false)
//!  - applicationWillTerminate is Closed
//!
//! Keep in mind that after Closed event is received every attempt to draw with
//! opengl will result in segfault.
//!
//! Also note that app will not receive Closed event if suspended, it will be SIGKILL'ed

#![cfg(target_os = "ios")]

use std::collections::VecDeque;
use std::ptr;
use std::mem;
use std::os::raw::c_void;

use libc;
use libc::c_int;
use objc::runtime::{Class, Object, Sel, BOOL, YES };
use objc::declare::{ ClassDecl };

use { CreationError, CursorState, MouseCursor, WindowAttributes };
use WindowId as RootEventId;
use WindowEvent;
use Event;
use events::{ Touch, TouchPhase };
use window::MonitorId as RootMonitorId;

mod ffi;
use self::ffi::{
    setjmp,
    UIApplicationMain,
    CFTimeInterval,
    CFRunLoopRunInMode,
    kCFRunLoopDefaultMode,
    kCFRunLoopRunHandledSource,
    id,
    nil,
    NSString,
    CGFloat,
    longjmp,
    CGRect,
    CGPoint
 };

static mut jmpbuf: [c_int;27] = [0;27];

#[derive(Clone)]
pub struct MonitorId;

pub struct Window {
    delegate_state: *mut DelegateState
}

#[derive(Clone)]
pub struct WindowProxy;

#[derive(Debug)]
struct DelegateState {
    events_queue: VecDeque<Event>,
    window: id,
    controller: id,
    size: (u32,u32),
    scale: f32
}


impl DelegateState {
    #[inline]
    fn new(window: id, controller:id, size: (u32,u32), scale: f32) -> DelegateState {
        DelegateState {
            events_queue: VecDeque::new(),
            window: window,
            controller: controller,
            size: size,
            scale: scale
        }
    }
}

impl MonitorId {
    #[inline]
    pub fn get_name(&self) -> Option<String> {
        Some("Primary".to_string())
    }

    #[inline]
    pub fn get_dimensions(&self) -> (u32, u32) {
        unimplemented!()
    }

    #[inline]
    pub fn get_position(&self) -> (i32, i32) {
        // iOS assumes single screen
        (0, 0)
    }

    #[inline]
    pub fn get_hidpi_factor(&self) -> f32 {
        1.0
    }
}

pub struct EventsLoop {
    delegate_state: *mut DelegateState
}

#[derive(Clone)]
pub struct EventsLoopProxy;

impl EventsLoop {
    pub fn new() -> EventsLoop {
        unsafe {
            if setjmp(mem::transmute(&mut jmpbuf)) != 0 {
                let app: id = msg_send![Class::get("UIApplication").unwrap(), sharedApplication];
                let delegate: id = msg_send![app, delegate];
                let state: *mut c_void = *(&*delegate).get_ivar("glutinState");
                let state = state as *mut DelegateState;

                let events_loop = EventsLoop {
                    delegate_state: state
                };

                return events_loop;
            }
        }

        create_delegate_class();
        create_view_class();
        start_app();

        panic!("Couldn't create UIApplication")
    }

    #[inline]
    pub fn get_available_monitors(&self) -> VecDeque<MonitorId> {
        let mut rb = VecDeque::new();
        rb.push_back(MonitorId);
        rb
    }

    #[inline]
    pub fn get_primary_monitor(&self) -> MonitorId {
        MonitorId
    }

    pub fn poll_events<F>(&mut self, mut callback: F)
        where F: FnMut(::Event)
    {
        unsafe {
            let state = &mut *self.delegate_state;

            if let Some(event) = state.events_queue.pop_front() {
                callback(event);
                return;
            }

            // jump hack, so we won't quit on willTerminate event before processing it
            if setjmp(mem::transmute(&mut jmpbuf)) != 0 {
                if let Some(event) = state.events_queue.pop_front() {
                    callback(event);
                    return;
                }
            }

            // run runloop
            let seconds: CFTimeInterval = 0.000002;
            while CFRunLoopRunInMode(kCFRunLoopDefaultMode, seconds, 1) == kCFRunLoopRunHandledSource {}

            if let Some(event) = state.events_queue.pop_front() {
                callback(event)
            }
        }
    }

    pub fn run_forever<F>(&mut self, mut callback: F)
        where F: FnMut(::Event) -> ::ControlFlow,
    {
        // Yeah that's a very bad implementation.
        loop {
            let mut control_flow = ::ControlFlow::Continue;
            self.poll_events(|e| {
                if let ::ControlFlow::Break = callback(e) {
                    control_flow = ::ControlFlow::Break;
                }
            });
            if let ::ControlFlow::Break = control_flow {
                break;
            }
            ::std::thread::sleep(::std::time::Duration::from_millis(5));
        }
    }

    pub fn create_proxy(&self) -> EventsLoopProxy {
        EventsLoopProxy
    }
}

impl EventsLoopProxy {
    pub fn wakeup(&self) -> Result<(), ::EventsLoopClosed> {
        unimplemented!()
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId;

#[derive(Clone, Default)]
pub struct PlatformSpecificWindowBuilderAttributes;

impl Window {
    pub fn new(ev: &EventsLoop, _: &WindowAttributes, _: &PlatformSpecificWindowBuilderAttributes)
               -> Result<Window, CreationError>
    {
        Ok(Window {
            delegate_state: ev.delegate_state,
        })
    }

    #[inline]
    pub fn set_title(&self, _: &str) {
    }

    #[inline]
    pub fn show(&self) {
    }

    #[inline]
    pub fn hide(&self) {
    }

    #[inline]
    pub fn get_position(&self) -> Option<(i32, i32)> {
        None
    }

    #[inline]
    pub fn set_position(&self, _x: i32, _y: i32) {
    }

    #[inline]
    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        unsafe { Some((&*self.delegate_state).size) }
    }

    #[inline]
    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        self.get_inner_size()
    }

    #[inline]
    pub fn set_inner_size(&self, _x: u32, _y: u32) {
    }

    #[inline]
    pub fn set_min_dimensions(&self, _dimensions: Option<(u32, u32)>) { }

    #[inline]
    pub fn set_max_dimensions(&self, _dimensions: Option<(u32, u32)>) { }

    #[inline]
    pub fn platform_display(&self) -> *mut libc::c_void {
        unimplemented!();
    }

    #[inline]
    pub fn platform_window(&self) -> *mut libc::c_void {
        unimplemented!()
    }

    #[inline]
    pub fn set_window_resize_callback(&mut self, _: Option<fn(u32, u32)>) {
    }

    #[inline]
    pub fn set_cursor(&self, _: MouseCursor) {
    }

    #[inline]
    pub fn set_cursor_state(&self, _: CursorState) -> Result<(), String> {
        Ok(())
    }

    #[inline]
    pub fn hidpi_factor(&self) -> f32 {
        unsafe { (&*self.delegate_state) }.scale
    }

    #[inline]
    pub fn set_cursor_position(&self, _x: i32, _y: i32) -> Result<(), ()> {
        unimplemented!();
    }

    #[inline]
    pub fn create_window_proxy(&self) -> WindowProxy {
        WindowProxy
    }

    #[inline]
    pub fn set_maximized(&self, _maximized: bool) {
        // iOS has single screen maximized apps so nothing to do
    }

    #[inline]
    pub fn set_fullscreen(&self, _monitor: Option<RootMonitorId>) {
        // iOS has single screen maximized apps so nothing to do
    }

    #[inline]
    pub fn set_decorations(&self, _decorations: bool) {
        // N/A
    }

    #[inline]
    pub fn get_current_monitor(&self) -> RootMonitorId {
        RootMonitorId{inner: MonitorId}
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        WindowId
    }
}

fn create_delegate_class() {
    extern fn did_finish_launching(this: &mut Object, _: Sel, _: id, _: id) -> BOOL {
        unsafe {
            let main_screen: id = msg_send![Class::get("UIScreen").unwrap(), mainScreen];
            let bounds: CGRect = msg_send![main_screen, bounds];
            let scale: CGFloat = msg_send![main_screen, nativeScale];

            let window: id = msg_send![Class::get("UIWindow").unwrap(), alloc];
            let window: id = msg_send![window, initWithFrame:bounds.clone()];

            let size = (bounds.size.width as u32, bounds.size.height as u32);

            let view_controller: id = msg_send![Class::get("MainViewController").unwrap(), alloc];
            let view_controller: id = msg_send![view_controller, init];

            let _: () = msg_send![window, setRootViewController:view_controller];
            let _: () = msg_send![window, makeKeyAndVisible];

            let state = Box::new(DelegateState::new(window, view_controller, size, scale as f32));
            let state_ptr: *mut DelegateState = mem::transmute(state);
            this.set_ivar("glutinState", state_ptr as *mut c_void);


            let _: () = msg_send![this, performSelector:sel!(postLaunch:) withObject:nil afterDelay:0.0];
        }
        YES
    }

    extern fn post_launch(_: &Object, _: Sel, _: id) {
        unsafe { longjmp(mem::transmute(&mut jmpbuf),1); }
    }

    extern fn did_become_active(this: &Object, _: Sel, _: id) {
        unsafe {
            let state: *mut c_void = *this.get_ivar("glutinState");
            let state = &mut *(state as *mut DelegateState);
            state.events_queue.push_back(Event::WindowEvent {
                window_id: RootEventId(WindowId),
                event: WindowEvent::Focused(true),
            });
        }
    }

    extern fn will_resign_active(this: &Object, _: Sel, _: id) {
        unsafe {
            let state: *mut c_void = *this.get_ivar("glutinState");
            let state = &mut *(state as *mut DelegateState);
            state.events_queue.push_back(Event::WindowEvent {
                window_id: RootEventId(WindowId),
                event: WindowEvent::Focused(false),
            });
        }
    }

    extern fn will_enter_foreground(this: &Object, _: Sel, _: id) {
        unsafe {
            let state: *mut c_void = *this.get_ivar("glutinState");
            let state = &mut *(state as *mut DelegateState);
            state.events_queue.push_back(Event::Suspended(false));
        }
    }

    extern fn did_enter_background(this: &Object, _: Sel, _: id) {
        unsafe {
            let state: *mut c_void = *this.get_ivar("glutinState");
            let state = &mut *(state as *mut DelegateState);
            state.events_queue.push_back(Event::Suspended(true));
        }
    }

    extern fn will_terminate(this: &Object, _: Sel, _: id) {
        unsafe {
            let state: *mut c_void = *this.get_ivar("glutinState");
            let state = &mut *(state as *mut DelegateState);
            // push event to the front to garantee that we'll process it
            // immidiatly after jump
            state.events_queue.push_front(Event::WindowEvent {
                window_id: RootEventId(WindowId),
                event: WindowEvent::Closed,
            });
            longjmp(mem::transmute(&mut jmpbuf),1);
        }
    }

    extern fn handle_touches(this: &Object, _: Sel, touches: id, _:id) {
        unsafe {
            let state: *mut c_void = *this.get_ivar("glutinState");
            let state = &mut *(state as *mut DelegateState);

            let touches_enum: id = msg_send![touches, objectEnumerator];

            loop {
                let touch: id = msg_send![touches_enum, nextObject];
                if touch == nil {
                    break
                }
                let location: CGPoint = msg_send![touch, locationInView:nil];
                let touch_id = touch as u64;
                let phase: i32 = msg_send![touch, phase];

                state.events_queue.push_back(Event::WindowEvent {
                    window_id: RootEventId(WindowId),
                    event: WindowEvent::Touch(Touch {
                        device_id: DEVICE_ID,
                        id: touch_id,
                        location: (location.x as f64, location.y as f64),
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

    let ui_responder = Class::get("UIResponder").unwrap();
    let mut decl = ClassDecl::new("AppDelegate", ui_responder).unwrap();

    unsafe {
        decl.add_method(sel!(application:didFinishLaunchingWithOptions:),
                        did_finish_launching as extern fn(&mut Object, Sel, id, id) -> BOOL);

        decl.add_method(sel!(applicationDidBecomeActive:),
                        did_become_active as extern fn(&Object, Sel, id));

        decl.add_method(sel!(applicationWillResignActive:),
                        will_resign_active as extern fn(&Object, Sel, id));

        decl.add_method(sel!(applicationWillEnterForeground:),
                        will_enter_foreground as extern fn(&Object, Sel, id));

        decl.add_method(sel!(applicationDidEnterBackground:),
                        did_enter_background as extern fn(&Object, Sel, id));

        decl.add_method(sel!(applicationWillTerminate:),
                        will_terminate as extern fn(&Object, Sel, id));


        decl.add_method(sel!(touchesBegan:withEvent:),
                        handle_touches as extern fn(this: &Object, _: Sel, _: id, _:id));

        decl.add_method(sel!(touchesMoved:withEvent:),
                        handle_touches as extern fn(this: &Object, _: Sel, _: id, _:id));

        decl.add_method(sel!(touchesEnded:withEvent:),
                        handle_touches as extern fn(this: &Object, _: Sel, _: id, _:id));

        decl.add_method(sel!(touchesCancelled:withEvent:),
                        handle_touches as extern fn(this: &Object, _: Sel, _: id, _:id));


        decl.add_method(sel!(postLaunch:),
                        post_launch as extern fn(&Object, Sel, id));

        decl.add_ivar::<*mut c_void>("glutinState");

        decl.register();
    }
}

fn create_view_class() {
    let ui_view_controller = Class::get("UIViewController").unwrap();
    let decl = ClassDecl::new("MainViewController", ui_view_controller).unwrap();

    decl.register();
}

#[inline]
fn start_app() {
    unsafe {
        UIApplicationMain(0, ptr::null(), nil, NSString::alloc(nil).init_str("AppDelegate"));
    }
}

// Constant device ID, to be removed when this backend is updated to report real device IDs.
const DEVICE_ID: ::DeviceId = ::DeviceId(DeviceId);
