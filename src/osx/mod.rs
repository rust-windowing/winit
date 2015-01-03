#[cfg(feature = "headless")]
pub use self::headless::HeadlessContext;

use {CreationError, Event};
use CreationError::OsError;
use libc;

use BuilderAttribs;

use cocoa::base::{id, NSUInteger, nil, objc_allocateClassPair, class, objc_registerClassPair};
use cocoa::base::{selector, msg_send, class_addMethod, class_addIvar};
use cocoa::base::{object_setInstanceVariable, object_getInstanceVariable};
use cocoa::appkit;
use cocoa::appkit::*;

use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use core_foundation::bundle::{CFBundleGetBundleWithIdentifier, CFBundleGetFunctionPointerForName};

use std::cell::Cell;
use std::c_str::CString;
use std::mem;
use std::ptr;
use std::collections::RingBuf;

use events::Event::{MouseInput, MouseMoved, ReceivedCharacter, KeyboardInput, MouseWheel};
use events::ElementState::{Pressed, Released};
use events::MouseButton::{LeftMouseButton, RightMouseButton};
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

static DELEGATE_NAME: &'static [u8] = b"glutin_window_delegate\0";
static DELEGATE_STATE_IVAR: &'static [u8] = b"glutin_state";

struct DelegateState<'a> {
    is_closed: bool,
    context: id,
    view: id,
    handler: Option<fn(uint, uint)>,
}

pub struct Window {
    view: id,
    window: id,
    context: id,
    delegate: id,
    resize: Option<fn(uint, uint)>,

    is_closed: Cell<bool>,
}

#[cfg(feature = "window")]
impl Window {
    pub fn new(builder: BuilderAttribs) -> Result<Window, CreationError> {
        if builder.sharing.is_some() {
            unimplemented!()
        }

        Window::new_impl(builder.dimensions, builder.title.as_slice(), builder.monitor, builder.vsync, builder.visible)
    }
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
                NSEvent::otherEventWithType_location_modifierFlags_timestamp_windowNumber_context_subtype_data1_data2(
                nil,
                NSApplicationDefined,
                NSPoint::new(0.0, 0.0),
                0,
                0.0,
                0,
                ptr::null_mut(),
                0,
                0,
                0);
            NSApp().postEvent_atStart_(event, true);
            pool.drain();
        }
    }
}

extern fn window_should_close(this: id, _: id) -> id {
    unsafe {
        let mut stored_value = ptr::null_mut();
        object_getInstanceVariable(this, DELEGATE_STATE_IVAR.as_ptr() as *const i8, &mut stored_value);
        let state = stored_value as *mut DelegateState;

        (*state).is_closed = true;
    }
    0
}

extern fn window_did_resize(this: id, _: id) -> id {
    unsafe {
        let mut stored_value = ptr::null_mut();
        object_getInstanceVariable(this, DELEGATE_STATE_IVAR.as_ptr() as *const i8, &mut stored_value);
        let state = &mut *(stored_value as *mut DelegateState);

        let _: id = msg_send()(state.context, selector("update"));

        match state.handler {
            Some(handler) => {
                let rect = NSView::frame(state.view);
                (handler)(rect.size.width as uint, rect.size.height as uint);
            }
            None => {}
        }
    }
    0
}

