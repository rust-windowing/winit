#![allow(clippy::unnecessary_cast)]

use std::{
    collections::VecDeque,
    f64, ops,
    os::raw::c_void,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, MutexGuard,
    },
};

use raw_window_handle::{
    AppKitDisplayHandle, AppKitWindowHandle, RawDisplayHandle, RawWindowHandle,
};

use crate::{
    dpi::{
        LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize, Position, Size, Size::Logical,
    },
    error::{ExternalError, NotSupportedError, OsError as RootOsError},
    event::WindowEvent,
    icon::Icon,
    platform::macos::{OptionAsAlt, WindowExtMacOS},
    platform_impl::platform::{
        app_state::AppState,
        appkit::NSWindowOrderingMode,
        ffi,
        monitor::{self, MonitorHandle, VideoMode},
        util,
        view::WinitView,
        window_delegate::WinitWindowDelegate,
        Fullscreen, OsError,
    },
    window::{
        CursorGrabMode, CursorIcon, ImePurpose, ResizeDirection, Theme, UserAttentionType,
        WindowAttributes, WindowButtons, WindowId as RootWindowId, WindowLevel,
    },
};
use core_graphics::display::{CGDisplay, CGPoint};
use objc2::declare::{Ivar, IvarDrop};
use objc2::foundation::{
    is_main_thread, CGFloat, NSArray, NSCopying, NSInteger, NSObject, NSPoint, NSRect, NSSize,
    NSString,
};
use objc2::rc::{autoreleasepool, Id, Owned, Shared};
use objc2::{declare_class, msg_send, msg_send_id, sel, ClassType};

use super::appkit::{
    NSApp, NSAppKitVersion, NSAppearance, NSApplicationPresentationOptions, NSBackingStoreType,
    NSColor, NSCursor, NSFilenamesPboardType, NSRequestUserAttentionType, NSResponder, NSScreen,
    NSView, NSWindow, NSWindowButton, NSWindowLevel, NSWindowSharingType, NSWindowStyleMask,
    NSWindowTitleVisibility,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(pub usize);

impl WindowId {
    pub const unsafe fn dummy() -> Self {
        Self(0)
    }
}

impl From<WindowId> for u64 {
    fn from(window_id: WindowId) -> Self {
        window_id.0 as u64
    }
}

impl From<u64> for WindowId {
    fn from(raw_id: u64) -> Self {
        Self(raw_id as usize)
    }
}

#[derive(Clone)]
pub struct PlatformSpecificWindowBuilderAttributes {
    pub movable_by_window_background: bool,
    pub titlebar_transparent: bool,
    pub title_hidden: bool,
    pub titlebar_hidden: bool,
    pub titlebar_buttons_hidden: bool,
    pub fullsize_content_view: bool,
    pub disallow_hidpi: bool,
    pub has_shadow: bool,
    pub accepts_first_mouse: bool,
    pub option_as_alt: OptionAsAlt,
}

impl Default for PlatformSpecificWindowBuilderAttributes {
    #[inline]
    fn default() -> Self {
        Self {
            movable_by_window_background: false,
            titlebar_transparent: false,
            title_hidden: false,
            titlebar_hidden: false,
            titlebar_buttons_hidden: false,
            fullsize_content_view: false,
            disallow_hidpi: false,
            has_shadow: true,
            accepts_first_mouse: true,
            option_as_alt: Default::default(),
        }
    }
}

declare_class!(
    #[derive(Debug)]
    pub(crate) struct WinitWindow {
        // TODO: Fix unnecessary boxing here
        // SAFETY: These are initialized in WinitWindow::new, right after it is created.
        shared_state: IvarDrop<Box<Mutex<SharedState>>>,
        decorations: IvarDrop<Box<AtomicBool>>,
    }

    unsafe impl ClassType for WinitWindow {
        #[inherits(NSResponder, NSObject)]
        type Super = NSWindow;
    }

    unsafe impl WinitWindow {
        #[sel(canBecomeMainWindow)]
        fn can_become_main_window(&self) -> bool {
            trace_scope!("canBecomeMainWindow");
            true
        }

        #[sel(canBecomeKeyWindow)]
        fn can_become_key_window(&self) -> bool {
            trace_scope!("canBecomeKeyWindow");
            true
        }
    }
);

#[derive(Debug, Default)]
pub struct SharedState {
    pub resizable: bool,
    /// This field tracks the current fullscreen state of the window
    /// (as seen by `WindowDelegate`).
    pub(crate) fullscreen: Option<Fullscreen>,
    // This is true between windowWillEnterFullScreen and windowDidEnterFullScreen
    // or windowWillExitFullScreen and windowDidExitFullScreen.
    // We must not toggle fullscreen when this is true.
    pub in_fullscreen_transition: bool,
    // If it is attempted to toggle fullscreen when in_fullscreen_transition is true,
    // Set target_fullscreen and do after fullscreen transition is end.
    pub(crate) target_fullscreen: Option<Option<Fullscreen>>,
    pub maximized: bool,
    pub standard_frame: Option<NSRect>,
    pub(crate) is_simple_fullscreen: bool,
    pub saved_style: Option<NSWindowStyleMask>,
    /// Presentation options saved before entering `set_simple_fullscreen`, and
    /// restored upon exiting it. Also used when transitioning from Borderless to
    /// Exclusive fullscreen in `set_fullscreen` because we need to disable the menu
    /// bar in exclusive fullscreen but want to restore the original options when
    /// transitioning back to borderless fullscreen.
    save_presentation_opts: Option<NSApplicationPresentationOptions>,
    pub current_theme: Option<Theme>,

    /// The current resize incerments for the window content.
    pub(crate) resize_increments: NSSize,

    /// The state of the `Option` as `Alt`.
    pub(crate) option_as_alt: OptionAsAlt,
}

impl SharedState {
    pub fn saved_standard_frame(&self) -> NSRect {
        self.standard_frame
            .unwrap_or_else(|| NSRect::new(NSPoint::new(50.0, 50.0), NSSize::new(800.0, 600.0)))
    }
}

pub(crate) struct SharedStateMutexGuard<'a> {
    guard: MutexGuard<'a, SharedState>,
    called_from_fn: &'static str,
}

