#[cfg(feature = "headless")]
pub use self::headless::HeadlessContext;

use {CreationError, Event, MouseCursor};
use CreationError::OsError;
use libc;

use Api;
use BuilderAttribs;
use GlRequest;

use cocoa::base::{Class, id, YES, NO, NSUInteger, nil, objc_allocateClassPair, class, objc_registerClassPair};
use cocoa::base::{selector, msg_send, msg_send_stret, class_addMethod, class_addIvar};
use cocoa::base::{object_setInstanceVariable, object_getInstanceVariable};
use cocoa::appkit;
use cocoa::appkit::*;
use cocoa::appkit::NSEventSubtype::*;

use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use core_foundation::bundle::{CFBundleGetBundleWithIdentifier, CFBundleGetFunctionPointerForName};

use std::cell::Cell;
use std::ffi::{CString, CStr};
use std::mem;
use std::ptr;
use std::collections::VecDeque;
use std::str::FromStr;
use std::str::from_utf8;
use std::sync::Mutex;
use std::ascii::AsciiExt;

use events::Event::{Awakened, MouseInput, MouseMoved, ReceivedCharacter, KeyboardInput, MouseWheel};
use events::ElementState::{Pressed, Released};
use events::MouseButton;
use events;

pub use self::monitor::{MonitorID, get_available_monitors, get_primary_monitor};

mod monitor;
mod event;

#[cfg(feature = "headless")]
mod headless;

static mut shift_pressed: bool = false;
static mut ctrl_pressed: bool = false;
static mut win_pressed: bool = false;
static mut alt_pressed: bool = false;

struct DelegateState {
    is_closed: bool,
    context: id,
    view: id,
    window: id,
    handler: Option<fn(u32, u32)>,
}

struct WindowDelegate {
    this: id,
}

impl WindowDelegate {
    fn class_name() -> &'static [u8] {
        b"GlutinWindowDelegate\0"
    }

    fn state_ivar_name() -> &'static [u8] {
        b"glutinState"
    }

    /// Get the delegate class, initiailizing it neccessary
    fn class() -> Class {
        use std::sync::{Once, ONCE_INIT};
        use std::rt;

        extern fn window_should_close(this: id, _: id) -> id {
            unsafe {
                let delegate = WindowDelegate { this: this };
                (*delegate.get_state()).is_closed = true;
                mem::forget(delegate);
            }
            0
        }

        extern fn window_did_resize(this: id, _: id) -> id {
            unsafe {
                let delegate = WindowDelegate { this: this };
                let state = &mut *delegate.get_state();
                mem::forget(delegate);

                let _: id = msg_send()(state.context, selector("update"));

                if let Some(handler) = state.handler {
                    let rect = NSView::frame(state.view);
                    let scale_factor = NSWindow::backingScaleFactor(state.window) as f32;
                    (handler)((scale_factor * rect.size.width as f32) as u32,
                              (scale_factor * rect.size.height as f32) as u32);
                }
            }
            0
        }

        static mut delegate_class: Class = nil;
        static mut init: Once = ONCE_INIT;

        unsafe {
            init.call_once(|| {
                let ptr_size = mem::size_of::<libc::intptr_t>();
                    // Create new NSWindowDelegate
                    delegate_class = objc_allocateClassPair(
                        class("NSObject"),
                        WindowDelegate::class_name().as_ptr() as *const i8, 0);
                    // Add callback methods
                    class_addMethod(delegate_class,
                                    selector("windowShouldClose:"),
                                    window_should_close,
                                    CString::new("B@:@").unwrap().as_ptr());
                    class_addMethod(delegate_class,
                                    selector("windowDidResize:"),
                                    window_did_resize,
                                    CString::new("V@:@").unwrap().as_ptr());
                    // Store internal state as user data
                    class_addIvar(delegate_class, WindowDelegate::state_ivar_name().as_ptr() as *const i8,
                                  ptr_size as u64, 3,
                                  CString::new("?").unwrap().as_ptr());
                    objc_registerClassPair(delegate_class);
                // Free class at exit
                rt::at_exit(|| {
                    // objc_disposeClassPair(delegate_class);
                });
            });
            delegate_class
        }
    }

    fn new(window: id) -> WindowDelegate {
        unsafe {
            let delegate: id = msg_send()(WindowDelegate::class(), selector("new"));
            let _: id = msg_send()(window, selector("setDelegate:"), delegate);
            WindowDelegate { this: delegate }
        }
    }

    unsafe fn set_state(&self, state: *mut DelegateState) {
        object_setInstanceVariable(self.this,
                                   WindowDelegate::state_ivar_name().as_ptr() as *const i8,
                                   state as *mut libc::c_void);
    }

    fn get_state(&self) -> *mut DelegateState {
        unsafe {
            let mut state = ptr::null_mut();
            object_getInstanceVariable(self.this,
                                       WindowDelegate::state_ivar_name().as_ptr() as *const i8,
                                       &mut state);
            state as *mut DelegateState
        }
    }
}

