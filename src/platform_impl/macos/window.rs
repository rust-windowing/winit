use std::{
    collections::VecDeque, f64, os::raw::c_void,
    sync::{Arc, atomic::{Ordering, AtomicBool}, Mutex, Weak},
};

use cocoa::{
    appkit::{
        self, CGFloat, NSApp, NSApplication, NSApplicationActivationPolicy,
        NSColor, NSRequestUserAttentionType, NSScreen, NSView, NSWindow,
        NSWindowButton, NSWindowStyleMask, NSApplicationPresentationOptions
    },
    base::{id, nil},
    foundation::{NSAutoreleasePool, NSDictionary, NSPoint, NSRect, NSSize, NSString},
};
use core_graphics::display::CGDisplay;
use objc::{runtime::{Class, Object, Sel, BOOL, YES, NO}, declare::ClassDecl};

use {
    dpi::{LogicalPosition, LogicalSize}, icon::Icon,
    monitor::MonitorHandle as RootMonitorHandle,
    window::{
        CreationError, MouseCursor, WindowAttributes, WindowId as RootWindowId,
    },
};
use platform::macos::{ActivationPolicy, WindowExtMacOS};
use platform_impl::platform::{
    app_state::AppState, ffi, monitor::{self, MonitorHandle},
    util::{self, IdRef}, view::{self, new_view},
    window_delegate::new_delegate,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(pub usize);

impl Id {
    pub unsafe fn dummy() -> Self {
        Id(0)
    }
}

// Convert the `cocoa::base::id` associated with a window to a usize to use as a unique identifier
// for the window.
pub fn get_window_id(window_cocoa_id: id) -> Id {
    Id(window_cocoa_id as *const Object as _)
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

fn create_app(activation_policy: ActivationPolicy) -> Option<id> {
    unsafe {
        let nsapp = NSApp();
        if nsapp == nil {
            None
        } else {
            use self::NSApplicationActivationPolicy::*;
            nsapp.setActivationPolicy_(match activation_policy {
                ActivationPolicy::Regular => NSApplicationActivationPolicyRegular,
                ActivationPolicy::Accessory => NSApplicationActivationPolicyAccessory,
                ActivationPolicy::Prohibited => NSApplicationActivationPolicyProhibited,
            });
            nsapp.finishLaunching();
            Some(nsapp)
        }
    }
}

unsafe fn create_view(nswindow: id) -> Option<(IdRef, Weak<Mutex<util::Cursor>>)> {
    let (nsview, cursor) = new_view(nswindow);
    nsview.non_nil().map(|nsview| {
        nsview.setWantsBestResolutionOpenGLSurface_(YES);

        // On Mojave, views automatically become layer-backed shortly after being added to
        // a window. Changing the layer-backedness of a view breaks the association between
        // the view and its associated OpenGL context. To work around this, on Mojave we
        // explicitly make the view layer-backed up front so that AppKit doesn't do it
        // itself and break the association with its context.
        if f64::floor(appkit::NSAppKitVersionNumber) > appkit::NSAppKitVersionNumber10_12 {
            nsview.setWantsLayer(YES);
        }

        nswindow.setContentView_(*nsview);
        nswindow.makeFirstResponder_(*nsview);
        (nsview, cursor)
    })
}

fn create_window(
    attrs: &WindowAttributes,
    pl_attrs: &PlatformSpecificWindowBuilderAttributes,
) -> Option<IdRef> {
    unsafe {
        let pool = NSAutoreleasePool::new(nil);
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
                    .unwrap_or_else(|| (800.0, 600.0));
                NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, height))
            },
        };

        let mut masks = if !attrs.decorations && !screen.is_some() {
            // Resizable UnownedWindow without a titlebar or borders
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

        let nswindow: id = msg_send![WINDOW_CLASS.0, alloc];
        let nswindow = IdRef::new(nswindow.initWithContentRect_styleMask_backing_defer_(
            frame,
            masks,
            appkit::NSBackingStoreBuffered,
            NO,
        ));
        let res = nswindow.non_nil().map(|nswindow| {
            let title = IdRef::new(NSString::alloc(nil).init_str(&attrs.title));
            nswindow.setReleasedWhenClosed_(NO);
            nswindow.setTitle_(*title);
            nswindow.setAcceptsMouseMovedEvents_(YES);

            if pl_attrs.titlebar_transparent {
                nswindow.setTitlebarAppearsTransparent_(YES);
            }
            if pl_attrs.title_hidden {
                nswindow.setTitleVisibility_(appkit::NSWindowTitleVisibility::NSWindowTitleHidden);
            }
            if pl_attrs.titlebar_buttons_hidden {
                for titlebar_button in &[
                    NSWindowButton::NSWindowFullScreenButton,
                    NSWindowButton::NSWindowMiniaturizeButton,
                    NSWindowButton::NSWindowCloseButton,
                    NSWindowButton::NSWindowZoomButton,
                ] {
                    let button = nswindow.standardWindowButton_(*titlebar_button);
                    let _: () = msg_send![button, setHidden:YES];
                }
            }
            if pl_attrs.movable_by_window_background {
                nswindow.setMovableByWindowBackground_(YES);
            }

            if attrs.always_on_top {
                let _: () = msg_send![*nswindow, setLevel:ffi::NSWindowLevel::NSFloatingWindowLevel];
            }

            if let Some(increments) = pl_attrs.resize_increments {
                let (x, y) = (increments.width, increments.height);
                if x >= 1.0 && y >= 1.0 {
                    let size = NSSize::new(x as CGFloat, y as CGFloat);
                    nswindow.setResizeIncrements_(size);
                }
            }

            nswindow.center();
            nswindow
        });
        pool.drain();
        res
    }
}

