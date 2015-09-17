#![cfg(target_os = "macos")]

pub use self::headless::HeadlessContext;

use {CreationError, Event, MouseCursor, CursorState};
use CreationError::OsError;
use libc;

use Api;
use BuilderAttribs;
use ContextError;
use GlContext;
use GlProfile;
use GlRequest;
use PixelFormat;
use Robustness;
use native_monitor::NativeMonitorId;

use objc::runtime::{Class, Object, Sel, BOOL, YES, NO};
use objc::declare::ClassDecl;

use cgl;
use cgl::{CGLEnable, kCGLCECrashOnRemovedFunctions, CGLSetParameter, kCGLCPSurfaceOpacity};
use cgl::CGLContextObj as CGL_CGLContextObj;

use cocoa::base::{id, nil};
use cocoa::foundation::{NSAutoreleasePool, NSDate, NSDefaultRunLoopMode, NSPoint, NSRect, NSSize, 
                        NSString, NSUInteger}; 
use cocoa::appkit;
use cocoa::appkit::*;
use cocoa::appkit::NSEventSubtype::*;

use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use core_foundation::bundle::{CFBundleGetBundleWithIdentifier, CFBundleGetFunctionPointerForName};

use core_graphics::display::{CGAssociateMouseAndMouseCursorPosition, CGMainDisplayID, CGDisplayPixelsHigh};

use std::ffi::CStr;
use std::collections::VecDeque;
use std::str::FromStr;
use std::str::from_utf8;
use std::sync::Mutex;
use std::ascii::AsciiExt;
use std::ops::Deref;

use events::Event::{Awakened, MouseInput, MouseMoved, ReceivedCharacter, KeyboardInput, MouseWheel, Closed, Focused};
use events::ElementState::{Pressed, Released};
use events::MouseButton;
use events;

pub use self::monitor::{MonitorID, get_available_monitors, get_primary_monitor};

mod monitor;
mod event;
mod headless;

static mut shift_pressed: bool = false;
static mut ctrl_pressed: bool = false;
static mut win_pressed: bool = false;
static mut alt_pressed: bool = false;

struct DelegateState {
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

        extern fn window_did_become_key(this: &Object, _: Sel, _: id) {
            unsafe {
                // TODO: center the cursor if the window had mouse grab when it
                // lost focus

                let state: *mut libc::c_void = *this.get_ivar("glutinState");
                let state = state as *mut DelegateState;
                (*state).pending_events.lock().unwrap().push_back(Focused(true));
            }
        }

        extern fn window_did_resign_key(this: &Object, _: Sel, _: id) {
            unsafe {
                let state: *mut libc::c_void = *this.get_ivar("glutinState");
                let state = state as *mut DelegateState;
                (*state).pending_events.lock().unwrap().push_back(Focused(false));
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
            
            decl.add_method(sel!(windowDidBecomeKey:),
                window_did_become_key as extern fn(&Object, Sel, id));
            decl.add_method(sel!(windowDidResignKey:),
                window_did_resign_key as extern fn(&Object, Sel, id));

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

        let event: Option<Event>;
        unsafe {
            let nsevent = NSApp().nextEventMatchingMask_untilDate_inMode_dequeue_(
                NSAnyEventMask.bits(),
                NSDate::distantPast(nil),
                NSDefaultRunLoopMode,
                YES);
            event = NSEventToEvent(self.window, nsevent);
        }
        event
    }
}

pub struct WaitEventsIterator<'a> {
    window: &'a Window,
}

impl<'a> Iterator for WaitEventsIterator<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        if let Some(ev) = self.window.delegate.state.pending_events.lock().unwrap().pop_front() {
            return Some(ev);
        }

        let event: Option<Event>;
        unsafe {
            let nsevent = NSApp().nextEventMatchingMask_untilDate_inMode_dequeue_(
                NSAnyEventMask.bits(),
                NSDate::distantFuture(nil),
                NSDefaultRunLoopMode,
                YES);
            event = NSEventToEvent(self.window, nsevent);
        }

        if event.is_none() {
            return Some(Awakened);
        } else {
            return event;
        }
    }
}