pub struct Window {
    view: id,
    window: id,
    context: id,
    delegate: WindowDelegate,
    resize: Option<fn(u32, u32)>,

    is_closed: Cell<bool>,

    /// Events that have been retreived with XLib but not dispatched with iterators yet
    pending_events: Mutex<VecDeque<Event>>,
}

#[cfg(feature = "window")]
unsafe impl Send for Window {}
#[cfg(feature = "window")]
unsafe impl Sync for Window {}

#[cfg(feature = "window")]
#[derive(Clone)]
pub struct WindowProxy;

impl WindowProxy {
    pub fn wakeup_event_loop(&self) {
        unsafe {
            let pool = NSAutoreleasePool::new(nil);
            let event =
                NSEvent::otherEventWithType_location_modifierFlags_timestamp_windowNumber_context_subtype_data1_data2_(
                    nil, NSApplicationDefined, NSPoint::new(0.0, 0.0), NSEventModifierFlags::empty(),
                    0.0, 0, nil, NSApplicationActivatedEventType, 0, 0);
            NSApp().postEvent_atStart_(event, YES);
            pool.drain();
        }
    }
}

pub struct PollEventsIterator<'a> {
    window: &'a Window,
}

impl<'a> Iterator for PollEventsIterator<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        if let Some(ev) = self.window.pending_events.lock().unwrap().pop_front() {
            return Some(ev);
        }

        unsafe {
            let event = NSApp().nextEventMatchingMask_untilDate_inMode_dequeue_(
                NSAnyEventMask.bits(),
                NSDate::distantPast(nil),
                NSDefaultRunLoopMode,
                YES);
            if event == nil { return None; }
            {
                // Create a temporary structure with state that delegates called internally
                // by sendEvent can read and modify. When that returns, update window state.
                // This allows the synchronous resize loop to continue issuing callbacks
                // to the user application, by passing handler through to the delegate state.
                let mut ds = DelegateState {
                    is_closed: self.window.is_closed.get(),
                    context: self.window.context,
                    window: self.window.window,
                    view: self.window.view,
                    handler: self.window.resize,
                };
                self.window.delegate.set_state(&mut ds);
                NSApp().sendEvent_(event);
                self.window.delegate.set_state(ptr::null_mut());
                self.window.is_closed.set(ds.is_closed);
            }

            let event = match msg_send()(event, selector("type")) {
                NSLeftMouseDown         => { Some(MouseInput(Pressed, MouseButton::Left)) },
                NSLeftMouseUp           => { Some(MouseInput(Released, MouseButton::Left)) },
                NSRightMouseDown        => { Some(MouseInput(Pressed, MouseButton::Right)) },
                NSRightMouseUp          => { Some(MouseInput(Released, MouseButton::Right)) },
                NSMouseMoved            |
                NSLeftMouseDragged      |
                NSOtherMouseDragged     |
                NSRightMouseDragged     => {
                    let window_point = event.locationInWindow();
                    let window: id = msg_send()(event, selector("window"));
                    let view_point = if window == 0 {
                        let window_rect = self.window.window.convertRectFromScreen_(NSRect::new(window_point, NSSize::new(0.0, 0.0)));
                        self.window.view.convertPoint_fromView_(window_rect.origin, nil)
                    } else {
                        self.window.view.convertPoint_fromView_(window_point, nil)
                    };
                    let view_rect = NSView::frame(self.window.view);
                    let scale_factor = self.window.hidpi_factor();
                    Some(MouseMoved(((scale_factor * view_point.x as f32) as i32,
                                    (scale_factor * (view_rect.size.height - view_point.y) as f32) as i32)))
                },
                NSKeyDown               => {
                    let mut events = VecDeque::new();
                    let received_c_str = event.characters().UTF8String();
                    let received_str = CStr::from_ptr(received_c_str);
                    for received_char in from_utf8(received_str.to_bytes()).unwrap().chars() {
                        if received_char.is_ascii() {
                            events.push_back(ReceivedCharacter(received_char));
                        }
                    }

                    let vkey =  event::vkeycode_to_element(NSEvent::keyCode(event));
                    events.push_back(KeyboardInput(Pressed, NSEvent::keyCode(event) as u8, vkey));
                    let event = events.pop_front();
                    self.window.pending_events.lock().unwrap().extend(events.into_iter());
                    event
                },
                NSKeyUp                 => {
                    let vkey =  event::vkeycode_to_element(NSEvent::keyCode(event));
                    Some(KeyboardInput(Released, NSEvent::keyCode(event) as u8, vkey))
                },
                NSFlagsChanged          => {
                    let mut events = VecDeque::new();
                    let shift_modifier = Window::modifier_event(event, appkit::NSShiftKeyMask, events::VirtualKeyCode::LShift, shift_pressed);
                    if shift_modifier.is_some() {
                        shift_pressed = !shift_pressed;
                        events.push_back(shift_modifier.unwrap());
                    }
                    let ctrl_modifier = Window::modifier_event(event, appkit::NSControlKeyMask, events::VirtualKeyCode::LControl, ctrl_pressed);
                    if ctrl_modifier.is_some() {
                        ctrl_pressed = !ctrl_pressed;
                        events.push_back(ctrl_modifier.unwrap());
                    }
                    let win_modifier = Window::modifier_event(event, appkit::NSCommandKeyMask, events::VirtualKeyCode::LWin, win_pressed);
                    if win_modifier.is_some() {
                        win_pressed = !win_pressed;
                        events.push_back(win_modifier.unwrap());
                    }
                    let alt_modifier = Window::modifier_event(event, appkit::NSAlternateKeyMask, events::VirtualKeyCode::LAlt, alt_pressed);
                    if alt_modifier.is_some() {
                        alt_pressed = !alt_pressed;
                        events.push_back(alt_modifier.unwrap());
                    }
                    let event = events.pop_front();
                    self.window.pending_events.lock().unwrap().extend(events.into_iter());
                    event
                },
                NSScrollWheel           => { Some(MouseWheel(event.scrollingDeltaY() as i32)) },
                _                       => { None },
            };

            event
        }
    }
}

