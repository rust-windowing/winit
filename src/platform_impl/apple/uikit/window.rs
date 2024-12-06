#![allow(clippy::unnecessary_cast)]

use std::collections::VecDeque;

use objc2::rc::Retained;
use objc2::{class, declare_class, msg_send, msg_send_id, mutability, ClassType, DeclaredClass};
use objc2_foundation::{
    CGFloat, CGPoint, CGRect, CGSize, MainThreadBound, MainThreadMarker, NSObject, NSObjectProtocol,
};
use objc2_ui_kit::{
    UIApplication, UICoordinateSpace, UIEdgeInsets, UIResponder, UIScreen,
    UIScreenOverscanCompensation, UIViewController, UIWindow,
};
use tracing::{debug, warn};

use super::app_state::EventWrapper;
use super::view::WinitView;
use super::view_controller::WinitViewController;
use super::{app_state, monitor, ActiveEventLoop, Fullscreen, MonitorHandle};
use crate::cursor::Cursor;
use crate::dpi::{
    LogicalInsets, LogicalPosition, LogicalSize, PhysicalInsets, PhysicalPosition, PhysicalSize,
    Position, Size,
};
use crate::error::{NotSupportedError, RequestError};
use crate::event::WindowEvent;
use crate::icon::Icon;
use crate::monitor::MonitorHandle as CoreMonitorHandle;
use crate::platform::ios::{ScreenEdge, StatusBarStyle, ValidOrientations};
use crate::window::{
    CursorGrabMode, ImePurpose, ResizeDirection, Theme, UserAttentionType, Window as CoreWindow,
    WindowAttributes, WindowButtons, WindowId, WindowLevel,
};

declare_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct WinitUIWindow;

    unsafe impl ClassType for WinitUIWindow {
        #[inherits(UIResponder, NSObject)]
        type Super = UIWindow;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "WinitUIWindow";
    }

    impl DeclaredClass for WinitUIWindow {}

    unsafe impl WinitUIWindow {
        #[method(becomeKeyWindow)]
        fn become_key_window(&self) {
            let mtm = MainThreadMarker::new().unwrap();
            app_state::handle_nonuser_event(
                mtm,
                EventWrapper::Window {
                    window_id: self.id(),
                    event: WindowEvent::Focused(true),
                },
            );
            let _: () = unsafe { msg_send![super(self), becomeKeyWindow] };
        }

        #[method(resignKeyWindow)]
        fn resign_key_window(&self) {
            let mtm = MainThreadMarker::new().unwrap();
            app_state::handle_nonuser_event(
                mtm,
                EventWrapper::Window {
                    window_id: self.id(),
                    event: WindowEvent::Focused(false),
                },
            );
            let _: () = unsafe { msg_send![super(self), resignKeyWindow] };
        }
    }
);

impl WinitUIWindow {
    pub(crate) fn new(
        mtm: MainThreadMarker,
        window_attributes: &WindowAttributes,
        frame: CGRect,
        view_controller: &UIViewController,
    ) -> Retained<Self> {
        // NOTE: This should only be created after the application has started launching,
        // (`application:willFinishLaunchingWithOptions:` at the earliest), otherwise you'll run
        // into very confusing issues with the window not being properly activated.
        //
        // Winit ensures this by not allowing access to `ActiveEventLoop` before handling events.
        let this: Retained<Self> = unsafe { msg_send_id![mtm.alloc(), initWithFrame: frame] };

        this.setRootViewController(Some(view_controller));

        match window_attributes.fullscreen.clone().map(Into::into) {
            Some(Fullscreen::Exclusive(ref video_mode)) => {
                let monitor = video_mode.monitor();
                let screen = monitor.ui_screen(mtm);
                screen.setCurrentMode(Some(video_mode.screen_mode(mtm)));
                this.setScreen(screen);
            },
            Some(Fullscreen::Borderless(Some(ref monitor))) => {
                let screen = monitor.ui_screen(mtm);
                this.setScreen(screen);
            },
            _ => (),
        }

        this
    }

