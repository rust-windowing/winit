#![allow(clippy::unnecessary_cast)]

use std::collections::VecDeque;
use std::f64;
use std::ops;
use std::sync::{Mutex, MutexGuard};

use crate::{
    cursor::Cursor,
    dpi::{
        LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize, Position, Size, Size::Logical,
    },
    error::{ExternalError, NotSupportedError, OsError as RootOsError},
    event::WindowEvent,
    icon::Icon,
    platform::macos::{OptionAsAlt, WindowExtMacOS},
    platform_impl::platform::{
        app_state::AppState,
        event_loop::EventLoopWindowTarget,
        ffi,
        monitor::{self, MonitorHandle, VideoMode},
        view::WinitView,
        window_delegate::WinitWindowDelegate,
        Fullscreen, OsError,
    },
    window::{
        CursorGrabMode, ImePurpose, ResizeDirection, Theme, UserAttentionType, WindowAttributes,
        WindowButtons, WindowId as RootWindowId, WindowLevel,
    },
};
use core_graphics::display::{CGDisplay, CGPoint};
use icrate::AppKit::{
    NSAppKitVersionNumber, NSAppKitVersionNumber10_12, NSAppearance, NSApplication,
    NSApplicationPresentationAutoHideDock, NSApplicationPresentationAutoHideMenuBar,
    NSApplicationPresentationFullScreen, NSApplicationPresentationHideDock,
    NSApplicationPresentationHideMenuBar, NSApplicationPresentationOptions, NSBackingStoreBuffered,
    NSColor, NSCriticalRequest, NSFilenamesPboardType, NSInformationalRequest, NSResponder,
    NSScreen, NSView, NSWindow, NSWindowAbove, NSWindowCloseButton, NSWindowFullScreenButton,
    NSWindowLevel, NSWindowMiniaturizeButton, NSWindowSharingNone, NSWindowSharingReadOnly,
    NSWindowStyleMask, NSWindowStyleMaskBorderless, NSWindowStyleMaskClosable,
    NSWindowStyleMaskFullSizeContentView, NSWindowStyleMaskMiniaturizable,
    NSWindowStyleMaskResizable, NSWindowStyleMaskTitled, NSWindowTabbingModePreferred,
    NSWindowTitleHidden, NSWindowZoomButton,
};
use icrate::Foundation::{
    CGFloat, MainThreadBound, MainThreadMarker, NSArray, NSCopying, NSObject, NSPoint, NSRect,
    NSSize, NSString,
};
use log::trace;
use objc2::rc::{autoreleasepool, Id};
use objc2::{declare_class, msg_send, msg_send_id, mutability, sel, ClassType, DeclaredClass};

use super::cursor::cursor_from_icon;
use super::ffi::kCGFloatingWindowLevel;
use super::ffi::{kCGNormalWindowLevel, CGSMainConnectionID, CGSSetWindowBackgroundBlurRadius};
use super::monitor::{flip_window_screen_coordinates, get_display_id};

pub(crate) struct Window {
    window: MainThreadBound<Id<WinitWindow>>,
    // We keep this around so that it doesn't get dropped until the window does.
    _delegate: MainThreadBound<Id<WinitWindowDelegate>>,
}

impl Drop for Window {
    fn drop(&mut self) {
        self.window
            .get_on_main(|window| autoreleasepool(|_| window.close()))
    }
}

impl Window {
    pub(crate) fn new<T: 'static>(
        _window_target: &EventLoopWindowTarget<T>,
        attributes: WindowAttributes,
        pl_attribs: PlatformSpecificWindowBuilderAttributes,
    ) -> Result<Self, RootOsError> {
        let mtm = MainThreadMarker::new()
            .expect("windows can only be created on the main thread on macOS");
        let (window, _delegate) =
            autoreleasepool(|_| WinitWindow::new(attributes, pl_attribs, mtm))?;
        Ok(Window {
            window: MainThreadBound::new(window, mtm),
            _delegate: MainThreadBound::new(_delegate, mtm),
        })
    }

    pub(crate) fn maybe_queue_on_main(&self, f: impl FnOnce(&WinitWindow) + Send + 'static) {
        // For now, don't actually do queuing, since it may be less predictable
        self.maybe_wait_on_main(f)
    }

    pub(crate) fn maybe_wait_on_main<R: Send>(
        &self,
        f: impl FnOnce(&WinitWindow) -> R + Send,
    ) -> R {
        self.window.get_on_main(|window| f(window))
    }

    #[cfg(feature = "rwh_06")]
    #[inline]
    pub(crate) fn raw_window_handle_rwh_06(
        &self,
    ) -> Result<rwh_06::RawWindowHandle, rwh_06::HandleError> {
        if let Some(mtm) = MainThreadMarker::new() {
            Ok(self.window.get(mtm).raw_window_handle_rwh_06())
        } else {
            Err(rwh_06::HandleError::Unavailable)
        }
    }
}

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
    pub tabbing_identifier: Option<String>,
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
            tabbing_identifier: None,
            option_as_alt: Default::default(),
        }
    }
}