pub struct WaitEventsIterator<'a> {
    window: &'a Window,
}

impl<'a> Iterator for WaitEventsIterator<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        loop {
            if let Some(ev) = self.window.pending_events.lock().unwrap().pop_front() {
                return Some(ev);
            }

            unsafe {
                let event = NSApp().nextEventMatchingMask_untilDate_inMode_dequeue_(
                    NSAnyEventMask.bits(),
                    NSDate::distantFuture(nil),
                    NSDefaultRunLoopMode,
                    NO);
            }

            // calling poll_events()
            if let Some(ev) = self.window.poll_events().next() {
                return Some(ev);
            } else {
                return Some(Awakened);
            }
        }
    }
}

impl Window {
    #[cfg(feature = "window")]
    pub fn new(builder: BuilderAttribs) -> Result<Window, CreationError> {
        if builder.sharing.is_some() {
            unimplemented!()
        }

        let app = match Window::create_app() {
            Some(app) => app,
            None      => { return Err(OsError(format!("Couldn't create NSApplication"))); },
        };
        let window = match Window::create_window(builder.dimensions.unwrap_or((800, 600)),
                                                 &*builder.title,
                                                 builder.monitor)
        {
            Some(window) => window,
            None         => { return Err(OsError(format!("Couldn't create NSWindow"))); },
        };
        let view = match Window::create_view(window) {
            Some(view) => view,
            None       => { return Err(OsError(format!("Couldn't create NSView"))); },
        };

        let context = match Window::create_context(view, builder.vsync, builder.gl_version) {
            Some(context) => context,
            None          => { return Err(OsError(format!("Couldn't create OpenGL context"))); },
        };

        unsafe {
            app.activateIgnoringOtherApps_(YES);
            if builder.visible {
                window.makeKeyAndOrderFront_(nil);
            } else {
                window.makeKeyWindow();
            }
        }

        let window = Window {
            view: view,
            window: window,
            context: context,
            delegate: WindowDelegate::new(window),
            resize: None,

            is_closed: Cell::new(false),
            pending_events: Mutex::new(VecDeque::new()),
        };

        Ok(window)
    }