impl<'a> SharedStateMutexGuard<'a> {
    #[inline]
    fn new(guard: MutexGuard<'a, SharedState>, called_from_fn: &'static str) -> Self {
        trace!("Locked shared state in `{}`", called_from_fn);
        Self {
            guard,
            called_from_fn,
        }
    }
}

impl ops::Deref for SharedStateMutexGuard<'_> {
    type Target = SharedState;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.guard.deref()
    }
}

impl ops::DerefMut for SharedStateMutexGuard<'_> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard.deref_mut()
    }
}

impl Drop for SharedStateMutexGuard<'_> {
    #[inline]
    fn drop(&mut self) {
        trace!("Unlocked shared state in `{}`", self.called_from_fn);
    }
}

impl WinitWindow {
    #[allow(clippy::type_complexity)]
    pub(crate) fn new(
        attrs: WindowAttributes,
        pl_attrs: PlatformSpecificWindowBuilderAttributes,
    ) -> Result<(Id<Self, Shared>, Id<WinitWindowDelegate, Shared>), RootOsError> {
        trace_scope!("WinitWindow::new");

        if !is_main_thread() {
            panic!("Windows can only be created on the main thread on macOS");
        }

        let this = autoreleasepool(|_| {
            let screen = match attrs.fullscreen.clone().map(Into::into) {
                Some(Fullscreen::Borderless(Some(monitor)))
                | Some(Fullscreen::Exclusive(VideoMode { monitor, .. })) => {
                    monitor.ns_screen().or_else(NSScreen::main)
                }
                Some(Fullscreen::Borderless(None)) => NSScreen::main(),
                None => None,
            };
            let frame = match &screen {
                Some(screen) => screen.frame(),
                None => {
                    let scale_factor = NSScreen::main()
                        .map(|screen| screen.backingScaleFactor() as f64)
                        .unwrap_or(1.0);
                    let (width, height) = match attrs.inner_size {
                        Some(size) => {
                            let logical = size.to_logical(scale_factor);
                            (logical.width, logical.height)
                        }
                        None => (800.0, 600.0),
                    };
                    let (left, bottom) = match attrs.position {
                        Some(position) => {
                            let logical = util::window_position(position.to_logical(scale_factor));
                            // macOS wants the position of the bottom left corner,
                            // but caller is setting the position of top left corner
                            (logical.x, logical.y - height)
                        }
                        // This value is ignored by calling win.center() below
                        None => (0.0, 0.0),
                    };
                    NSRect::new(NSPoint::new(left, bottom), NSSize::new(width, height))
                }
            };

            let mut masks = if (!attrs.decorations && screen.is_none()) || pl_attrs.titlebar_hidden
            {
                // Resizable without a titlebar or borders
                // if decorations is set to false, ignore pl_attrs
                //
                // if the titlebar is hidden, ignore other pl_attrs
                NSWindowStyleMask::NSBorderlessWindowMask
                    | NSWindowStyleMask::NSResizableWindowMask
                    | NSWindowStyleMask::NSMiniaturizableWindowMask
            } else {
                // default case, resizable window with titlebar and titlebar buttons
                NSWindowStyleMask::NSClosableWindowMask
                    | NSWindowStyleMask::NSMiniaturizableWindowMask
                    | NSWindowStyleMask::NSResizableWindowMask
                    | NSWindowStyleMask::NSTitledWindowMask
            };

            if !attrs.resizable {
                masks &= !NSWindowStyleMask::NSResizableWindowMask;
            }

            if !attrs.enabled_buttons.contains(WindowButtons::MINIMIZE) {
                masks &= !NSWindowStyleMask::NSMiniaturizableWindowMask;
            }

            if !attrs.enabled_buttons.contains(WindowButtons::CLOSE) {
                masks &= !NSWindowStyleMask::NSClosableWindowMask;
            }

            if pl_attrs.fullsize_content_view {
                masks |= NSWindowStyleMask::NSFullSizeContentViewWindowMask;
            }

            let this: Option<Id<Self, Owned>> = unsafe {
                msg_send_id![
                    msg_send_id![WinitWindow::class(), alloc],
                    initWithContentRect: frame,
                    styleMask: masks,
                    backing: NSBackingStoreType::NSBackingStoreBuffered,
                    defer: false,
                ]
            };

            this.map(|mut this| {
                let resize_increments = match attrs
                    .resize_increments
                    .map(|i| i.to_logical::<f64>(this.scale_factor()))
                {
                    Some(LogicalSize { width, height }) if width >= 1. && height >= 1. => {
                        NSSize::new(width, height)
                    }
                    _ => NSSize::new(1., 1.),
                };

                // Properly initialize the window's variables
                //
                // Ideally this should be done in an `init` method,
                // but for convenience we do it here instead.
                let state = SharedState {
                    resizable: attrs.resizable,
                    maximized: attrs.maximized,
                    resize_increments,
                    ..Default::default()
                };
                Ivar::write(&mut this.shared_state, Box::new(Mutex::new(state)));
                Ivar::write(
                    &mut this.decorations,
                    Box::new(AtomicBool::new(attrs.decorations)),
                );

                this.setReleasedWhenClosed(false);
                this.setTitle(&NSString::from_str(&attrs.title));
                this.setAcceptsMouseMovedEvents(true);

                if attrs.content_protected {
                    this.setSharingType(NSWindowSharingType::NSWindowSharingNone);
                }

                if pl_attrs.titlebar_transparent {
                    this.setTitlebarAppearsTransparent(true);
                }
                if pl_attrs.title_hidden {
                    this.setTitleVisibility(NSWindowTitleVisibility::Hidden);
                }
                if pl_attrs.titlebar_buttons_hidden {
                    for titlebar_button in &[
                        #[allow(deprecated)]
                        NSWindowButton::FullScreen,
                        NSWindowButton::Miniaturize,
                        NSWindowButton::Close,
                        NSWindowButton::Zoom,
                    ] {
                        if let Some(button) = this.standardWindowButton(*titlebar_button) {
                            button.setHidden(true);
                        }
                    }
                }
                if pl_attrs.movable_by_window_background {
                    this.setMovableByWindowBackground(true);
                }

                if !attrs.enabled_buttons.contains(WindowButtons::MAXIMIZE) {
                    if let Some(button) = this.standardWindowButton(NSWindowButton::Zoom) {
                        button.setEnabled(false);
                    }
                }

                if !pl_attrs.has_shadow {
                    this.setHasShadow(false);
                }
                if attrs.position.is_none() {
                    this.center();
                }

                this.set_option_as_alt(pl_attrs.option_as_alt);

                Id::into_shared(this)
            })
        })
        .ok_or_else(|| os_error!(OsError::CreationError("Couldn't create `NSWindow`")))?;

        match attrs.parent_window {
            Some(RawWindowHandle::AppKit(handle)) => {
                // SAFETY: Caller ensures the pointer is valid or NULL
                let parent: Id<NSWindow, Shared> =
                    match unsafe { Id::retain(handle.ns_window.cast()) } {
                        Some(window) => window,
                        None => {
                            // SAFETY: Caller ensures the pointer is valid or NULL
                            let parent_view: Id<NSView, Shared> =
                                match unsafe { Id::retain(handle.ns_view.cast()) } {
                                    Some(view) => view,
                                    None => {
                                        return Err(os_error!(OsError::CreationError(
                                            "raw window handle should be non-empty"
                                        )))
                                    }
                                };
                            parent_view.window().ok_or_else(|| {
                                os_error!(OsError::CreationError(
                                    "parent view should be installed in a window"
                                ))
                            })?
                        }
                    };
                // SAFETY: We know that there are no parent -> child -> parent cycles since the only place in `winit`
                // where we allow making a window a child window is right here, just after it's been created.
                unsafe { parent.addChildWindow(&this, NSWindowOrderingMode::NSWindowAbove) };
            }
            Some(raw) => panic!("Invalid raw window handle {raw:?} on macOS"),
            None => (),
        }

        let view = WinitView::new(&this, pl_attrs.accepts_first_mouse);

        // The default value of `setWantsBestResolutionOpenGLSurface:` was `false` until
        // macos 10.14 and `true` after 10.15, we should set it to `YES` or `NO` to avoid
        // always the default system value in favour of the user's code
        view.setWantsBestResolutionOpenGLSurface(!pl_attrs.disallow_hidpi);

        // On Mojave, views automatically become layer-backed shortly after being added to
        // a window. Changing the layer-backedness of a view breaks the association between
        // the view and its associated OpenGL context. To work around this, on Mojave we
        // explicitly make the view layer-backed up front so that AppKit doesn't do it
        // itself and break the association with its context.
        if NSAppKitVersion::current().floor() > NSAppKitVersion::NSAppKitVersionNumber10_12 {
            view.setWantsLayer(true);
        }

        // Configure the new view as the "key view" for the window
        this.setContentView(&view);
        this.setInitialFirstResponder(&view);

        if attrs.transparent {
            this.setOpaque(false);
            this.setBackgroundColor(&NSColor::clear());
        }

        if let Some(dim) = attrs.min_inner_size {
            this.set_min_inner_size(Some(dim));
        }
        if let Some(dim) = attrs.max_inner_size {
            this.set_max_inner_size(Some(dim));
        }

        this.set_window_level(attrs.window_level);

        // register for drag and drop operations.
        this.registerForDraggedTypes(&NSArray::from_slice(&[
            unsafe { NSFilenamesPboardType }.copy()
        ]));

        match attrs.preferred_theme {
            Some(theme) => {
                set_ns_theme(Some(theme));
                let mut state = this.lock_shared_state("WinitWindow::new");
                state.current_theme = Some(theme);
            }
            None => {
                let mut state = this.lock_shared_state("WinitWindow::new");
                state.current_theme = Some(get_ns_theme());
            }
        }

        let delegate = WinitWindowDelegate::new(&this, attrs.fullscreen.is_some());

        // XXX Send `Focused(false)` right after creating the window delegate, so we won't
        // obscure the real focused events on the startup.
        delegate.queue_event(WindowEvent::Focused(false));

        // Set fullscreen mode after we setup everything
        this.set_fullscreen(attrs.fullscreen.map(Into::into));

        // Setting the window as key has to happen *after* we set the fullscreen
        // state, since otherwise we'll briefly see the window at normal size
        // before it transitions.
        if attrs.visible {
            if attrs.active {
                // Tightly linked with `app_state::window_activation_hack`
                this.makeKeyAndOrderFront(None);
            } else {
                this.orderFront(None);
            }
        }

        if attrs.maximized {
            this.set_maximized(attrs.maximized);
        }

        Ok((this, delegate))
    }

