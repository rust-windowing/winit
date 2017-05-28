use {CreationError, Event, WindowEvent, WindowId, MouseCursor, CursorState};
use CreationError::OsError;
use libc;

use WindowAttributes;
use native_monitor::NativeMonitorId;
use os::macos::ActivationPolicy;

use objc;
use objc::runtime::{Class, Object, Sel, BOOL, YES, NO};
use objc::declare::ClassDecl;

use cocoa;
use cocoa::base::{id, nil};
use cocoa::foundation::{NSPoint, NSRect, NSSize, NSString, NSUInteger};
use cocoa::appkit::{self, NSApplication, NSColor, NSView, NSWindow};

use core_graphics::display::{CGAssociateMouseAndMouseCursorPosition, CGMainDisplayID, CGDisplayPixelsHigh, CGWarpMouseCursorPosition};

use std;
use std::ops::Deref;
use std::os::raw::c_void;

use os::macos::WindowExt;


#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(pub usize);

struct DelegateState {
    view: IdRef,
    window: IdRef,
    events_loop: std::sync::Weak<super::EventsLoop>,
}

pub struct WindowDelegate {
    state: Box<DelegateState>,
    _this: IdRef,
}

impl WindowDelegate {
    /// Get the delegate class, initiailizing it neccessary
    fn class() -> *const Class {
        use std::os::raw::c_void;

        // Emits an event via the `EventsLoop`'s callback or stores it in the pending queue.
        unsafe fn emit_event(state: &mut DelegateState, window_event: WindowEvent) {
            let window_id = get_window_id(*state.window);
            let event = Event::WindowEvent {
                window_id: WindowId(window_id),
                event: window_event,
            };

            if let Some(events_loop) = state.events_loop.upgrade() {
                events_loop.call_user_callback_with_event_or_store_in_pending(event);
            }
        }

        // Called when the window is resized or when the window was moved to a different screen.
        unsafe fn emit_resize_event(state: &mut DelegateState) {
            let rect = NSView::frame(*state.view);
            let scale_factor = NSWindow::backingScaleFactor(*state.window) as f32;
            let width = (scale_factor * rect.size.width as f32) as u32;
            let height = (scale_factor * rect.size.height as f32) as u32;
            emit_event(state, WindowEvent::Resized(width, height));
        }

        extern fn window_should_close(this: &Object, _: Sel, _: id) -> BOOL {
            unsafe {
                let state: *mut c_void = *this.get_ivar("winitState");
                let state = &mut *(state as *mut DelegateState);
                emit_event(state, WindowEvent::Closed);

                // Remove the window from the events_loop.
                if let Some(events_loop) = state.events_loop.upgrade() {
                    let window_id = get_window_id(*state.window);
                    events_loop.find_and_remove_window(window_id);
                }
            }
            YES
        }

        extern fn window_did_resize(this: &Object, _: Sel, _: id) {
            unsafe {
                let state: *mut c_void = *this.get_ivar("winitState");
                let state = &mut *(state as *mut DelegateState);
                emit_resize_event(state);
            }
        }

        extern fn window_did_change_screen(this: &Object, _: Sel, _: id) {
            unsafe {
                let state: *mut c_void = *this.get_ivar("winitState");
                let state = &mut *(state as *mut DelegateState);
                emit_resize_event(state);
            }
        }

        extern fn window_did_become_key(this: &Object, _: Sel, _: id) {
            unsafe {
                // TODO: center the cursor if the window had mouse grab when it
                // lost focus
                let state: *mut c_void = *this.get_ivar("winitState");
                let state = &mut *(state as *mut DelegateState);
                emit_event(state, WindowEvent::Focused(true));
            }
        }

        extern fn window_did_resign_key(this: &Object, _: Sel, _: id) {
            unsafe {
                let state: *mut c_void = *this.get_ivar("winitState");
                let state = &mut *(state as *mut DelegateState);
                emit_event(state, WindowEvent::Focused(false));
            }
        }

        static mut DELEGATE_CLASS: *const Class = 0 as *const Class;
        static INIT: std::sync::Once = std::sync::ONCE_INIT;

        INIT.call_once(|| unsafe {
            // Create new NSWindowDelegate
            let superclass = Class::get("NSObject").unwrap();
            let mut decl = ClassDecl::new("WinitWindowDelegate", superclass).unwrap();

            // Add callback methods
            decl.add_method(sel!(windowShouldClose:),
                window_should_close as extern fn(&Object, Sel, id) -> BOOL);
            decl.add_method(sel!(windowDidResize:),
                window_did_resize as extern fn(&Object, Sel, id));
            decl.add_method(sel!(windowDidChangeScreen:),
                window_did_change_screen as extern fn(&Object, Sel, id));

            decl.add_method(sel!(windowDidBecomeKey:),
                window_did_become_key as extern fn(&Object, Sel, id));
            decl.add_method(sel!(windowDidResignKey:),
                window_did_resign_key as extern fn(&Object, Sel, id));

            // Store internal state as user data
            decl.add_ivar::<*mut c_void>("winitState");

            DELEGATE_CLASS = decl.register();
        });

        unsafe {
            DELEGATE_CLASS
        }
    }