struct WindowClass(*const Class);
unsafe impl Send for WindowClass {}
unsafe impl Sync for WindowClass {}

lazy_static! {
    static ref WINDOW_CLASS: WindowClass = unsafe {
        let window_superclass = class!(NSWindow);
        let mut decl = ClassDecl::new("WinitWindow", window_superclass).unwrap();
        decl.add_method(sel!(canBecomeMainWindow), util::yes as extern fn(&Object, Sel) -> BOOL);
        decl.add_method(sel!(canBecomeKeyWindow), util::yes as extern fn(&Object, Sel) -> BOOL);
        WindowClass(decl.register())
    };
}

#[derive(Default)]
pub struct SharedState {
    pub resizable: bool,
    pub fullscreen: Option<RootMonitorHandle>,
    pub maximized: bool,
    standard_frame: Option<NSRect>,
    is_simple_fullscreen: bool,
    pub saved_style: Option<NSWindowStyleMask>,
    save_presentation_opts: Option<NSApplicationPresentationOptions>,
}

impl From<WindowAttributes> for SharedState {
    fn from(attribs: WindowAttributes) -> Self {
        SharedState {
            resizable: attribs.resizable,
            // This fullscreen field tracks the current state of the window
            // (as seen by `WindowDelegate`), and since the window hasn't
            // actually been fullscreened yet, we can't set it yet. This is
            // necessary for state transitions to work right, since otherwise
            // the initial value and the first `set_fullscreen` call would be
            // identical, resulting in a no-op.
            fullscreen: None,
            maximized: attribs.maximized,
            .. Default::default()
        }
    }
}

pub struct UnownedWindow {
    pub nswindow: IdRef, // never changes
    pub nsview: IdRef, // never changes
    input_context: IdRef, // never changes
    pub shared_state: Arc<Mutex<SharedState>>,
    decorations: AtomicBool,
    cursor: Weak<Mutex<util::Cursor>>,
    cursor_hidden: AtomicBool,
}

unsafe impl Send for UnownedWindow {}
unsafe impl Sync for UnownedWindow {}