    pub(super) fn retain(&self) -> Id<WinitWindow, Shared> {
        // SAFETY: The pointer is valid, and the window is always `Shared`
        // TODO(madsmtm): Remove the need for unsafety here
        unsafe { Id::retain(self as *const Self as *mut Self).unwrap() }
    }

    pub(super) fn view(&self) -> Id<WinitView, Shared> {
        // SAFETY: The view inside WinitWindow is always `WinitView`
        unsafe { Id::cast(self.contentView()) }
    }

    #[track_caller]
    pub(crate) fn lock_shared_state(
        &self,
        called_from_fn: &'static str,
    ) -> SharedStateMutexGuard<'_> {
        SharedStateMutexGuard::new(self.shared_state.lock().unwrap(), called_from_fn)
    }

    fn set_style_mask_sync(&self, mask: NSWindowStyleMask) {
        util::set_style_mask_sync(self, mask);
    }

    pub fn id(&self) -> WindowId {
        WindowId(self as *const Self as usize)
    }

    pub fn set_title(&self, title: &str) {
        util::set_title_sync(self, title);
    }

    pub fn set_transparent(&self, transparent: bool) {
        self.setOpaque(!transparent)
    }

    pub fn set_visible(&self, visible: bool) {
        match visible {
            true => util::make_key_and_order_front_sync(self),
            false => util::order_out_sync(self),
        }
    }

    #[inline]
    pub fn is_visible(&self) -> Option<bool> {
        Some(self.isVisible())
    }

    pub fn request_redraw(&self) {
        AppState::queue_redraw(RootWindowId(self.id()));
    }

    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        let frame_rect = self.frame();
        let position = LogicalPosition::new(
            frame_rect.origin.x as f64,
            util::bottom_left_to_top_left(frame_rect),
        );
        let scale_factor = self.scale_factor();
        Ok(position.to_physical(scale_factor))
    }

    pub fn inner_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        let content_rect = self.contentRectForFrameRect(self.frame());
        let position = LogicalPosition::new(
            content_rect.origin.x as f64,
            util::bottom_left_to_top_left(content_rect),
        );
        let scale_factor = self.scale_factor();
        Ok(position.to_physical(scale_factor))
    }

    pub fn set_outer_position(&self, position: Position) {
        let scale_factor = self.scale_factor();
        let position = position.to_logical(scale_factor);
        util::set_frame_top_left_point_sync(self, util::window_position(position));
    }

    #[inline]
    pub fn inner_size(&self) -> PhysicalSize<u32> {
        let frame = self.contentView().frame();
        let logical: LogicalSize<f64> = (frame.size.width as f64, frame.size.height as f64).into();
        let scale_factor = self.scale_factor();
        logical.to_physical(scale_factor)
    }

    #[inline]
    pub fn outer_size(&self) -> PhysicalSize<u32> {
        let frame = self.frame();
        let logical: LogicalSize<f64> = (frame.size.width as f64, frame.size.height as f64).into();
        let scale_factor = self.scale_factor();
        logical.to_physical(scale_factor)
    }

    #[inline]
    pub fn set_inner_size(&self, size: Size) {
        let scale_factor = self.scale_factor();
        util::set_content_size_sync(self, size.to_logical(scale_factor));
    }

    pub fn set_min_inner_size(&self, dimensions: Option<Size>) {
        let dimensions = dimensions.unwrap_or(Logical(LogicalSize {
            width: 0.0,
            height: 0.0,
        }));
        let min_size = dimensions.to_logical::<CGFloat>(self.scale_factor());

        let mut current_rect = self.frame();
        let content_rect = self.contentRectForFrameRect(current_rect);
        // Convert from client area size to window size
        let min_size = NSSize::new(
            min_size.width + (current_rect.size.width - content_rect.size.width), // this tends to be 0
            min_size.height + (current_rect.size.height - content_rect.size.height),
        );
        self.setMinSize(min_size);
        // If necessary, resize the window to match constraint
        if current_rect.size.width < min_size.width {
            current_rect.size.width = min_size.width;
            self.setFrame_display(current_rect, false)
        }
        if current_rect.size.height < min_size.height {
            // The origin point of a rectangle is at its bottom left in Cocoa.
            // To ensure the window's top-left point remains the same:
            current_rect.origin.y += current_rect.size.height - min_size.height;
            current_rect.size.height = min_size.height;
            self.setFrame_display(current_rect, false)
        }
    }

    pub fn set_max_inner_size(&self, dimensions: Option<Size>) {
        let dimensions = dimensions.unwrap_or(Logical(LogicalSize {
            width: std::f32::MAX as f64,
            height: std::f32::MAX as f64,
        }));
        let scale_factor = self.scale_factor();
        let max_size = dimensions.to_logical::<CGFloat>(scale_factor);

        let mut current_rect = self.frame();
        let content_rect = self.contentRectForFrameRect(current_rect);
        // Convert from client area size to window size
        let max_size = NSSize::new(
            max_size.width + (current_rect.size.width - content_rect.size.width), // this tends to be 0
            max_size.height + (current_rect.size.height - content_rect.size.height),
        );
        self.setMaxSize(max_size);
        // If necessary, resize the window to match constraint
        if current_rect.size.width > max_size.width {
            current_rect.size.width = max_size.width;
            self.setFrame_display(current_rect, false)
        }
        if current_rect.size.height > max_size.height {
            // The origin point of a rectangle is at its bottom left in Cocoa.
            // To ensure the window's top-left point remains the same:
            current_rect.origin.y += current_rect.size.height - max_size.height;
            current_rect.size.height = max_size.height;
            self.setFrame_display(current_rect, false)
        }
    }

    pub fn resize_increments(&self) -> Option<PhysicalSize<u32>> {
        let increments = self
            .lock_shared_state("set_resize_increments")
            .resize_increments;
        let (w, h) = (increments.width, increments.height);
        if w > 1.0 || h > 1.0 {
            Some(LogicalSize::new(w, h).to_physical(self.scale_factor()))
        } else {
            None
        }
    }

    pub fn set_resize_increments(&self, increments: Option<Size>) {
        // XXX the resize increments are only used during live resizes.
        let mut shared_state_lock = self.lock_shared_state("set_resize_increments");
        shared_state_lock.resize_increments = increments
            .map(|increments| {
                let logical = increments.to_logical::<f64>(self.scale_factor());
                NSSize::new(logical.width.max(1.0), logical.height.max(1.0))
            })
            .unwrap_or_else(|| NSSize::new(1.0, 1.0));
    }

    pub(crate) fn set_resize_increments_inner(&self, size: NSSize) {
        // It was concluded (#2411) that there is never a use-case for
        // "outer" resize increments, hence we set "inner" ones here.
        // ("outer" in macOS being just resizeIncrements, and "inner" - contentResizeIncrements)
        // This is consistent with X11 size hints behavior
        self.setContentResizeIncrements(size);
    }

    #[inline]
    pub fn set_resizable(&self, resizable: bool) {
        let fullscreen = {
            let mut shared_state_lock = self.lock_shared_state("set_resizable");
            shared_state_lock.resizable = resizable;
            shared_state_lock.fullscreen.is_some()
        };
        if !fullscreen {
            let mut mask = self.styleMask();
            if resizable {
                mask |= NSWindowStyleMask::NSResizableWindowMask;
            } else {
                mask &= !NSWindowStyleMask::NSResizableWindowMask;
            }
            self.set_style_mask_sync(mask);
        }
        // Otherwise, we don't change the mask until we exit fullscreen.
    }

    #[inline]
    pub fn is_resizable(&self) -> bool {
        self.isResizable()
    }

    #[inline]
    pub fn set_enabled_buttons(&self, buttons: WindowButtons) {
        let mut mask = self.styleMask();

        if buttons.contains(WindowButtons::CLOSE) {
            mask |= NSWindowStyleMask::NSClosableWindowMask;
        } else {
            mask &= !NSWindowStyleMask::NSClosableWindowMask;
        }

        if buttons.contains(WindowButtons::MINIMIZE) {
            mask |= NSWindowStyleMask::NSMiniaturizableWindowMask;
        } else {
            mask &= !NSWindowStyleMask::NSMiniaturizableWindowMask;
        }

        // This must happen before the button's "enabled" status has been set,
        // hence we do it synchronously.
        self.set_style_mask_sync(mask);

        // We edit the button directly instead of using `NSResizableWindowMask`,
        // since that mask also affect the resizability of the window (which is
        // controllable by other means in `winit`).
        if let Some(button) = self.standardWindowButton(NSWindowButton::Zoom) {
            button.setEnabled(buttons.contains(WindowButtons::MAXIMIZE));
        }
    }

    #[inline]
    pub fn enabled_buttons(&self) -> WindowButtons {
        let mut buttons = WindowButtons::empty();
        if self.isMiniaturizable() {
            buttons |= WindowButtons::MINIMIZE;
        }
        if self
            .standardWindowButton(NSWindowButton::Zoom)
            .map(|b| b.isEnabled())
            .unwrap_or(true)
        {
            buttons |= WindowButtons::MAXIMIZE;
        }
        if self.hasCloseBox() {
            buttons |= WindowButtons::CLOSE;
        }
        buttons
    }

    pub fn set_cursor_icon(&self, icon: CursorIcon) {
        let view = self.view();
        let mut cursor_state = view.state.cursor_state.lock().unwrap();
        cursor_state.cursor = NSCursor::from_icon(icon);
        drop(cursor_state);
        self.invalidateCursorRectsForView(&view);
    }

    #[inline]
    pub fn set_cursor_grab(&self, mode: CursorGrabMode) -> Result<(), ExternalError> {
        let associate_mouse_cursor = match mode {
            CursorGrabMode::Locked => false,
            CursorGrabMode::None => true,
            CursorGrabMode::Confined => {
                return Err(ExternalError::NotSupported(NotSupportedError::new()))
            }
        };

        // TODO: Do this for real https://stackoverflow.com/a/40922095/5435443
        CGDisplay::associate_mouse_and_mouse_cursor_position(associate_mouse_cursor)
            .map_err(|status| ExternalError::Os(os_error!(OsError::CGError(status))))
    }

    #[inline]
    pub fn set_cursor_visible(&self, visible: bool) {
        let view = self.view();
        let mut cursor_state = view.state.cursor_state.lock().unwrap();
        if visible != cursor_state.visible {
            cursor_state.visible = visible;
            drop(cursor_state);
            self.invalidateCursorRectsForView(&view);
        }
    }

    #[inline]
    pub fn scale_factor(&self) -> f64 {
        self.backingScaleFactor() as f64
    }

    #[inline]
    pub fn set_cursor_position(&self, cursor_position: Position) -> Result<(), ExternalError> {
        let physical_window_position = self.inner_position().unwrap();
        let scale_factor = self.scale_factor();
        let window_position = physical_window_position.to_logical::<CGFloat>(scale_factor);
        let logical_cursor_position = cursor_position.to_logical::<CGFloat>(scale_factor);
        let point = CGPoint {
            x: logical_cursor_position.x + window_position.x,
            y: logical_cursor_position.y + window_position.y,
        };
        CGDisplay::warp_mouse_cursor_position(point)
            .map_err(|e| ExternalError::Os(os_error!(OsError::CGError(e))))?;
        CGDisplay::associate_mouse_and_mouse_cursor_position(true)
            .map_err(|e| ExternalError::Os(os_error!(OsError::CGError(e))))?;

        Ok(())
    }

    #[inline]
    pub fn drag_window(&self) -> Result<(), ExternalError> {
        let event = NSApp().currentEvent();
        self.performWindowDragWithEvent(event.as_deref());
        Ok(())
    }

    #[inline]
    pub fn drag_resize_window(&self, _direction: ResizeDirection) -> Result<(), ExternalError> {
        Err(ExternalError::NotSupported(NotSupportedError::new()))
    }

    #[inline]
    pub fn set_cursor_hittest(&self, hittest: bool) -> Result<(), ExternalError> {
        util::set_ignore_mouse_events_sync(self, !hittest);
        Ok(())
    }

    pub(crate) fn is_zoomed(&self) -> bool {
        // because `isZoomed` doesn't work if the window's borderless,
        // we make it resizable temporalily.
        let curr_mask = self.styleMask();

        let required =
            NSWindowStyleMask::NSTitledWindowMask | NSWindowStyleMask::NSResizableWindowMask;
        let needs_temp_mask = !curr_mask.contains(required);
        if needs_temp_mask {
            self.set_style_mask_sync(required);
        }

        let is_zoomed = self.isZoomed();

        // Roll back temp styles
        if needs_temp_mask {
            self.set_style_mask_sync(curr_mask);
        }

        is_zoomed
    }

    fn saved_style(&self, shared_state: &mut SharedState) -> NSWindowStyleMask {
        let base_mask = shared_state
            .saved_style
            .take()
            .unwrap_or_else(|| self.styleMask());
        if shared_state.resizable {
            base_mask | NSWindowStyleMask::NSResizableWindowMask
        } else {
            base_mask & !NSWindowStyleMask::NSResizableWindowMask
        }
    }

    /// This is called when the window is exiting fullscreen, whether by the
    /// user clicking on the green fullscreen button or programmatically by
    /// `toggleFullScreen:`
    pub(crate) fn restore_state_from_fullscreen(&self) {
        let mut shared_state_lock = self.lock_shared_state("restore_state_from_fullscreen");

        shared_state_lock.fullscreen = None;

        let maximized = shared_state_lock.maximized;
        let mask = self.saved_style(&mut shared_state_lock);

        drop(shared_state_lock);

        self.set_style_mask_sync(mask);
        self.set_maximized(maximized);
    }

    #[inline]
    pub fn set_minimized(&self, minimized: bool) {
        let is_minimized = self.isMiniaturized();
        if is_minimized == minimized {
            return;
        }

        if minimized {
            self.miniaturize(Some(self));
        } else {
            self.deminiaturize(Some(self));
        }
    }

    #[inline]
    pub fn is_minimized(&self) -> Option<bool> {
        Some(self.isMiniaturized())
    }

    #[inline]
    pub fn set_maximized(&self, maximized: bool) {
        let is_zoomed = self.is_zoomed();
        if is_zoomed == maximized {
            return;
        };
        util::set_maximized_sync(self, is_zoomed, maximized);
    }

    #[inline]
    pub(crate) fn fullscreen(&self) -> Option<Fullscreen> {
        let shared_state_lock = self.lock_shared_state("fullscreen");
        shared_state_lock.fullscreen.clone()
    }

    #[inline]
    pub fn is_maximized(&self) -> bool {
        self.is_zoomed()
    }

    #[inline]
    pub(crate) fn set_fullscreen(&self, fullscreen: Option<Fullscreen>) {
        let mut shared_state_lock = self.lock_shared_state("set_fullscreen");
        if shared_state_lock.is_simple_fullscreen {
            return;
        }
        if shared_state_lock.in_fullscreen_transition {
            // We can't set fullscreen here.
            // Set fullscreen after transition.
            shared_state_lock.target_fullscreen = Some(fullscreen);
            return;
        }
        let old_fullscreen = shared_state_lock.fullscreen.clone();
        if fullscreen == old_fullscreen {
            return;
        }
        drop(shared_state_lock);

        // If the fullscreen is on a different monitor, we must move the window
        // to that monitor before we toggle fullscreen (as `toggleFullScreen`
        // does not take a screen parameter, but uses the current screen)
        if let Some(ref fullscreen) = fullscreen {
            let new_screen = match fullscreen {
                Fullscreen::Borderless(Some(monitor)) => monitor.clone(),
                Fullscreen::Borderless(None) => {
                    if let Some(monitor) = self.current_monitor_inner() {
                        monitor
                    } else {
                        return;
                    }
                }
                Fullscreen::Exclusive(video_mode) => video_mode.monitor(),
            }
            .ns_screen()
            .unwrap();

            let old_screen = self.screen().unwrap();
            if old_screen != new_screen {
                let mut screen_frame = new_screen.frame();
                // The coordinate system here has its origin at bottom-left
                // and Y goes up
                screen_frame.origin.y += screen_frame.size.height;
                util::set_frame_top_left_point_sync(self, screen_frame.origin);
            }
        }

        if let Some(Fullscreen::Exclusive(ref video_mode)) = fullscreen {
            // Note: `enterFullScreenMode:withOptions:` seems to do the exact
            // same thing as we're doing here (captures the display, sets the
            // video mode, and hides the menu bar and dock), with the exception
            // of that I couldn't figure out how to set the display mode with
            // it. I think `enterFullScreenMode:withOptions:` is still using the
            // older display mode API where display modes were of the type
            // `CFDictionary`, but this has changed, so we can't obtain the
            // correct parameter for this any longer. Apple's code samples for
            // this function seem to just pass in "YES" for the display mode
            // parameter, which is not consistent with the docs saying that it
            // takes a `NSDictionary`..

            let display_id = video_mode.monitor().native_identifier();

            let mut fade_token = ffi::kCGDisplayFadeReservationInvalidToken;

            if matches!(old_fullscreen, Some(Fullscreen::Borderless(_))) {
                let app = NSApp();
                let mut shared_state_lock = self.lock_shared_state("set_fullscreen");
                shared_state_lock.save_presentation_opts = Some(app.presentationOptions());
            }

            unsafe {
                // Fade to black (and wait for the fade to complete) to hide the
                // flicker from capturing the display and switching display mode
                if ffi::CGAcquireDisplayFadeReservation(5.0, &mut fade_token)
                    == ffi::kCGErrorSuccess
                {
                    ffi::CGDisplayFade(
                        fade_token,
                        0.3,
                        ffi::kCGDisplayBlendNormal,
                        ffi::kCGDisplayBlendSolidColor,
                        0.0,
                        0.0,
                        0.0,
                        ffi::TRUE,
                    );
                }

                assert_eq!(ffi::CGDisplayCapture(display_id), ffi::kCGErrorSuccess);
            }

            unsafe {
                let result = ffi::CGDisplaySetDisplayMode(
                    display_id,
                    video_mode.native_mode.0,
                    std::ptr::null(),
                );
                assert!(result == ffi::kCGErrorSuccess, "failed to set video mode");

                // After the display has been configured, fade back in
                // asynchronously
                if fade_token != ffi::kCGDisplayFadeReservationInvalidToken {
                    ffi::CGDisplayFade(
                        fade_token,
                        0.6,
                        ffi::kCGDisplayBlendSolidColor,
                        ffi::kCGDisplayBlendNormal,
                        0.0,
                        0.0,
                        0.0,
                        ffi::FALSE,
                    );
                    ffi::CGReleaseDisplayFadeReservation(fade_token);
                }
            }
        }

        self.lock_shared_state("set_fullscreen").fullscreen = fullscreen.clone();

        match (&old_fullscreen, &fullscreen) {
            (&None, &Some(_)) => {
                util::toggle_full_screen_sync(self, old_fullscreen.is_none());
            }
            (&Some(Fullscreen::Borderless(_)), &None) => {
                // State is restored by `window_did_exit_fullscreen`
                util::toggle_full_screen_sync(self, old_fullscreen.is_none());
            }
            (&Some(Fullscreen::Exclusive(ref video_mode)), &None) => {
                unsafe {
                    util::restore_display_mode_sync(video_mode.monitor().native_identifier())
                };
                // Rest of the state is restored by `window_did_exit_fullscreen`
                util::toggle_full_screen_sync(self, old_fullscreen.is_none());
            }
            (&Some(Fullscreen::Borderless(_)), &Some(Fullscreen::Exclusive(_))) => {
                // If we're already in fullscreen mode, calling
                // `CGDisplayCapture` will place the shielding window on top of
                // our window, which results in a black display and is not what
                // we want. So, we must place our window on top of the shielding
                // window. Unfortunately, this also makes our window be on top
                // of the menu bar, and this looks broken, so we must make sure
                // that the menu bar is disabled. This is done in the window
                // delegate in `window:willUseFullScreenPresentationOptions:`.
                let app = NSApp();
                self.lock_shared_state("set_fullscreen")
                    .save_presentation_opts = Some(app.presentationOptions());

                let presentation_options =
                    NSApplicationPresentationOptions::NSApplicationPresentationFullScreen
                        | NSApplicationPresentationOptions::NSApplicationPresentationHideDock
                        | NSApplicationPresentationOptions::NSApplicationPresentationHideMenuBar;
                app.setPresentationOptions(presentation_options);

                let window_level =
                    NSWindowLevel(unsafe { ffi::CGShieldingWindowLevel() } as NSInteger + 1);
                self.setLevel(window_level);
            }
            (&Some(Fullscreen::Exclusive(ref video_mode)), &Some(Fullscreen::Borderless(_))) => {
                let presentation_options = self
                    .lock_shared_state("set_fullscreen")
                    .save_presentation_opts
                    .unwrap_or_else(|| {
                        NSApplicationPresentationOptions::NSApplicationPresentationFullScreen
                        | NSApplicationPresentationOptions::NSApplicationPresentationAutoHideDock
                        | NSApplicationPresentationOptions::NSApplicationPresentationAutoHideMenuBar
                    });
                NSApp().setPresentationOptions(presentation_options);

                unsafe {
                    util::restore_display_mode_sync(video_mode.monitor().native_identifier())
                };

                // Restore the normal window level following the Borderless fullscreen
                // `CGShieldingWindowLevel() + 1` hack.
                self.setLevel(NSWindowLevel::Normal);
            }
            _ => {}
        };
    }

    #[inline]
    pub fn set_decorations(&self, decorations: bool) {
        if decorations != self.decorations.load(Ordering::Acquire) {
            self.decorations.store(decorations, Ordering::Release);

            let (fullscreen, resizable) = {
                let shared_state_lock = self.lock_shared_state("set_decorations");
                (
                    shared_state_lock.fullscreen.is_some(),
                    shared_state_lock.resizable,
                )
            };

            // If we're in fullscreen mode, we wait to apply decoration changes
            // until we're in `window_did_exit_fullscreen`.
            if fullscreen {
                return;
            }

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
            self.set_style_mask_sync(new_mask);
        }
    }

    #[inline]
    pub fn is_decorated(&self) -> bool {
        self.decorations.load(Ordering::Acquire)
    }

    #[inline]
    pub fn set_window_level(&self, level: WindowLevel) {
        let level = match level {
            WindowLevel::AlwaysOnTop => NSWindowLevel::Floating,
            WindowLevel::AlwaysOnBottom => NSWindowLevel::BELOW_NORMAL,
            WindowLevel::Normal => NSWindowLevel::Normal,
        };
        util::set_level_sync(self, level);
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
    pub fn set_ime_position(&self, spot: Position) {
        let scale_factor = self.scale_factor();
        let logical_spot = spot.to_logical(scale_factor);
        // TODO(madsmtm): Remove the need for this
        unsafe { Id::from_shared(self.view()) }.set_ime_position(logical_spot);
    }

    #[inline]
    pub fn set_ime_allowed(&self, allowed: bool) {
        // TODO(madsmtm): Remove the need for this
        unsafe { Id::from_shared(self.view()) }.set_ime_allowed(allowed);
    }

    #[inline]
    pub fn set_ime_purpose(&self, _purpose: ImePurpose) {}

    #[inline]
    pub fn focus_window(&self) {
        let is_minimized = self.isMiniaturized();
        let is_visible = self.isVisible();

        if !is_minimized && is_visible {
            NSApp().activateIgnoringOtherApps(true);
            util::make_key_and_order_front_sync(self);
        }
    }

    #[inline]
    pub fn request_user_attention(&self, request_type: Option<UserAttentionType>) {
        let ns_request_type = request_type.map(|ty| match ty {
            UserAttentionType::Critical => NSRequestUserAttentionType::NSCriticalRequest,
            UserAttentionType::Informational => NSRequestUserAttentionType::NSInformationalRequest,
        });
        if let Some(ty) = ns_request_type {
            NSApp().requestUserAttention(ty);
        }
    }

    #[inline]
    // Allow directly accessing the current monitor internally without unwrapping.
    pub(crate) fn current_monitor_inner(&self) -> Option<MonitorHandle> {
        let display_id = self.screen()?.display_id();
        Some(MonitorHandle::new(display_id))
    }

    #[inline]
    pub fn current_monitor(&self) -> Option<MonitorHandle> {
        self.current_monitor_inner()
    }

    #[inline]
    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        monitor::available_monitors()
    }

    #[inline]
    pub fn primary_monitor(&self) -> Option<MonitorHandle> {
        let monitor = monitor::primary_monitor();
        Some(monitor)
    }

    #[inline]
    pub fn raw_window_handle(&self) -> RawWindowHandle {
        let mut window_handle = AppKitWindowHandle::empty();
        window_handle.ns_window = self.ns_window();
        window_handle.ns_view = self.ns_view();
        RawWindowHandle::AppKit(window_handle)
    }

    #[inline]
    pub fn raw_display_handle(&self) -> RawDisplayHandle {
        RawDisplayHandle::AppKit(AppKitDisplayHandle::empty())
    }

    fn toggle_style_mask(&self, mask: NSWindowStyleMask, on: bool) {
        let current_style_mask = self.styleMask();
        if on {
            util::set_style_mask_sync(self, current_style_mask | mask);
        } else {
            util::set_style_mask_sync(self, current_style_mask & (!mask));
        }
    }

    #[inline]
    pub fn theme(&self) -> Option<Theme> {
        self.lock_shared_state("theme").current_theme
    }

    #[inline]
    pub fn has_focus(&self) -> bool {
        self.isKeyWindow()
    }

    pub fn set_theme(&self, theme: Option<Theme>) {
        set_ns_theme(theme);
        self.lock_shared_state("set_theme").current_theme = theme.or_else(|| Some(get_ns_theme()));
    }

    #[inline]
    pub fn set_content_protected(&self, protected: bool) {
        self.setSharingType(if protected {
            NSWindowSharingType::NSWindowSharingNone
        } else {
            NSWindowSharingType::NSWindowSharingReadOnly
        })
    }

    pub fn title(&self) -> String {
        self.title_().to_string()
    }
}

