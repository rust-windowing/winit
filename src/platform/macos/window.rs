use {CreationError, Event, WindowEvent, WindowId, MouseCursor, CursorState};
use CreationError::OsError;
use libc;

use WindowAttributes;
use os::macos::ActivationPolicy;
use os::macos::WindowExt;

use objc;
use objc::runtime::{Class, Object, Sel, BOOL, YES, NO};
use objc::declare::ClassDecl;

use cocoa;
use cocoa::base::{id, nil};
use cocoa::foundation::{NSPoint, NSRect, NSSize, NSString};
use cocoa::appkit::{self, NSApplication, NSColor, NSView, NSWindow, NSWindowStyleMask, NSWindowButton};

use core_graphics::display::CGDisplay;

use std;
use std::ops::Deref;
use std::os::raw::c_void;
use std::sync::Weak;

use super::events_loop::Shared;

use window::MonitorId as RootMonitorId;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(pub usize);

struct DelegateState {
    view: IdRef,
    window: IdRef,
    shared: Weak<Shared>,
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

            if let Some(shared) = state.shared.upgrade() {
                shared.call_user_callback_with_event_or_store_in_pending(event);
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

                // Remove the window from the shared state.
                if let Some(shared) = state.shared.upgrade() {
                    let window_id = get_window_id(*state.window);
                    shared.find_and_remove_window(window_id);
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

        /// Invoked when the dragged image enters destination bounds or frame
        extern fn dragging_entered(this: &Object, _: Sel, sender: id) -> BOOL {
            use cocoa::appkit::NSPasteboard;
            use cocoa::foundation::NSFastEnumeration;
            use std::path::PathBuf;

            let pb: id = unsafe { msg_send![sender, draggingPasteboard] };
            let filenames = unsafe { NSPasteboard::propertyListForType(pb, appkit::NSFilenamesPboardType) };

            for file in unsafe { filenames.iter() } {
                use cocoa::foundation::NSString;
                use std::ffi::CStr;

                unsafe {
                    let f = NSString::UTF8String(file);
                    let path = CStr::from_ptr(f).to_string_lossy().into_owned();

                    let state: *mut c_void = *this.get_ivar("winitState");
                    let state = &mut *(state as *mut DelegateState);
                    emit_event(state, WindowEvent::HoveredFile(PathBuf::from(path)));
                }
            };

            YES
        }

        /// Invoked when the image is released
        extern fn prepare_for_drag_operation(_: &Object, _: Sel, _: id) {}

        /// Invoked after the released image has been removed from the screen
        extern fn perform_drag_operation(this: &Object, _: Sel, sender: id) -> BOOL {
            use cocoa::appkit::NSPasteboard;
            use cocoa::foundation::NSFastEnumeration;
            use std::path::PathBuf;

            let pb: id = unsafe { msg_send![sender, draggingPasteboard] };
            let filenames = unsafe { NSPasteboard::propertyListForType(pb, appkit::NSFilenamesPboardType) };

            for file in unsafe { filenames.iter() } {
                use cocoa::foundation::NSString;
                use std::ffi::CStr;

                unsafe {
                    let f = NSString::UTF8String(file);
                    let path = CStr::from_ptr(f).to_string_lossy().into_owned();

                    let state: *mut c_void = *this.get_ivar("winitState");
                    let state = &mut *(state as *mut DelegateState);
                    emit_event(state, WindowEvent::DroppedFile(PathBuf::from(path)));
                }
            };

            YES
        }

        /// Invoked when the dragging operation is complete
        extern fn conclude_drag_operation(_: &Object, _: Sel, _: id) {}

        /// Invoked when the dragging operation is cancelled
        extern fn dragging_exited(this: &Object, _: Sel, _: id) {
            unsafe {
                let state: *mut c_void = *this.get_ivar("winitState");
                let state = &mut *(state as *mut DelegateState);
                emit_event(state, WindowEvent::HoveredFileCancelled);
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

            // callbacks for drag and drop events
            decl.add_method(sel!(draggingEntered:),
                dragging_entered as extern fn(&Object, Sel, id) -> BOOL);
           decl.add_method(sel!(prepareForDragOperation:),
                prepare_for_drag_operation as extern fn(&Object, Sel, id));
           decl.add_method(sel!(performDragOperation:),
                perform_drag_operation as extern fn(&Object, Sel, id) -> BOOL);
           decl.add_method(sel!(concludeDragOperation:),
                conclude_drag_operation as extern fn(&Object, Sel, id));
           decl.add_method(sel!(draggingExited:),
                dragging_exited as extern fn(&Object, Sel, id));

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
    pub movable_by_window_background: bool,
    pub titlebar_transparent: bool,
    pub title_hidden: bool,
    pub titlebar_hidden: bool,
    pub titlebar_buttons_hidden: bool,
    pub fullsize_content_view: bool,
}

pub struct Window2 {
    pub view: IdRef,
    pub window: IdRef,
    pub delegate: WindowDelegate,
}

unsafe impl Send for Window2 {}
unsafe impl Sync for Window2 {}

impl Drop for Window2 {
    fn drop(&mut self) {
        // Remove this window from the `EventLoop`s list of windows.
        let id = self.id();
        if let Some(shared) = self.delegate.state.shared.upgrade() {
            shared.find_and_remove_window(id);
        }

        // Close the window if it has not yet been closed.
        let nswindow = *self.window;
        if nswindow != nil {
            unsafe {
                let () = msg_send![nswindow, close];
            }
        }
    }
}

impl WindowExt for Window2 {
    #[inline]
    fn get_nswindow(&self) -> *mut c_void {
        *self.window as *mut c_void
    }

    #[inline]
    fn get_nsview(&self) -> *mut c_void {
        *self.view as *mut c_void
    }
}

impl Window2 {
    pub fn new(shared: Weak<Shared>,
               win_attribs: &WindowAttributes,
               pl_attribs: &PlatformSpecificWindowBuilderAttributes)
               -> Result<Window2, CreationError>
    {
        unsafe {
            if !msg_send![cocoa::base::class("NSThread"), isMainThread] {
                panic!("Windows can only be created on the main thread on macOS");
            }
        }

        let app = match Window2::create_app(pl_attribs.activation_policy) {
            Some(app) => app,
            None      => { return Err(OsError(format!("Couldn't create NSApplication"))); },
        };

        let window = match Window2::create_window(win_attribs, pl_attribs)
        {
            Some(window) => window,
            None         => { return Err(OsError(format!("Couldn't create NSWindow"))); },
        };
        let view = match Window2::create_view(*window) {
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

            use cocoa::foundation::NSArray;
            // register for drag and drop operations.
            let () = msg_send![(*window as id),
                registerForDraggedTypes:NSArray::arrayWithObject(nil, appkit::NSFilenamesPboardType)];
        }

        let ds = DelegateState {
            view: view.clone(),
            window: window.clone(),
            shared: shared,
        };

        let window = Window2 {
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

    fn create_window(
        attrs: &WindowAttributes,
        pl_attrs: &PlatformSpecificWindowBuilderAttributes)
        -> Option<IdRef> {
        unsafe {
            let screen = match attrs.fullscreen {
                Some(ref monitor_id) => {
                    let monitor_screen = monitor_id.inner.get_nsscreen();
                    Some(monitor_screen.unwrap_or(appkit::NSScreen::mainScreen(nil)))
                },
                _ => None,
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
                NSWindowStyleMask::NSBorderlessWindowMask |
                    NSWindowStyleMask::NSResizableWindowMask |
                    NSWindowStyleMask::NSTitledWindowMask
            } else if !attrs.decorations {
                // Window2 without a titlebar
                NSWindowStyleMask::NSBorderlessWindowMask
            } else if pl_attrs.titlebar_hidden {
                NSWindowStyleMask::NSBorderlessWindowMask |
                    NSWindowStyleMask::NSResizableWindowMask
            } else if !pl_attrs.titlebar_transparent {
                // Window2 with a titlebar
                NSWindowStyleMask::NSClosableWindowMask |
                    NSWindowStyleMask::NSMiniaturizableWindowMask |
                    NSWindowStyleMask::NSResizableWindowMask |
                    NSWindowStyleMask::NSTitledWindowMask
            } else if pl_attrs.fullsize_content_view {
                // Window2 with a transparent titlebar and fullsize content view
                NSWindowStyleMask::NSClosableWindowMask |
                    NSWindowStyleMask::NSMiniaturizableWindowMask |
                    NSWindowStyleMask::NSResizableWindowMask |
                    NSWindowStyleMask::NSTitledWindowMask |
                    NSWindowStyleMask::NSFullSizeContentViewWindowMask
            } else {
                // Window2 with a transparent titlebar and regular content view
                NSWindowStyleMask::NSClosableWindowMask |
                    NSWindowStyleMask::NSMiniaturizableWindowMask |
                    NSWindowStyleMask::NSResizableWindowMask |
                    NSWindowStyleMask::NSTitledWindowMask
            };

            let winit_window = Class::get("WinitWindow").unwrap_or_else(|| {
                let window_superclass = Class::get("NSWindow").unwrap();
                let mut decl = ClassDecl::new("WinitWindow", window_superclass).unwrap();
                decl.add_method(sel!(canBecomeMainWindow), yes as extern fn(&Object, Sel) -> BOOL);
                decl.add_method(sel!(canBecomeKeyWindow), yes as extern fn(&Object, Sel) -> BOOL);
                decl.register();
                Class::get("WinitWindow").unwrap()
            });

            let window: id = msg_send![winit_window, alloc];

            let window = IdRef::new(window.initWithContentRect_styleMask_backing_defer_(
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

                if pl_attrs.titlebar_transparent {
                    window.setTitlebarAppearsTransparent_(YES);
                }
                if pl_attrs.title_hidden {
                    window.setTitleVisibility_(appkit::NSWindowTitleVisibility::NSWindowTitleHidden);
                }
                if pl_attrs.titlebar_buttons_hidden {
                    let button = window.standardWindowButton_(NSWindowButton::NSWindowFullScreenButton);
                    msg_send![button, setHidden:YES];
                    let button = window.standardWindowButton_(NSWindowButton::NSWindowMiniaturizeButton);
                    msg_send![button, setHidden:YES];
                    let button = window.standardWindowButton_(NSWindowButton::NSWindowCloseButton);
                    msg_send![button, setHidden:YES];
                    let button = window.standardWindowButton_(NSWindowButton::NSWindowZoomButton);
                    msg_send![button, setHidden:YES];
                }
                if pl_attrs.movable_by_window_background {
                    window.setMovableByWindowBackground_(YES);
                }

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
            Some((content_rect.origin.x as i32, (CGDisplay::main().pixels_high() as f64 - (content_rect.origin.y + content_rect.size.height)) as i32))
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
            let dummy = NSRect::new(NSPoint::new(x as f64, CGDisplay::main().pixels_high() as f64 - (frame.size.height + y as f64)), NSSize::new(0f64, 0f64));
            let conv = NSWindow::frameRectForContentRect_(*self.window, dummy);

            // NSWindow::setFrameTopLeftPoint_(*self.window, conv.origin);
            NSWindow::setFrameOrigin_(*self.window, conv.origin);
        }
    }

    #[inline]
    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        unsafe {
            let view_frame = NSView::frame(*self.view);
            let factor = self.hidpi_factor() as f64; // API convention is that size is in physical pixels
            Some(((view_frame.size.width*factor) as u32, (view_frame.size.height*factor) as u32))
        }
    }

    #[inline]
    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        let factor = self.hidpi_factor() as f64; // API convention is that size is in physical pixels
        unsafe {
            let window_frame = NSWindow::frame(*self.window);
            Some(((window_frame.size.width*factor) as u32, (window_frame.size.height*factor) as u32))
        }
    }

    #[inline]
    pub fn set_inner_size(&self, width: u32, height: u32) {
        let factor = self.hidpi_factor() as f64; // API convention is that size is in physical pixels
        unsafe {
            NSWindow::setContentSize_(*self.window, NSSize::new((width as f64)/factor, (height as f64)/factor));
        }
    }

    pub fn set_min_dimensions(&self, dimensions: Option<(u32, u32)>) {
        unsafe {
            let (width, height) = dimensions.unwrap_or((0, 0));
            nswindow_set_min_dimensions(self.window.0, width.into(), height.into());
        }
    }

    pub fn set_max_dimensions(&self, dimensions: Option<(u32, u32)>) {
        unsafe {
            let (width, height) = dimensions.unwrap_or((!0, !0));
            nswindow_set_max_dimensions(self.window.0, width.into(), height.into());
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

            // TODO: Find appropriate OSX cursors
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
                let _ = CGDisplay::associate_mouse_and_mouse_cursor_position(true);
                Ok(())
            },
            CursorState::Hide => {
                let _: () = unsafe { msg_send![cls, hide] };
                Ok(())
            },
            CursorState::Grab => {
                let _: () = unsafe { msg_send![cls, hide] };
                let _ = CGDisplay::associate_mouse_and_mouse_cursor_position(false);
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

        // TODO: Check for errors.
        let _ = CGDisplay::warp_mouse_cursor_position(appkit::CGPoint {
            x: cursor_x as appkit::CGFloat,
            y: cursor_y as appkit::CGFloat,
        });
        let _ = CGDisplay::associate_mouse_and_mouse_cursor_position(true);

        Ok(())
    }

    #[inline]
    pub fn set_maximized(&self, _maximized: bool) {
        unimplemented!()
    }

    #[inline]
    pub fn set_fullscreen(&self, _monitor: Option<RootMonitorId>) {
        unimplemented!()
    }

    #[inline]
    pub fn set_decorations(&self, _decorations: bool) {
        unimplemented!()
    }

    #[inline]
    pub fn get_current_monitor(&self) -> RootMonitorId {
        unimplemented!()
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
    pub fn new(i: id) -> IdRef {
        IdRef(i)
    }

    #[allow(dead_code)]
    pub fn retain(i: id) -> IdRef {
        if i != nil {
            let _: id = unsafe { msg_send![i, retain] };
        }
        IdRef(i)
    }

    pub fn non_nil(self) -> Option<IdRef> {
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

extern fn yes(_: &Object, _: Sel) -> BOOL {
    YES
}
