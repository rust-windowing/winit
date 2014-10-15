use Event;
use std::sync::atomics::AtomicBool;

#[cfg(feature = "window")]
use WindowBuilder;

#[cfg(feature = "headless")]
use HeadlessRendererBuilder;

use cocoa::base::{id, NSUInteger, nil};
use cocoa::appkit::*;

use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use core_foundation::bundle::{CFBundleGetBundleWithIdentifier, CFBundleGetFunctionPointerForName};

use std::c_str::CString;
use {MouseInput, Pressed, Released, LeftMouseButton, RightMouseButton, MouseMoved, ReceivedCharacter,
     KeyboardInput};

mod event;

pub struct Window {
    view: id,
    context: id,
    is_closed: AtomicBool,
}

pub struct HeadlessContext(Window);

impl Deref<Window> for HeadlessContext {
    fn deref(&self) -> &Window {
        &self.0
    }
}

pub struct MonitorID;

pub fn get_available_monitors() -> Vec<MonitorID> {
    unimplemented!()
}

pub fn get_primary_monitor() -> MonitorID {
    unimplemented!()
}

impl MonitorID {
    pub fn get_name(&self) -> Option<String> {
        unimplemented!()
    }

    pub fn get_dimensions(&self) -> (uint, uint) {
        unimplemented!()
    }
}

#[cfg(feature = "window")]
impl Window {
    pub fn new(builder: WindowBuilder) -> Result<Window, String> {
        Window::new_impl(builder.dimensions, builder.title.as_slice(), true)
    }
}

#[cfg(feature = "headless")]
impl HeadlessContext {
    pub fn new(builder: HeadlessRendererBuilder) -> Result<HeadlessContext, String> {
        Window::new_impl(Some(builder.dimensions), "", false)
            .map(|w| HeadlessContext(w))
    }
}

impl Window {
    fn new_impl(dimensions: Option<(uint, uint)>, title: &str, visible: bool) -> Result<Window, String> {
        let app = match Window::create_app() {
            Some(app) => app,
            None      => { return Err(format!("Couldn't create NSApplication")); },
        };
        let window = match Window::create_window(dimensions.unwrap_or((800, 600)), title) {
            Some(window) => window,
            None         => { return Err(format!("Couldn't create NSWindow")); },
        };
        let view = match Window::create_view(window) {
            Some(view) => view,
            None       => { return Err(format!("Couldn't create NSView")); },
        };

        let context = match Window::create_context(view) {
            Some(context) => context,
            None          => { return Err(format!("Couldn't create OpenGL context")); },
        };

        unsafe {
            app.activateIgnoringOtherApps_(true);
            window.makeKeyAndOrderFront_(nil);
        }

        let window = Window {
            view: view,
            context: context,
            is_closed: AtomicBool::new(false),
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

    fn create_window(dimensions: (uint, uint), title: &str) -> Option<id> {
        unsafe {
            let (width, height) = dimensions;
            let scr_frame = NSRect::new(NSPoint::new(0., 0.), NSSize::new(width as f64, height as f64));

            let window = NSWindow::alloc(nil).initWithContentRect_styleMask_backing_defer_(
                scr_frame,
                NSTitledWindowMask as NSUInteger | NSClosableWindowMask as NSUInteger | NSMiniaturizableWindowMask as NSUInteger,
                NSBackingStoreBuffered,
                false
            );

            if window == nil {
                None
            } else {
                let title = NSString::alloc(nil).init_str(title);
                window.setTitle_(title);
                window.center();
                window.setAcceptsMouseMovedEvents_(true);
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

            let pixelformat = NSOpenGLPixelFormat::alloc(nil).initWithAttributes_(attributes);
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
        use std::sync::atomics::Relaxed;
        self.is_closed.load(Relaxed)
    }

    pub fn set_title(&self, _title: &str) {
        unimplemented!()
    }

    pub fn get_position(&self) -> Option<(int, int)> {
        unimplemented!()
    }

    pub fn set_position(&self, _x: int, _y: int) {
        unimplemented!()
    }

    pub fn get_inner_size(&self) -> Option<(uint, uint)> {
        unimplemented!()
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
                        let modifiers = event::modifierflag_to_element(event.modifierFlags());
                        events.push(KeyboardInput(Pressed, event.keycode() as u8, vkey, modifiers));
                    },
                    NSKeyUp                 => {
                        let vkey =  event::vkeycode_to_element(event.keycode());
                        let modifiers = event::modifierflag_to_element(event.modifierFlags());
                        events.push(KeyboardInput(Released, event.keycode() as u8, vkey, modifiers));
                    },
                    NSFlagsChanged          => {
                        println!("Modifiers: {}", event.modifierFlags());
                        // Need to keep an array of the modified flags
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
}