impl WindowExtMacOS for WinitWindow {
    #[inline]
    fn ns_window(&self) -> *mut c_void {
        self as *const Self as *mut _
    }

    #[inline]
    fn ns_view(&self) -> *mut c_void {
        Id::as_ptr(&self.contentView()) as *mut _
    }

    #[inline]
    fn simple_fullscreen(&self) -> bool {
        self.lock_shared_state("simple_fullscreen")
            .is_simple_fullscreen
    }

    #[inline]
    fn set_simple_fullscreen(&self, fullscreen: bool) -> bool {
        let mut shared_state_lock = self.lock_shared_state("set_simple_fullscreen");

        let app = NSApp();
        let is_native_fullscreen = shared_state_lock.fullscreen.is_some();
        let is_simple_fullscreen = shared_state_lock.is_simple_fullscreen;

        // Do nothing if native fullscreen is active.
        if is_native_fullscreen
            || (fullscreen && is_simple_fullscreen)
            || (!fullscreen && !is_simple_fullscreen)
        {
            return false;
        }

        if fullscreen {
            // Remember the original window's settings
            // Exclude title bar
            shared_state_lock.standard_frame = Some(self.contentRectForFrameRect(self.frame()));
            shared_state_lock.saved_style = Some(self.styleMask());
            shared_state_lock.save_presentation_opts = Some(app.presentationOptions());

            // Tell our window's state that we're in fullscreen
            shared_state_lock.is_simple_fullscreen = true;

            // Drop shared state lock before calling app.setPresentationOptions, because
            // it will call our windowDidChangeScreen listener which reacquires the lock
            drop(shared_state_lock);

            // Simulate pre-Lion fullscreen by hiding the dock and menu bar
            let presentation_options =
                NSApplicationPresentationOptions::NSApplicationPresentationAutoHideDock
                    | NSApplicationPresentationOptions::NSApplicationPresentationAutoHideMenuBar;
            app.setPresentationOptions(presentation_options);

            // Hide the titlebar
            self.toggle_style_mask(NSWindowStyleMask::NSTitledWindowMask, false);

            // Set the window frame to the screen frame size
            let screen = self.screen().expect("expected screen to be available");
            self.setFrame_display(screen.frame(), true);

            // Fullscreen windows can't be resized, minimized, or moved
            self.toggle_style_mask(NSWindowStyleMask::NSMiniaturizableWindowMask, false);
            self.toggle_style_mask(NSWindowStyleMask::NSResizableWindowMask, false);
            self.setMovable(false);

            true
        } else {
            let new_mask = self.saved_style(&mut shared_state_lock);
            self.set_style_mask_sync(new_mask);
            shared_state_lock.is_simple_fullscreen = false;

            let save_presentation_opts = shared_state_lock.save_presentation_opts;
            let frame = shared_state_lock.saved_standard_frame();

            // Drop shared state lock before calling app.setPresentationOptions, because
            // it will call our windowDidChangeScreen listener which reacquires the lock
            drop(shared_state_lock);

            if let Some(presentation_opts) = save_presentation_opts {
                app.setPresentationOptions(presentation_opts);
            }

            self.setFrame_display(frame, true);
            self.setMovable(true);

            true
        }
    }