    pub(crate) fn id(&self) -> WindowId {
        WindowId::from_raw(self as *const Self as usize)
    }
}

pub struct Inner {
    window: Retained<WinitUIWindow>,
    view_controller: Retained<WinitViewController>,
    view: Retained<WinitView>,
    gl_or_metal_backed: bool,
}

impl Inner {
    pub fn set_title(&self, _title: &str) {
        debug!("`Window::set_title` is ignored on iOS")
    }

    pub fn set_transparent(&self, _transparent: bool) {
        debug!("`Window::set_transparent` is ignored on iOS")
    }

    pub fn set_blur(&self, _blur: bool) {
        debug!("`Window::set_blur` is ignored on iOS")
    }

    pub fn set_visible(&self, visible: bool) {
        self.window.setHidden(!visible)
    }

    pub fn is_visible(&self) -> Option<bool> {
        warn!("`Window::is_visible` is ignored on iOS");
        None
    }

    pub fn request_redraw(&self) {
        if self.gl_or_metal_backed {
            let mtm = MainThreadMarker::new().unwrap();
            // `setNeedsDisplay` does nothing on UIViews which are directly backed by CAEAGLLayer or
            // CAMetalLayer. Ordinarily the OS sets up a bunch of UIKit state before
            // calling drawRect: on a UIView, but when using raw or gl/metal for drawing
            // this work is completely avoided.
            //
            // The docs for `setNeedsDisplay` don't mention `CAMetalLayer`; however, this has been
            // confirmed via testing.
            //
            // https://developer.apple.com/documentation/uikit/uiview/1622437-setneedsdisplay?language=objc
            app_state::queue_gl_or_metal_redraw(mtm, self.window.clone());
        } else {
            self.view.setNeedsDisplay();
        }
    }

    pub fn pre_present_notify(&self) {}

    pub fn surface_position(&self) -> PhysicalPosition<i32> {
        let view_position = self.view.frame().origin;
        let position =
            unsafe { self.window.convertPoint_fromView(view_position, Some(&self.view)) };
        let position = LogicalPosition::new(position.x, position.y);
        position.to_physical(self.scale_factor())
    }

    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, RequestError> {
        let screen_frame = self.screen_frame();
        let position =
            LogicalPosition { x: screen_frame.origin.x as f64, y: screen_frame.origin.y as f64 };
        Ok(position.to_physical(self.scale_factor()))
    }

    pub fn set_outer_position(&self, physical_position: Position) {
        let scale_factor = self.scale_factor();
        let position = physical_position.to_logical::<f64>(scale_factor);
        let screen_frame = self.screen_frame();
        let new_screen_frame = CGRect {
            origin: CGPoint { x: position.x as _, y: position.y as _ },
            size: screen_frame.size,
        };
        let bounds = self.rect_from_screen_space(new_screen_frame);
        self.window.setBounds(bounds);
    }

    pub fn surface_size(&self) -> PhysicalSize<u32> {
        let frame = self.view.frame();
        let size = LogicalSize::new(frame.size.width, frame.size.height);
        size.to_physical(self.scale_factor())
    }

    pub fn outer_size(&self) -> PhysicalSize<u32> {
        let frame = self.window.frame();
        let size = LogicalSize::new(frame.size.width, frame.size.height);
        size.to_physical(self.scale_factor())
    }

    pub fn request_surface_size(&self, _size: Size) -> Option<PhysicalSize<u32>> {
        Some(self.surface_size())
    }

    pub fn safe_area(&self) -> PhysicalInsets<u32> {
        // Only available on iOS 11.0
        let insets = if app_state::os_capabilities().safe_area {
            self.view.safeAreaInsets()
        } else {
            // Assume the status bar frame is the only thing that obscures the view
            let app = UIApplication::sharedApplication(MainThreadMarker::new().unwrap());
            #[allow(deprecated)]
            let status_bar_frame = app.statusBarFrame();
            UIEdgeInsets { top: status_bar_frame.size.height, left: 0.0, bottom: 0.0, right: 0.0 }
        };
        let insets = LogicalInsets::new(insets.top, insets.left, insets.bottom, insets.right);
        insets.to_physical(self.scale_factor())
    }