    fn create_app() -> Option<id> {
        unsafe {
            let app = NSApp();
            if app == nil {
                None
            } else {
                app.setActivationPolicy_(NSApplicationActivationPolicyRegular);
                app.finishLaunching();
                Some(app)
            }
        }
    }

    fn create_window(dimensions: (u32, u32), title: &str, monitor: Option<MonitorID>) -> Option<id> {
        unsafe {
            let frame = if monitor.is_some() {
                let screen = NSScreen::mainScreen(nil);
                NSScreen::frame(screen)
            } else {
                let (width, height) = dimensions;
                NSRect::new(NSPoint::new(0., 0.), NSSize::new(width as f64, height as f64))
            };

            let masks = if monitor.is_some() {
                NSBorderlessWindowMask as NSUInteger
            } else {
                NSTitledWindowMask as NSUInteger |
                NSClosableWindowMask as NSUInteger |
                NSMiniaturizableWindowMask as NSUInteger |
                NSResizableWindowMask as NSUInteger
            };

            let window = NSWindow::alloc(nil).initWithContentRect_styleMask_backing_defer_(
                frame,
                masks,
                NSBackingStoreBuffered,
                NO,
            );

            if window == nil {
                None
            } else {
                let title = NSString::alloc(nil).init_str(title);
                window.setTitle_(title);
                window.setAcceptsMouseMovedEvents_(YES);
                if monitor.is_some() {
                    window.setLevel_(NSMainMenuWindowLevel as i64 + 1);
                }
                else {
                    window.center();
                }
                Some(window)
            }
        }
    }

    fn create_view(window: id) -> Option<id> {
        unsafe {
            let view = NSView::alloc(nil).init();
            if view == nil {
                None
            } else {
                view.setWantsBestResolutionOpenGLSurface_(YES);
                window.setContentView_(view);
                Some(view)
            }
        }
    }

