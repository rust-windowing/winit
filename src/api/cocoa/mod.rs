#![cfg(target_os = "macos")]

#[cfg(feature = "headless")]
pub use self::headless::HeadlessContext;

use {CreationError, Event, MouseCursor, CursorState};
use CreationError::OsError;
use libc;

use Api;
use BuilderAttribs;
use GlRequest;
use PixelFormat;
use native_monitor::NativeMonitorId;

use objc::runtime::{Class, Object, Sel, BOOL, YES, NO};
use objc::declare::ClassDecl;

use cocoa::base::{id, nil};
use cocoa::foundation::{NSAutoreleasePool, NSDate, NSDefaultRunLoopMode, NSPoint, NSRect, NSSize, 
                        NSString, NSUInteger}; 
use cocoa::appkit;
use cocoa::appkit::*;
use cocoa::appkit::NSEventSubtype::*;

use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use core_foundation::bundle::{CFBundleGetBundleWithIdentifier, CFBundleGetFunctionPointerForName};

use core_graphics::display::{CGMainDisplayID, CGDisplayPixelsHigh};

use std::ffi::CStr;
use std::collections::VecDeque;
use std::str::FromStr;
use std::str::from_utf8;
use std::sync::Mutex;
use std::ascii::AsciiExt;
use std::ops::Deref;

use events::Event::{Awakened, MouseInput, MouseMoved, ReceivedCharacter, KeyboardInput, MouseWheel, Closed};
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
    context: IdRef,
    view: IdRef,
    window: IdRef,
    resize_handler: Option<fn(u32, u32)>,

    /// Events that have been retreived with XLib but not dispatched with iterators yet
    pending_events: Mutex<VecDeque<Event>>,
}

struct WindowDelegate {
    state: Box<DelegateState>,
    _this: IdRef,
}

impl WindowDelegate {
    /// Get the delegate class, initiailizing it neccessary
    fn class() -> *const Class {
        use std::sync::{Once, ONCE_INIT};

        extern fn window_should_close(this: &Object, _: Sel, _: id) -> BOOL {
            unsafe {
                let state: *mut libc::c_void = *this.get_ivar("glutinState");
                let state = state as *mut DelegateState;
                (*state).is_closed = true;

                (*state).pending_events.lock().unwrap().push_back(Closed);
            }
            YES
        }

        extern fn window_did_resize(this: &Object, _: Sel, _: id) {
            unsafe {
                let state: *mut libc::c_void = *this.get_ivar("glutinState");
                let state = &mut *(state as *mut DelegateState);

                let _: () = msg_send![*state.context, update];

                if let Some(handler) = state.resize_handler {
                    let rect = NSView::frame(*state.view);
                    let scale_factor = NSWindow::backingScaleFactor(*state.window) as f32;
                    (handler)((scale_factor * rect.size.width as f32) as u32,
                              (scale_factor * rect.size.height as f32) as u32);
                }
            }
        }

        static mut delegate_class: *const Class = 0 as *const Class;
        static INIT: Once = ONCE_INIT;

        INIT.call_once(|| unsafe {
            // Create new NSWindowDelegate
            let superclass = Class::get("NSObject").unwrap();
            let mut decl = ClassDecl::new(superclass, "GlutinWindowDelegate").unwrap();

            // Add callback methods
            decl.add_method(sel!(windowShouldClose:),
                window_should_close as extern fn(&Object, Sel, id) -> BOOL);
            decl.add_method(sel!(windowDidResize:),
                window_did_resize as extern fn(&Object, Sel, id));

            // Store internal state as user data
            decl.add_ivar::<*mut libc::c_void>("glutinState");

            delegate_class = decl.register();
        });

        unsafe {
            delegate_class
        }
    }

    fn new(state: DelegateState) -> WindowDelegate {
        // Box the state so we can give a pointer to it
        let mut state = Box::new(state);
        let state_ptr: *mut DelegateState = &mut *state;
        unsafe {
            let delegate = IdRef::new(msg_send![WindowDelegate::class(), new]);

            (&mut **delegate).set_ivar("glutinState", state_ptr as *mut libc::c_void);
            let _: () = msg_send![*state.window, setDelegate:*delegate];

            WindowDelegate { state: state, _this: delegate }
        }
    }
}

