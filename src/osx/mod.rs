use {Event, WindowBuilder};

use cocoa::base::{id, NSUInteger, nil};
use cocoa::appkit::*;

use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use core_foundation::bundle::{CFBundleGetBundleWithIdentifier, CFBundleGetFunctionPointerForName};

pub struct Window {
    context: id,
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

impl Window {
    pub fn new(_builder: WindowBuilder) -> Result<Window, String> {
        let context = unsafe {
            // Create the NSApplication
            let app = NSApp();
            app.setActivationPolicy_(NSApplicationActivationPolicyRegular);

            app.finishLaunching();

            // Create the window
            let scr_frame = NSRect::new(NSPoint::new(0., 0.), NSSize::new(800., 600.));

            let window = NSWindow::alloc(nil).initWithContentRect_styleMask_backing_defer_(
                scr_frame,
                NSTitledWindowMask as NSUInteger | NSClosableWindowMask as NSUInteger | NSMiniaturizableWindowMask as NSUInteger,
                NSBackingStoreBuffered,
                false
            );
            let view = NSView::alloc(nil).init();
            view.setWantsBestResolutionOpenGLSurface_(true);

            let title = NSString::alloc(nil).init_str("Hello World!\0");
            window.setTitle_(title);
            window.setContentView(view);
            window.center();

            // Create the context
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
                return Err(format!("Couldn't create the pixel format"));
            }

            let context = NSOpenGLContext::alloc(nil).initWithFormat_shareContext_(pixelformat, nil);
            if context == nil {
                return Err(format!("No valid OpenGL context can be created with that pixelformat"));
            }

            context.setView_(view);

            app.activateIgnoringOtherApps_(true);
            window.makeKeyAndOrderFront_(nil);
            context
        };

        let window = Window {
            context: context,
        };

        Ok(window)
    }

    pub fn is_closed(&self) -> bool {
        // TODO: remove fake implementation
        false
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
        unimplemented!()
    }

    pub fn wait_events(&self) -> Vec<Event> {
        loop {
            unsafe {
                let event = NSApp().nextEventMatchingMask_untilDate_inMode_dequeue_(
                    NSAnyEventMask as u64,
                    nil,
                    NSDefaultRunLoopMode,
                    true);
                if event == nil { break; }
                NSApp().sendEvent_(event);
            }
        }
        // TODO: Remove fake implementation
        Vec::new()
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
