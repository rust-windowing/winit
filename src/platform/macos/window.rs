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
use cocoa::appkit::{self, NSApplication, NSColor, NSScreen, NSView, NSWindow, NSWindowButton,
    NSWindowStyleMask};
use cocoa::base::{id, nil};
use cocoa::foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};

use core_graphics::display::CGDisplay;

use std;
use std::ops::Deref;
use std::os::raw::c_void;
use std::sync::Weak;
use std::cell::{Cell,RefCell};

use super::events_loop::{EventsLoop, Shared};

use window::MonitorId as RootMonitorId;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(pub usize);

struct DelegateState {
    view: IdRef,
    window: IdRef,
    shared: Weak<Shared>,

    win_attribs: RefCell<WindowAttributes>,
    standard_frame: Cell<Option<NSRect>>,
    save_style_mask: Cell<Option<NSWindowStyleMask>>,

    // This is set when WindowBuilder::with_fullscreen was set,
    // see comments of `window_did_fail_to_enter_fullscreen`
    handle_with_fullscreen: bool,
}

impl DelegateState {
    fn is_zoomed(&self) -> bool {
        unsafe {
            // Because isZoomed do not work in Borderless mode, we set it
            // resizable temporality
            let curr_mask = self.window.styleMask();

            if !curr_mask.contains(NSWindowStyleMask::NSTitledWindowMask) {
                self.window
                    .setStyleMask_(NSWindowStyleMask::NSResizableWindowMask);
            }

            let is_zoomed: BOOL = msg_send![*self.window, isZoomed];

            // Roll back temp styles
            if !curr_mask.contains(NSWindowStyleMask::NSTitledWindowMask) {
                self.window.setStyleMask_(curr_mask);
            }

            is_zoomed != 0
        }
    }

    fn restore_state_from_fullscreen(&mut self) {
        let maximized = unsafe {
            let mut win_attribs = self.win_attribs.borrow_mut();

            win_attribs.fullscreen = None;
            let save_style_opt = self.save_style_mask.take();

            if let Some(save_style) = save_style_opt {
                self.window.setStyleMask_(save_style);
            }

            win_attribs.maximized
        };

        self.perform_maximized(maximized);
    }