impl UnownedWindow {
    pub fn new(
        mut win_attribs: WindowAttributes,
        pl_attribs: PlatformSpecificWindowBuilderAttributes,
    ) -> Result<(Arc<Self>, IdRef), CreationError> {
        unsafe {
            if !msg_send![class!(NSThread), isMainThread] {
                panic!("Windows can only be created on the main thread on macOS");
            }
        }

        let pool = unsafe { NSAutoreleasePool::new(nil) };

        let nsapp = create_app(pl_attribs.activation_policy).ok_or_else(|| {
            unsafe { pool.drain() };
            CreationError::OsError(format!("Couldn't create `NSApplication`"))
        })?;

        let nswindow = create_window(&win_attribs, &pl_attribs).ok_or_else(|| {
            unsafe { pool.drain() };
            CreationError::OsError(format!("Couldn't create `NSWindow`"))
        })?;

        let (nsview, cursor) = unsafe { create_view(*nswindow) }.ok_or_else(|| {
            unsafe { pool.drain() };
            CreationError::OsError(format!("Couldn't create `NSView`"))
        })?;

        let input_context = unsafe { util::create_input_context(*nsview) };

        unsafe {
            if win_attribs.transparent {
                nswindow.setOpaque_(NO);
                nswindow.setBackgroundColor_(NSColor::clearColor(nil));
            }

            nsapp.activateIgnoringOtherApps_(YES);

            win_attribs.min_dimensions.map(|dim| set_min_dimensions(*nswindow, dim));
            win_attribs.max_dimensions.map(|dim| set_max_dimensions(*nswindow, dim));

            use cocoa::foundation::NSArray;
            // register for drag and drop operations.
            let () = msg_send![*nswindow, registerForDraggedTypes:NSArray::arrayWithObject(
                nil,
                appkit::NSFilenamesPboardType,
            )];
        }

        // Since `win_attribs` is put into a mutex below, we'll just copy these
        // attributes now instead of bothering to lock it later.
        // Also, `SharedState` doesn't carry `fullscreen` over; it's set
        // indirectly by us calling `set_fullscreen` below, causing handlers in
        // `WindowDelegate` to update the state.
        let fullscreen = win_attribs.fullscreen.take();
        let maximized = win_attribs.maximized;
        let visible = win_attribs.visible;
        let decorations = win_attribs.decorations;

        let window = Arc::new(UnownedWindow {
            nsview,
            nswindow,
            input_context,
            shared_state: Arc::new(Mutex::new(win_attribs.into())),
            decorations: AtomicBool::new(decorations),
            cursor,
            cursor_hidden: Default::default(),
        });

        let delegate = new_delegate(&window, fullscreen.is_some());

        // Set fullscreen mode after we setup everything
        if let Some(monitor) = fullscreen {
            if monitor.inner != window.get_current_monitor().inner {
                // To do this with native fullscreen, we probably need to
                // warp the window... while we could use
                // `enterFullScreenMode`, they're idiomatically different
                // fullscreen modes, so we'd have to support both anyway.
                unimplemented!();
            }
            window.set_fullscreen(Some(monitor));
        }

        // Setting the window as key has to happen *after* we set the fullscreen
        // state, since otherwise we'll briefly see the window at normal size
        // before it transitions.
        unsafe {
            if visible {
                window.nswindow.makeKeyAndOrderFront_(nil);
            } else {
                window.nswindow.makeKeyWindow();
            }
        }

        if maximized {
            window.set_maximized(maximized);
        }

        unsafe { pool.drain() };

        Ok((window, delegate))
    }

    fn set_style_mask_async(&self, mask: NSWindowStyleMask) {
        unsafe { util::set_style_mask_async(
            *self.nswindow,
            *self.nsview,
            mask,
        ) };
    }

    fn set_style_mask_sync(&self, mask: NSWindowStyleMask) {
        unsafe { util::set_style_mask_sync(
            *self.nswindow,
            *self.nsview,
            mask,
        ) };
    }

    pub fn id(&self) -> Id {
        get_window_id(*self.nswindow)
    }