    fn create_context(view: id, vsync: bool, gl_version: GlRequest) -> Option<id> {
        let profile = match gl_version {
            GlRequest::Latest => NSOpenGLProfileVersion4_1Core as u32,
            GlRequest::Specific(Api::OpenGl, (1 ... 2, _)) => NSOpenGLProfileVersionLegacy as u32,
            GlRequest::Specific(Api::OpenGl, (3, 0)) => NSOpenGLProfileVersionLegacy as u32,
            GlRequest::Specific(Api::OpenGl, (3, 1 ... 2)) => NSOpenGLProfileVersion3_2Core as u32,
            GlRequest::Specific(Api::OpenGl, _) => NSOpenGLProfileVersion4_1Core as u32,
            GlRequest::Specific(_, _) => panic!("Only the OpenGL API is supported"),    // FIXME: return Result
            GlRequest::GlThenGles { opengl_version: (1 ... 2, _), .. } => NSOpenGLProfileVersionLegacy as u32,
            GlRequest::GlThenGles { opengl_version: (3, 0), .. } => NSOpenGLProfileVersionLegacy as u32,
            GlRequest::GlThenGles { opengl_version: (3, 1 ... 2), .. } => NSOpenGLProfileVersion3_2Core as u32,
            GlRequest::GlThenGles { .. } => NSOpenGLProfileVersion4_1Core as u32,
        };
        unsafe {
            let attributes = [
                NSOpenGLPFADoubleBuffer as u32,
                NSOpenGLPFAClosestPolicy as u32,
                NSOpenGLPFAColorSize as u32, 24,
                NSOpenGLPFAAlphaSize as u32, 8,
                NSOpenGLPFADepthSize as u32, 24,
                NSOpenGLPFAStencilSize as u32, 8,
                NSOpenGLPFAOpenGLProfile as u32, profile,
                0
            ];

            let pixelformat = NSOpenGLPixelFormat::alloc(nil).initWithAttributes_(&attributes);
            if pixelformat == nil {
                return None;
            }

            let context = NSOpenGLContext::alloc(nil).initWithFormat_shareContext_(pixelformat, nil);
            if context == nil {
                None
            } else {
                context.setView_(view);
                if vsync {
                    let value = 1;
                    context.setValues_forParameter_(&value, NSOpenGLContextParameter::NSOpenGLCPSwapInterval);
                }
                Some(context)
            }
        }
    }

    pub fn is_closed(&self) -> bool {
        self.is_closed.get()
    }

    pub fn set_title(&self, title: &str) {
        unsafe {
            let title = NSString::alloc(nil).init_str(title);
            self.window.setTitle_(title);
        }
    }

    pub fn show(&self) {
        unsafe { NSWindow::makeKeyAndOrderFront_(self.window, nil); }
    }

    pub fn hide(&self) {
        unsafe { NSWindow::orderOut_(self.window, nil); }
    }

    pub fn get_position(&self) -> Option<(i32, i32)> {
        unsafe {
            let content_rect = NSWindow::contentRectForFrameRect_(self.window, NSWindow::frame(self.window));
            // NOTE: coordinate system might be inconsistent with other backends
            Some((content_rect.origin.x as i32, content_rect.origin.y as i32))
        }
    }

