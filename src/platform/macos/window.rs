use std;
use std::cell::{Cell, RefCell};
use std::ops::Deref;
use std::os::raw::c_void;
use std::sync::Weak;
use std::sync::atomic::{Ordering, AtomicBool};

use cocoa;
use cocoa::appkit::{
    self,
    CGFloat,
    NSApplication,
    NSColor,
    NSScreen,
    NSView,
    NSWindow,
    NSWindowButton,
    NSWindowStyleMask,
};
use cocoa::base::{id, nil};
use cocoa::foundation::{NSAutoreleasePool, NSDictionary, NSPoint, NSRect, NSSize, NSString};

use core_graphics::display::CGDisplay;

use objc;
use objc::runtime::{Class, Object, Sel, BOOL, YES, NO};
use objc::declare::ClassDecl;

use {
    CreationError,
    Event,
    LogicalPosition,
    LogicalSize,
    MouseCursor,
    WindowAttributes,
    WindowEvent,
    WindowId,
};
use CreationError::OsError;
use os::macos::{ActivationPolicy, WindowExt};
use platform::platform::{ffi, util};
use platform::platform::events_loop::{EventsLoop, Shared};
use platform::platform::view::{new_view, set_ime_spot};
use window::MonitorId as RootMonitorId;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(pub usize);

// TODO: It's possible for delegate methods to be called asynchronously, causing data races / `RefCell` panics.
pub struct DelegateState {
    view: IdRef,
    window: IdRef,
    shared: Weak<Shared>,

    win_attribs: RefCell<WindowAttributes>,
    standard_frame: Cell<Option<NSRect>>,
    save_style_mask: Cell<Option<NSWindowStyleMask>>,

    // This is set when WindowBuilder::with_fullscreen was set,
    // see comments of `window_did_fail_to_enter_fullscreen`
    handle_with_fullscreen: bool,

    // During `windowDidResize`, we use this to only send Moved if the position changed.
    previous_position: Option<(f64, f64)>,

    // Used to prevent redundant events.
    previous_dpi_factor: f64,
}

impl DelegateState {
    fn is_zoomed(&self) -> bool {
        unsafe {
            // Because isZoomed do not work in Borderless mode, we set it
            // resizable temporality
            let curr_mask = self.window.styleMask();

            let required = NSWindowStyleMask::NSTitledWindowMask | NSWindowStyleMask::NSResizableWindowMask;
            let needs_temp_mask = !curr_mask.contains(required);
            if needs_temp_mask {
                util::set_style_mask(*self.window, *self.view, required);
            }

            let is_zoomed: BOOL = msg_send![*self.window, isZoomed];

            // Roll back temp styles
            if needs_temp_mask {
                util::set_style_mask(*self.window, *self.view, curr_mask);
            }

            is_zoomed != 0
        }
    }