    pub fn set_title(&self, title: &str) {
        unsafe {
            let title = IdRef::new(NSString::alloc(nil).init_str(title));
            self.nswindow.setTitle_(*title);
        }
    }

    #[inline]
    pub fn show(&self) {
        unsafe { util::make_key_and_order_front_async(*self.nswindow) };
    }

    #[inline]
    pub fn hide(&self) {
        unsafe { util::order_out_async(*self.nswindow) };
    }

    pub fn request_redraw(&self) {
        AppState::queue_redraw(RootWindowId(self.id()));
    }

    pub fn get_position(&self) -> Option<LogicalPosition> {
        let frame_rect = unsafe { NSWindow::frame(*self.nswindow) };
        Some((
            frame_rect.origin.x as f64,
            util::bottom_left_to_top_left(frame_rect),
        ).into())
    }

    pub fn get_inner_position(&self) -> Option<LogicalPosition> {
        let content_rect = unsafe {
            NSWindow::contentRectForFrameRect_(
                *self.nswindow,
                NSWindow::frame(*self.nswindow),
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
                // While it's true that we're setting the top-left position,
                // it still needs to be in a bottom-left coordinate system.
                CGDisplay::main().pixels_high() as f64 - position.y,
            ),
            NSSize::new(0f64, 0f64),
        );
        unsafe {
            util::set_frame_top_left_point_async(*self.nswindow, dummy.origin);
        }
    }

    #[inline]
    pub fn get_inner_size(&self) -> Option<LogicalSize> {
        let view_frame = unsafe { NSView::frame(*self.nsview) };
        Some((view_frame.size.width as f64, view_frame.size.height as f64).into())
    }

    #[inline]
    pub fn get_outer_size(&self) -> Option<LogicalSize> {
        let view_frame = unsafe { NSWindow::frame(*self.nswindow) };
        Some((view_frame.size.width as f64, view_frame.size.height as f64).into())
    }

    #[inline]
    pub fn set_inner_size(&self, size: LogicalSize) {
        unsafe {
            util::set_content_size_async(*self.nswindow, size);
        }
    }

    pub fn set_min_dimensions(&self, dimensions: Option<LogicalSize>) {
        unsafe {
            let dimensions = dimensions.unwrap_or_else(|| (0, 0).into());
            set_min_dimensions(*self.nswindow, dimensions);
        }
    }

    pub fn set_max_dimensions(&self, dimensions: Option<LogicalSize>) {
        unsafe {
            let dimensions = dimensions.unwrap_or_else(|| (!0, !0).into());
            set_max_dimensions(*self.nswindow, dimensions);
        }
    }

    #[inline]
    pub fn set_resizable(&self, resizable: bool) {
        let fullscreen = {
            trace!("Locked shared state in `set_resizable`");
            let mut shared_state_lock = self.shared_state.lock().unwrap();
            shared_state_lock.resizable = resizable;
            trace!("Unlocked shared state in `set_resizable`");
            shared_state_lock.fullscreen.is_some()
        };
        if !fullscreen {
            let mut mask = unsafe { self.nswindow.styleMask() };
            if resizable {
                mask |= NSWindowStyleMask::NSResizableWindowMask;
            } else {
                mask &= !NSWindowStyleMask::NSResizableWindowMask;
            }
            self.set_style_mask_async(mask);
        } // Otherwise, we don't change the mask until we exit fullscreen.
    }