    pub fn set_position(&self, x: i32, y: i32) {
        unsafe {
            // NOTE: coordinate system might be inconsistent with other backends
            NSWindow::setFrameOrigin_(self.window, NSPoint::new(x as f64, y as f64));
        }
    }

    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        unsafe {
            let view_frame = NSView::frame(self.view);
            Some((view_frame.size.width as u32, view_frame.size.height as u32))
        }
    }

    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        unsafe {
            let window_frame = NSWindow::frame(self.window);
            Some((window_frame.size.width as u32, window_frame.size.height as u32))
        }
    }

    pub fn set_inner_size(&self, width: u32, height: u32) {
        unsafe {
            NSWindow::setContentSize_(self.window, NSSize::new(width as f64, height as f64));
        }
    }

    pub fn create_window_proxy(&self) -> WindowProxy {
        WindowProxy
    }

    pub fn poll_events(&self) -> PollEventsIterator {
        PollEventsIterator {
            window: self
        }
    }

    pub fn wait_events(&self) -> WaitEventsIterator {
        WaitEventsIterator {
            window: self
        }
    }

    unsafe fn modifier_event(event: id, keymask: NSEventModifierFlags, key: events::VirtualKeyCode, key_pressed: bool) -> Option<Event> {
        if !key_pressed && NSEvent::modifierFlags(event).contains(keymask) {
            return Some(KeyboardInput(Pressed, NSEvent::keyCode(event) as u8, Some(key)));
        } else if key_pressed && !NSEvent::modifierFlags(event).contains(keymask) {
            return Some(KeyboardInput(Released, NSEvent::keyCode(event) as u8, Some(key)));
        }

        return None;
    }

    pub unsafe fn make_current(&self) {
        let _: id = msg_send()(self.context, selector("update"));
        self.context.makeCurrentContext();
    }

    pub fn is_current(&self) -> bool {
        unimplemented!()
    }

    pub fn get_proc_address(&self, _addr: &str) -> *const () {
        let symbol_name: CFString = FromStr::from_str(_addr).unwrap();
        let framework_name: CFString = FromStr::from_str("com.apple.opengl").unwrap();
        let framework = unsafe {
            CFBundleGetBundleWithIdentifier(framework_name.as_concrete_TypeRef())
        };
        let symbol = unsafe {
            CFBundleGetFunctionPointerForName(framework, symbol_name.as_concrete_TypeRef())
        };
        symbol as *const ()
    }

    pub fn swap_buffers(&self) {
        unsafe { self.context.flushBuffer(); }
    }

    pub fn platform_display(&self) -> *mut libc::c_void {
        unimplemented!()
    }

    pub fn platform_window(&self) -> *mut libc::c_void {
        unimplemented!()
    }

    pub fn get_api(&self) -> ::Api {
        ::Api::OpenGl
    }

    pub fn set_window_resize_callback(&mut self, callback: Option<fn(u32, u32)>) {
        self.resize = callback;
    }

    pub fn set_cursor(&self, cursor: MouseCursor) {
        let cursor_name = match cursor {                
            MouseCursor::Arrow => "arrowCursor",
            MouseCursor::Text => "IBeamCursor",
            MouseCursor::ContextMenu => "contextualMenuCursor",
            MouseCursor::Copy => "dragCopyCursor",
            MouseCursor::Crosshair => "crosshairCursor",
            MouseCursor::Default => "arrowCursor",
            MouseCursor::Grabbing => "openHandCursor",
            MouseCursor::Hand | MouseCursor::Grab => "pointingHandCursor",
            MouseCursor::NoDrop => "operationNotAllowedCursor",
            MouseCursor::NotAllowed => "operationNotAllowedCursor",
            MouseCursor::Alias => "dragLinkCursor",
            
            
            /// Resize cursors
            MouseCursor::EResize | MouseCursor::NResize |
            MouseCursor::NeResize | MouseCursor::NwResize |
            MouseCursor::SResize | MouseCursor::SeResize |
            MouseCursor::SwResize | MouseCursor::WResize |
            MouseCursor::EwResize | MouseCursor::ColResize |
            MouseCursor::NsResize | MouseCursor::RowResize |
            MouseCursor::NwseResize | MouseCursor::NeswResize => "arrowCursor",

            /// TODO: Find appropriate OSX cursors
             MouseCursor::Cell | MouseCursor::VerticalText | MouseCursor::NoneCursor |
            MouseCursor::Wait | MouseCursor::Progress | MouseCursor::Help |
            MouseCursor::Move | MouseCursor::AllScroll | MouseCursor::ZoomIn |
            MouseCursor::ZoomOut => "arrowCursor",
        };
        unsafe {
            let cursor : id = msg_send()(class("NSCursor"), selector(cursor_name));
            let _ : id = msg_send()(cursor, selector("set"));
        }
    }

    pub fn hidpi_factor(&self) -> f32 {
        unsafe {
            NSWindow::backingScaleFactor(self.window) as f32
        }
    }

    pub fn set_cursor_position(&self, x: i32, y: i32) -> Result<(), ()> {
        unimplemented!();
    }
}