impl Window {
    #[cfg(feature = "window")]
    pub fn new(builder: BuilderAttribs) -> Result<Window, CreationError> {
        if builder.sharing.is_some() {
            unimplemented!()
        }

        match builder.gl_robustness {
            Robustness::RobustNoResetNotification | Robustness::RobustLoseContextOnReset => {
                return Err(CreationError::RobustnessNotSupported);
            },
            _ => ()
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
            Ok((context, pf)) => (context, pf),
            Err(e) => { return Err(OsError(format!("Couldn't create OpenGL context: {}", e))); },
        };

        unsafe {
            if builder.transparent {
                let clear_col = {
                    let cls = Class::get("NSColor").unwrap();

                    msg_send![cls, clearColor]
                };
                window.setOpaque_(NO);
                window.setBackgroundColor_(clear_col);

                let obj = context.CGLContextObj();

                let mut opacity = 0;
                CGLSetParameter(obj, kCGLCPSurfaceOpacity, &mut opacity);
            }
            
            app.activateIgnoringOtherApps_(YES);
            if builder.visible {
                window.makeKeyAndOrderFront_(nil);
            } else {
                window.makeKeyWindow();
            }
        }

        let ds = DelegateState {
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

            let masks = if screen.is_some() || !builder.decorations {
                NSBorderlessWindowMask as NSUInteger |
                NSResizableWindowMask as NSUInteger
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

    fn create_context(view: id, builder: &BuilderAttribs) -> Result<(IdRef, PixelFormat), CreationError> {
        let profile = match (builder.gl_version, builder.gl_version.to_gl_version(), builder.gl_profile) {

            // Note: we are not using ranges because of a rust bug that should be fixed here:
            // https://github.com/rust-lang/rust/pull/27050

            (GlRequest::Latest, _, Some(GlProfile::Compatibility)) => NSOpenGLProfileVersionLegacy as u32,
            (GlRequest::Latest, _, _) => {
                if NSAppKitVersionNumber.floor() >= NSAppKitVersionNumber10_9 {
                    NSOpenGLProfileVersion4_1Core as u32
                } else if NSAppKitVersionNumber.floor() >= NSAppKitVersionNumber10_7 {
                    NSOpenGLProfileVersion3_2Core as u32
                } else {
                    NSOpenGLProfileVersionLegacy as u32
                }
            },

            (_, Some((1, _)), _) => NSOpenGLProfileVersionLegacy as u32,
            (_, Some((2, _)), _) => NSOpenGLProfileVersionLegacy as u32,
            (_, Some((3, 0)), _) => NSOpenGLProfileVersionLegacy as u32,
            (_, Some((3, 1)), _) => NSOpenGLProfileVersionLegacy as u32,
            (_, Some((3, 2)), _) => NSOpenGLProfileVersion3_2Core as u32,
            (_, Some((3, _)), Some(GlProfile::Compatibility)) => return Err(CreationError::OpenGlVersionNotSupported),
            (_, Some((3, _)), _) => NSOpenGLProfileVersion4_1Core as u32,
            (_, Some((4, _)), Some(GlProfile::Compatibility)) => return Err(CreationError::OpenGlVersionNotSupported),
            (_, Some((4, _)), _) => NSOpenGLProfileVersion4_1Core as u32,
            _ => return Err(CreationError::OpenGlVersionNotSupported),
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
                        let get_attr = |attrib: NSOpenGLPixelFormatAttribute| -> i32 {
                            let mut value = 0;

                            NSOpenGLPixelFormat::getValues_forAttribute_forVirtualScreen_(
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
                    let value = if builder.vsync { 1 } else { 0 };
                    cxt.setValues_forParameter_(&value, NSOpenGLContextParameter::NSOpenGLCPSwapInterval);

                    CGLEnable(cxt.CGLContextObj(), kCGLCECrashOnRemovedFunctions);

                    Ok((cxt, pf))
                } else {
                    Err(CreationError::NotSupported)
                }
            } else {
                Err(CreationError::NoAvailablePixelFormat)
            }
        }
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

    pub fn platform_display(&self) -> *mut libc::c_void {
        unimplemented!()
    }

    pub fn platform_window(&self) -> *mut libc::c_void {
        unimplemented!()
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

        // TODO: Check for errors.
        match state {
            CursorState::Normal => {
                let _: () = unsafe { msg_send![cls, unhide] };
                let _: i32 = unsafe { CGAssociateMouseAndMouseCursorPosition(true) };
                Ok(())
            },
            CursorState::Hide => {
                let _: () = unsafe { msg_send![cls, hide] };
                Ok(())
            },
            CursorState::Grab => {
                let _: i32 = unsafe { CGAssociateMouseAndMouseCursorPosition(false) };
                Ok(())
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

impl GlContext for Window {
    unsafe fn make_current(&self) -> Result<(), ContextError> {
        let _: () = msg_send![*self.context, update];
        self.context.makeCurrentContext();
        Ok(())
    }

    fn is_current(&self) -> bool {
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

    fn get_proc_address(&self, addr: &str) -> *const libc::c_void {
        let symbol_name: CFString = FromStr::from_str(addr).unwrap();
        let framework_name: CFString = FromStr::from_str("com.apple.opengl").unwrap();
        let framework = unsafe {
            CFBundleGetBundleWithIdentifier(framework_name.as_concrete_TypeRef())
        };
        let symbol = unsafe {
            CFBundleGetFunctionPointerForName(framework, symbol_name.as_concrete_TypeRef())
        };
        symbol as *const _
    }

    fn swap_buffers(&self) -> Result<(), ContextError> {
        unsafe { self.context.flushBuffer(); }
        Ok(())
    }

    fn get_api(&self) -> ::Api {
        ::Api::OpenGl
    }

    fn get_pixel_format(&self) -> PixelFormat {
        self.pixel_format.clone()
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

unsafe fn NSEventToEvent(window: &Window, nsevent: id) -> Option<Event> {
    if nsevent == nil { return None; }

    let event_type = msg_send![nsevent, type];
    NSApp().sendEvent_(if let NSKeyDown = event_type { nil } else { nsevent });

    match event_type {
        NSLeftMouseDown         => { Some(MouseInput(Pressed, MouseButton::Left)) },
        NSLeftMouseUp           => { Some(MouseInput(Released, MouseButton::Left)) },
        NSRightMouseDown        => { Some(MouseInput(Pressed, MouseButton::Right)) },
        NSRightMouseUp          => { Some(MouseInput(Released, MouseButton::Right)) },
        NSMouseMoved            |
        NSLeftMouseDragged      |
        NSOtherMouseDragged     |
        NSRightMouseDragged     => {
            let window_point = nsevent.locationInWindow();
            let cWindow: id = msg_send![nsevent, window];
            let view_point = if cWindow == nil {
                let window_rect = window.window.convertRectFromScreen_(NSRect::new(window_point, NSSize::new(0.0, 0.0)));
                window.view.convertPoint_fromView_(window_rect.origin, nil)
            } else {
                window.view.convertPoint_fromView_(window_point, nil)
            };
            let view_rect = NSView::frame(*window.view);
            let scale_factor = window.hidpi_factor();

            Some(MouseMoved(((scale_factor * view_point.x as f32) as i32,
                            (scale_factor * (view_rect.size.height - view_point.y) as f32) as i32)))
        },
        NSKeyDown => {
            let mut events = VecDeque::new();
            let received_c_str = nsevent.characters().UTF8String();
            let received_str = CStr::from_ptr(received_c_str);
            for received_char in from_utf8(received_str.to_bytes()).unwrap().chars() {
                if received_char.is_ascii() {
                    events.push_back(ReceivedCharacter(received_char));
                }
            }

            let vkey =  event::vkeycode_to_element(NSEvent::keyCode(nsevent));
            events.push_back(KeyboardInput(Pressed, NSEvent::keyCode(nsevent) as u8, vkey));
            let event = events.pop_front();
            window.delegate.state.pending_events.lock().unwrap().extend(events.into_iter());
            event
        },
        NSKeyUp => {
            let vkey =  event::vkeycode_to_element(NSEvent::keyCode(nsevent));

            Some(KeyboardInput(Released, NSEvent::keyCode(nsevent) as u8, vkey))
        },
        NSFlagsChanged => {
            let mut events = VecDeque::new();
            let shift_modifier = Window::modifier_event(nsevent, appkit::NSShiftKeyMask, events::VirtualKeyCode::LShift, shift_pressed);
            if shift_modifier.is_some() {
                shift_pressed = !shift_pressed;
                events.push_back(shift_modifier.unwrap());
            }
            let ctrl_modifier = Window::modifier_event(nsevent, appkit::NSControlKeyMask, events::VirtualKeyCode::LControl, ctrl_pressed);
            if ctrl_modifier.is_some() {
                ctrl_pressed = !ctrl_pressed;
                events.push_back(ctrl_modifier.unwrap());
            }
            let win_modifier = Window::modifier_event(nsevent, appkit::NSCommandKeyMask, events::VirtualKeyCode::LWin, win_pressed);
            if win_modifier.is_some() {
                win_pressed = !win_pressed;
                events.push_back(win_modifier.unwrap());
            }
            let alt_modifier = Window::modifier_event(nsevent, appkit::NSAlternateKeyMask, events::VirtualKeyCode::LAlt, alt_pressed);
            if alt_modifier.is_some() {
                alt_pressed = !alt_pressed;
                events.push_back(alt_modifier.unwrap());
            }
            let event = events.pop_front();
            window.delegate.state.pending_events.lock().unwrap().extend(events.into_iter());
            event
        },
        NSScrollWheel => {
            use events::MouseScrollDelta::{LineDelta, PixelDelta};
            let delta = if nsevent.hasPreciseScrollingDeltas() == YES {
                PixelDelta(nsevent.scrollingDeltaX() as f32, nsevent.scrollingDeltaY() as f32)
            } else {
                LineDelta(nsevent.scrollingDeltaX() as f32, nsevent.scrollingDeltaY() as f32)
            };
            Some(MouseWheel(delta))
        },
        _  => { None },
    }
}