    pub fn set_min_surface_size(&self, _dimensions: Option<Size>) {
        warn!("`Window::set_min_surface_size` is ignored on iOS")
    }

    pub fn set_max_surface_size(&self, _dimensions: Option<Size>) {
        warn!("`Window::set_max_surface_size` is ignored on iOS")
    }

    pub fn surface_resize_increments(&self) -> Option<PhysicalSize<u32>> {
        None
    }

    #[inline]
    pub fn set_surface_resize_increments(&self, _increments: Option<Size>) {
        warn!("`Window::set_surface_resize_increments` is ignored on iOS")
    }

    pub fn set_resizable(&self, _resizable: bool) {
        warn!("`Window::set_resizable` is ignored on iOS")
    }

    pub fn is_resizable(&self) -> bool {
        warn!("`Window::is_resizable` is ignored on iOS");
        false
    }

    #[inline]
    pub fn set_enabled_buttons(&self, _buttons: WindowButtons) {
        warn!("`Window::set_enabled_buttons` is ignored on iOS");
    }

    #[inline]
    pub fn enabled_buttons(&self) -> WindowButtons {
        warn!("`Window::enabled_buttons` is ignored on iOS");
        WindowButtons::all()
    }

    pub fn scale_factor(&self) -> f64 {
        self.view.contentScaleFactor() as _
    }

    pub fn set_cursor(&self, _cursor: Cursor) {
        debug!("`Window::set_cursor` ignored on iOS")
    }

    pub fn set_cursor_position(&self, _position: Position) -> Result<(), NotSupportedError> {
        Err(NotSupportedError::new("set_cursor_position is not supported"))
    }

    pub fn set_cursor_grab(&self, _: CursorGrabMode) -> Result<(), NotSupportedError> {
        Err(NotSupportedError::new("set_cursor_grab is not supported"))
    }

    pub fn set_cursor_visible(&self, _visible: bool) {
        debug!("`Window::set_cursor_visible` is ignored on iOS")
    }

    pub fn drag_window(&self) -> Result<(), NotSupportedError> {
        Err(NotSupportedError::new("drag_window is not supported"))
    }

    pub fn drag_resize_window(&self, _direction: ResizeDirection) -> Result<(), NotSupportedError> {
        Err(NotSupportedError::new("drag_resize_window is not supported"))
    }

    #[inline]
    pub fn show_window_menu(&self, _position: Position) {}

    pub fn set_cursor_hittest(&self, _hittest: bool) -> Result<(), NotSupportedError> {
        Err(NotSupportedError::new("set_cursor_hittest is not supported"))
    }

    pub fn set_minimized(&self, _minimized: bool) {
        warn!("`Window::set_minimized` is ignored on iOS")
    }

    pub fn is_minimized(&self) -> Option<bool> {
        warn!("`Window::is_minimized` is ignored on iOS");
        None
    }

    pub fn set_maximized(&self, _maximized: bool) {
        warn!("`Window::set_maximized` is ignored on iOS")
    }

    pub fn is_maximized(&self) -> bool {
        warn!("`Window::is_maximized` is ignored on iOS");
        false
    }

    pub(crate) fn set_fullscreen(&self, monitor: Option<Fullscreen>) {
        let mtm = MainThreadMarker::new().unwrap();
        let uiscreen = match &monitor {
            Some(Fullscreen::Exclusive(video_mode)) => {
                let uiscreen = video_mode.monitor.ui_screen(mtm);
                uiscreen.setCurrentMode(Some(video_mode.screen_mode(mtm)));
                uiscreen.clone()
            },
            Some(Fullscreen::Borderless(Some(monitor))) => monitor.ui_screen(mtm).clone(),
            Some(Fullscreen::Borderless(None)) => {
                self.current_monitor_inner().ui_screen(mtm).clone()
            },
            None => {
                warn!("`Window::set_fullscreen(None)` ignored on iOS");
                return;
            },
        };

        // this is pretty slow on iOS, so avoid doing it if we can
        let current = self.window.screen();
        if uiscreen != current {
            self.window.setScreen(&uiscreen);
        }

        let bounds = uiscreen.bounds();
        self.window.setFrame(bounds);

        // For external displays, we must disable overscan compensation or
        // the displayed image will have giant black bars surrounding it on
        // each side
        uiscreen.setOverscanCompensation(UIScreenOverscanCompensation::None);
    }