    fn restore_state_from_fullscreen(&mut self) {
        let maximized = unsafe {
            let mut win_attribs = self.win_attribs.borrow_mut();
            win_attribs.fullscreen = None;

            let mask = {
                let base_mask = self.save_style_mask
                    .take()
                    .unwrap_or_else(|| self.window.styleMask());
                if win_attribs.resizable {
                    base_mask | NSWindowStyleMask::NSResizableWindowMask
                } else {
                    base_mask & !NSWindowStyleMask::NSResizableWindowMask
                }
            };

            util::set_style_mask(*self.window, *self.view, mask);

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

        let curr_mask = unsafe { self.window.styleMask() };
        if win_attribs.fullscreen.is_some() {
            // Handle it in window_did_exit_fullscreen
            return;
        } else if curr_mask.contains(NSWindowStyleMask::NSResizableWindowMask) {
            // Just use the native zoom if resizable
            unsafe {
                self.window.zoom_(nil);
            }
        } else {
            // if it's not resizable, we set the frame directly
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
    // Emits an event via the `EventsLoop`'s callback or stores it in the pending queue.
    pub fn emit_event(state: &mut DelegateState, window_event: WindowEvent) {
        let window_id = get_window_id(*state.window);
        let event = Event::WindowEvent {
            window_id: WindowId(window_id),
            event: window_event,
        };
        if let Some(shared) = state.shared.upgrade() {
            shared.call_user_callback_with_event_or_store_in_pending(event);
        }
    }

    pub fn emit_resize_event(state: &mut DelegateState) {
        let rect = unsafe { NSView::frame(*state.view) };
        let size = LogicalSize::new(rect.size.width as f64, rect.size.height as f64);
        WindowDelegate::emit_event(state, WindowEvent::Resized(size));
    }

    pub fn emit_move_event(state: &mut DelegateState) {
        let rect = unsafe { NSWindow::frame(*state.window) };
        let x = rect.origin.x as f64;
        let y = util::bottom_left_to_top_left(rect);
        let moved = state.previous_position != Some((x, y));
        if moved {
            state.previous_position = Some((x, y));
            WindowDelegate::emit_event(state, WindowEvent::Moved((x, y).into()));
        }
    }

    /// Get the delegate class, initiailizing it neccessary
    fn class() -> *const Class {
        use std::os::raw::c_void;

        extern fn window_should_close(this: &Object, _: Sel, _: id) -> BOOL {
            unsafe {
                let state: *mut c_void = *this.get_ivar("winitState");
                let state = &mut *(state as *mut DelegateState);
                WindowDelegate::emit_event(state, WindowEvent::CloseRequested);
            }
            NO
        }

        extern fn window_will_close(this: &Object, _: Sel, _: id) {
            unsafe {
                let state: *mut c_void = *this.get_ivar("winitState");
                let state = &mut *(state as *mut DelegateState);

                WindowDelegate::emit_event(state, WindowEvent::Destroyed);

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
                WindowDelegate::emit_resize_event(state);
                WindowDelegate::emit_move_event(state);
            }
        }

        // This won't be triggered if the move was part of a resize.
        extern fn window_did_move(this: &Object, _: Sel, _: id) {
            unsafe {
                let state: *mut c_void = *this.get_ivar("winitState");
                let state = &mut *(state as *mut DelegateState);
                WindowDelegate::emit_move_event(state);
            }
        }

        extern fn window_did_change_screen(this: &Object, _: Sel, _: id) {
            unsafe {
                let state: *mut c_void = *this.get_ivar("winitState");
                let state = &mut *(state as *mut DelegateState);
                let dpi_factor = NSWindow::backingScaleFactor(*state.window) as f64;
                if state.previous_dpi_factor != dpi_factor {
                    state.previous_dpi_factor = dpi_factor;
                    WindowDelegate::emit_event(state, WindowEvent::HiDpiFactorChanged(dpi_factor));
                    WindowDelegate::emit_resize_event(state);
                }
            }
        }

        // This will always be called before `window_did_change_screen`.
        extern fn window_did_change_backing_properties(this: &Object, _:Sel, _:id) {
            unsafe {
                let state: *mut c_void = *this.get_ivar("winitState");
                let state = &mut *(state as *mut DelegateState);
                let dpi_factor = NSWindow::backingScaleFactor(*state.window) as f64;
                if state.previous_dpi_factor != dpi_factor {
                    state.previous_dpi_factor = dpi_factor;
                    WindowDelegate::emit_event(state, WindowEvent::HiDpiFactorChanged(dpi_factor));
                    WindowDelegate::emit_resize_event(state);
                }
            }
        }

        extern fn window_did_become_key(this: &Object, _: Sel, _: id) {
            unsafe {
                // TODO: center the cursor if the window had mouse grab when it
                // lost focus
                let state: *mut c_void = *this.get_ivar("winitState");
                let state = &mut *(state as *mut DelegateState);
                WindowDelegate::emit_event(state, WindowEvent::Focused(true));
            }
        }

        extern fn window_did_resign_key(this: &Object, _: Sel, _: id) {
            unsafe {
                let state: *mut c_void = *this.get_ivar("winitState");
                let state = &mut *(state as *mut DelegateState);
                WindowDelegate::emit_event(state, WindowEvent::Focused(false));
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
                    WindowDelegate::emit_event(state, WindowEvent::HoveredFile(PathBuf::from(path)));
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
                    WindowDelegate::emit_event(state, WindowEvent::DroppedFile(PathBuf::from(path)));
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
                WindowDelegate::emit_event(state, WindowEvent::HoveredFileCancelled);
            }
        }

        /// Invoked when entered fullscreen
        extern fn window_did_enter_fullscreen(this: &Object, _: Sel, _: id){
            unsafe {
                let state: *mut c_void = *this.get_ivar("winitState");
                let state = &mut *(state as *mut DelegateState);
                state.win_attribs.borrow_mut().fullscreen = Some(get_current_monitor(*state.window));

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
            let superclass = class!(NSObject);
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

            // setDelegate uses autorelease on objects,
            // so need autorelease
            let autoreleasepool = NSAutoreleasePool::new(nil);

            (&mut **delegate).set_ivar("winitState", state_ptr as *mut ::std::os::raw::c_void);
            let _: () = msg_send![*state.window, setDelegate:*delegate];

            let _: () = msg_send![autoreleasepool, drain];

            WindowDelegate { state: state, _this: delegate }
        }
    }
}

impl Drop for WindowDelegate {
    fn drop(&mut self) {
        unsafe {
            // Nil the window's delegate so it doesn't still reference us
            // NOTE: setDelegate:nil at first retains the previous value,
            // and then autoreleases it, so autorelease pool is needed
            let autoreleasepool = NSAutoreleasePool::new(nil);
            let _: () = msg_send![*self.state.window, setDelegate:nil];
            let _: () = msg_send![autoreleasepool, drain];
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
    pub resize_increments: Option<LogicalSize>,
}

pub struct Window2 {
    pub view: IdRef,
    pub window: IdRef,
    pub delegate: WindowDelegate,
    pub input_context: IdRef,
    cursor_hidden: AtomicBool,
}

unsafe impl Send for Window2 {}
unsafe impl Sync for Window2 {}

unsafe fn get_current_monitor(window: id) -> RootMonitorId {
    let screen: id = msg_send![window, screen];
    let desc = NSScreen::deviceDescription(screen);
    let key = IdRef::new(NSString::alloc(nil).init_str("NSScreenNumber"));
    let value = NSDictionary::valueForKey_(desc, *key);
    let display_id = msg_send![value, unsignedIntegerValue];
    RootMonitorId { inner: EventsLoop::make_monitor_from_display(display_id) }
}

impl Drop for Window2 {
    fn drop(&mut self) {
        // Remove this window from the `EventLoop`s list of windows.
        // The destructor order is:
        // Window ->
        // Rc<Window2> (makes Weak<..> in shared.windows None) ->
        // Window2
        // needed to remove the element from array
        let id = self.id();
        if let Some(shared) = self.delegate.state.shared.upgrade() {
            shared.find_and_remove_window(id);
        }

        // nswindow::close uses autorelease
        // so autorelease pool
        let autoreleasepool = unsafe {
            NSAutoreleasePool::new(nil)
        };

        // Close the window if it has not yet been closed.
        let nswindow = *self.window;
        if nswindow != nil {
            unsafe {
                let () = msg_send![nswindow, close];
            }
        }

        let _: () = unsafe { msg_send![autoreleasepool, drain] };
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
        mut win_attribs: WindowAttributes,
        pl_attribs: PlatformSpecificWindowBuilderAttributes,
    ) -> Result<Window2, CreationError> {
        unsafe {
            if !msg_send![class!(NSThread), isMainThread] {
                panic!("Windows can only be created on the main thread on macOS");
            }
        }

        // Might as well save some RAM...
        win_attribs.window_icon.take();

        let autoreleasepool = unsafe {
            NSAutoreleasePool::new(nil)
        };

        let app = match Window2::create_app(pl_attribs.activation_policy) {
            Some(app) => app,
            None => {
                let _: () = unsafe { msg_send![autoreleasepool, drain] };
                return Err(OsError(format!("Couldn't create NSApplication")));
            },
        };

        let window = match Window2::create_window(&win_attribs, &pl_attribs) {
            Some(res) => res,
            None => {
                let _: () = unsafe { msg_send![autoreleasepool, drain] };
                return Err(OsError(format!("Couldn't create NSWindow")));
            },
        };
        let view = match Window2::create_view(*window, Weak::clone(&shared)) {
            Some(view) => view,
            None => {
                let _: () = unsafe { msg_send![autoreleasepool, drain] };
                return Err(OsError(format!("Couldn't create NSView")));
            },
        };

        let input_context = unsafe { util::create_input_context(*view) };

        unsafe {
            if win_attribs.transparent {
                (*window as id).setOpaque_(NO);
                (*window as id).setBackgroundColor_(NSColor::clearColor(nil));
            }

            app.activateIgnoringOtherApps_(YES);

            if let Some(dimensions) = win_attribs.min_dimensions {
                nswindow_set_min_dimensions(window.0, dimensions);
            }
            if let Some(dimensions) = win_attribs.max_dimensions {
                nswindow_set_max_dimensions(window.0, dimensions);
            }

            use cocoa::foundation::NSArray;
            // register for drag and drop operations.
            let () = msg_send![(*window as id),
                registerForDraggedTypes:NSArray::arrayWithObject(nil, appkit::NSFilenamesPboardType)];
        }

        let dpi_factor = unsafe { NSWindow::backingScaleFactor(*window) as f64 };

        let mut delegate_state = DelegateState {
            view: view.clone(),
            window: window.clone(),
            shared,
            win_attribs: RefCell::new(win_attribs.clone()),
            standard_frame: Cell::new(None),
            save_style_mask: Cell::new(None),
            handle_with_fullscreen: win_attribs.fullscreen.is_some(),
            previous_position: None,
            previous_dpi_factor: dpi_factor,
        };
        delegate_state.win_attribs.borrow_mut().fullscreen = None;

        if dpi_factor != 1.0 {
            WindowDelegate::emit_event(&mut delegate_state, WindowEvent::HiDpiFactorChanged(dpi_factor));
            WindowDelegate::emit_resize_event(&mut delegate_state);
        }

        let window = Window2 {
            view: view,
            window: window,
            delegate: WindowDelegate::new(delegate_state),
            input_context,
            cursor_hidden: Default::default(),
        };

        // Set fullscreen mode after we setup everything
        if let Some(ref monitor) = win_attribs.fullscreen {
            unsafe {
                if monitor.inner != get_current_monitor(*window.window).inner {
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

        let _: () = unsafe { msg_send![autoreleasepool, drain] };

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

    fn class() -> *const Class {
        static mut WINDOW2_CLASS: *const Class = 0 as *const Class;
        static INIT: std::sync::Once = std::sync::ONCE_INIT;

        INIT.call_once(|| unsafe {
            let window_superclass = class!(NSWindow);
            let mut decl = ClassDecl::new("WinitWindow", window_superclass).unwrap();
            decl.add_method(sel!(canBecomeMainWindow), yes as extern fn(&Object, Sel) -> BOOL);
            decl.add_method(sel!(canBecomeKeyWindow), yes as extern fn(&Object, Sel) -> BOOL);
            WINDOW2_CLASS = decl.register();
        });

        unsafe {
            WINDOW2_CLASS
        }
    }

    fn create_window(
        attrs: &WindowAttributes,
        pl_attrs: &PlatformSpecificWindowBuilderAttributes
    ) -> Option<IdRef> {
        unsafe {
            let autoreleasepool = NSAutoreleasePool::new(nil);
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
                    let (width, height) = attrs.dimensions
                        .map(|logical| (logical.width, logical.height))
                        .unwrap_or((800.0, 600.0));
                    NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, height))
                }
            };

            let mut masks = if !attrs.decorations && !screen.is_some() {
                // Resizable Window2 without a titlebar or borders
                // if decorations is set to false, ignore pl_attrs
                NSWindowStyleMask::NSBorderlessWindowMask
                    | NSWindowStyleMask::NSResizableWindowMask
            } else if pl_attrs.titlebar_hidden {
                // if the titlebar is hidden, ignore other pl_attrs
                NSWindowStyleMask::NSBorderlessWindowMask |
                    NSWindowStyleMask::NSResizableWindowMask
            } else {
                // default case, resizable window with titlebar and titlebar buttons
                NSWindowStyleMask::NSClosableWindowMask |
                    NSWindowStyleMask::NSMiniaturizableWindowMask |
                    NSWindowStyleMask::NSResizableWindowMask |
                    NSWindowStyleMask::NSTitledWindowMask
            };

            if !attrs.resizable {
                masks &= !NSWindowStyleMask::NSResizableWindowMask;
            }

            if pl_attrs.fullsize_content_view {
                masks |= NSWindowStyleMask::NSFullSizeContentViewWindowMask;
            }

            let winit_window = Window2::class();

            let window: id = msg_send![winit_window, alloc];

            let window = IdRef::new(window.initWithContentRect_styleMask_backing_defer_(
                frame,
                masks,
                appkit::NSBackingStoreBuffered,
                NO,
            ));
            let res = window.non_nil().map(|window| {
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

                if attrs.always_on_top {
                    let _: () = msg_send![*window, setLevel:ffi::NSWindowLevel::NSFloatingWindowLevel];
                }

                if let Some(increments) = pl_attrs.resize_increments {
                    let (x, y) = (increments.width, increments.height);
                    if x >= 1.0 && y >= 1.0 {
                        let size = NSSize::new(x as CGFloat, y as CGFloat);
                        window.setResizeIncrements_(size);
                    }
                }

                window.center();
                window
            });
            let _: () = msg_send![autoreleasepool, drain];
            res
        }
    }

    fn create_view(window: id, shared: Weak<Shared>) -> Option<IdRef> {
        unsafe {
            let view = new_view(window, shared);
            view.non_nil().map(|view| {
                view.setWantsBestResolutionOpenGLSurface_(YES);
                window.setContentView_(*view);
                window.makeFirstResponder_(*view);
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

    pub fn get_position(&self) -> Option<LogicalPosition> {
        let frame_rect = unsafe { NSWindow::frame(*self.window) };
        Some((
            frame_rect.origin.x as f64,
            util::bottom_left_to_top_left(frame_rect),
        ).into())
    }

    pub fn get_inner_position(&self) -> Option<LogicalPosition> {
        let content_rect = unsafe {
            NSWindow::contentRectForFrameRect_(
                *self.window,
                NSWindow::frame(*self.window),
            )
        };
        Some((
            content_rect.origin.x as f64,
            util::bottom_left_to_top_left(content_rect),
        ).into())
    }

    pub fn set_position(&self, position: LogicalPosition) {
        let dummy = NSRect::new(
            NSPoint::new(
                position.x,
                // While it's true that we're setting the top-left position, it still needs to be
                // in a bottom-left coordinate system.
                CGDisplay::main().pixels_high() as f64 - position.y,
            ),
            NSSize::new(0f64, 0f64),
        );
        unsafe {
            NSWindow::setFrameTopLeftPoint_(*self.window, dummy.origin);
        }
    }

    #[inline]
    pub fn get_inner_size(&self) -> Option<LogicalSize> {
        let view_frame = unsafe { NSView::frame(*self.view) };
        Some((view_frame.size.width as f64, view_frame.size.height as f64).into())
    }

    #[inline]
    pub fn get_outer_size(&self) -> Option<LogicalSize> {
        let view_frame = unsafe { NSWindow::frame(*self.window) };
        Some((view_frame.size.width as f64, view_frame.size.height as f64).into())
    }

    #[inline]
    pub fn set_inner_size(&self, size: LogicalSize) {
        unsafe {
            NSWindow::setContentSize_(*self.window, NSSize::new(size.width as CGFloat, size.height as CGFloat));
        }
    }

    pub fn set_min_dimensions(&self, dimensions: Option<LogicalSize>) {
        unsafe {
            let dimensions = dimensions.unwrap_or_else(|| (0, 0).into());
            nswindow_set_min_dimensions(self.window.0, dimensions);
        }
    }

    pub fn set_max_dimensions(&self, dimensions: Option<LogicalSize>) {
        unsafe {
            let dimensions = dimensions.unwrap_or_else(|| (!0, !0).into());
            nswindow_set_max_dimensions(self.window.0, dimensions);
        }
    }

    #[inline]
    pub fn set_resizable(&self, resizable: bool) {
        let mut win_attribs = self.delegate.state.win_attribs.borrow_mut();
        win_attribs.resizable = resizable;
        if win_attribs.fullscreen.is_none() {
            let mut mask = unsafe { self.window.styleMask() };
            if resizable {
                mask |= NSWindowStyleMask::NSResizableWindowMask;
            } else {
                mask &= !NSWindowStyleMask::NSResizableWindowMask;
            }
            unsafe { util::set_style_mask(*self.window, *self.view, mask) };
        } // Otherwise, we don't change the mask until we exit fullscreen.
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

            MouseCursor::Cell |
            MouseCursor::Wait | MouseCursor::Progress | MouseCursor::Help |
            MouseCursor::Move | MouseCursor::AllScroll | MouseCursor::ZoomIn |
            MouseCursor::ZoomOut => "arrowCursor",
        };
        let sel = Sel::register(cursor_name);
        let cls = class!(NSCursor);
        unsafe {
            use objc::Message;
            let cursor: id = cls.send_message(sel, ()).unwrap();
            let _: () = msg_send![cursor, set];
        }
    }

    #[inline]
    pub fn grab_cursor(&self, grab: bool) -> Result<(), String> {
        // TODO: Do this for real https://stackoverflow.com/a/40922095/5435443
        CGDisplay::associate_mouse_and_mouse_cursor_position(!grab)
            .map_err(|status| format!("Failed to grab cursor: `CGError` {:?}", status))
    }

    #[inline]
    pub fn hide_cursor(&self, hide: bool) {
        let cursor_class = class!(NSCursor);
        // macOS uses a "hide counter" like Windows does, so we avoid incrementing it more than once.
        // (otherwise, `hide_cursor(false)` would need to be called n times!)
        if hide != self.cursor_hidden.load(Ordering::Acquire) {
            if hide {
                let _: () = unsafe { msg_send![cursor_class, hide] };
            } else {
                let _: () = unsafe { msg_send![cursor_class, unhide] };
            }
            self.cursor_hidden.store(hide, Ordering::Release);
        }
    }

    #[inline]
    pub fn get_hidpi_factor(&self) -> f64 {
        unsafe {
            NSWindow::backingScaleFactor(*self.window) as f64
        }
    }

    #[inline]
    pub fn set_cursor_position(&self, cursor_position: LogicalPosition) -> Result<(), String> {
        let window_position = self.get_inner_position()
            .ok_or("`get_inner_position` failed".to_owned())?;
        let point = appkit::CGPoint {
            x: (cursor_position.x + window_position.x) as CGFloat,
            y: (cursor_position.y + window_position.y) as CGFloat,
        };
        CGDisplay::warp_mouse_cursor_position(point)
            .map_err(|e| format!("`CGWarpMouseCursorPosition` failed: {:?}", e))?;
        CGDisplay::associate_mouse_and_mouse_cursor_position(true)
            .map_err(|e| format!("`CGAssociateMouseAndMouseCursorPosition` failed: {:?}", e))?;

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
                let required = NSWindowStyleMask::NSTitledWindowMask | NSWindowStyleMask::NSResizableWindowMask;
                if !curr_mask.contains(required) {
                    util::set_style_mask(*self.window, *self.view, required);
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
            let mut new_mask = if decorations {
                NSWindowStyleMask::NSClosableWindowMask
                    | NSWindowStyleMask::NSMiniaturizableWindowMask
                    | NSWindowStyleMask::NSResizableWindowMask
                    | NSWindowStyleMask::NSTitledWindowMask
            } else {
                NSWindowStyleMask::NSBorderlessWindowMask
                    | NSWindowStyleMask::NSResizableWindowMask
            };
            if !win_attribs.resizable {
                new_mask &= !NSWindowStyleMask::NSResizableWindowMask;
            }
            util::set_style_mask(*state.window, *state.view, new_mask);
        }
    }

    #[inline]
    pub fn set_always_on_top(&self, always_on_top: bool) {
        unsafe {
            let level = if always_on_top {
                ffi::NSWindowLevel::NSFloatingWindowLevel
            } else {
                ffi::NSWindowLevel::NSNormalWindowLevel
            };
            let _: () = msg_send![*self.window, setLevel:level];
        }
    }

    #[inline]
    pub fn set_window_icon(&self, _icon: Option<::Icon>) {
        // macOS doesn't have window icons. Though, there is `setRepresentedFilename`, but that's
        // semantically distinct and should only be used when the window is in some way
        // representing a specific file/directory. For instance, Terminal.app uses this for the
        // CWD. Anyway, that should eventually be implemented as
        // `WindowBuilderExt::with_represented_file` or something, and doesn't have anything to do
        // with `set_window_icon`.
        // https://developer.apple.com/library/content/documentation/Cocoa/Conceptual/WinPanel/Tasks/SettingWindowTitle.html
    }

    #[inline]
    pub fn set_ime_spot(&self, logical_spot: LogicalPosition) {
        set_ime_spot(*self.view, *self.input_context, logical_spot.x, logical_spot.y);
    }

    #[inline]
    pub fn get_current_monitor(&self) -> RootMonitorId {
        unsafe {
            self::get_current_monitor(*self.window)
        }
    }
}

// Convert the `cocoa::base::id` associated with a window to a usize to use as a unique identifier
// for the window.
pub fn get_window_id(window_cocoa_id: id) -> Id {
    Id(window_cocoa_id as *const objc::runtime::Object as usize)
}

unsafe fn nswindow_set_min_dimensions<V: NSWindow + Copy>(window: V, mut min_size: LogicalSize) {
    let mut current_rect = NSWindow::frame(window);
    let content_rect = NSWindow::contentRectForFrameRect_(window, NSWindow::frame(window));
    // Convert from client area size to window size
    min_size.width += (current_rect.size.width - content_rect.size.width) as f64; // this tends to be 0
    min_size.height += (current_rect.size.height - content_rect.size.height) as f64;
    window.setMinSize_(NSSize {
        width: min_size.width as CGFloat,
        height: min_size.height as CGFloat,
    });
    // If necessary, resize the window to match constraint
    if current_rect.size.width < min_size.width {
        current_rect.size.width = min_size.width;
        window.setFrame_display_(current_rect, 0)
    }
    if current_rect.size.height < min_size.height {
        // The origin point of a rectangle is at its bottom left in Cocoa.
        // To ensure the window's top-left point remains the same:
        current_rect.origin.y += current_rect.size.height - min_size.height;
        current_rect.size.height = min_size.height;
        window.setFrame_display_(current_rect, 0)
    }
}

unsafe fn nswindow_set_max_dimensions<V: NSWindow + Copy>(window: V, mut max_size: LogicalSize) {
    let mut current_rect = NSWindow::frame(window);
    let content_rect = NSWindow::contentRectForFrameRect_(window, NSWindow::frame(window));
    // Convert from client area size to window size
    max_size.width += (current_rect.size.width - content_rect.size.width) as f64; // this tends to be 0
    max_size.height += (current_rect.size.height - content_rect.size.height) as f64;
    window.setMaxSize_(NSSize {
        width: max_size.width as CGFloat,
        height: max_size.height as CGFloat,
    });
    // If necessary, resize the window to match constraint
    if current_rect.size.width > max_size.width {
        current_rect.size.width = max_size.width;
        window.setFrame_display_(current_rect, 0)
    }
    if current_rect.size.height > max_size.height {
        // The origin point of a rectangle is at its bottom left in Cocoa.
        // To ensure the window's top-left point remains the same:
        current_rect.origin.y += current_rect.size.height - max_size.height;
        current_rect.size.height = max_size.height;
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
            unsafe {
                let autoreleasepool = NSAutoreleasePool::new(nil);
                let _ : () = msg_send![self.0, release];
                let _ : () = msg_send![autoreleasepool, release];
            };
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
