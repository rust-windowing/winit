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
//! ```c
//! void start_glutin_app();
//! ```
//!
//! Use start_glutin_app inside your xcode's main function.
//!
//!
//! # App lifecycle and events
//!
//! iOS environment is very different from other platforms and you must be very
//! careful with it's events. Familiarize yourself with [app lifecycle](https://developer.apple.com/library/ios/documentation/UIKit/Reference/UIApplicationDelegate_Protocol/).
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
//! Keep in mind that after Closed event is received every attempt to draw with opengl will result in segfault.
//!
//! Also note that app will not receive Closed event if suspended, it will be SIGKILL'ed




#![cfg(target_os = "ios")]
#![deny(warnings)]

use std::collections::VecDeque;
use std::ptr;
use std::io;
use std::mem;
use std::ffi::CString;

use libc;
use objc::runtime::{Class, BOOL, YES, NO };

use native_monitor::NativeMonitorId;
use { Api, PixelFormat, CreationError, GlContext, CursorState, MouseCursor, Event };
use { PixelFormatRequirements, GlAttributes, WindowAttributes, ContextError };
use CreationError::OsError;

mod delegate;
use self::delegate::{ create_delegate_class, create_view_class };

mod ffi;
use self::ffi::{
    gles,
    setjmp,
    dlopen,
    dlsym,
    UIApplicationMain,
    kEAGLColorFormatRGB565,
    CFTimeInterval,
    CFRunLoopRunInMode,
    kCFRunLoopDefaultMode,
    kCFRunLoopRunHandledSource,
    kEAGLDrawablePropertyRetainedBacking,
    kEAGLDrawablePropertyColorFormat,
    RTLD_LAZY,
    RTLD_GLOBAL,
    id,
    nil,
    NSString,
    CGFloat
 };


static mut jmpbuf: [libc::c_int;27] = [0;27];

#[derive(Clone)]
pub struct MonitorId;

pub struct Window {
    eagl_context: id,
    delegate_state: *mut DelegateState
}

#[derive(Clone)]
pub struct WindowProxy;

pub struct PollEventsIterator<'a> {
    window: &'a Window,
}

pub struct WaitEventsIterator<'a> {
    window: &'a Window,
}

#[derive(Debug)]
struct DelegateState {
    events_queue: VecDeque<Event>,
    window: id,
    controller: id,
    view: id,
    size: (u32,u32),
    scale: f32
}


impl DelegateState {
    #[inline]
    fn new(window: id, controller:id, view: id, size: (u32,u32), scale: f32) -> DelegateState {
        DelegateState {
            events_queue: VecDeque::new(),
            window: window,
            controller: controller,
            view: view,
            size: size,
            scale: scale
        }
    }
}

#[inline]
pub fn get_available_monitors() -> VecDeque<MonitorId> {
    let mut rb = VecDeque::new();
    rb.push_back(MonitorId);
    rb
}

#[inline]
pub fn get_primary_monitor() -> MonitorId {
    MonitorId
}

impl MonitorId {
    #[inline]
    pub fn get_name(&self) -> Option<String> {
        Some("Primary".to_string())
    }

    #[inline]
    pub fn get_native_identifier(&self) -> NativeMonitorId {
        NativeMonitorId::Unavailable
    }

    #[inline]
    pub fn get_dimensions(&self) -> (u32, u32) {
        unimplemented!()
    }
}

#[derive(Clone, Default)]
pub struct PlatformSpecificWindowBuilderAttributes;

impl Window {

    pub fn new(builder: &WindowAttributes, _: &PixelFormatRequirements, _: &GlAttributes<&Window>,
               _: &PlatformSpecificWindowBuilderAttributes) -> Result<Window, CreationError>
    {
        unsafe {
            if setjmp(mem::transmute(&mut jmpbuf)) != 0 {
                let app: id = msg_send![Class::get("UIApplication").unwrap(), sharedApplication];
                let delegate: id = msg_send![app, delegate];
                let state: *mut libc::c_void = *(&*delegate).get_ivar("glutinState");
                let state = state as *mut DelegateState;

                let context = Window::create_context();

                let mut window = Window {
                    eagl_context: context,
                    delegate_state: state
                };

                window.init_context(builder);

                return Ok(window)
            }
        }

        create_delegate_class();
        create_view_class();
        Window::start_app();

        Err(CreationError::OsError(format!("Couldn't create UIApplication")))
    }