    pub(crate) fn fullscreen(&self) -> Option<Fullscreen> {
        let mtm = MainThreadMarker::new().unwrap();
        let monitor = self.current_monitor_inner();
        let uiscreen = monitor.ui_screen(mtm);
        let screen_space_bounds = self.screen_frame();
        let screen_bounds = uiscreen.bounds();

        // TODO: track fullscreen instead of relying on brittle float comparisons
        if screen_space_bounds.origin.x == screen_bounds.origin.x
            && screen_space_bounds.origin.y == screen_bounds.origin.y
            && screen_space_bounds.size.width == screen_bounds.size.width
            && screen_space_bounds.size.height == screen_bounds.size.height
        {
            Some(Fullscreen::Borderless(Some(monitor)))
        } else {
            None
        }
    }

    pub fn set_decorations(&self, _decorations: bool) {}

    pub fn is_decorated(&self) -> bool {
        true
    }

    pub fn set_window_level(&self, _level: WindowLevel) {
        warn!("`Window::set_window_level` is ignored on iOS")
    }

    pub fn set_window_icon(&self, _icon: Option<Icon>) {
        warn!("`Window::set_window_icon` is ignored on iOS")
    }

    pub fn set_ime_cursor_area(&self, _position: Position, _size: Size) {
        warn!("`Window::set_ime_cursor_area` is ignored on iOS")
    }

    /// Show / hide the keyboard. To show the keyboard, we call `becomeFirstResponder`,
    /// requesting focus for the [WinitView]. Since [WinitView] implements
    /// [objc2_ui_kit::UIKeyInput], the keyboard will be shown.
    /// <https://developer.apple.com/documentation/uikit/uiresponder/1621113-becomefirstresponder>
    pub fn set_ime_allowed(&self, allowed: bool) {
        if allowed {
            unsafe {
                self.view.becomeFirstResponder();
            }
        } else {
            unsafe {
                self.view.resignFirstResponder();
            }
        }
    }

    pub fn set_ime_purpose(&self, _purpose: ImePurpose) {
        warn!("`Window::set_ime_purpose` is ignored on iOS")
    }

    pub fn focus_window(&self) {
        warn!("`Window::set_focus` is ignored on iOS")
    }

    pub fn request_user_attention(&self, _request_type: Option<UserAttentionType>) {
        warn!("`Window::request_user_attention` is ignored on iOS")
    }

    // Allow directly accessing the current monitor internally without unwrapping.
    fn current_monitor_inner(&self) -> MonitorHandle {
        MonitorHandle::new(self.window.screen())
    }

    pub fn current_monitor(&self) -> Option<MonitorHandle> {
        Some(self.current_monitor_inner())
    }

    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        monitor::uiscreens(MainThreadMarker::new().unwrap())
    }

    pub fn primary_monitor(&self) -> Option<MonitorHandle> {
        #[allow(deprecated)]
        Some(MonitorHandle::new(UIScreen::mainScreen(MainThreadMarker::new().unwrap())))
    }

    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    pub fn raw_window_handle_rwh_06(&self) -> rwh_06::RawWindowHandle {
        let mut window_handle = rwh_06::UiKitWindowHandle::new({
            let ui_view = Retained::as_ptr(&self.view) as _;
            std::ptr::NonNull::new(ui_view).expect("Retained<T> should never be null")
        });
        window_handle.ui_view_controller =
            std::ptr::NonNull::new(Retained::as_ptr(&self.view_controller) as _);
        rwh_06::RawWindowHandle::UiKit(window_handle)
    }

    pub fn theme(&self) -> Option<Theme> {
        warn!("`Window::theme` is ignored on iOS");
        None
    }

    pub fn set_content_protected(&self, _protected: bool) {}

    pub fn has_focus(&self) -> bool {
        self.window.isKeyWindow()
    }

    #[inline]
    pub fn set_theme(&self, _theme: Option<Theme>) {
        warn!("`Window::set_theme` is ignored on iOS");
    }

    pub fn title(&self) -> String {
        warn!("`Window::title` is ignored on iOS");
        String::new()
    }

    pub fn reset_dead_keys(&self) {
        // Noop
    }
}