declare_class!(
    #[derive(Debug)]
    pub struct WinitWindow;

    unsafe impl ClassType for WinitWindow {
        #[inherits(NSResponder, NSObject)]
        type Super = NSWindow;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "WinitWindow";
    }

    impl DeclaredClass for WinitWindow {
        type Ivars = Mutex<SharedState>;
    }

    unsafe impl WinitWindow {
        #[method(canBecomeMainWindow)]
        fn can_become_main_window(&self) -> bool {
            trace_scope!("canBecomeMainWindow");
            true
        }

        #[method(canBecomeKeyWindow)]
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

    decorations: bool,
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
    fn new(
        attrs: WindowAttributes,
        pl_attrs: PlatformSpecificWindowBuilderAttributes,
        mtm: MainThreadMarker,
    ) -> Result<(Id<Self>, Id<WinitWindowDelegate>), RootOsError> {
        trace_scope!("WinitWindow::new");

        let this = autoreleasepool(|_| {
            let screen = match attrs.fullscreen.0.clone().map(Into::into) {
                Some(Fullscreen::Borderless(Some(monitor)))
                | Some(Fullscreen::Exclusive(VideoMode { monitor, .. })) => {
                    monitor.ns_screen(mtm).or_else(|| NSScreen::mainScreen(mtm))
                }
                Some(Fullscreen::Borderless(None)) => NSScreen::mainScreen(mtm),
                None => None,
            };
            let frame = match &screen {
                Some(screen) => screen.frame(),
                None => {
                    let scale_factor = NSScreen::mainScreen(mtm)
                        .map(|screen| screen.backingScaleFactor() as f64)
                        .unwrap_or(1.0);
                    let size = match attrs.inner_size {
                        Some(size) => {
                            let size = size.to_logical(scale_factor);
                            NSSize::new(size.width, size.height)
                        }
                        None => NSSize::new(800.0, 600.0),
                    };
                    let position = match attrs.position {
                        Some(position) => {
                            let position = position.to_logical(scale_factor);
                            flip_window_screen_coordinates(NSRect::new(
                                NSPoint::new(position.x, position.y),
                                size,
                            ))
                        }
                        // This value is ignored by calling win.center() below
                        None => NSPoint::new(0.0, 0.0),
                    };
                    NSRect::new(position, size)
                }
            };

            let mut masks = if (!attrs.decorations && screen.is_none()) || pl_attrs.titlebar_hidden
            {
                // Resizable without a titlebar or borders
                // if decorations is set to false, ignore pl_attrs
                //
                // if the titlebar is hidden, ignore other pl_attrs
                NSWindowStyleMaskBorderless
                    | NSWindowStyleMaskResizable
                    | NSWindowStyleMaskMiniaturizable
            } else {
                // default case, resizable window with titlebar and titlebar buttons
                NSWindowStyleMaskClosable
                    | NSWindowStyleMaskMiniaturizable
                    | NSWindowStyleMaskResizable
                    | NSWindowStyleMaskTitled
            };

            if !attrs.resizable {
                masks &= !NSWindowStyleMaskResizable;
            }

            if !attrs.enabled_buttons.contains(WindowButtons::MINIMIZE) {
                masks &= !NSWindowStyleMaskMiniaturizable;
            }

            if !attrs.enabled_buttons.contains(WindowButtons::CLOSE) {
                masks &= !NSWindowStyleMaskClosable;
            }

            if pl_attrs.fullsize_content_view {
                masks |= NSWindowStyleMaskFullSizeContentView;
            }

            let state = SharedState {
                resizable: attrs.resizable,
                maximized: attrs.maximized,
                decorations: attrs.decorations,
                ..Default::default()
            };

            let this = mtm.alloc().set_ivars(Mutex::new(state));
            let this: Option<Id<Self>> = unsafe {
                msg_send_id![
                    super(this),
                    initWithContentRect: frame,
                    styleMask: masks,
                    backing: NSBackingStoreBuffered,
                    defer: false,
                ]
            };
            let this = this?;

            // It is very important for correct memory management that we
            // disable the extra release that would otherwise happen when
            // calling `close` on the window.
            unsafe { this.setReleasedWhenClosed(false) };

            let resize_increments = match attrs
                .resize_increments
                .map(|i| i.to_logical::<f64>(this.scale_factor()))
            {
                Some(LogicalSize { width, height }) if width >= 1. && height >= 1. => {
                    NSSize::new(width, height)
                }
                _ => NSSize::new(1., 1.),
            };

            this.lock_shared_state("init").resize_increments = resize_increments;

            this.setTitle(&NSString::from_str(&attrs.title));
            this.setAcceptsMouseMovedEvents(true);

            if let Some(identifier) = pl_attrs.tabbing_identifier {
                this.setTabbingIdentifier(&NSString::from_str(&identifier));
                this.setTabbingMode(NSWindowTabbingModePreferred);
            }

            if attrs.content_protected {
                this.setSharingType(NSWindowSharingNone);
            }

            if pl_attrs.titlebar_transparent {
                this.setTitlebarAppearsTransparent(true);
            }
            if pl_attrs.title_hidden {
                this.setTitleVisibility(NSWindowTitleHidden);
            }
            if pl_attrs.titlebar_buttons_hidden {
                for titlebar_button in &[
                    #[allow(deprecated)]
                    NSWindowFullScreenButton,
                    NSWindowMiniaturizeButton,
                    NSWindowCloseButton,
                    NSWindowZoomButton,
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
                if let Some(button) = this.standardWindowButton(NSWindowZoomButton) {
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

            Some(this)
        })
        .ok_or_else(|| os_error!(OsError::CreationError("Couldn't create `NSWindow`")))?;

        #[cfg(feature = "rwh_06")]
        match attrs.parent_window.0 {
            Some(rwh_06::RawWindowHandle::AppKit(handle)) => {
                // SAFETY: Caller ensures the pointer is valid or NULL
                // Unwrap is fine, since the pointer comes from `NonNull`.
                let parent_view: Id<NSView> =
                    unsafe { Id::retain(handle.ns_view.as_ptr().cast()) }.unwrap();
                let parent = parent_view.window().ok_or_else(|| {
                    os_error!(OsError::CreationError(
                        "parent view should be installed in a window"
                    ))
                })?;

                // SAFETY: We know that there are no parent -> child -> parent cycles since the only place in `winit`
                // where we allow making a window a child window is right here, just after it's been created.
                unsafe { parent.addChildWindow_ordered(&this, NSWindowAbove) };
            }
            Some(raw) => panic!("Invalid raw window handle {raw:?} on macOS"),
            None => (),
        }

        let view = WinitView::new(&this, pl_attrs.accepts_first_mouse);

        // The default value of `setWantsBestResolutionOpenGLSurface:` was `false` until
        // macos 10.14 and `true` after 10.15, we should set it to `YES` or `NO` to avoid
        // always the default system value in favour of the user's code
        #[allow(deprecated)]
        view.setWantsBestResolutionOpenGLSurface(!pl_attrs.disallow_hidpi);

        // On Mojave, views automatically become layer-backed shortly after being added to
        // a window. Changing the layer-backedness of a view breaks the association between
        // the view and its associated OpenGL context. To work around this, on Mojave we
        // explicitly make the view layer-backed up front so that AppKit doesn't do it
        // itself and break the association with its context.
        if unsafe { NSAppKitVersionNumber }.floor() > NSAppKitVersionNumber10_12 {
            view.setWantsLayer(true);
        }

        // Configure the new view as the "key view" for the window
        this.setContentView(Some(&view));
        this.setInitialFirstResponder(Some(&view));

        if attrs.transparent {
            this.setOpaque(false);
            this.setBackgroundColor(Some(unsafe { &NSColor::clearColor() }));
        }

        if attrs.blur {
            this.set_blur(attrs.blur);
        }

        if let Some(dim) = attrs.min_inner_size {
            this.set_min_inner_size(Some(dim));
        }
        if let Some(dim) = attrs.max_inner_size {
            this.set_max_inner_size(Some(dim));
        }

        this.set_window_level(attrs.window_level);

        // register for drag and drop operations.
        this.registerForDraggedTypes(&NSArray::from_id_slice(&[
            unsafe { NSFilenamesPboardType }.copy()
        ]));

        match attrs.preferred_theme {
            Some(theme) => {
                set_ns_theme(Some(theme), mtm);
                let mut state = this.lock_shared_state("WinitWindow::new");
                state.current_theme = Some(theme);
            }
            None => {
                let mut state = this.lock_shared_state("WinitWindow::new");
                state.current_theme = Some(get_ns_theme(mtm));
            }
        }

        let delegate = WinitWindowDelegate::new(&this, attrs.fullscreen.0.is_some());

        // XXX Send `Focused(false)` right after creating the window delegate, so we won't
        // obscure the real focused events on the startup.
        delegate.queue_event(WindowEvent::Focused(false));

        // Set fullscreen mode after we setup everything
        this.set_fullscreen(attrs.fullscreen.0.map(Into::into));

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

    #[track_caller]
    pub(super) fn view(&self) -> Id<WinitView> {
        // SAFETY: The view inside WinitWindow is always `WinitView`
        unsafe { Id::cast(self.contentView().unwrap()) }
    }

    #[track_caller]
    pub(crate) fn lock_shared_state(
        &self,
        called_from_fn: &'static str,
    ) -> SharedStateMutexGuard<'_> {
        SharedStateMutexGuard::new(self.ivars().lock().unwrap(), called_from_fn)
    }

    fn set_style_mask(&self, mask: NSWindowStyleMask) {
        self.setStyleMask(mask);
        // If we don't do this, key handling will break
        // (at least until the window is clicked again/etc.)
        let _ = self.makeFirstResponder(Some(&self.contentView().unwrap()));
    }
}

impl WinitWindow {
    pub fn id(&self) -> WindowId {
        WindowId(self as *const Self as usize)
    }

    pub fn set_title(&self, title: &str) {
        self.setTitle(&NSString::from_str(title))
    }

    pub fn set_transparent(&self, transparent: bool) {
        self.setOpaque(!transparent)
    }

    pub fn set_blur(&self, blur: bool) {
        // NOTE: in general we want to specify the blur radius, but the choice of 80
        // should be a reasonable default.
        let radius = if blur { 80 } else { 0 };
        let window_number = unsafe { self.windowNumber() };
        unsafe {
            CGSSetWindowBackgroundBlurRadius(CGSMainConnectionID(), window_number, radius);
        }
    }

    pub fn set_visible(&self, visible: bool) {
        match visible {
            true => self.makeKeyAndOrderFront(None),
            false => self.orderOut(None),
        }
    }

    #[inline]
    pub fn is_visible(&self) -> Option<bool> {
        Some(self.isVisible())
    }

    pub fn request_redraw(&self) {
        AppState::queue_redraw(RootWindowId(self.id()));
    }

    #[inline]
    pub fn pre_present_notify(&self) {}

    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        let position = flip_window_screen_coordinates(self.frame());
        Ok(LogicalPosition::new(position.x, position.y).to_physical(self.scale_factor()))
    }

    pub fn inner_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        let content_rect = self.contentRectForFrameRect(self.frame());
        let position = flip_window_screen_coordinates(content_rect);
        Ok(LogicalPosition::new(position.x, position.y).to_physical(self.scale_factor()))
    }

    pub fn set_outer_position(&self, position: Position) {
        let position = position.to_logical(self.scale_factor());
        let point = flip_window_screen_coordinates(NSRect::new(
            NSPoint::new(position.x, position.y),
            self.frame().size,
        ));
        unsafe { self.setFrameOrigin(point) };
    }

    #[inline]
    pub fn inner_size(&self) -> PhysicalSize<u32> {
        let content_rect = self.contentRectForFrameRect(self.frame());
        let logical = LogicalSize::new(content_rect.size.width, content_rect.size.height);
        logical.to_physical(self.scale_factor())
    }

    #[inline]
    pub fn outer_size(&self) -> PhysicalSize<u32> {
        let frame = self.frame();
        let logical = LogicalSize::new(frame.size.width, frame.size.height);
        logical.to_physical(self.scale_factor())
    }

    #[inline]
    pub fn request_inner_size(&self, size: Size) -> Option<PhysicalSize<u32>> {
        let scale_factor = self.scale_factor();
        let size = size.to_logical(scale_factor);
        self.setContentSize(NSSize::new(size.width, size.height));
        None
    }

    pub fn set_min_inner_size(&self, dimensions: Option<Size>) {
        let dimensions = dimensions.unwrap_or(Logical(LogicalSize {
            width: 0.0,
            height: 0.0,
        }));
        let min_size = dimensions.to_logical::<CGFloat>(self.scale_factor());

        let min_size = NSSize::new(min_size.width, min_size.height);
        unsafe { self.setContentMinSize(min_size) };

        // If necessary, resize the window to match constraint
        let mut current_size = self.contentRectForFrameRect(self.frame()).size;
        if current_size.width < min_size.width {
            current_size.width = min_size.width;
        }
        if current_size.height < min_size.height {
            current_size.height = min_size.height;
        }
        self.setContentSize(current_size);
    }

    pub fn set_max_inner_size(&self, dimensions: Option<Size>) {
        let dimensions = dimensions.unwrap_or(Logical(LogicalSize {
            width: std::f32::MAX as f64,
            height: std::f32::MAX as f64,
        }));
        let scale_factor = self.scale_factor();
        let max_size = dimensions.to_logical::<CGFloat>(scale_factor);

        let max_size = NSSize::new(max_size.width, max_size.height);
        unsafe { self.setContentMaxSize(max_size) };

        // If necessary, resize the window to match constraint
        let mut current_size = self.contentRectForFrameRect(self.frame()).size;
        if max_size.width < current_size.width {
            current_size.width = max_size.width;
        }
        if max_size.height < current_size.height {
            current_size.height = max_size.height;
        }
        self.setContentSize(current_size);
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
                mask |= NSWindowStyleMaskResizable;
            } else {
                mask &= !NSWindowStyleMaskResizable;
            }
            self.set_style_mask(mask);
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
            mask |= NSWindowStyleMaskClosable;
        } else {
            mask &= !NSWindowStyleMaskClosable;
        }

        if buttons.contains(WindowButtons::MINIMIZE) {
            mask |= NSWindowStyleMaskMiniaturizable;
        } else {
            mask &= !NSWindowStyleMaskMiniaturizable;
        }

        // This must happen before the button's "enabled" status has been set,
        // hence we do it synchronously.
        self.set_style_mask(mask);

        // We edit the button directly instead of using `NSResizableWindowMask`,
        // since that mask also affect the resizability of the window (which is
        // controllable by other means in `winit`).
        if let Some(button) = self.standardWindowButton(NSWindowZoomButton) {
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
            .standardWindowButton(NSWindowZoomButton)
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

    pub fn set_cursor(&self, cursor: Cursor) {
        match cursor {
            Cursor::Icon(icon) => {
                let view = self.view();
                view.set_cursor_icon(cursor_from_icon(icon));
                self.invalidateCursorRectsForView(&view);
            }
            Cursor::Custom(cursor) => {
                let view = self.view();
                view.set_cursor_icon(cursor.inner.0.clone());
                self.invalidateCursorRectsForView(&view);
            }
        }
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
        let state_changed = view.set_cursor_visible(visible);
        if state_changed {
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
        let mtm = MainThreadMarker::from(self);
        let event = NSApplication::sharedApplication(mtm)
            .currentEvent()
            .unwrap();
        self.performWindowDragWithEvent(&event);
        Ok(())
    }

    #[inline]
    pub fn drag_resize_window(&self, _direction: ResizeDirection) -> Result<(), ExternalError> {
        Err(ExternalError::NotSupported(NotSupportedError::new()))
    }

    #[inline]
    pub fn show_window_menu(&self, _position: Position) {}

    #[inline]
    pub fn set_cursor_hittest(&self, hittest: bool) -> Result<(), ExternalError> {
        self.setIgnoresMouseEvents(!hittest);
        Ok(())
    }

    pub(crate) fn is_zoomed(&self) -> bool {
        // because `isZoomed` doesn't work if the window's borderless,
        // we make it resizable temporalily.
        let curr_mask = self.styleMask();

        let required = NSWindowStyleMaskTitled | NSWindowStyleMaskResizable;
        let needs_temp_mask = !mask_contains(curr_mask, required);
        if needs_temp_mask {
            self.set_style_mask(required);
        }

        let is_zoomed = self.isZoomed();

        // Roll back temp styles
        if needs_temp_mask {
            self.set_style_mask(curr_mask);
        }

        is_zoomed
    }

    fn saved_style(&self, shared_state: &mut SharedState) -> NSWindowStyleMask {
        let base_mask = shared_state
            .saved_style
            .take()
            .unwrap_or_else(|| self.styleMask());
        if shared_state.resizable {
            base_mask | NSWindowStyleMaskResizable
        } else {
            base_mask & !NSWindowStyleMaskResizable
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

        self.set_style_mask(mask);
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
            unsafe { self.deminiaturize(Some(self)) };
        }
    }

    #[inline]
    pub fn is_minimized(&self) -> Option<bool> {
        Some(self.isMiniaturized())
    }

    #[inline]
    pub fn set_maximized(&self, maximized: bool) {
        let mtm = MainThreadMarker::from(self);
        let is_zoomed = self.is_zoomed();
        if is_zoomed == maximized {
            return;
        };

        let mut shared_state = self.lock_shared_state("set_maximized");
        // Save the standard frame sized if it is not zoomed
        if !is_zoomed {
            shared_state.standard_frame = Some(self.frame());
        }

        shared_state.maximized = maximized;

        if shared_state.fullscreen.is_some() {
            // Handle it in window_did_exit_fullscreen
            return;
        }

        if mask_contains(self.styleMask(), NSWindowStyleMaskResizable) {
            drop(shared_state);
            // Just use the native zoom if resizable
            self.zoom(None);
        } else {
            // if it's not resizable, we set the frame directly
            let new_rect = if maximized {
                let screen = NSScreen::mainScreen(mtm).expect("no screen found");
                screen.visibleFrame()
            } else {
                shared_state.saved_standard_frame()
            };
            drop(shared_state);
            self.setFrame_display(new_rect, false);
        }
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
        let mtm = MainThreadMarker::from(self);
        let app = NSApplication::sharedApplication(mtm);

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
            .ns_screen(mtm)
            .unwrap();

            let old_screen = self.screen().unwrap();
            if old_screen != new_screen {
                unsafe { self.setFrameOrigin(new_screen.frame().origin) };
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

        fn toggle_fullscreen(window: &WinitWindow) {
            // Window level must be restored from `CGShieldingWindowLevel()
            // + 1` back to normal in order for `toggleFullScreen` to do
            // anything
            window.setLevel(kCGNormalWindowLevel as NSWindowLevel);
            window.toggleFullScreen(None);
        }

        match (old_fullscreen, fullscreen) {
            (None, Some(_)) => {
                // `toggleFullScreen` doesn't work if the `StyleMask` is none, so we
                // set a normal style temporarily. The previous state will be
                // restored in `WindowDelegate::window_did_exit_fullscreen`.
                let curr_mask = self.styleMask();
                let required = NSWindowStyleMaskTitled | NSWindowStyleMaskResizable;
                if !mask_contains(curr_mask, required) {
                    self.set_style_mask(required);
                    self.lock_shared_state("set_fullscreen").saved_style = Some(curr_mask);
                }
                toggle_fullscreen(self);
            }
            (Some(Fullscreen::Borderless(_)), None) => {
                // State is restored by `window_did_exit_fullscreen`
                toggle_fullscreen(self);
            }
            (Some(Fullscreen::Exclusive(ref video_mode)), None) => {
                unsafe {
                    ffi::CGRestorePermanentDisplayConfiguration();
                    assert_eq!(
                        ffi::CGDisplayRelease(video_mode.monitor().native_identifier()),
                        ffi::kCGErrorSuccess
                    );
                };
                toggle_fullscreen(self);
            }
            (Some(Fullscreen::Borderless(_)), Some(Fullscreen::Exclusive(_))) => {
                // If we're already in fullscreen mode, calling
                // `CGDisplayCapture` will place the shielding window on top of
                // our window, which results in a black display and is not what
                // we want. So, we must place our window on top of the shielding
                // window. Unfortunately, this also makes our window be on top
                // of the menu bar, and this looks broken, so we must make sure
                // that the menu bar is disabled. This is done in the window
                // delegate in `window:willUseFullScreenPresentationOptions:`.
                self.lock_shared_state("set_fullscreen")
                    .save_presentation_opts = Some(app.presentationOptions());

                let presentation_options = NSApplicationPresentationFullScreen
                    | NSApplicationPresentationHideDock
                    | NSApplicationPresentationHideMenuBar;
                app.setPresentationOptions(presentation_options);

                let window_level = unsafe { ffi::CGShieldingWindowLevel() } as NSWindowLevel + 1;
                self.setLevel(window_level);
            }
            (Some(Fullscreen::Exclusive(ref video_mode)), Some(Fullscreen::Borderless(_))) => {
                let presentation_options = self
                    .lock_shared_state("set_fullscreen")
                    .save_presentation_opts
                    .unwrap_or(
                        NSApplicationPresentationFullScreen
                            | NSApplicationPresentationAutoHideDock
                            | NSApplicationPresentationAutoHideMenuBar,
                    );
                app.setPresentationOptions(presentation_options);

                unsafe {
                    ffi::CGRestorePermanentDisplayConfiguration();
                    assert_eq!(
                        ffi::CGDisplayRelease(video_mode.monitor().native_identifier()),
                        ffi::kCGErrorSuccess
                    );
                };

                // Restore the normal window level following the Borderless fullscreen
                // `CGShieldingWindowLevel() + 1` hack.
                self.setLevel(kCGNormalWindowLevel as NSWindowLevel);
            }
            _ => {}
        };
    }

    #[inline]
    pub fn set_decorations(&self, decorations: bool) {
        let mut shared_state_lock = self.lock_shared_state("set_decorations");
        if decorations == shared_state_lock.decorations {
            return;
        }

        shared_state_lock.decorations = decorations;

        let fullscreen = shared_state_lock.fullscreen.is_some();
        let resizable = shared_state_lock.resizable;

        drop(shared_state_lock);

        // If we're in fullscreen mode, we wait to apply decoration changes
        // until we're in `window_did_exit_fullscreen`.
        if fullscreen {
            return;
        }

        let new_mask = {
            let mut new_mask = if decorations {
                NSWindowStyleMaskClosable
                    | NSWindowStyleMaskMiniaturizable
                    | NSWindowStyleMaskResizable
                    | NSWindowStyleMaskTitled
            } else {
                NSWindowStyleMaskBorderless | NSWindowStyleMaskResizable
            };
            if !resizable {
                new_mask &= !NSWindowStyleMaskResizable;
            }
            new_mask
        };
        self.set_style_mask(new_mask);
    }

    #[inline]
    pub fn is_decorated(&self) -> bool {
        self.lock_shared_state("is_decorated").decorations
    }

    #[inline]
    pub fn set_window_level(&self, level: WindowLevel) {
        let level = match level {
            WindowLevel::AlwaysOnTop => kCGFloatingWindowLevel as NSWindowLevel,
            WindowLevel::AlwaysOnBottom => (kCGNormalWindowLevel - 1) as NSWindowLevel,
            WindowLevel::Normal => kCGNormalWindowLevel as NSWindowLevel,
        };
        self.setLevel(level);
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
    pub fn set_ime_cursor_area(&self, spot: Position, size: Size) {
        let scale_factor = self.scale_factor();
        let logical_spot = spot.to_logical(scale_factor);
        let logical_spot = NSPoint::new(logical_spot.x, logical_spot.y);

        let size = size.to_logical(scale_factor);
        let size = NSSize::new(size.width, size.height);

        self.view().set_ime_cursor_area(logical_spot, size);
    }

    #[inline]
    pub fn set_ime_allowed(&self, allowed: bool) {
        self.view().set_ime_allowed(allowed);
    }

    #[inline]
    pub fn set_ime_purpose(&self, _purpose: ImePurpose) {}

    #[inline]
    pub fn focus_window(&self) {
        let mtm = MainThreadMarker::from(self);
        let is_minimized = self.isMiniaturized();
        let is_visible = self.isVisible();

        if !is_minimized && is_visible {
            #[allow(deprecated)]
            NSApplication::sharedApplication(mtm).activateIgnoringOtherApps(true);
            self.makeKeyAndOrderFront(None);
        }
    }

    #[inline]
    pub fn request_user_attention(&self, request_type: Option<UserAttentionType>) {
        let mtm = MainThreadMarker::from(self);
        let ns_request_type = request_type.map(|ty| match ty {
            UserAttentionType::Critical => NSCriticalRequest,
            UserAttentionType::Informational => NSInformationalRequest,
        });
        if let Some(ty) = ns_request_type {
            NSApplication::sharedApplication(mtm).requestUserAttention(ty);
        }
    }

    #[inline]
    // Allow directly accessing the current monitor internally without unwrapping.
    pub(crate) fn current_monitor_inner(&self) -> Option<MonitorHandle> {
        let display_id = get_display_id(&*self.screen()?);
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

    #[cfg(feature = "rwh_04")]
    #[inline]
    pub fn raw_window_handle_rwh_04(&self) -> rwh_04::RawWindowHandle {
        let mut window_handle = rwh_04::AppKitHandle::empty();
        window_handle.ns_window = self as *const Self as *mut _;
        window_handle.ns_view = Id::as_ptr(&self.contentView().unwrap()) as *mut _;
        rwh_04::RawWindowHandle::AppKit(window_handle)
    }

    #[cfg(feature = "rwh_05")]
    #[inline]
    pub fn raw_window_handle_rwh_05(&self) -> rwh_05::RawWindowHandle {
        let mut window_handle = rwh_05::AppKitWindowHandle::empty();
        window_handle.ns_window = self as *const Self as *mut _;
        window_handle.ns_view = Id::as_ptr(&self.contentView().unwrap()) as *mut _;
        rwh_05::RawWindowHandle::AppKit(window_handle)
    }

    #[cfg(feature = "rwh_05")]
    #[inline]
    pub fn raw_display_handle_rwh_05(&self) -> rwh_05::RawDisplayHandle {
        rwh_05::RawDisplayHandle::AppKit(rwh_05::AppKitDisplayHandle::empty())
    }

    #[cfg(feature = "rwh_06")]
    #[inline]
    pub fn raw_window_handle_rwh_06(&self) -> rwh_06::RawWindowHandle {
        let window_handle = rwh_06::AppKitWindowHandle::new({
            let ptr = Id::as_ptr(&self.contentView().unwrap()) as *mut _;
            std::ptr::NonNull::new(ptr).expect("Id<T> should never be null")
        });
        rwh_06::RawWindowHandle::AppKit(window_handle)
    }

    #[cfg(feature = "rwh_06")]
    #[inline]
    pub fn raw_display_handle_rwh_06(
        &self,
    ) -> Result<rwh_06::RawDisplayHandle, rwh_06::HandleError> {
        Ok(rwh_06::RawDisplayHandle::AppKit(
            rwh_06::AppKitDisplayHandle::new(),
        ))
    }

    fn toggle_style_mask(&self, mask: NSWindowStyleMask, on: bool) {
        let current_style_mask = self.styleMask();
        if on {
            self.set_style_mask(current_style_mask | mask);
        } else {
            self.set_style_mask(current_style_mask & (!mask));
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
        let mtm = MainThreadMarker::from(self);
        set_ns_theme(theme, mtm);
        self.lock_shared_state("set_theme").current_theme =
            theme.or_else(|| Some(get_ns_theme(mtm)));
    }

    #[inline]
    pub fn set_content_protected(&self, protected: bool) {
        self.setSharingType(if protected {
            NSWindowSharingNone
        } else {
            NSWindowSharingReadOnly
        })
    }

    pub fn title(&self) -> String {
        let window: &NSWindow = self;
        window.title().to_string()
    }

    pub fn reset_dead_keys(&self) {
        // (Artur) I couldn't find a way to implement this.
    }
}

impl WindowExtMacOS for WinitWindow {
    #[inline]
    fn simple_fullscreen(&self) -> bool {
        self.lock_shared_state("simple_fullscreen")
            .is_simple_fullscreen
    }

    #[inline]
    fn set_simple_fullscreen(&self, fullscreen: bool) -> bool {
        let mtm = MainThreadMarker::from(self);
        let mut shared_state_lock = self.lock_shared_state("set_simple_fullscreen");

        let app = NSApplication::sharedApplication(mtm);
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
                NSApplicationPresentationAutoHideDock | NSApplicationPresentationAutoHideMenuBar;
            app.setPresentationOptions(presentation_options);

            // Hide the titlebar
            self.toggle_style_mask(NSWindowStyleMaskTitled, false);

            // Set the window frame to the screen frame size
            let screen = self.screen().expect("expected screen to be available");
            self.setFrame_display(screen.frame(), true);

            // Fullscreen windows can't be resized, minimized, or moved
            self.toggle_style_mask(NSWindowStyleMaskMiniaturizable, false);
            self.toggle_style_mask(NSWindowStyleMaskResizable, false);
            self.setMovable(false);

            true
        } else {
            let new_mask = self.saved_style(&mut shared_state_lock);
            self.set_style_mask(new_mask);
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

    #[inline]
    fn set_tabbing_identifier(&self, identifier: &str) {
        self.setTabbingIdentifier(&NSString::from_str(identifier))
    }

    #[inline]
    fn tabbing_identifier(&self) -> String {
        self.tabbingIdentifier().to_string()
    }

    #[inline]
    fn select_next_tab(&self) {
        self.selectNextTab(None)
    }

    #[inline]
    fn select_previous_tab(&self) {
        unsafe { self.selectPreviousTab(None) }
    }

    #[inline]
    fn select_tab_at_index(&self, index: usize) {
        if let Some(group) = self.tabGroup() {
            if let Some(windows) = unsafe { self.tabbedWindows() } {
                if index < windows.len() {
                    group.setSelectedWindow(Some(&windows[index]));
                }
            }
        }
    }

    #[inline]
    fn num_tabs(&self) -> usize {
        unsafe { self.tabbedWindows() }
            .map(|windows| windows.len())
            .unwrap_or(1)
    }

    fn is_document_edited(&self) -> bool {
        self.isDocumentEdited()
    }

    fn set_document_edited(&self, edited: bool) {
        self.setDocumentEdited(edited)
    }

    fn set_option_as_alt(&self, option_as_alt: OptionAsAlt) {
        let mut shared_state_lock = self.lock_shared_state("set_option_as_alt");
        shared_state_lock.option_as_alt = option_as_alt;
    }

    fn option_as_alt(&self) -> OptionAsAlt {
        let shared_state_lock = self.lock_shared_state("option_as_alt");
        shared_state_lock.option_as_alt
    }
}

fn mask_contains(mask: NSWindowStyleMask, value: NSWindowStyleMask) -> bool {
    mask & value == value
}

pub(super) fn get_ns_theme(mtm: MainThreadMarker) -> Theme {
    let app = NSApplication::sharedApplication(mtm);
    let has_theme: bool = unsafe { msg_send![&app, respondsToSelector: sel!(effectiveAppearance)] };
    if !has_theme {
        return Theme::Light;
    }
    let appearance = app.effectiveAppearance();
    let name = appearance
        .bestMatchFromAppearancesWithNames(&NSArray::from_id_slice(&[
            NSString::from_str("NSAppearanceNameAqua"),
            NSString::from_str("NSAppearanceNameDarkAqua"),
        ]))
        .unwrap();
    match &*name.to_string() {
        "NSAppearanceNameDarkAqua" => Theme::Dark,
        _ => Theme::Light,
    }
}

fn set_ns_theme(theme: Option<Theme>, mtm: MainThreadMarker) {
    let app = NSApplication::sharedApplication(mtm);
    let has_theme: bool = unsafe { msg_send![&app, respondsToSelector: sel!(effectiveAppearance)] };
    if has_theme {
        let appearance = theme.map(|t| {
            let name = match t {
                Theme::Dark => NSString::from_str("NSAppearanceNameDarkAqua"),
                Theme::Light => NSString::from_str("NSAppearanceNameAqua"),
            };
            NSAppearance::appearanceNamed(&name).unwrap()
        });
        app.setAppearance(appearance.as_ref().map(|a| a.as_ref()));
    }
}