impl Window {
    fn new_impl(dimensions: Option<(uint, uint)>, title: &str, monitor: Option<MonitorID>,
                vsync: bool, visible: bool) -> Result<Window, CreationError> {
        let app = match Window::create_app() {
            Some(app) => app,
            None      => { return Err(OsError(format!("Couldn't create NSApplication"))); },
        };
        let window = match Window::create_window(dimensions.unwrap_or((800, 600)), title, monitor) {
            Some(window) => window,
            None         => { return Err(OsError(format!("Couldn't create NSWindow"))); },
        };
        let view = match Window::create_view(window) {
            Some(view) => view,
            None       => { return Err(OsError(format!("Couldn't create NSView"))); },
        };

        let context = match Window::create_context(view, vsync) {
            Some(context) => context,
            None          => { return Err(OsError(format!("Couldn't create OpenGL context"))); },
        };

        unsafe {
            app.activateIgnoringOtherApps_(true);
            if visible {
                window.makeKeyAndOrderFront_(nil);
            } else {
                window.makeKeyWindow();
            }
        }

        // Set up the window delegate to receive events
        let ptr_size = mem::size_of::<libc::intptr_t>() as u64;
        let ns_object = class("NSObject");

        let delegate = unsafe {
            // Create a delegate class, add callback methods and store InternalState as user data.
            let delegate = objc_allocateClassPair(ns_object, DELEGATE_NAME.as_ptr() as *const i8, 0);
            class_addMethod(delegate, selector("windowShouldClose:"), window_should_close, "B@:@".to_c_str().as_ptr());
            class_addMethod(delegate, selector("windowDidResize:"), window_did_resize, "V@:@".to_c_str().as_ptr());
            class_addIvar(delegate, DELEGATE_STATE_IVAR.as_ptr() as *const i8, ptr_size, 3, "?".to_c_str().as_ptr());
            objc_registerClassPair(delegate);

            let del_obj = msg_send()(delegate, selector("alloc"));
            let del_obj: id = msg_send()(del_obj, selector("init"));
            let _: id = msg_send()(window, selector("setDelegate:"), del_obj);
            del_obj
        };

        let window = Window {
            view: view,
            window: window,
            context: context,
            delegate: delegate,
            resize: None,

            is_closed: Cell::new(false),
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

    fn create_window(dimensions: (uint, uint), title: &str, monitor: Option<MonitorID>) -> Option<id> {
        unsafe {
            let scr_frame = match monitor {
                Some(_) => {
                    let screen = NSScreen::mainScreen(nil);
                    NSScreen::frame(screen)
                }
                None    => {
                    let (width, height) = dimensions;
                    NSRect::new(NSPoint::new(0., 0.), NSSize::new(width as f64, height as f64))
                }
            };

             let masks = match monitor {
                Some(_) => NSBorderlessWindowMask as NSUInteger,
                None    => NSTitledWindowMask as NSUInteger |
                           NSClosableWindowMask as NSUInteger |
                           NSMiniaturizableWindowMask as NSUInteger |
                           NSResizableWindowMask as NSUInteger,
            };

            let window = NSWindow::alloc(nil).initWithContentRect_styleMask_backing_defer_(
                scr_frame,
                masks,
                NSBackingStoreBuffered,
                false,
            );

            if window == nil {
                None
            } else {
                let title = NSString::alloc(nil).init_str(title);
                window.setTitle_(title);
                window.setAcceptsMouseMovedEvents_(true);
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
                view.setWantsBestResolutionOpenGLSurface_(true);
                window.setContentView_(view);
                Some(view)
            }
        }
    }

    fn create_context(view: id, vsync: bool) -> Option<id> {
        unsafe {
            let attributes = [
                NSOpenGLPFADoubleBuffer as uint,
                NSOpenGLPFAClosestPolicy as uint,
                NSOpenGLPFAColorSize as uint, 24,
                NSOpenGLPFAAlphaSize as uint, 8,
                NSOpenGLPFADepthSize as uint, 24,
                NSOpenGLPFAStencilSize as uint, 8,
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
    }

    pub fn hide(&self) {
    }

    pub fn get_position(&self) -> Option<(int, int)> {
        unimplemented!()
    }

    pub fn set_position(&self, _x: int, _y: int) {
        unimplemented!()
    }

    pub fn get_inner_size(&self) -> Option<(uint, uint)> {
        let rect = unsafe { NSView::frame(self.view) };
        Some((rect.size.width as uint, rect.size.height as uint))
    }

    pub fn get_outer_size(&self) -> Option<(uint, uint)> {
        unimplemented!()
    }

    pub fn set_inner_size(&self, _x: uint, _y: uint) {
        unimplemented!()
    }

    pub fn create_window_proxy(&self) -> WindowProxy {
        WindowProxy
    }

    pub fn poll_events(&self) -> RingBuf<Event> {
        let mut events = RingBuf::new();

        loop {
            unsafe {
                let event = NSApp().nextEventMatchingMask_untilDate_inMode_dequeue_(
                    NSAnyEventMask as u64,
                    NSDate::distantPast(nil),
                    NSDefaultRunLoopMode,
                    true);
                if event == nil { break; }
                {
                    // Create a temporary structure with state that delegates called internally
                    // by sendEvent can read and modify. When that returns, update window state.
                    // This allows the synchronous resize loop to continue issuing callbacks
                    // to the user application, by passing handler through to the delegate state.
                    let mut ds = DelegateState {
                        is_closed: self.is_closed.get(),
                        context: self.context,
                        view: self.view,
                        handler: self.resize,
                    };
                    object_setInstanceVariable(self.delegate,
                        DELEGATE_STATE_IVAR.as_ptr() as *const i8,
                        &mut ds as *mut DelegateState as *mut libc::c_void);
                    NSApp().sendEvent_(event);
                    object_setInstanceVariable(self.delegate,
                        DELEGATE_STATE_IVAR.as_ptr() as *const i8,
                        ptr::null_mut());
                    self.is_closed.set(ds.is_closed);
}

                match event.get_type() {
                    NSLeftMouseDown         => { events.push_back(MouseInput(Pressed, LeftMouseButton)); },
                    NSLeftMouseUp           => { events.push_back(MouseInput(Released, LeftMouseButton)); },
                    NSRightMouseDown        => { events.push_back(MouseInput(Pressed, RightMouseButton)); },
                    NSRightMouseUp          => { events.push_back(MouseInput(Released, RightMouseButton)); },
                    NSMouseMoved            => {
                        let window_point = event.locationInWindow();
                        let view_point = self.view.convertPoint_fromView_(window_point, nil);
                        events.push_back(MouseMoved((view_point.x as int, view_point.y as int)));
                    },
                    NSKeyDown               => {
                        let received_str = CString::new(event.characters().UTF8String(), false);
                        for received_char in received_str.as_str().unwrap().chars() {
                            if received_char.is_ascii() {
                                events.push_back(ReceivedCharacter(received_char));
                            }
                        }

                        let vkey =  event::vkeycode_to_element(event.keycode());
                        events.push_back(KeyboardInput(Pressed, event.keycode() as u8, vkey));
                    },
                    NSKeyUp                 => {
                        let vkey =  event::vkeycode_to_element(event.keycode());
                        events.push_back(KeyboardInput(Released, event.keycode() as u8, vkey));
                    },
                    NSFlagsChanged          => {
                        let shift_modifier = Window::modifier_event(event, appkit::NSShiftKeyMask as u64, events::VirtualKeyCode::LShift, shift_pressed);
                        if shift_modifier.is_some() {
                            shift_pressed = !shift_pressed;
                            events.push_back(shift_modifier.unwrap());
                        }
                        let ctrl_modifier = Window::modifier_event(event, appkit::NSControlKeyMask as u64, events::VirtualKeyCode::LControl, ctrl_pressed);
                        if ctrl_modifier.is_some() {
                            ctrl_pressed = !ctrl_pressed;
                            events.push_back(ctrl_modifier.unwrap());
                        }
                        let win_modifier = Window::modifier_event(event, appkit::NSCommandKeyMask as u64, events::VirtualKeyCode::LWin, win_pressed);
                        if win_modifier.is_some() {
                            win_pressed = !win_pressed;
                            events.push_back(win_modifier.unwrap());
                        }
                        let alt_modifier = Window::modifier_event(event, appkit::NSAlternateKeyMask as u64, events::VirtualKeyCode::LAlt, alt_pressed);
                        if alt_modifier.is_some() {
                            alt_pressed = !alt_pressed;
                            events.push_back(alt_modifier.unwrap());
                        }
                    },
                    NSScrollWheel           => { events.push_back(MouseWheel(-event.scrollingDeltaY() as i32)); },
                    NSOtherMouseDown        => { },
                    NSOtherMouseUp          => { },
                    NSOtherMouseDragged     => { },
                    _                       => { },
                }
            }
        }
        events
    }

    unsafe fn modifier_event(event: id, keymask: u64, key: events::VirtualKeyCode, key_pressed: bool) -> Option<Event> {
        if !key_pressed && Window::modifier_key_pressed(event, keymask) {
            return Some(KeyboardInput(Pressed, event.keycode() as u8, Some(key)));
        }
        else if key_pressed && !Window::modifier_key_pressed(event, keymask) {
            return Some(KeyboardInput(Released, event.keycode() as u8, Some(key)));
        }

        return None;
    }

    unsafe fn modifier_key_pressed(event: id, modifier: u64) -> bool {
        event.modifierFlags() & modifier != 0
    }

    pub fn wait_events(&self) -> RingBuf<Event> {
        unsafe {
            let event = NSApp().nextEventMatchingMask_untilDate_inMode_dequeue_(
                NSAnyEventMask as u64,
                NSDate::distantFuture(nil),
                NSDefaultRunLoopMode,
                false);
            NSApp().sendEvent_(event);

            self.poll_events()
        }
    }

    pub unsafe fn make_current(&self) {
        self.context.makeCurrentContext();
    }

    pub fn get_proc_address(&self, _addr: &str) -> *const () {
        let symbol_name: CFString = from_str(_addr).unwrap();
        let framework_name: CFString = from_str("com.apple.opengl").unwrap();
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

    pub fn get_api(&self) -> ::Api {
        ::Api::OpenGl
    }

    pub fn set_window_resize_callback(&mut self, callback: Option<fn(uint, uint)>) {
        self.resize = callback;
    }
}
