use {CreationError, Event};
use CreationError::OsError;
use libc;

#[cfg(feature = "window")]
use WindowBuilder;

#[cfg(feature = "headless")]
use HeadlessRendererBuilder;

use cocoa::base::{id, NSUInteger, nil, objc_allocateClassPair, class, objc_registerClassPair};
use cocoa::base::{selector, msg_send, class_addMethod, class_addIvar};
use cocoa::base::{object_setInstanceVariable, object_getInstanceVariable};
use cocoa::appkit;
use cocoa::appkit::*;

use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use core_foundation::bundle::{CFBundleGetBundleWithIdentifier, CFBundleGetFunctionPointerForName};

use std::c_str::CString;
use std::mem;
use std::ptr;
use std::sync::atomic::{AtomicBool, Relaxed};

use events::Event::{MouseInput, MouseMoved, ReceivedCharacter, KeyboardInput};
use events::ElementState::{Pressed, Released};
use events::MouseButton::{LeftMouseButton, RightMouseButton};
use events;

pub use self::monitor::{MonitorID, get_available_monitors, get_primary_monitor};

mod monitor;
mod event;

static mut shift_pressed: bool = false;
static mut ctrl_pressed: bool = false;
static mut win_pressed: bool = false;
static mut alt_pressed: bool = false;

static DELEGATE_NAME: &'static [u8] = b"glutin_window_delegate\0";
static DELEGATE_THIS_IVAR: &'static [u8] = b"glutin_this";

struct InternalState {
    is_closed: AtomicBool,
}

impl InternalState {
    fn new() -> InternalState {
        InternalState {
            is_closed: AtomicBool::new(false),
        }
    }
}

pub struct Window {
    view: id,
    window: id,
    context: id,
    state: Box<InternalState>,
}

pub struct HeadlessContext(Window);

impl Deref<Window> for HeadlessContext {
    fn deref(&self) -> &Window {
        &self.0
    }
}

#[cfg(feature = "window")]
impl Window {
    pub fn new(builder: WindowBuilder) -> Result<Window, CreationError> {
        if builder.sharing.is_some() {
            unimplemented!()
        }

        Window::new_impl(builder.dimensions, builder.title.as_slice(), builder.monitor, true)
    }
}

#[cfg(feature = "headless")]
impl HeadlessContext {
    pub fn new(builder: HeadlessRendererBuilder) -> Result<HeadlessContext, CreationError> {
        Window::new_impl(Some(builder.dimensions), "", None, false)
            .map(|w| HeadlessContext(w))
    }
}

extern fn window_should_close(this: id, _: id) -> id {
    unsafe {
        let mut stored_value = ptr::null_mut();
        object_getInstanceVariable(this, DELEGATE_THIS_IVAR.as_ptr() as *const i8, &mut stored_value);
        let state = stored_value as *mut InternalState;
        (*state).is_closed.store(true, Relaxed);
    }
    0
}

impl Window {
    fn new_impl(dimensions: Option<(uint, uint)>, title: &str, monitor: Option<MonitorID>, visible: bool) -> Result<Window, CreationError> {
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

        let context = match Window::create_context(view) {
            Some(context) => context,
            None          => { return Err(OsError(format!("Couldn't create OpenGL context"))); },
        };

        unsafe {
            app.activateIgnoringOtherApps_(true);
            window.makeKeyAndOrderFront_(nil);
        }

        let window = Window {
            view: view,
            window: window,
            context: context,
            state: box InternalState::new(),
        };

        // Set up the window delegate to receive events
        let ptr_size = mem::size_of::<libc::intptr_t>() as u64;
        let ns_object = class("NSObject");

        unsafe {
            // Create a delegate class, add callback methods and store InternalState as user data.
            let delegate = objc_allocateClassPair(ns_object, DELEGATE_NAME.as_ptr() as *const i8, 0);
            class_addMethod(delegate, selector("windowShouldClose:"), window_should_close, "B@:@".to_c_str().as_ptr());
            class_addIvar(delegate, DELEGATE_THIS_IVAR.as_ptr() as *const i8, ptr_size, 3, "?".to_c_str().as_ptr());
            objc_registerClassPair(delegate);

            let del_obj = msg_send()(delegate, selector("alloc"));
            let del_obj: id = msg_send()(del_obj, selector("init"));
            object_setInstanceVariable(del_obj, DELEGATE_THIS_IVAR.as_ptr() as *const i8,
                                        &*window.state as *const InternalState as *mut libc::c_void);
            let _: id = msg_send()(window.window, selector("setDelegate:"), del_obj);
        }

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
                window.center();
                window.setAcceptsMouseMovedEvents_(true);
                if monitor.is_some() {
                    window.setLevel_(NSMainMenuWindowLevel as i64 + 1);
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

    fn create_context(view: id) -> Option<id> {
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
                Some(context)
            }
        }
    }