impl Drop for WindowDelegate {
    fn drop(&mut self) {
        unsafe {
            // Nil the window's delegate so it doesn't still reference us
            let _: () = msg_send![*self.state.window, setDelegate:nil];
        }
    }
}

pub struct Window {
    view: IdRef,
    window: IdRef,
    context: IdRef,
    pixel_format: PixelFormat,
    delegate: WindowDelegate,
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
        if let Some(ev) = self.window.delegate.state.pending_events.lock().unwrap().pop_front() {
            return Some(ev);
        }

        unsafe {
            let event = NSApp().nextEventMatchingMask_untilDate_inMode_dequeue_(
                NSAnyEventMask.bits(),
                NSDate::distantPast(nil),
                NSDefaultRunLoopMode,
                YES);
            if event == nil { return None; }

            let event_type = msg_send![event, type];
            NSApp().sendEvent_(if let NSKeyDown = event_type { nil } else { event });

            let event = match event_type {
                NSLeftMouseDown         => { Some(MouseInput(Pressed, MouseButton::Left)) },
                NSLeftMouseUp           => { Some(MouseInput(Released, MouseButton::Left)) },
                NSRightMouseDown        => { Some(MouseInput(Pressed, MouseButton::Right)) },
                NSRightMouseUp          => { Some(MouseInput(Released, MouseButton::Right)) },
                NSMouseMoved            |
                NSLeftMouseDragged      |
                NSOtherMouseDragged     |
                NSRightMouseDragged     => {
                    let window_point = event.locationInWindow();
                    let window: id = msg_send![event, window];
                    let view_point = if window == nil {
                        let window_rect = self.window.window.convertRectFromScreen_(NSRect::new(window_point, NSSize::new(0.0, 0.0)));
                        self.window.view.convertPoint_fromView_(window_rect.origin, nil)
                    } else {
                        self.window.view.convertPoint_fromView_(window_point, nil)
                    };
                    let view_rect = NSView::frame(*self.window.view);
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
                    self.window.delegate.state.pending_events.lock().unwrap().extend(events.into_iter());
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
                    self.window.delegate.state.pending_events.lock().unwrap().extend(events.into_iter());
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
            if let Some(ev) = self.window.delegate.state.pending_events.lock().unwrap().pop_front() {
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

        let window = match Window::create_window(&builder)
        {
            Some(window) => window,
            None         => { return Err(OsError(format!("Couldn't create NSWindow"))); },
        };

        let view = match Window::create_view(*window) {
            Some(view) => view,
            None       => { return Err(OsError(format!("Couldn't create NSView"))); },
        };

        // TODO: perhaps we should return error from create_context so we can
        // determine the cause of failure and possibly recover?
        let (context, pf) = match Window::create_context(*view, &builder) {
            (Some(context), Some(pf)) => (context, pf),
            (_,             _)        => { return Err(OsError(format!("Couldn't create OpenGL context"))); },
        };

        unsafe {
            app.activateIgnoringOtherApps_(YES);
            if builder.visible {
                window.makeKeyAndOrderFront_(nil);
            } else {
                window.makeKeyWindow();
            }
        }

        let ds = DelegateState {
            is_closed: false,
            context: context.clone(),
            view: view.clone(),
            window: window.clone(),
            resize_handler: None,
            pending_events: Mutex::new(VecDeque::new()),
        };

        let window = Window {
            view: view,
            window: window,
            context: context,
            pixel_format: pf,
            delegate: WindowDelegate::new(ds),
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

    fn create_window(builder: &BuilderAttribs) -> Option<IdRef> {
        unsafe { 
            let screen = match builder.monitor {
                Some(ref monitor_id) => {
                    let native_id = match monitor_id.get_native_identifier() {
                        NativeMonitorId::Numeric(num) => num,
                        _ => panic!("OS X monitors should always have a numeric native ID")
                    };
                    let matching_screen = {
                        let screens = NSScreen::screens(nil);
                        let count: NSUInteger = msg_send![screens, count];
                        let key = IdRef::new(NSString::alloc(nil).init_str("NSScreenNumber"));
                        let mut matching_screen: Option<id> = None;
                        for i in (0..count) {
                            let screen = msg_send![screens, objectAtIndex:i as NSUInteger];
                            let device_description = NSScreen::deviceDescription(screen);
                            let value: id = msg_send![device_description, objectForKey:*key];
                            if value != nil {
                                let screen_number: NSUInteger = msg_send![value, unsignedIntegerValue];
                                if screen_number as u32 == native_id {
                                    matching_screen = Some(screen);
                                    break;
                                }
                            }
                        }
                        matching_screen
                    };
                    Some(matching_screen.unwrap_or(NSScreen::mainScreen(nil)))
                },
                None => None
            };
            let frame = match screen {
                Some(screen) => NSScreen::frame(screen),
                None => {
                    let (width, height) = builder.dimensions.unwrap_or((800, 600));
                    NSRect::new(NSPoint::new(0., 0.), NSSize::new(width as f64, height as f64))
                }
            };

            let masks = if screen.is_some() {
                NSBorderlessWindowMask as NSUInteger
            } else {
                NSTitledWindowMask as NSUInteger |
                NSClosableWindowMask as NSUInteger |
                NSMiniaturizableWindowMask as NSUInteger |
                NSResizableWindowMask as NSUInteger
            };

            let window = IdRef::new(NSWindow::alloc(nil).initWithContentRect_styleMask_backing_defer_(
                frame,
                masks,
                NSBackingStoreBuffered,
                NO,
            ));
            window.non_nil().map(|window| {
                let title = IdRef::new(NSString::alloc(nil).init_str(&builder.title));
                window.setTitle_(*title);
                window.setAcceptsMouseMovedEvents_(YES);
                if screen.is_some() {
                    window.setLevel_(NSMainMenuWindowLevel as i64 + 1);
                }
                else {
                    window.center();
                }
                window
            })
        }
    }

    fn create_view(window: id) -> Option<IdRef> {
        unsafe {
            let view = IdRef::new(NSView::alloc(nil).init());
            view.non_nil().map(|view| {
                view.setWantsBestResolutionOpenGLSurface_(YES);
                window.setContentView_(*view);
                view
            })
        }
    }

    fn create_context(view: id, builder: &BuilderAttribs) -> (Option<IdRef>, Option<PixelFormat>) {
        let profile = match builder.gl_version {
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

        // NOTE: OS X no longer has the concept of setting individual
        // color component's bit size. Instead we can only specify the
        // full color size and hope for the best. Another hiccup is that
        // `NSOpenGLPFAColorSize` also includes `NSOpenGLPFAAlphaSize`,
        // so we have to account for that as well.
        let alpha_depth = builder.alpha_bits.unwrap_or(8);
        let color_depth = builder.color_bits.unwrap_or(24) + alpha_depth;

        let mut attributes = vec![
            NSOpenGLPFADoubleBuffer as u32,
            NSOpenGLPFAClosestPolicy as u32,
            NSOpenGLPFAColorSize as u32, color_depth as u32,
            NSOpenGLPFAAlphaSize as u32, alpha_depth as u32,
            NSOpenGLPFADepthSize as u32, builder.depth_bits.unwrap_or(24) as u32,
            NSOpenGLPFAStencilSize as u32, builder.stencil_bits.unwrap_or(8) as u32,
            NSOpenGLPFAOpenGLProfile as u32, profile,
        ];

        // A color depth higher than 64 implies we're using either 16-bit
        // floats or 32-bit floats and OS X requires a flag to be set
        // accordingly. 
        if color_depth >= 64 {
            attributes.push(NSOpenGLPFAColorFloat as u32);
        }

        builder.multisampling.map(|samples| {
            attributes.push(NSOpenGLPFAMultisample as u32);
            attributes.push(NSOpenGLPFASampleBuffers as u32); attributes.push(1);
            attributes.push(NSOpenGLPFASamples as u32); attributes.push(samples as u32);
        });

        // attribute list must be null terminated.
        attributes.push(0);

        unsafe {
            let pixelformat = IdRef::new(NSOpenGLPixelFormat::alloc(nil).initWithAttributes_(&attributes));

            if let Some(pixelformat) = pixelformat.non_nil() {

                // TODO: Add context sharing
                let context = IdRef::new(NSOpenGLContext::alloc(nil).initWithFormat_shareContext_(*pixelformat, nil));

                if let Some(cxt) = context.non_nil() {
                    let pf = {
                        let getValues_forAttribute_forVirtualScreen_ = |fmt: id,
                                                                        vals: *mut GLint,
                                                                        attrib: NSOpenGLPixelFormatAttribute,
                                                                        screen: GLint| -> () {
                            msg_send![fmt, getValues:vals forAttribute:attrib forVirtualScreen:screen]
                        };
                        let get_attr = |attrib: NSOpenGLPixelFormatAttribute| -> i32 {
                            let mut value = 0;
                            // TODO: Wait for servo/rust-cocoa/#85 to get merged
                            /*NSOpenGLPixelFormat::*/getValues_forAttribute_forVirtualScreen_(
                                *pixelformat,
                                &mut value,
                                attrib,
                                NSOpenGLContext::currentVirtualScreen(*cxt));

                            value
                        };

                        PixelFormat {
                            hardware_accelerated: get_attr(NSOpenGLPFAAccelerated) != 0,
                            color_bits: (get_attr(NSOpenGLPFAColorSize) - get_attr(NSOpenGLPFAAlphaSize)) as u8,
                            alpha_bits: get_attr(NSOpenGLPFAAlphaSize) as u8,
                            depth_bits: get_attr(NSOpenGLPFADepthSize) as u8,
                            stencil_bits: get_attr(NSOpenGLPFAStencilSize) as u8,
                            stereoscopy: get_attr(NSOpenGLPFAStereo) != 0,
                            double_buffer: get_attr(NSOpenGLPFADoubleBuffer) != 0,
                            multisampling: if get_attr(NSOpenGLPFAMultisample) > 0 {
                                Some(get_attr(NSOpenGLPFASamples) as u16)
                            } else {
                                None
                            },
                            srgb: true,
                        }
                    };

                    cxt.setView_(view);
                    if builder.vsync {
                        let value = 1;
                        cxt.setValues_forParameter_(&value, NSOpenGLContextParameter::NSOpenGLCPSwapInterval);
                    }

                    (Some(cxt), Some(pf))
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            }
        }
    }

    pub fn is_closed(&self) -> bool {
        self.delegate.state.is_closed
    }

    pub fn set_title(&self, title: &str) {
        unsafe {
            let title = IdRef::new(NSString::alloc(nil).init_str(title));
            self.window.setTitle_(*title);
        }
    }

    pub fn show(&self) {
        unsafe { NSWindow::makeKeyAndOrderFront_(*self.window, nil); }
    }

    pub fn hide(&self) {
        unsafe { NSWindow::orderOut_(*self.window, nil); }
    }

    pub fn get_position(&self) -> Option<(i32, i32)> {
        unsafe {
            let content_rect = NSWindow::contentRectForFrameRect_(*self.window, NSWindow::frame(*self.window));
            
            // TODO: consider extrapolating the calculations for the y axis to
            // a private method
            Some((content_rect.origin.x as i32, (CGDisplayPixelsHigh(CGMainDisplayID()) as f64 - (content_rect.origin.y + content_rect.size.height)) as i32))
        }
    }

    pub fn set_position(&self, x: i32, y: i32) {
        unsafe {
            let frame = NSWindow::frame(*self.view);

            // NOTE: `setFrameOrigin` might not give desirable results when
            // setting window, as it treats bottom left as origin. 
            // `setFrameTopLeftPoint` treats top left as origin (duh), but
            // does not equal the value returned by `get_window_position`
            // (there is a difference by 22 for me on yosemite)

            // TODO: consider extrapolating the calculations for the y axis to
            // a private method
            let dummy = NSRect::new(NSPoint::new(x as f64, CGDisplayPixelsHigh(CGMainDisplayID()) as f64 - (frame.size.height + y as f64)), NSSize::new(0f64, 0f64));
            let conv = NSWindow::frameRectForContentRect_(*self.window, dummy);

            // NSWindow::setFrameTopLeftPoint_(*self.window, conv.origin);
            NSWindow::setFrameOrigin_(*self.window, conv.origin);
        }
    }

    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        unsafe {
            let view_frame = NSView::frame(*self.view);
            Some((view_frame.size.width as u32, view_frame.size.height as u32))
        }
    }

    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        unsafe {
            let window_frame = NSWindow::frame(*self.window);
            Some((window_frame.size.width as u32, window_frame.size.height as u32))
        }
    }

    pub fn set_inner_size(&self, width: u32, height: u32) {
        unsafe {
            NSWindow::setContentSize_(*self.window, NSSize::new(width as f64, height as f64));
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
        let _: () = msg_send![*self.context, update];
        self.context.makeCurrentContext();
    }

    pub fn is_current(&self) -> bool {
        unsafe {
            let current = NSOpenGLContext::currentContext(nil);
            if current != nil {
                let is_equal: BOOL = msg_send![current, isEqual:*self.context];
                is_equal != NO
            } else {
                false
            }
        }
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

    pub fn get_pixel_format(&self) -> PixelFormat {
        self.pixel_format.clone()
    }

    pub fn set_window_resize_callback(&mut self, callback: Option<fn(u32, u32)>) {
        self.delegate.state.resize_handler = callback;
    }

    pub fn set_cursor(&self, cursor: MouseCursor) {
        let cursor_name = match cursor {                
            MouseCursor::Arrow | MouseCursor::Default => "arrowCursor",
            MouseCursor::Hand => "pointingHandCursor",
            MouseCursor::Grabbing | MouseCursor::Grab => "closedHandCursor",
            MouseCursor::Text => "IBeamCursor",
            MouseCursor::VerticalText => "IBeamCursorForVerticalLayout",
            MouseCursor::Copy => "dragCopyCursor",
            MouseCursor::Alias => "dragLinkCursor",
            MouseCursor::NotAllowed | MouseCursor::NoDrop => "operationNotAllowedCursor",
            MouseCursor::ContextMenu => "contextualMenuCursor",
            MouseCursor::Crosshair => "crosshairCursor",
            MouseCursor::EResize => "resizeRightCursor",
            MouseCursor::NResize => "resizeUpCursor",
            MouseCursor::WResize => "resizeLeftCursor",
            MouseCursor::SResize => "resizeDownCursor",
            MouseCursor::EwResize | MouseCursor::ColResize => "resizeLeftRightCursor",
            MouseCursor::NsResize | MouseCursor::RowResize => "resizeUpDownCursor",

            /// TODO: Find appropriate OSX cursors
            MouseCursor::NeResize | MouseCursor::NwResize |
            MouseCursor::SeResize | MouseCursor::SwResize |
            MouseCursor::NwseResize | MouseCursor::NeswResize |

            MouseCursor::Cell | MouseCursor::NoneCursor |
            MouseCursor::Wait | MouseCursor::Progress | MouseCursor::Help |
            MouseCursor::Move | MouseCursor::AllScroll | MouseCursor::ZoomIn |
            MouseCursor::ZoomOut => "arrowCursor",
        };
        let sel = Sel::register(cursor_name);
        let cls = Class::get("NSCursor").unwrap();
        unsafe {
            use objc::MessageArguments;
            let cursor: id = ().send(cls as *const _ as id, sel);
            let _: () = msg_send![cursor, set];
        }
    }

    pub fn set_cursor_state(&self, state: CursorState) -> Result<(), String> {
        let cls = Class::get("NSCursor").unwrap();
        match state {
            CursorState::Normal => {
                let _: () = unsafe { msg_send![cls, unhide] };
                Ok(())
            },
            CursorState::Hide => {
                let _: () = unsafe { msg_send![cls, hide] };
                Ok(())
            },
            CursorState::Grab => {
                Err("Mouse grabbing is unimplemented".to_string())
            }
        }
    }

    pub fn hidpi_factor(&self) -> f32 {
        unsafe {
            NSWindow::backingScaleFactor(*self.window) as f32
        }
    }

    pub fn set_cursor_position(&self, x: i32, y: i32) -> Result<(), ()> {
        unimplemented!();
    }
}

struct IdRef(id);

impl IdRef {
    fn new(i: id) -> IdRef {
        IdRef(i)
    }

    fn retain(i: id) -> IdRef {
        if i != nil {
            let _: id = unsafe { msg_send![i, retain] };
        }
        IdRef(i)
    }

    fn non_nil(self) -> Option<IdRef> {
        if self.0 == nil { None } else { Some(self) }
    }
}

impl Drop for IdRef {
    fn drop(&mut self) {
        if self.0 != nil {
            let _: () = unsafe { msg_send![self.0, release] };
        }
    }
}

impl Deref for IdRef {
    type Target = id;
    fn deref<'a>(&'a self) -> &'a id {
        &self.0
    }
}

impl Clone for IdRef {
    fn clone(&self) -> IdRef {
        if self.0 != nil {
            let _: id = unsafe { msg_send![self.0, retain] };
        }
        IdRef(self.0)
    }
}