    unsafe fn init_context(&mut self, builder: &WindowAttributes) {
        let draw_props: id = msg_send![Class::get("NSDictionary").unwrap(), alloc];
            let draw_props: id = msg_send![draw_props,
                    initWithObjects:
                        vec![
                            msg_send![Class::get("NSNumber").unwrap(), numberWithBool: NO],
                            kEAGLColorFormatRGB565
                        ].as_ptr()
                    forKeys:
                        vec![
                            kEAGLDrawablePropertyRetainedBacking,
                            kEAGLDrawablePropertyColorFormat
                        ].as_ptr()
                    count: 2
            ];
        let _ = self.make_current();

        let state = &mut *self.delegate_state;

        if builder.multitouch {
            let _: () = msg_send![state.view, setMultipleTouchEnabled:YES];
        }

        let _: () = msg_send![state.view, setContentScaleFactor:state.scale as CGFloat];

        let layer: id = msg_send![state.view, layer];
        let _: () = msg_send![layer, setContentsScale:state.scale as CGFloat];
        let _: () = msg_send![layer, setDrawableProperties: draw_props];

        let gl = gles::Gles2::load_with(|symbol| self.get_proc_address(symbol));
        let mut color_render_buf: gles::types::GLuint = 0;
        let mut frame_buf: gles::types::GLuint = 0;
        gl.GenRenderbuffers(1, &mut color_render_buf);
        gl.BindRenderbuffer(gles::RENDERBUFFER, color_render_buf);

        let ok: BOOL = msg_send![self.eagl_context, renderbufferStorage:gles::RENDERBUFFER fromDrawable:layer];
        if ok != YES {
            panic!("EAGL: could not set renderbufferStorage");
        }

        gl.GenFramebuffers(1, &mut frame_buf);
        gl.BindFramebuffer(gles::FRAMEBUFFER, frame_buf);

        gl.FramebufferRenderbuffer(gles::FRAMEBUFFER, gles::COLOR_ATTACHMENT0, gles::RENDERBUFFER, color_render_buf);

        let status = gl.CheckFramebufferStatus(gles::FRAMEBUFFER);
        if gl.CheckFramebufferStatus(gles::FRAMEBUFFER) != gles::FRAMEBUFFER_COMPLETE {
            panic!("framebuffer status: {:?}", status);
        }
    }

    fn create_context() -> id {
        unsafe {
            let eagl_context: id = msg_send![Class::get("EAGLContext").unwrap(), alloc];
            let eagl_context: id = msg_send![eagl_context, initWithAPI:2]; // es2
            eagl_context
        }
    }

    #[inline]
    fn start_app() {
        unsafe {
            UIApplicationMain(0, ptr::null(), nil, NSString::alloc(nil).init_str("AppDelegate"));
        }
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
    pub fn poll_events(&self) -> PollEventsIterator {
        PollEventsIterator {
            window: self
        }
    }

    #[inline]
    pub fn wait_events(&self) -> WaitEventsIterator {
        WaitEventsIterator {
            window: self
        }
    }

    #[inline]
    pub fn platform_display(&self) -> *mut libc::c_void {
        unimplemented!();
    }

    #[inline]
    pub fn platform_window(&self) -> *mut libc::c_void {
        unimplemented!()
    }

    #[inline]
    pub fn get_pixel_format(&self) -> PixelFormat {
        unimplemented!();
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

}

impl GlContext for Window {
    #[inline]
    unsafe fn make_current(&self) -> Result<(), ContextError> {
        let res: BOOL = msg_send![Class::get("EAGLContext").unwrap(), setCurrentContext: self.eagl_context];
        if res == YES {
            Ok(())
        } else {
            Err(ContextError::IoError(io::Error::new(io::ErrorKind::Other, "EAGLContext::setCurrentContext unsuccessful")))
        }
    }

    #[inline]
    fn is_current(&self) -> bool {
        false
    }

    fn get_proc_address(&self, addr: &str) -> *const () {
        let addr_c = CString::new(addr).unwrap();
        let path = CString::new("/System/Library/Frameworks/OpenGLES.framework/OpenGLES").unwrap();
        unsafe {
            let lib = dlopen(path.as_ptr(), RTLD_LAZY | RTLD_GLOBAL);
            dlsym(lib, addr_c.as_ptr()) as *const _
        }
    }

    #[inline]
    fn swap_buffers(&self) -> Result<(), ContextError> {
        unsafe {
            let res: BOOL = msg_send![self.eagl_context, presentRenderbuffer: gles::RENDERBUFFER];
            if res == YES {
                Ok(())
            } else {
                Err(ContextError::IoError(io::Error::new(io::ErrorKind::Other, "EAGLContext.presentRenderbuffer unsuccessful")))
            }
        }
    }

    #[inline]
    fn get_api(&self) -> Api {
        unimplemented!()
    }

    #[inline]
    fn get_pixel_format(&self) -> PixelFormat {
        unimplemented!()
    }
}

impl WindowProxy {
    #[inline]
    pub fn wakeup_event_loop(&self) {
        unimplemented!()
    }
}


impl<'a> Iterator for WaitEventsIterator<'a> {
    type Item = Event;

    #[inline]
    fn next(&mut self) -> Option<Event> {
        loop {
            if let Some(ev) = self.window.poll_events().next() {
                return Some(ev);
            }
        }
    }
}

impl<'a> Iterator for PollEventsIterator<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        unsafe {
            let state = &mut *self.window.delegate_state;

            if let Some(event) = state.events_queue.pop_front() {
                return Some(event)
            }

            // jump hack, so we won't quit on willTerminate event before processing it
            if setjmp(mem::transmute(&mut jmpbuf)) != 0 {
                return state.events_queue.pop_front()
            }

            // run runloop
            let seconds: CFTimeInterval = 0.000002;
            while CFRunLoopRunInMode(kCFRunLoopDefaultMode, seconds, 1) == kCFRunLoopRunHandledSource {}

            state.events_queue.pop_front()
        }
    }
}