pub struct Window {
    inner: MainThreadBound<Inner>,
}

impl Window {
    pub(crate) fn new(
        event_loop: &ActiveEventLoop,
        window_attributes: WindowAttributes,
    ) -> Result<Window, RequestError> {
        let mtm = event_loop.mtm;

        if window_attributes.min_surface_size.is_some() {
            warn!("`WindowAttributes::min_surface_size` is ignored on iOS");
        }
        if window_attributes.max_surface_size.is_some() {
            warn!("`WindowAttributes::max_surface_size` is ignored on iOS");
        }

        // TODO: transparency, visible

        #[allow(deprecated)]
        let main_screen = UIScreen::mainScreen(mtm);
        let fullscreen = window_attributes.fullscreen.clone().map(Into::into);
        let screen = match fullscreen {
            Some(Fullscreen::Exclusive(ref video_mode)) => video_mode.monitor.ui_screen(mtm),
            Some(Fullscreen::Borderless(Some(ref monitor))) => monitor.ui_screen(mtm),
            Some(Fullscreen::Borderless(None)) | None => &main_screen,
        };

        let screen_bounds = screen.bounds();

        let frame = match window_attributes.surface_size {
            Some(dim) => {
                let scale_factor = screen.scale();
                let size = dim.to_logical::<f64>(scale_factor as f64);
                CGRect {
                    origin: screen_bounds.origin,
                    size: CGSize { width: size.width as _, height: size.height as _ },
                }
            },
            None => screen_bounds,
        };

        let view = WinitView::new(mtm, &window_attributes, frame);

        let gl_or_metal_backed =
            view.isKindOfClass(class!(CAMetalLayer)) || view.isKindOfClass(class!(CAEAGLLayer));

        let view_controller = WinitViewController::new(mtm, &window_attributes, &view);
        let window = WinitUIWindow::new(mtm, &window_attributes, frame, &view_controller);
        window.makeKeyAndVisible();

        let inner = Inner { window, view_controller, view, gl_or_metal_backed };
        Ok(Window { inner: MainThreadBound::new(inner, mtm) })
    }

    pub(crate) fn maybe_wait_on_main<R: Send>(&self, f: impl FnOnce(&Inner) -> R + Send) -> R {
        self.inner.get_on_main(|inner| f(inner))
    }

    #[inline]
    pub(crate) fn raw_window_handle_rwh_06(
        &self,
    ) -> Result<rwh_06::RawWindowHandle, rwh_06::HandleError> {
        if let Some(mtm) = MainThreadMarker::new() {
            Ok(self.inner.get(mtm).raw_window_handle_rwh_06())
        } else {
            Err(rwh_06::HandleError::Unavailable)
        }
    }

    #[inline]
    pub(crate) fn raw_display_handle_rwh_06(
        &self,
    ) -> Result<rwh_06::RawDisplayHandle, rwh_06::HandleError> {
        Ok(rwh_06::RawDisplayHandle::UiKit(rwh_06::UiKitDisplayHandle::new()))
    }
}

impl rwh_06::HasDisplayHandle for Window {
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        let raw = self.raw_display_handle_rwh_06()?;
        unsafe { Ok(rwh_06::DisplayHandle::borrow_raw(raw)) }
    }
}

impl rwh_06::HasWindowHandle for Window {
    fn window_handle(&self) -> Result<rwh_06::WindowHandle<'_>, rwh_06::HandleError> {
        let raw = self.raw_window_handle_rwh_06()?;
        unsafe { Ok(rwh_06::WindowHandle::borrow_raw(raw)) }
    }
}