    fn new(state: DelegateState) -> WindowDelegate {
        // Box the state so we can give a pointer to it
        let mut state = Box::new(state);
        let state_ptr: *mut DelegateState = &mut *state;
        unsafe {
            let delegate = IdRef::new(msg_send![WindowDelegate::class(), new]);

            (&mut **delegate).set_ivar("winitState", state_ptr as *mut ::std::os::raw::c_void);
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

#[derive(Clone, Default)]
pub struct PlatformSpecificWindowBuilderAttributes {
    pub activation_policy: ActivationPolicy,
}

pub struct Window {
    pub view: IdRef,
    pub window: IdRef,
    pub delegate: WindowDelegate,
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

impl Drop for Window {
    fn drop(&mut self) {
        // Remove this window from the `EventLoop`s list of windows.
        let id = self.id();
        if let Some(ev) = self.delegate.state.events_loop.upgrade() {
            ev.find_and_remove_window(id);
        }

        // Close the window if it has not yet been closed.
        let nswindow = *self.window;
        if nswindow != nil {
            unsafe {
                msg_send![nswindow, close];
            }
        }
    }
}

impl WindowExt for Window {
    #[inline]
    fn get_nswindow(&self) -> *mut c_void {
        *self.window as *mut c_void
    }

    #[inline]
    fn get_nsview(&self) -> *mut c_void {
        *self.view as *mut c_void
    }
}

impl Window {
    pub fn with_handle(events_loop: std::sync::Weak<super::EventsLoop>, handle: *mut libc::c_void) -> Result<Window, CreationError> {

        let view = IdRef::new(handle as id);

        let ns_window_ptr: cocoa::base::id = unsafe { msg_send![handle as cocoa::base::id, window] };
        let window = IdRef::new(ns_window_ptr as id);

        let ds = DelegateState {
            view: view.clone(),
            window: window.clone(),
            events_loop: events_loop,
        };

        let window = Window {
            view: view,
            window: window,
            delegate: WindowDelegate::new(ds),
        };

        Ok(window)
    }

    pub fn new(events_loop: std::sync::Weak<super::EventsLoop>,
               win_attribs: &WindowAttributes,
               pl_attribs: &PlatformSpecificWindowBuilderAttributes)
               -> Result<Window, CreationError>
    {
        unsafe {
            if !msg_send![cocoa::base::class("NSThread"), isMainThread] {
                panic!("Windows can only be created on the main thread on macOS");
            }
        }

        let app = match Window::create_app(pl_attribs.activation_policy) {
            Some(app) => app,
            None      => { return Err(OsError(format!("Couldn't create NSApplication"))); },
        };

        let window = match Window::create_window(win_attribs)
        {
            Some(window) => window,
            None         => { return Err(OsError(format!("Couldn't create NSWindow"))); },
        };
        let view = match Window::create_view(*window) {
            Some(view) => view,
            None       => { return Err(OsError(format!("Couldn't create NSView"))); },
        };

        unsafe {
            if win_attribs.transparent {
                (*window as id).setOpaque_(NO);
                (*window as id).setBackgroundColor_(NSColor::clearColor(nil));
            }

            app.activateIgnoringOtherApps_(YES);
            if win_attribs.visible {
                window.makeKeyAndOrderFront_(nil);
            } else {
                window.makeKeyWindow();
            }

            if let Some((width, height)) = win_attribs.min_dimensions {
                nswindow_set_min_dimensions(window.0, width.into(), height.into());
            }

            if let Some((width, height)) = win_attribs.max_dimensions {
                nswindow_set_max_dimensions(window.0, width.into(), height.into());
            }
        }

        let ds = DelegateState {
            view: view.clone(),
            window: window.clone(),
            events_loop: events_loop,
        };

        let window = Window {
            view: view,
            window: window,
            delegate: WindowDelegate::new(ds),
        };

        Ok(window)
    }

    pub fn id(&self) -> Id {
        get_window_id(*self.window)
    }

    fn create_app(activation_policy: ActivationPolicy) -> Option<id> {
        unsafe {
            let app = appkit::NSApp();
            if app == nil {
                None
            } else {
                app.setActivationPolicy_(activation_policy.into());
                app.finishLaunching();
                Some(app)
            }
        }
    }

    fn create_window(attrs: &WindowAttributes) -> Option<IdRef> {
        unsafe {
            let screen = match attrs.monitor {
                Some(ref monitor_id) => {
                    let native_id = match monitor_id.get_native_identifier() {
                        NativeMonitorId::Numeric(num) => num,
                        _ => panic!("OS X monitors should always have a numeric native ID")
                    };
                    let matching_screen = {
                        let screens = appkit::NSScreen::screens(nil);
                        let count: NSUInteger = msg_send![screens, count];
                        let key = IdRef::new(NSString::alloc(nil).init_str("NSScreenNumber"));
                        let mut matching_screen: Option<id> = None;
                        for i in 0..count {
                            let screen = msg_send![screens, objectAtIndex:i as NSUInteger];
                            let device_description = appkit::NSScreen::deviceDescription(screen);
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
                    Some(matching_screen.unwrap_or(appkit::NSScreen::mainScreen(nil)))
                },
                None => None
            };
            let frame = match screen {
                Some(screen) => appkit::NSScreen::frame(screen),
                None => {
                    let (width, height) = attrs.dimensions.unwrap_or((800, 600));
                    NSRect::new(NSPoint::new(0., 0.), NSSize::new(width as f64, height as f64))
                }
            };

            let masks = if screen.is_some() {
                // Fullscreen window
                appkit::NSBorderlessWindowMask as NSUInteger |
                appkit::NSResizableWindowMask as NSUInteger |
                appkit::NSTitledWindowMask as NSUInteger
            } else if attrs.decorations {
                // Window with a titlebar
                appkit::NSClosableWindowMask as NSUInteger |
                appkit::NSMiniaturizableWindowMask as NSUInteger |
                appkit::NSResizableWindowMask as NSUInteger |
                appkit::NSTitledWindowMask as NSUInteger
            } else {
                // Window without a titlebar
                appkit::NSClosableWindowMask as NSUInteger |
                appkit::NSMiniaturizableWindowMask as NSUInteger |
                appkit::NSResizableWindowMask as NSUInteger |
                appkit::NSFullSizeContentViewWindowMask as NSUInteger
            };

            let window = IdRef::new(NSWindow::alloc(nil).initWithContentRect_styleMask_backing_defer_(
                frame,
                masks,
                appkit::NSBackingStoreBuffered,
                NO,
            ));
            window.non_nil().map(|window| {
                let title = IdRef::new(NSString::alloc(nil).init_str(&attrs.title));
                window.setReleasedWhenClosed_(NO);
                window.setTitle_(*title);
                window.setAcceptsMouseMovedEvents_(YES);

                if !attrs.decorations {
                    window.setTitleVisibility_(appkit::NSWindowTitleVisibility::NSWindowTitleHidden);
                    window.setTitlebarAppearsTransparent_(YES);
                }

                if screen.is_some() {
                    window.setLevel_(appkit::NSMainMenuWindowLevel as i64 + 1);
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

    pub fn set_title(&self, title: &str) {
        unsafe {
            let title = IdRef::new(NSString::alloc(nil).init_str(title));
            self.window.setTitle_(*title);
        }
    }

    #[inline]
    pub fn show(&self) {
        unsafe { NSWindow::makeKeyAndOrderFront_(*self.window, nil); }
    }

    #[inline]
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

    #[inline]
    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        unsafe {
            let view_frame = NSView::frame(*self.view);
            Some((view_frame.size.width as u32, view_frame.size.height as u32))
        }
    }

    #[inline]
    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        unsafe {
            let window_frame = NSWindow::frame(*self.window);
            Some((window_frame.size.width as u32, window_frame.size.height as u32))
        }
    }

    #[inline]
    pub fn set_inner_size(&self, width: u32, height: u32) {
        unsafe {
            NSWindow::setContentSize_(*self.window, NSSize::new(width as f64, height as f64));
        }
    }

    #[inline]
    pub fn platform_display(&self) -> *mut libc::c_void {
        unimplemented!()
    }

    #[inline]
    pub fn platform_window(&self) -> *mut libc::c_void {
        *self.window as *mut libc::c_void
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
            use objc::Message;
            let cursor: id = cls.send_message(sel, ()).unwrap();
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

    #[inline]
    pub fn hidpi_factor(&self) -> f32 {
        unsafe {
            NSWindow::backingScaleFactor(*self.window) as f32
        }
    }

    #[inline]
    pub fn set_cursor_position(&self, x: i32, y: i32) -> Result<(), ()> {
        let (window_x, window_y) = self.get_position().unwrap_or((0, 0));
        let (cursor_x, cursor_y) = (window_x + x, window_y + y);

        unsafe {
            // TODO: Check for errors.
            let _ = CGWarpMouseCursorPosition(appkit::CGPoint {
                x: cursor_x as appkit::CGFloat,
                y: cursor_y as appkit::CGFloat,
            });
            let _ = CGAssociateMouseAndMouseCursorPosition(true);
        }

        Ok(())
    }
}

// Convert the `cocoa::base::id` associated with a window to a usize to use as a unique identifier
// for the window.
pub fn get_window_id(window_cocoa_id: cocoa::base::id) -> Id {
    Id(window_cocoa_id as *const objc::runtime::Object as usize)
}

unsafe fn nswindow_set_min_dimensions<V: NSWindow + Copy>(
    window: V, min_width: f64, min_height: f64)
{
    window.setMinSize_(NSSize {
        width: min_width,
        height: min_height,
    });
    // If necessary, resize the window to match constraint
    let mut current_rect = NSWindow::frame(window);
    if current_rect.size.width < min_width {
        current_rect.size.width = min_width;
        window.setFrame_display_(current_rect, 0)
    }
    if current_rect.size.height < min_height {
        // The origin point of a rectangle is at its bottom left in Cocoa. To
        // ensure the window's top-left point remains the same:
        current_rect.origin.y +=
            current_rect.size.height - min_height;

        current_rect.size.height = min_height;
        window.setFrame_display_(current_rect, 0)
    }
}

unsafe fn nswindow_set_max_dimensions<V: NSWindow + Copy>(
    window: V, max_width: f64, max_height: f64)
{
    window.setMaxSize_(NSSize {
        width: max_width,
        height: max_height,
    });
    // If necessary, resize the window to match constraint
    let mut current_rect = NSWindow::frame(window);
    if current_rect.size.width > max_width {
        current_rect.size.width = max_width;
        window.setFrame_display_(current_rect, 0)
    }
    if current_rect.size.height > max_height {
        // The origin point of a rectangle is at its bottom left in
        // Cocoa. To ensure the window's top-left point remains the
        // same:
        current_rect.origin.y +=
            current_rect.size.height - max_height;

        current_rect.size.height = max_height;
        window.setFrame_display_(current_rect, 0)
    }
}

pub struct IdRef(id);

impl IdRef {
    fn new(i: id) -> IdRef {
        IdRef(i)
    }

    #[allow(dead_code)]
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