    pub fn is_closed(&self) -> bool {
        self.state.is_closed.load(Relaxed)
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

    pub fn poll_events(&self) -> Vec<Event> {
        let mut events = Vec::new();

        loop {
            unsafe {
                let event = NSApp().nextEventMatchingMask_untilDate_inMode_dequeue_(
                    NSAnyEventMask as u64,
                    NSDate::distantPast(nil),
                    NSDefaultRunLoopMode,
                    true);
                if event == nil { break; }
                NSApp().sendEvent_(event);

                match event.get_type() {
                    NSLeftMouseDown         => { events.push(MouseInput(Pressed, LeftMouseButton)); },
                    NSLeftMouseUp           => { events.push(MouseInput(Released, LeftMouseButton)); },
                    NSRightMouseDown        => { events.push(MouseInput(Pressed, RightMouseButton)); },
                    NSRightMouseUp          => { events.push(MouseInput(Released, RightMouseButton)); },
                    NSMouseMoved            => {
                        let window_point = event.locationInWindow();
                        let view_point = self.view.convertPoint_fromView_(window_point, nil);
                        events.push(MouseMoved((view_point.x as int, view_point.y as int)));
                    },
                    NSKeyDown               => {
                        let received_str = CString::new(event.characters().UTF8String(), false);
                        for received_char in received_str.as_str().unwrap().chars() {
                            if received_char.is_ascii() {
                                events.push(ReceivedCharacter(received_char));
                            }
                        }

                        let vkey =  event::vkeycode_to_element(event.keycode());
                        events.push(KeyboardInput(Pressed, event.keycode() as u8, vkey));
                    },
                    NSKeyUp                 => {
                        let vkey =  event::vkeycode_to_element(event.keycode());
                        events.push(KeyboardInput(Released, event.keycode() as u8, vkey));
                    },
                    NSFlagsChanged          => {
                        let shift_modifier = Window::modifier_event(event, appkit::NSShiftKeyMask as u64, events::VirtualKeyCode::LShift, shift_pressed);
                        if shift_modifier.is_some() {
                            shift_pressed = !shift_pressed;
                            events.push(shift_modifier.unwrap());
                        }
                        let ctrl_modifier = Window::modifier_event(event, appkit::NSControlKeyMask as u64, events::VirtualKeyCode::LControl, ctrl_pressed);
                        if ctrl_modifier.is_some() {
                            ctrl_pressed = !ctrl_pressed;
                            events.push(ctrl_modifier.unwrap());
                        }
                        let win_modifier = Window::modifier_event(event, appkit::NSCommandKeyMask as u64, events::VirtualKeyCode::LWin, win_pressed);
                        if win_modifier.is_some() {
                            win_pressed = !win_pressed;
                            events.push(win_modifier.unwrap());
                        }
                        let alt_modifier = Window::modifier_event(event, appkit::NSAlternateKeyMask as u64, events::VirtualKeyCode::LAlt, alt_pressed);
                        if alt_modifier.is_some() {
                            alt_pressed = !alt_pressed;
                            events.push(alt_modifier.unwrap());
                        }
                    },
                    NSScrollWheel           => { },
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

    pub fn wait_events(&self) -> Vec<Event> {
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
}