impl CoreWindow for Window {
    fn id(&self) -> crate::window::WindowId {
        self.maybe_wait_on_main(|delegate| delegate.id())
    }

    fn scale_factor(&self) -> f64 {
        self.maybe_wait_on_main(|delegate| delegate.scale_factor())
    }

    fn request_redraw(&self) {
        self.maybe_wait_on_main(|delegate| delegate.request_redraw());
    }

    fn pre_present_notify(&self) {
        self.maybe_wait_on_main(|delegate| delegate.pre_present_notify());
    }

    fn reset_dead_keys(&self) {
        self.maybe_wait_on_main(|delegate| delegate.reset_dead_keys());
    }

    fn surface_position(&self) -> PhysicalPosition<i32> {
        self.maybe_wait_on_main(|delegate| delegate.surface_position())
    }

    fn outer_position(&self) -> Result<PhysicalPosition<i32>, RequestError> {
        self.maybe_wait_on_main(|delegate| delegate.outer_position())
    }

    fn set_outer_position(&self, position: Position) {
        self.maybe_wait_on_main(|delegate| delegate.set_outer_position(position));
    }

    fn surface_size(&self) -> PhysicalSize<u32> {
        self.maybe_wait_on_main(|delegate| delegate.surface_size())
    }

    fn request_surface_size(&self, size: Size) -> Option<PhysicalSize<u32>> {
        self.maybe_wait_on_main(|delegate| delegate.request_surface_size(size))
    }

    fn outer_size(&self) -> PhysicalSize<u32> {
        self.maybe_wait_on_main(|delegate| delegate.outer_size())
    }

    fn safe_area(&self) -> PhysicalInsets<u32> {
        self.maybe_wait_on_main(|delegate| delegate.safe_area())
    }

    fn set_min_surface_size(&self, min_size: Option<Size>) {
        self.maybe_wait_on_main(|delegate| delegate.set_min_surface_size(min_size))
    }

    fn set_max_surface_size(&self, max_size: Option<Size>) {
        self.maybe_wait_on_main(|delegate| delegate.set_max_surface_size(max_size));
    }

    fn surface_resize_increments(&self) -> Option<PhysicalSize<u32>> {
        self.maybe_wait_on_main(|delegate| delegate.surface_resize_increments())
    }

    fn set_surface_resize_increments(&self, increments: Option<Size>) {
        self.maybe_wait_on_main(|delegate| delegate.set_surface_resize_increments(increments));
    }

    fn set_title(&self, title: &str) {
        self.maybe_wait_on_main(|delegate| delegate.set_title(title));
    }

    fn set_transparent(&self, transparent: bool) {
        self.maybe_wait_on_main(|delegate| delegate.set_transparent(transparent));
    }

    fn set_blur(&self, blur: bool) {
        self.maybe_wait_on_main(|delegate| delegate.set_blur(blur));
    }

    fn set_visible(&self, visible: bool) {
        self.maybe_wait_on_main(|delegate| delegate.set_visible(visible));
    }

    fn is_visible(&self) -> Option<bool> {
        self.maybe_wait_on_main(|delegate| delegate.is_visible())
    }

    fn set_resizable(&self, resizable: bool) {
        self.maybe_wait_on_main(|delegate| delegate.set_resizable(resizable))
    }

    fn is_resizable(&self) -> bool {
        self.maybe_wait_on_main(|delegate| delegate.is_resizable())
    }

    fn set_enabled_buttons(&self, buttons: WindowButtons) {
        self.maybe_wait_on_main(|delegate| delegate.set_enabled_buttons(buttons))
    }

    fn enabled_buttons(&self) -> WindowButtons {
        self.maybe_wait_on_main(|delegate| delegate.enabled_buttons())
    }

    fn set_minimized(&self, minimized: bool) {
        self.maybe_wait_on_main(|delegate| delegate.set_minimized(minimized));
    }

    fn is_minimized(&self) -> Option<bool> {
        self.maybe_wait_on_main(|delegate| delegate.is_minimized())
    }

    fn set_maximized(&self, maximized: bool) {
        self.maybe_wait_on_main(|delegate| delegate.set_maximized(maximized));
    }