    #[inline]
    fn has_shadow(&self) -> bool {
        self.hasShadow()
    }

    #[inline]
    fn set_has_shadow(&self, has_shadow: bool) {
        self.setHasShadow(has_shadow)
    }

    fn is_document_edited(&self) -> bool {
        self.isDocumentEdited()
    }

    fn set_document_edited(&self, edited: bool) {
        self.setDocumentEdited(edited)
    }

    fn set_option_as_alt(&self, option_as_alt: OptionAsAlt) {
        let mut shared_state_lock = self.shared_state.lock().unwrap();
        shared_state_lock.option_as_alt = option_as_alt;
    }

    fn option_as_alt(&self) -> OptionAsAlt {
        let shared_state_lock = self.shared_state.lock().unwrap();
        shared_state_lock.option_as_alt
    }
}

pub(super) fn get_ns_theme() -> Theme {
    let app = NSApp();
    let has_theme: bool = unsafe { msg_send![&app, respondsToSelector: sel!(effectiveAppearance)] };
    if !has_theme {
        return Theme::Light;
    }
    let appearance = app.effectiveAppearance();
    let name = appearance.bestMatchFromAppearancesWithNames(&NSArray::from_slice(&[
        NSString::from_str("NSAppearanceNameAqua"),
        NSString::from_str("NSAppearanceNameDarkAqua"),
    ]));
    match &*name.to_string() {
        "NSAppearanceNameDarkAqua" => Theme::Dark,
        _ => Theme::Light,
    }
}

fn set_ns_theme(theme: Option<Theme>) {
    let app = NSApp();
    let has_theme: bool = unsafe { msg_send![&app, respondsToSelector: sel!(effectiveAppearance)] };
    if has_theme {
        let appearance = theme.map(|t| {
            let name = match t {
                Theme::Dark => NSString::from_str("NSAppearanceNameDarkAqua"),
                Theme::Light => NSString::from_str("NSAppearanceNameAqua"),
            };
            NSAppearance::appearanceNamed(&name)
        });
        app.setAppearance(appearance.as_ref().map(|a| a.as_ref()));
    }
}