    fn perform_maximized(&self, maximized: bool) {
        let is_zoomed = self.is_zoomed();

        if is_zoomed == maximized {
            return;
        }

        // Save the standard frame sized if it is not zoomed
        if !is_zoomed {
            unsafe {
                self.standard_frame.set(Some(NSWindow::frame(*self.window)));
            }
        }

        let mut win_attribs = self.win_attribs.borrow_mut();
        win_attribs.maximized = maximized;

        if win_attribs.fullscreen.is_some() {
            // Handle it in window_did_exit_fullscreen
            return;
        } else if win_attribs.decorations {
            // Just use the native zoom if not borderless
            unsafe {
                self.window.zoom_(nil);
            }
        } else {
            // if it is borderless, we set the frame directly
            unsafe {
                let new_rect = if maximized {
                    let screen = NSScreen::mainScreen(nil);
                    NSScreen::visibleFrame(screen)
                } else {
                    self.standard_frame.get().unwrap_or(NSRect::new(
                        NSPoint::new(50.0, 50.0),
                        NSSize::new(800.0, 600.0),
                    ))
                };

                self.window.setFrame_display_(new_rect, 0);
            }
        }
    }
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
                emit_event(state, WindowEvent::CloseRequested);
            }
            NO
        }

        extern fn window_will_close(this: &Object, _: Sel, _: id) {
            unsafe {
                let state: *mut c_void = *this.get_ivar("winitState");
                let state = &mut *(state as *mut DelegateState);

                emit_event(state, WindowEvent::Destroyed);

                // Remove the window from the shared state.
                if let Some(shared) = state.shared.upgrade() {
                    let window_id = get_window_id(*state.window);
                    shared.find_and_remove_window(window_id);
                }
            }
        }

        extern fn window_did_resize(this: &Object, _: Sel, _: id) {
            unsafe {
                let state: *mut c_void = *this.get_ivar("winitState");
                let state = &mut *(state as *mut DelegateState);
                emit_resize_event(state);
            }
        }

        extern fn window_did_move(this: &Object, _: Sel, _: id) {
            unsafe {
                let state: *mut c_void = *this.get_ivar("winitState");
                let state = &mut *(state as *mut DelegateState);

                let frame_rect = NSWindow::frame(*state.window);
                let x = frame_rect.origin.x as _;
                let y = Window2::bottom_left_to_top_left(frame_rect);
                emit_event(state, WindowEvent::Moved(x, y));
            }
        }

        extern fn window_did_change_screen(this: &Object, _: Sel, _: id) {
            unsafe {
                let state: *mut c_void = *this.get_ivar("winitState");
                let state = &mut *(state as *mut DelegateState);
                emit_resize_event(state);
                let scale_factor = NSWindow::backingScaleFactor(*state.window) as f32;
                emit_event(state, WindowEvent::HiDPIFactorChanged(scale_factor));
            }
        }

        extern fn window_did_change_backing_properties(this: &Object, _:Sel, _:id) {
            unsafe {
                let state: *mut c_void = *this.get_ivar("winitState");
                let state = &mut *(state as *mut DelegateState);
                let scale_factor = NSWindow::backingScaleFactor(*state.window) as f32;
                emit_event(state, WindowEvent::HiDPIFactorChanged(scale_factor));
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

        /// Invoked when entered fullscreen
        extern fn window_did_enter_fullscreen(this: &Object, _: Sel, _: id){
            unsafe {
                let state: *mut c_void = *this.get_ivar("winitState");
                let state = &mut *(state as *mut DelegateState);
                state.win_attribs.borrow_mut().fullscreen = Some(get_current_monitor());

                state.handle_with_fullscreen = false;
            }
        }

        /// Invoked when before enter fullscreen
        extern fn window_will_enter_fullscreen(this: &Object, _: Sel, _: id) {
            unsafe {
                let state: *mut c_void = *this.get_ivar("winitState");
                let state = &mut *(state as *mut DelegateState);
                let is_zoomed = state.is_zoomed();

                state.win_attribs.borrow_mut().maximized = is_zoomed;
            }
        }

        /// Invoked when exited fullscreen
        extern fn window_did_exit_fullscreen(this: &Object, _: Sel, _: id){
            let state = unsafe {
                let state: *mut c_void = *this.get_ivar("winitState");
                &mut *(state as *mut DelegateState)
            };

            state.restore_state_from_fullscreen();
        }

        /// Invoked when fail to enter fullscreen
        ///
        /// When this window launch from a fullscreen app (e.g. launch from VS Code
        /// terminal), it creates a new virtual destkop and a transition animation.
        /// This animation takes one second and cannot be disable without
        /// elevated privileges. In this animation time, all toggleFullscreen events
        /// will be failed. In this implementation, we will try again by using
        /// performSelector:withObject:afterDelay: until window_did_enter_fullscreen.
        /// It should be fine as we only do this at initialzation (i.e with_fullscreen
        /// was set).
        ///
        /// From Apple doc:
        /// In some cases, the transition to enter full-screen mode can fail,
        /// due to being in the midst of handling some other animation or user gesture.
        /// This method indicates that there was an error, and you should clean up any
        /// work you may have done to prepare to enter full-screen mode.
        extern fn window_did_fail_to_enter_fullscreen(this: &Object, _: Sel, _: id) {
            unsafe {
                let state: *mut c_void = *this.get_ivar("winitState");
                let state = &mut *(state as *mut DelegateState);

                if state.handle_with_fullscreen {
                    let _: () = msg_send![*state.window,
                        performSelector:sel!(toggleFullScreen:)
                        withObject:nil
                        afterDelay: 0.5
                    ];
                } else {
                    state.restore_state_from_fullscreen();
                }
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
            decl.add_method(sel!(windowWillClose:),
                window_will_close as extern fn(&Object, Sel, id));
            decl.add_method(sel!(windowDidResize:),
                window_did_resize as extern fn(&Object, Sel, id));
            decl.add_method(sel!(windowDidMove:),
                window_did_move as extern fn(&Object, Sel, id));
            decl.add_method(sel!(windowDidChangeScreen:),
                window_did_change_screen as extern fn(&Object, Sel, id));
            decl.add_method(sel!(windowDidChangeBackingProperties:),
                window_did_change_backing_properties as extern fn(&Object, Sel, id));
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

            // callbacks for fullscreen events
            decl.add_method(sel!(windowDidEnterFullScreen:),
                window_did_enter_fullscreen as extern fn(&Object, Sel, id));
            decl.add_method(sel!(windowWillEnterFullScreen:),
                window_will_enter_fullscreen as extern fn(&Object, Sel, id));
            decl.add_method(sel!(windowDidExitFullScreen:),
                window_did_exit_fullscreen as extern fn(&Object, Sel, id));
            decl.add_method(sel!(windowDidFailToEnterFullScreen:),
                window_did_fail_to_enter_fullscreen as extern fn(&Object, Sel, id));

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

/// Helpper funciton to convert NSScreen::mainScreen to MonitorId
unsafe fn get_current_monitor() -> RootMonitorId {
    let screen = NSScreen::mainScreen(nil);
    let desc = NSScreen::deviceDescription(screen);
    let key = IdRef::new(NSString::alloc(nil).init_str("NSScreenNumber"));

    let value = NSDictionary::valueForKey_(desc, *key);
    let display_id = msg_send![value, unsignedIntegerValue];

    RootMonitorId {
        inner: EventsLoop::make_monitor_from_display(display_id),
    }
}

impl Drop for Window2 {
    fn drop(&mut self) {
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
    pub fn new(
        shared: Weak<Shared>,
        win_attribs: &WindowAttributes,
        pl_attribs: &PlatformSpecificWindowBuilderAttributes,
    ) -> Result<Window2, CreationError> {
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
            win_attribs: RefCell::new(win_attribs.clone()),
            standard_frame: Cell::new(None),
            save_style_mask: Cell::new(None),
            handle_with_fullscreen: win_attribs.fullscreen.is_some(),
            shared: shared,
        };
        ds.win_attribs.borrow_mut().fullscreen = None;

        let window = Window2 {
            view: view,
            window: window,
            delegate: WindowDelegate::new(ds),
        };

        // Set fullscreen mode after we setup everything
        if let Some(ref monitor) = win_attribs.fullscreen {
            unsafe {
                if monitor.inner != get_current_monitor().inner {
                    unimplemented!();
                }
            }
            window.set_fullscreen(Some(monitor.clone()));
        }

        // Make key have to be after set fullscreen
        // to prevent normal size window brefly appears
        unsafe {
            if win_attribs.visible {
                window.window.makeKeyAndOrderFront_(nil);
            } else {
                window.window.makeKeyWindow();
            }
        }

        if win_attribs.maximized {
            window.delegate.state.perform_maximized(win_attribs.maximized);
        }

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

            let masks = if pl_attrs.titlebar_hidden {
                NSWindowStyleMask::NSBorderlessWindowMask |
                    NSWindowStyleMask::NSResizableWindowMask
            } else if pl_attrs.titlebar_transparent {
                // Window2 with a transparent titlebar and regular content view
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
                if !attrs.decorations && !screen.is_some() {
                    // Window2 without a titlebar
                    NSWindowStyleMask::NSBorderlessWindowMask
                } else {
                    // Window2 with a titlebar
                    NSWindowStyleMask::NSClosableWindowMask |
                        NSWindowStyleMask::NSMiniaturizableWindowMask |
                        NSWindowStyleMask::NSResizableWindowMask |
                        NSWindowStyleMask::NSTitledWindowMask
                }
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
                    let () = msg_send![button, setHidden:YES];
                    let button = window.standardWindowButton_(NSWindowButton::NSWindowMiniaturizeButton);
                    let () = msg_send![button, setHidden:YES];
                    let button = window.standardWindowButton_(NSWindowButton::NSWindowCloseButton);
                    let () = msg_send![button, setHidden:YES];
                    let button = window.standardWindowButton_(NSWindowButton::NSWindowZoomButton);
                    let () = msg_send![button, setHidden:YES];
                }
                if pl_attrs.movable_by_window_background {
                    window.setMovableByWindowBackground_(YES);
                }

                if !attrs.decorations {
                    window.setTitleVisibility_(appkit::NSWindowTitleVisibility::NSWindowTitleHidden);
                    window.setTitlebarAppearsTransparent_(YES);
                }

                window.center();
                window
            })
        }
    }

    fn create_view(window: id) -> Option<IdRef> {
        unsafe {
            let view = IdRef::new(NSView::init(NSView::alloc(nil)));
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

    // For consistency with other platforms, this will...
    // 1. translate the bottom-left window corner into the top-left window corner
    // 2. translate the coordinate from a bottom-left origin coordinate system to a top-left one
    fn bottom_left_to_top_left(rect: NSRect) -> i32 {
        (CGDisplay::main().pixels_high() as f64 - (rect.origin.y + rect.size.height)) as _
    }

    pub fn get_position(&self) -> Option<(i32, i32)> {
        let frame_rect = unsafe { NSWindow::frame(*self.window) };
        Some((
            frame_rect.origin.x as i32,
            Self::bottom_left_to_top_left(frame_rect),
        ))
    }

    pub fn get_inner_position(&self) -> Option<(i32, i32)> {
        let content_rect = unsafe {
            NSWindow::contentRectForFrameRect_(
                *self.window,
                NSWindow::frame(*self.window),
            )
        };
        Some((
            content_rect.origin.x as i32,
            Self::bottom_left_to_top_left(content_rect),
        ))
    }

    pub fn set_position(&self, x: i32, y: i32) {
        let dummy = NSRect::new(
            NSPoint::new(
                x as f64,
                // While it's true that we're setting the top-left position, it still needs to be
                // in a bottom-left coordinate system.
                CGDisplay::main().pixels_high() as f64 - y as f64,
            ),
            NSSize::new(0f64, 0f64),
        );
        unsafe {
            NSWindow::setFrameTopLeftPoint_(*self.window, dummy.origin);
        }
    }

    #[inline]
    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        let factor = self.hidpi_factor() as f64; // API convention is that size is in physical pixels
        unsafe {
            let view_frame = NSView::frame(*self.view);
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
    pub fn set_maximized(&self, maximized: bool) {
        self.delegate.state.perform_maximized(maximized)
    }

    #[inline]
    /// TODO: Right now set_fullscreen do not work on switching monitors
    /// in fullscreen mode
    pub fn set_fullscreen(&self, monitor: Option<RootMonitorId>) {
        let state = &self.delegate.state;
        let current = {
            let win_attribs = state.win_attribs.borrow_mut();

            let current = win_attribs.fullscreen.clone();
            match (&current, monitor) {
                (&None, None) => {
                    return;
                }
                (&Some(ref a), Some(ref b)) if a.inner != b.inner => {
                    unimplemented!();
                }
                (&Some(_), Some(_)) => {
                    return;
                }
                _ => (),
            }

            current
        };

        unsafe {
            // Because toggleFullScreen will not work if the StyleMask is none,
            // We set a normal style to it temporary.
            // It will clean up at window_did_exit_fullscreen.
            if current.is_none() {
                let curr_mask = state.window.styleMask();

                if !curr_mask.contains(NSWindowStyleMask::NSTitledWindowMask) {
                    state.window.setStyleMask_(
                        NSWindowStyleMask::NSTitledWindowMask
                            | NSWindowStyleMask::NSResizableWindowMask,
                    );
                    state.save_style_mask.set(Some(curr_mask));
                }
            }

            self.window.toggleFullScreen_(nil);
        }
    }

    #[inline]
    pub fn set_decorations(&self, decorations: bool) {
        let state = &self.delegate.state;
        let mut win_attribs = state.win_attribs.borrow_mut();

        if win_attribs.decorations == decorations {
            return;
        }

        win_attribs.decorations = decorations;

        // Skip modifiy if we are in fullscreen mode,
        // window_did_exit_fullscreen will handle it
        if win_attribs.fullscreen.is_some() {
            return;
        }

        unsafe {
            let new_mask = if decorations {
                NSWindowStyleMask::NSClosableWindowMask
                    | NSWindowStyleMask::NSMiniaturizableWindowMask
                    | NSWindowStyleMask::NSResizableWindowMask
                    | NSWindowStyleMask::NSTitledWindowMask
            } else {
                NSWindowStyleMask::NSBorderlessWindowMask
            };

            state.window.setStyleMask_(new_mask);
        }
    }

    #[inline]
    pub fn get_current_monitor(&self) -> RootMonitorId {
        unsafe {
            self::get_current_monitor()
        }
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