    fn is_maximized(&self) -> bool {
        self.maybe_wait_on_main(|delegate| delegate.is_maximized())
    }

    fn set_fullscreen(&self, fullscreen: Option<crate::window::Fullscreen>) {
        self.maybe_wait_on_main(|delegate| delegate.set_fullscreen(fullscreen.map(Into::into)))
    }

    fn fullscreen(&self) -> Option<crate::window::Fullscreen> {
        self.maybe_wait_on_main(|delegate| delegate.fullscreen().map(Into::into))
    }

    fn set_decorations(&self, decorations: bool) {
        self.maybe_wait_on_main(|delegate| delegate.set_decorations(decorations));
    }

    fn is_decorated(&self) -> bool {
        self.maybe_wait_on_main(|delegate| delegate.is_decorated())
    }

    fn set_window_level(&self, level: WindowLevel) {
        self.maybe_wait_on_main(|delegate| delegate.set_window_level(level));
    }

    fn set_window_icon(&self, window_icon: Option<Icon>) {
        self.maybe_wait_on_main(|delegate| delegate.set_window_icon(window_icon));
    }

    fn set_ime_cursor_area(&self, position: Position, size: Size) {
        self.maybe_wait_on_main(|delegate| delegate.set_ime_cursor_area(position, size));
    }

    fn set_ime_allowed(&self, allowed: bool) {
        self.maybe_wait_on_main(|delegate| delegate.set_ime_allowed(allowed));
    }

    fn set_ime_purpose(&self, purpose: ImePurpose) {
        self.maybe_wait_on_main(|delegate| delegate.set_ime_purpose(purpose));
    }

    fn focus_window(&self) {
        self.maybe_wait_on_main(|delegate| delegate.focus_window());
    }

    fn has_focus(&self) -> bool {
        self.maybe_wait_on_main(|delegate| delegate.has_focus())
    }

    fn request_user_attention(&self, request_type: Option<UserAttentionType>) {
        self.maybe_wait_on_main(|delegate| delegate.request_user_attention(request_type));
    }

    fn set_theme(&self, theme: Option<Theme>) {
        self.maybe_wait_on_main(|delegate| delegate.set_theme(theme));
    }

    fn theme(&self) -> Option<Theme> {
        self.maybe_wait_on_main(|delegate| delegate.theme())
    }

    fn set_content_protected(&self, protected: bool) {
        self.maybe_wait_on_main(|delegate| delegate.set_content_protected(protected));
    }

    fn title(&self) -> String {
        self.maybe_wait_on_main(|delegate| delegate.title())
    }

    fn set_cursor(&self, cursor: Cursor) {
        self.maybe_wait_on_main(|delegate| delegate.set_cursor(cursor));
    }

    fn set_cursor_position(&self, position: Position) -> Result<(), RequestError> {
        Ok(self.maybe_wait_on_main(|delegate| delegate.set_cursor_position(position))?)
    }

    fn set_cursor_grab(&self, mode: crate::window::CursorGrabMode) -> Result<(), RequestError> {
        Ok(self.maybe_wait_on_main(|delegate| delegate.set_cursor_grab(mode))?)
    }

    fn set_cursor_visible(&self, visible: bool) {
        self.maybe_wait_on_main(|delegate| delegate.set_cursor_visible(visible))
    }

    fn drag_window(&self) -> Result<(), RequestError> {
        Ok(self.maybe_wait_on_main(|delegate| delegate.drag_window())?)
    }

    fn drag_resize_window(
        &self,
        direction: crate::window::ResizeDirection,
    ) -> Result<(), RequestError> {
        Ok(self.maybe_wait_on_main(|delegate| delegate.drag_resize_window(direction))?)
    }

    fn show_window_menu(&self, position: Position) {
        self.maybe_wait_on_main(|delegate| delegate.show_window_menu(position))
    }

    fn set_cursor_hittest(&self, hittest: bool) -> Result<(), RequestError> {
        Ok(self.maybe_wait_on_main(|delegate| delegate.set_cursor_hittest(hittest))?)
    }