    pub fn set_cursor(&self, cursor: MouseCursor) {
        let cursor = util::Cursor::from(cursor);
        if let Some(cursor_access) = self.cursor.upgrade() {
            *cursor_access.lock().unwrap() = cursor;
        }
        unsafe {
            let _: () = msg_send![*self.nswindow,
                invalidateCursorRectsForView:*self.nsview
            ];
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
        unsafe { NSWindow::backingScaleFactor(*self.nswindow) as _ }
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

    pub(crate) fn is_zoomed(&self) -> bool {
        // because `isZoomed` doesn't work if the window's borderless,
        // we make it resizable temporalily.
        let curr_mask = unsafe { self.nswindow.styleMask() };

        let required = NSWindowStyleMask::NSTitledWindowMask
            | NSWindowStyleMask::NSResizableWindowMask;
        let needs_temp_mask = !curr_mask.contains(required);
        if needs_temp_mask {
            self.set_style_mask_sync(required);
        }

        let is_zoomed: BOOL = unsafe { msg_send![*self.nswindow, isZoomed] };

        // Roll back temp styles
        if needs_temp_mask {
            self.set_style_mask_async(curr_mask);
        }

        is_zoomed != 0
    }

    fn saved_style(&self, shared_state: &mut SharedState) -> NSWindowStyleMask {
        let base_mask = shared_state.saved_style
            .take()
            .unwrap_or_else(|| unsafe { self.nswindow.styleMask() });
        if shared_state.resizable {
            base_mask | NSWindowStyleMask::NSResizableWindowMask
        } else {
            base_mask & !NSWindowStyleMask::NSResizableWindowMask
        }
    }

    fn saved_standard_frame(shared_state: &mut SharedState) -> NSRect {
        shared_state.standard_frame.unwrap_or_else(|| NSRect::new(
            NSPoint::new(50.0, 50.0),
            NSSize::new(800.0, 600.0),
        ))
    }

    pub(crate) fn restore_state_from_fullscreen(&self) {
        let maximized = {
            trace!("Locked shared state in `restore_state_from_fullscreen`");
            let mut shared_state_lock = self.shared_state.lock().unwrap();

            shared_state_lock.fullscreen = None;

            let mask = self.saved_style(&mut *shared_state_lock);

            self.set_style_mask_async(mask);
            shared_state_lock.maximized
        };
        trace!("Unocked shared state in `restore_state_from_fullscreen`");
        self.set_maximized(maximized);
    }

    #[inline]
    pub fn set_maximized(&self, maximized: bool) {
        let is_zoomed = self.is_zoomed();
        if is_zoomed == maximized { return };

        trace!("Locked shared state in `set_maximized`");
        let mut shared_state_lock = self.shared_state.lock().unwrap();

        // Save the standard frame sized if it is not zoomed
        if !is_zoomed {
            unsafe {
                shared_state_lock.standard_frame = Some(NSWindow::frame(*self.nswindow));
            }
        }

        shared_state_lock.maximized = maximized;

        let curr_mask = unsafe { self.nswindow.styleMask() };
        if shared_state_lock.fullscreen.is_some() {
            // Handle it in window_did_exit_fullscreen
            return;
        } else if curr_mask.contains(NSWindowStyleMask::NSResizableWindowMask) {
            // Just use the native zoom if resizable
            unsafe { self.nswindow.zoom_(nil) };
        } else {
            // if it's not resizable, we set the frame directly
            unsafe {
                let new_rect = if maximized {
                    let screen = NSScreen::mainScreen(nil);
                    NSScreen::visibleFrame(screen)
                } else {
                    Self::saved_standard_frame(&mut *shared_state_lock)
                };
                // This probably isn't thread-safe!
                self.nswindow.setFrame_display_(new_rect, 0);
            }
        }

        trace!("Unlocked shared state in `set_maximized`");
    }

    #[inline]
    pub fn get_fullscreen(&self) -> Option<RootMonitorHandle> {
        let shared_state_lock = self.shared_state.lock().unwrap();
        shared_state_lock.fullscreen.clone()
    }

    #[inline]
    /// TODO: Right now set_fullscreen do not work on switching monitors
    /// in fullscreen mode
    pub fn set_fullscreen(&self, monitor: Option<RootMonitorHandle>) {
        let shared_state_lock = self.shared_state.lock().unwrap();
        if shared_state_lock.is_simple_fullscreen {
            return
        }

        let not_fullscreen = {
            trace!("Locked shared state in `set_fullscreen`");
            let current = &shared_state_lock.fullscreen;
            match (current, monitor) {
                (&Some(ref a), Some(ref b)) if a.inner != b.inner => {
                    // Our best bet is probably to move to the origin of the
                    // target monitor.
                    unimplemented!()
                },
                (&None, None) | (&Some(_), Some(_)) => return,
                _ => (),
            }
            trace!("Unlocked shared state in `set_fullscreen`");
            current.is_none()
        };

        unsafe { util::toggle_full_screen_async(
            *self.nswindow,
            *self.nsview,
            not_fullscreen,
            Arc::downgrade(&self.shared_state),
        ) };
    }

    #[inline]
    pub fn set_decorations(&self, decorations: bool) {
        if decorations != self.decorations.load(Ordering::Acquire) {
            self.decorations.store(decorations, Ordering::Release);

            let (fullscreen, resizable) = {
                trace!("Locked shared state in `set_decorations`");
                let shared_state_lock = self.shared_state.lock().unwrap();
                trace!("Unlocked shared state in `set_decorations`");
                (
                    shared_state_lock.fullscreen.is_some(),
                    shared_state_lock.resizable,
                )
            };

            // If we're in fullscreen mode, we wait to apply decoration changes
            // until we're in `window_did_exit_fullscreen`.
            if fullscreen { return }

            let new_mask = {
                let mut new_mask = if decorations {
                    NSWindowStyleMask::NSClosableWindowMask
                        | NSWindowStyleMask::NSMiniaturizableWindowMask
                        | NSWindowStyleMask::NSResizableWindowMask
                        | NSWindowStyleMask::NSTitledWindowMask
                } else {
                    NSWindowStyleMask::NSBorderlessWindowMask
                        | NSWindowStyleMask::NSResizableWindowMask
                };
                if !resizable {
                    new_mask &= !NSWindowStyleMask::NSResizableWindowMask;
                }
                new_mask
            };
            self.set_style_mask_async(new_mask);
        }
    }

    #[inline]
    pub fn set_always_on_top(&self, always_on_top: bool) {
        let level = if always_on_top {
            ffi::NSWindowLevel::NSFloatingWindowLevel
        } else {
            ffi::NSWindowLevel::NSNormalWindowLevel
        };
        unsafe { util::set_level_async(*self.nswindow, level) };
    }

    #[inline]
    pub fn set_window_icon(&self, _icon: Option<Icon>) {
        // macOS doesn't have window icons. Though, there is
        // `setRepresentedFilename`, but that's semantically distinct and should
        // only be used when the window is in some way representing a specific
        // file/directory. For instance, Terminal.app uses this for the CWD.
        // Anyway, that should eventually be implemented as
        // `WindowBuilderExt::with_represented_file` or something, and doesn't
        // have anything to do with `set_window_icon`.
        // https://developer.apple.com/library/content/documentation/Cocoa/Conceptual/WinPanel/Tasks/SettingWindowTitle.html
    }

    #[inline]
    pub fn set_ime_spot(&self, logical_spot: LogicalPosition) {
        unsafe {
            view::set_ime_spot(
                *self.nsview,
                *self.input_context,
                logical_spot.x,
                logical_spot.y,
            );
        }
    }

    #[inline]
    pub fn get_current_monitor(&self) -> RootMonitorHandle {
        unsafe {
            let screen: id = msg_send![*self.nswindow, screen];
            let desc = NSScreen::deviceDescription(screen);
            let key = IdRef::new(NSString::alloc(nil).init_str("NSScreenNumber"));
            let value = NSDictionary::valueForKey_(desc, *key);
            let display_id = msg_send![value, unsignedIntegerValue];
            RootMonitorHandle { inner: MonitorHandle::new(display_id) }
        }
    }

    #[inline]
    pub fn get_available_monitors(&self) -> VecDeque<MonitorHandle> {
        monitor::get_available_monitors()
    }

    #[inline]
    pub fn get_primary_monitor(&self) -> MonitorHandle {
        monitor::get_primary_monitor()
    }
}

impl WindowExtMacOS for UnownedWindow {
    #[inline]
    fn get_nswindow(&self) -> *mut c_void {
        *self.nswindow as *mut _
    }

    #[inline]
    fn get_nsview(&self) -> *mut c_void {
        *self.nsview as *mut _
    }

    #[inline]
    fn request_user_attention(&self, is_critical: bool) {
        unsafe {
            NSApp().requestUserAttention_(match is_critical {
                true => NSRequestUserAttentionType::NSCriticalRequest,
                false => NSRequestUserAttentionType::NSInformationalRequest,
            });
        }
    }

    #[inline]
    fn get_simple_fullscreen(&self) -> bool {
        let shared_state_lock = self.shared_state.lock().unwrap();
        shared_state_lock.is_simple_fullscreen
    }

    #[inline]
    fn set_simple_fullscreen(&self, fullscreen: bool) -> bool {
        let mut shared_state_lock = self.shared_state.lock().unwrap();

        unsafe {
            let app = NSApp();
            let is_native_fullscreen = shared_state_lock.fullscreen.is_some();
            let is_simple_fullscreen = shared_state_lock.is_simple_fullscreen;

            // Do nothing if native fullscreen is active.
            if is_native_fullscreen || (fullscreen && is_simple_fullscreen) || (!fullscreen && !is_simple_fullscreen) {
                return false;
            }

            if fullscreen {
                // Remember the original window's settings
                shared_state_lock.standard_frame = Some(NSWindow::frame(*self.nswindow));
                shared_state_lock.saved_style = Some(self.nswindow.styleMask());
                shared_state_lock.save_presentation_opts = Some(app.presentationOptions_());

                // Tell our window's state that we're in fullscreen
                shared_state_lock.is_simple_fullscreen = true;

                // Simulate pre-Lion fullscreen by hiding the dock and menu bar
                let presentation_options =
                    NSApplicationPresentationOptions::NSApplicationPresentationAutoHideDock |
                    NSApplicationPresentationOptions::NSApplicationPresentationAutoHideMenuBar;
                app.setPresentationOptions_(presentation_options);

                // Hide the titlebar
                util::toggle_style_mask(*self.nswindow, *self.nsview, NSWindowStyleMask::NSTitledWindowMask, false);

                // Set the window frame to the screen frame size
                let screen = self.nswindow.screen();
                let screen_frame = NSScreen::frame(screen);
                NSWindow::setFrame_display_(*self.nswindow, screen_frame, YES);

                // Fullscreen windows can't be resized, minimized, or moved
                util::toggle_style_mask(*self.nswindow, *self.nsview, NSWindowStyleMask::NSMiniaturizableWindowMask, false);
                util::toggle_style_mask(*self.nswindow, *self.nsview, NSWindowStyleMask::NSResizableWindowMask, false);
                NSWindow::setMovable_(*self.nswindow, NO);

                true
            } else {
                let new_mask = self.saved_style(&mut *shared_state_lock);
                self.set_style_mask_async(new_mask);
                shared_state_lock.is_simple_fullscreen = false;

                if let Some(presentation_opts) = shared_state_lock.save_presentation_opts {
                    app.setPresentationOptions_(presentation_opts);
                }

                let frame = Self::saved_standard_frame(&mut *shared_state_lock);
                NSWindow::setFrame_display_(*self.nswindow, frame, YES);
                NSWindow::setMovable_(*self.nswindow, YES);

                true
            }
        }
    }
}

impl Drop for UnownedWindow {
    fn drop(&mut self) {
        trace!("Dropping `UnownedWindow` ({:?})", self as *mut _);
        // Close the window if it has not yet been closed.
        if *self.nswindow != nil {
            unsafe { util::close_async(*self.nswindow) };
        }
    }
}

unsafe fn set_min_dimensions<V: NSWindow + Copy>(window: V, mut min_size: LogicalSize) {
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

unsafe fn set_max_dimensions<V: NSWindow + Copy>(window: V, mut max_size: LogicalSize) {
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