    fn current_monitor(&self) -> Option<CoreMonitorHandle> {
        self.maybe_wait_on_main(|delegate| {
            delegate.current_monitor().map(|inner| CoreMonitorHandle { inner })
        })
    }

    fn available_monitors(&self) -> Box<dyn Iterator<Item = CoreMonitorHandle>> {
        self.maybe_wait_on_main(|delegate| {
            Box::new(
                delegate.available_monitors().into_iter().map(|inner| CoreMonitorHandle { inner }),
            )
        })
    }

    fn primary_monitor(&self) -> Option<CoreMonitorHandle> {
        self.maybe_wait_on_main(|delegate| {
            delegate.primary_monitor().map(|inner| CoreMonitorHandle { inner })
        })
    }

    fn rwh_06_display_handle(&self) -> &dyn rwh_06::HasDisplayHandle {
        self
    }

    fn rwh_06_window_handle(&self) -> &dyn rwh_06::HasWindowHandle {
        self
    }
}

// WindowExtIOS
impl Inner {
    pub fn set_scale_factor(&self, scale_factor: f64) {
        assert!(
            dpi::validate_scale_factor(scale_factor),
            "`WindowExtIOS::set_scale_factor` received an invalid hidpi factor"
        );
        let scale_factor = scale_factor as CGFloat;
        self.view.setContentScaleFactor(scale_factor);
    }

    pub fn set_valid_orientations(&self, valid_orientations: ValidOrientations) {
        self.view_controller.set_supported_interface_orientations(
            MainThreadMarker::new().unwrap(),
            valid_orientations,
        );
    }

    pub fn set_prefers_home_indicator_hidden(&self, hidden: bool) {
        self.view_controller.set_prefers_home_indicator_auto_hidden(hidden);
    }

    pub fn set_preferred_screen_edges_deferring_system_gestures(&self, edges: ScreenEdge) {
        self.view_controller.set_preferred_screen_edges_deferring_system_gestures(edges);
    }

    pub fn set_prefers_status_bar_hidden(&self, hidden: bool) {
        self.view_controller.set_prefers_status_bar_hidden(hidden);
    }

    pub fn set_preferred_status_bar_style(&self, status_bar_style: StatusBarStyle) {
        self.view_controller.set_preferred_status_bar_style(status_bar_style);
    }

    pub fn recognize_pinch_gesture(&self, should_recognize: bool) {
        self.view.recognize_pinch_gesture(should_recognize);
    }

    pub fn recognize_pan_gesture(
        &self,
        should_recognize: bool,
        minimum_number_of_touches: u8,
        maximum_number_of_touches: u8,
    ) {
        self.view.recognize_pan_gesture(
            should_recognize,
            minimum_number_of_touches,
            maximum_number_of_touches,
        );
    }

    pub fn recognize_doubletap_gesture(&self, should_recognize: bool) {
        self.view.recognize_doubletap_gesture(should_recognize);
    }

    pub fn recognize_rotation_gesture(&self, should_recognize: bool) {
        self.view.recognize_rotation_gesture(should_recognize);
    }
}

impl Inner {
    fn screen_frame(&self) -> CGRect {
        self.rect_to_screen_space(self.window.frame())
    }

    fn rect_to_screen_space(&self, rect: CGRect) -> CGRect {
        let screen_space = self.window.screen().coordinateSpace();
        self.window.convertRect_toCoordinateSpace(rect, &screen_space)
    }

    fn rect_from_screen_space(&self, rect: CGRect) -> CGRect {
        let screen_space = self.window.screen().coordinateSpace();
        self.window.convertRect_fromCoordinateSpace(rect, &screen_space)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlatformSpecificWindowAttributes {
    pub scale_factor: Option<f64>,
    pub valid_orientations: ValidOrientations,
    pub prefers_home_indicator_hidden: bool,
    pub prefers_status_bar_hidden: bool,
    pub preferred_status_bar_style: StatusBarStyle,
    pub preferred_screen_edges_deferring_system_gestures: ScreenEdge,
}
