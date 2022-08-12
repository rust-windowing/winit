use std::{
    collections::VecDeque,
    ops::{Deref, DerefMut},
};

use objc::runtime::{Class, Object, BOOL, NO, YES};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle, UiKitDisplayHandle, UiKitWindowHandle};

use crate::{
    dpi::{self, LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize, Position, Size},
    error::{ExternalError, NotSupportedError, OsError as RootOsError},
    event::{Event, WindowEvent},
    icon::Icon,
    monitor::MonitorHandle as RootMonitorHandle,
    platform::ios::{MonitorHandleExtIOS, ScreenEdge, ValidOrientations},
    platform_impl::platform::{
        app_state,
        event_loop::{self, EventProxy, EventWrapper},
        ffi::{
            id, CGFloat, CGPoint, CGRect, CGSize, UIEdgeInsets, UIInterfaceOrientationMask,
            UIRectEdge, UIScreenOverscanCompensation,
        },
        monitor, view, EventLoopWindowTarget, MonitorHandle,
    },
    window::{
        CursorGrabMode, CursorIcon, Fullscreen, UserAttentionType, WindowAttributes,
        WindowId as RootWindowId,
    },
};

pub struct Inner {
    pub window: id,
    pub view_controller: id,
    pub view: id,
    gl_or_metal_backed: bool,
}

impl Drop for Inner {
    fn drop(&mut self) {
        unsafe {
            let _: () = msg_send![self.view, release];
            let _: () = msg_send![self.view_controller, release];
            let _: () = msg_send![self.window, release];
        }
    }
}

impl Inner {
    pub fn set_title(&self, _title: &str) {
        debug!("`Window::set_title` is ignored on iOS")
    }

    pub fn set_visible(&self, visible: bool) {
        match visible {
            true => unsafe {
                let _: () = msg_send![self.window, setHidden: NO];
            },
            false => unsafe {
                let _: () = msg_send![self.window, setHidden: YES];
            },
        }
    }

    pub fn is_visible(&self) -> Option<bool> {
        warn!("`Window::is_visible` is ignored on iOS");
        None
    }

    pub fn request_redraw(&self) {
        unsafe {
            if self.gl_or_metal_backed {
                // `setNeedsDisplay` does nothing on UIViews which are directly backed by CAEAGLLayer or CAMetalLayer.
                // Ordinarily the OS sets up a bunch of UIKit state before calling drawRect: on a UIView, but when using
                // raw or gl/metal for drawing this work is completely avoided.
                //
                // The docs for `setNeedsDisplay` don't mention `CAMetalLayer`; however, this has been confirmed via
                // testing.
                //
                // https://developer.apple.com/documentation/uikit/uiview/1622437-setneedsdisplay?language=objc
                app_state::queue_gl_or_metal_redraw(self.window);
            } else {
                let _: () = msg_send![self.view, setNeedsDisplay];
            }
        }
    }

    pub fn inner_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        unsafe {
            let safe_area = self.safe_area_screen_space();
            let position = LogicalPosition {
                x: safe_area.origin.x as f64,
                y: safe_area.origin.y as f64,
            };
            let scale_factor = self.scale_factor();
            Ok(position.to_physical(scale_factor))
        }
    }

    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        unsafe {
            let screen_frame = self.screen_frame();
            let position = LogicalPosition {
                x: screen_frame.origin.x as f64,
                y: screen_frame.origin.y as f64,
            };
            let scale_factor = self.scale_factor();
            Ok(position.to_physical(scale_factor))
        }
    }

    pub fn set_outer_position(&self, physical_position: Position) {
        unsafe {
            let scale_factor = self.scale_factor();
            let position = physical_position.to_logical::<f64>(scale_factor);
            let screen_frame = self.screen_frame();
            let new_screen_frame = CGRect {
                origin: CGPoint {
                    x: position.x as _,
                    y: position.y as _,
                },
                size: screen_frame.size,
            };
            let bounds = self.rect_from_screen_space(new_screen_frame);
            let _: () = msg_send![self.window, setBounds: bounds];
        }
    }

    pub fn inner_size(&self) -> PhysicalSize<u32> {
        unsafe {
            let scale_factor = self.scale_factor();
            let safe_area = self.safe_area_screen_space();
            let size = LogicalSize {
                width: safe_area.size.width as f64,
                height: safe_area.size.height as f64,
            };
            size.to_physical(scale_factor)
        }
    }

    pub fn outer_size(&self) -> PhysicalSize<u32> {
        unsafe {
            let scale_factor = self.scale_factor();
            let screen_frame = self.screen_frame();
            let size = LogicalSize {
                width: screen_frame.size.width as f64,
                height: screen_frame.size.height as f64,
            };
            size.to_physical(scale_factor)
        }
    }

    pub fn set_inner_size(&self, _size: Size) {
        warn!("not clear what `Window::set_inner_size` means on iOS");
    }

    pub fn set_min_inner_size(&self, _dimensions: Option<Size>) {
        warn!("`Window::set_min_inner_size` is ignored on iOS")
    }

    pub fn set_max_inner_size(&self, _dimensions: Option<Size>) {
        warn!("`Window::set_max_inner_size` is ignored on iOS")
    }

    pub fn set_resizable(&self, _resizable: bool) {
        warn!("`Window::set_resizable` is ignored on iOS")
    }

    pub fn is_resizable(&self) -> bool {
        warn!("`Window::is_resizable` is ignored on iOS");
        false
    }

    pub fn scale_factor(&self) -> f64 {
        unsafe {
            let hidpi: CGFloat = msg_send![self.view, contentScaleFactor];
            hidpi as _
        }
    }

    pub fn set_cursor_icon(&self, _cursor: CursorIcon) {
        debug!("`Window::set_cursor_icon` ignored on iOS")
    }

    pub fn set_cursor_position(&self, _position: Position) -> Result<(), ExternalError> {
        Err(ExternalError::NotSupported(NotSupportedError::new()))
    }

    pub fn set_cursor_grab(&self, _: CursorGrabMode) -> Result<(), ExternalError> {
        Err(ExternalError::NotSupported(NotSupportedError::new()))
    }

    pub fn set_cursor_visible(&self, _visible: bool) {
        debug!("`Window::set_cursor_visible` is ignored on iOS")
    }

    pub fn drag_window(&self) -> Result<(), ExternalError> {
        Err(ExternalError::NotSupported(NotSupportedError::new()))
    }

    pub fn set_cursor_hittest(&self, _hittest: bool) -> Result<(), ExternalError> {
        Err(ExternalError::NotSupported(NotSupportedError::new()))
    }

    pub fn set_minimized(&self, _minimized: bool) {
        warn!("`Window::set_minimized` is ignored on iOS")
    }

    pub fn set_maximized(&self, _maximized: bool) {
        warn!("`Window::set_maximized` is ignored on iOS")
    }

    pub fn is_maximized(&self) -> bool {
        warn!("`Window::is_maximized` is ignored on iOS");
        false
    }

    pub fn set_fullscreen(&self, monitor: Option<Fullscreen>) {
        unsafe {
            let uiscreen = match monitor {
                Some(Fullscreen::Exclusive(video_mode)) => {
                    let uiscreen = video_mode.video_mode.monitor.ui_screen() as id;
                    let _: () =
                        msg_send![uiscreen, setCurrentMode: video_mode.video_mode.screen_mode.0];
                    uiscreen
                }
                Some(Fullscreen::Borderless(monitor)) => monitor
                    .unwrap_or_else(|| self.current_monitor_inner())
                    .ui_screen() as id,
                None => {
                    warn!("`Window::set_fullscreen(None)` ignored on iOS");
                    return;
                }
            };

            // this is pretty slow on iOS, so avoid doing it if we can
            let current: id = msg_send![self.window, screen];
            if uiscreen != current {
                let _: () = msg_send![self.window, setScreen: uiscreen];
            }

            let bounds: CGRect = msg_send![uiscreen, bounds];
            let _: () = msg_send![self.window, setFrame: bounds];

            // For external displays, we must disable overscan compensation or
            // the displayed image will have giant black bars surrounding it on
            // each side
            let _: () = msg_send![
                uiscreen,
                setOverscanCompensation: UIScreenOverscanCompensation::None
            ];
        }
    }

    pub fn fullscreen(&self) -> Option<Fullscreen> {
        unsafe {
            let monitor = self.current_monitor_inner();
            let uiscreen = monitor.inner.ui_screen();
            let screen_space_bounds = self.screen_frame();
            let screen_bounds: CGRect = msg_send![uiscreen, bounds];

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
    }

    pub fn set_decorations(&self, _decorations: bool) {
        warn!("`Window::set_decorations` is ignored on iOS")
    }

    pub fn is_decorated(&self) -> bool {
        warn!("`Window::is_decorated` is ignored on iOS");
        true
    }

    pub fn set_always_on_top(&self, _always_on_top: bool) {
        warn!("`Window::set_always_on_top` is ignored on iOS")
    }

    pub fn set_window_icon(&self, _icon: Option<Icon>) {
        warn!("`Window::set_window_icon` is ignored on iOS")
    }

    pub fn set_ime_position(&self, _position: Position) {
        warn!("`Window::set_ime_position` is ignored on iOS")
    }

    pub fn set_ime_allowed(&self, _allowed: bool) {
        warn!("`Window::set_ime_allowed` is ignored on iOS")
    }

    pub fn focus_window(&self) {
        warn!("`Window::set_focus` is ignored on iOS")
    }

    pub fn request_user_attention(&self, _request_type: Option<UserAttentionType>) {
        warn!("`Window::request_user_attention` is ignored on iOS")
    }

    // Allow directly accessing the current monitor internally without unwrapping.
    fn current_monitor_inner(&self) -> RootMonitorHandle {
        unsafe {
            let uiscreen: id = msg_send![self.window, screen];
            RootMonitorHandle {
                inner: MonitorHandle::retained_new(uiscreen),
            }
        }
    }

    pub fn current_monitor(&self) -> Option<RootMonitorHandle> {
        Some(self.current_monitor_inner())
    }

    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        unsafe { monitor::uiscreens() }
    }

    pub fn primary_monitor(&self) -> Option<RootMonitorHandle> {
        let monitor = unsafe { monitor::main_uiscreen() };
        Some(RootMonitorHandle { inner: monitor })
    }

    pub fn id(&self) -> WindowId {
        self.window.into()
    }

    pub fn raw_window_handle(&self) -> RawWindowHandle {
        let mut window_handle = UiKitWindowHandle::empty();
        window_handle.ui_window = self.window as _;
        window_handle.ui_view = self.view as _;
        window_handle.ui_view_controller = self.view_controller as _;
        RawWindowHandle::UiKit(window_handle)
    }

    pub fn raw_display_handle(&self) -> RawDisplayHandle {
        RawDisplayHandle::UiKit(UiKitDisplayHandle::empty())
    }
}

pub struct Window {
    pub inner: Inner,
}

impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            assert_main_thread!("`Window::drop` can only be run on the main thread on iOS");
        }
    }
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

impl Deref for Window {
    type Target = Inner;

    fn deref(&self) -> &Inner {
        unsafe {
            assert_main_thread!("`Window` methods can only be run on the main thread on iOS");
        }
        &self.inner
    }
}

impl DerefMut for Window {
    fn deref_mut(&mut self) -> &mut Inner {
        unsafe {
            assert_main_thread!("`Window` methods can only be run on the main thread on iOS");
        }
        &mut self.inner
    }
}

impl Window {
    pub(crate) fn new<T>(
        _event_loop: &EventLoopWindowTarget<T>,
        window_attributes: WindowAttributes,
        platform_attributes: PlatformSpecificWindowBuilderAttributes,
    ) -> Result<Window, RootOsError> {
        if window_attributes.min_inner_size.is_some() {
            warn!("`WindowAttributes::min_inner_size` is ignored on iOS");
        }
        if window_attributes.max_inner_size.is_some() {
            warn!("`WindowAttributes::max_inner_size` is ignored on iOS");
        }
        if window_attributes.always_on_top {
            warn!("`WindowAttributes::always_on_top` is unsupported on iOS");
        }
        // TODO: transparency, visible

        unsafe {
            let screen = match window_attributes.fullscreen {
                Some(Fullscreen::Exclusive(ref video_mode)) => {
                    video_mode.video_mode.monitor.ui_screen() as id
                }
                Some(Fullscreen::Borderless(Some(ref monitor))) => monitor.inner.ui_screen(),
                Some(Fullscreen::Borderless(None)) | None => {
                    monitor::main_uiscreen().ui_screen() as id
                }
            };

            let screen_bounds: CGRect = msg_send![screen, bounds];

            let frame = match window_attributes.inner_size {
                Some(dim) => {
                    let scale_factor: CGFloat = msg_send![screen, scale];
                    let size = dim.to_logical::<f64>(scale_factor as f64);
                    CGRect {
                        origin: screen_bounds.origin,
                        size: CGSize {
                            width: size.width as _,
                            height: size.height as _,
                        },
                    }
                }
                None => screen_bounds,
            };

            let view = view::create_view(&window_attributes, &platform_attributes, frame);

            let gl_or_metal_backed = {
                let view_class: *const Class = msg_send![view, class];
                let layer_class: *const Class = msg_send![view_class, layerClass];
                let is_metal: BOOL =
                    msg_send![layer_class, isSubclassOfClass: class!(CAMetalLayer)];
                let is_gl: BOOL = msg_send![layer_class, isSubclassOfClass: class!(CAEAGLLayer)];
                is_metal == YES || is_gl == YES
            };

            let view_controller =
                view::create_view_controller(&window_attributes, &platform_attributes, view);
            let window = view::create_window(
                &window_attributes,
                &platform_attributes,
                frame,
                view_controller,
            );

            let result = Window {
                inner: Inner {
                    window,
                    view_controller,
                    view,
                    gl_or_metal_backed,
                },
            };
            app_state::set_key_window(window);

            // Like the Windows and macOS backends, we send a `ScaleFactorChanged` and `Resized`
            // event on window creation if the DPI factor != 1.0
            let scale_factor: CGFloat = msg_send![view, contentScaleFactor];
            let scale_factor = scale_factor as f64;
            if scale_factor != 1.0 {
                let bounds: CGRect = msg_send![view, bounds];
                let screen: id = msg_send![window, screen];
                let screen_space: id = msg_send![screen, coordinateSpace];
                let screen_frame: CGRect =
                    msg_send![view, convertRect:bounds toCoordinateSpace:screen_space];
                let size = crate::dpi::LogicalSize {
                    width: screen_frame.size.width as _,
                    height: screen_frame.size.height as _,
                };
                app_state::handle_nonuser_events(
                    std::iter::once(EventWrapper::EventProxy(EventProxy::DpiChangedProxy {
                        window_id: window,
                        scale_factor,
                        suggested_size: size,
                    }))
                    .chain(std::iter::once(EventWrapper::StaticEvent(
                        Event::WindowEvent {
                            window_id: RootWindowId(window.into()),
                            event: WindowEvent::Resized(size.to_physical(scale_factor)),
                        },
                    ))),
                );
            }

            Ok(result)
        }
    }
}

// WindowExtIOS
impl Inner {
    pub fn ui_window(&self) -> id {
        self.window
    }
    pub fn ui_view_controller(&self) -> id {
        self.view_controller
    }
    pub fn ui_view(&self) -> id {
        self.view
    }

    pub fn set_scale_factor(&self, scale_factor: f64) {
        unsafe {
            assert!(
                dpi::validate_scale_factor(scale_factor),
                "`WindowExtIOS::set_scale_factor` received an invalid hidpi factor"
            );
            let scale_factor = scale_factor as CGFloat;
            let _: () = msg_send![self.view, setContentScaleFactor: scale_factor];
        }
    }

    pub fn set_valid_orientations(&self, valid_orientations: ValidOrientations) {
        unsafe {
            let idiom = event_loop::get_idiom();
            let supported_orientations = UIInterfaceOrientationMask::from_valid_orientations_idiom(
                valid_orientations,
                idiom,
            );
            msg_send![
                self.view_controller,
                setSupportedInterfaceOrientations: supported_orientations
            ]
        }
    }

    pub fn set_prefers_home_indicator_hidden(&self, hidden: bool) {
        unsafe {
            let prefers_home_indicator_hidden = if hidden { YES } else { NO };
            let _: () = msg_send![
                self.view_controller,
                setPrefersHomeIndicatorAutoHidden: prefers_home_indicator_hidden
            ];
        }
    }

    pub fn set_preferred_screen_edges_deferring_system_gestures(&self, edges: ScreenEdge) {
        let edges: UIRectEdge = edges.into();
        unsafe {
            let _: () = msg_send![
                self.view_controller,
                setPreferredScreenEdgesDeferringSystemGestures: edges
            ];
        }
    }

    pub fn set_prefers_status_bar_hidden(&self, hidden: bool) {
        unsafe {
            let status_bar_hidden = if hidden { YES } else { NO };
            let _: () = msg_send![
                self.view_controller,
                setPrefersStatusBarHidden: status_bar_hidden
            ];
        }
    }
}

impl Inner {
    // requires main thread
    unsafe fn screen_frame(&self) -> CGRect {
        self.rect_to_screen_space(msg_send![self.window, bounds])
    }

    // requires main thread
    unsafe fn rect_to_screen_space(&self, rect: CGRect) -> CGRect {
        let screen: id = msg_send![self.window, screen];
        if !screen.is_null() {
            let screen_space: id = msg_send![screen, coordinateSpace];
            msg_send![self.window, convertRect:rect toCoordinateSpace:screen_space]
        } else {
            rect
        }
    }

    // requires main thread
    unsafe fn rect_from_screen_space(&self, rect: CGRect) -> CGRect {
        let screen: id = msg_send![self.window, screen];
        if !screen.is_null() {
            let screen_space: id = msg_send![screen, coordinateSpace];
            msg_send![self.window, convertRect:rect fromCoordinateSpace:screen_space]
        } else {
            rect
        }
    }

    // requires main thread
    unsafe fn safe_area_screen_space(&self) -> CGRect {
        let bounds: CGRect = msg_send![self.window, bounds];
        if app_state::os_capabilities().safe_area {
            let safe_area: UIEdgeInsets = msg_send![self.window, safeAreaInsets];
            let safe_bounds = CGRect {
                origin: CGPoint {
                    x: bounds.origin.x + safe_area.left,
                    y: bounds.origin.y + safe_area.top,
                },
                size: CGSize {
                    width: bounds.size.width - safe_area.left - safe_area.right,
                    height: bounds.size.height - safe_area.top - safe_area.bottom,
                },
            };
            self.rect_to_screen_space(safe_bounds)
        } else {
            let screen_frame = self.rect_to_screen_space(bounds);
            let status_bar_frame: CGRect = {
                let app: id = msg_send![class!(UIApplication), sharedApplication];
                assert!(
                    !app.is_null(),
                    "`Window::get_inner_position` cannot be called before `EventLoop::run` on iOS"
                );
                msg_send![app, statusBarFrame]
            };
            let (y, height) = if screen_frame.origin.y > status_bar_frame.size.height {
                (screen_frame.origin.y, screen_frame.size.height)
            } else {
                let y = status_bar_frame.size.height;
                let height = screen_frame.size.height
                    - (status_bar_frame.size.height - screen_frame.origin.y);
                (y, height)
            };
            CGRect {
                origin: CGPoint {
                    x: screen_frame.origin.x,
                    y,
                },
                size: CGSize {
                    width: screen_frame.size.width,
                    height,
                },
            }
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId {
    window: id,
}

impl WindowId {
    pub const unsafe fn dummy() -> Self {
        WindowId {
            window: std::ptr::null_mut(),
        }
    }
}

impl From<WindowId> for u64 {
    fn from(window_id: WindowId) -> Self {
        window_id.window as u64
    }
}

impl From<u64> for WindowId {
    fn from(raw_id: u64) -> Self {
        Self {
            window: raw_id as _,
        }
    }
}

unsafe impl Send for WindowId {}
unsafe impl Sync for WindowId {}

impl From<&Object> for WindowId {
    fn from(window: &Object) -> WindowId {
        WindowId {
            window: window as *const _ as _,
        }
    }
}

impl From<&mut Object> for WindowId {
    fn from(window: &mut Object) -> WindowId {
        WindowId {
            window: window as _,
        }
    }
}

impl From<id> for WindowId {
    fn from(window: id) -> WindowId {
        WindowId { window }
    }
}

#[derive(Clone)]
pub struct PlatformSpecificWindowBuilderAttributes {
    pub root_view_class: &'static Class,
    pub scale_factor: Option<f64>,
    pub valid_orientations: ValidOrientations,
    pub prefers_home_indicator_hidden: bool,
    pub prefers_status_bar_hidden: bool,
    pub preferred_screen_edges_deferring_system_gestures: ScreenEdge,
}

impl Default for PlatformSpecificWindowBuilderAttributes {
    fn default() -> PlatformSpecificWindowBuilderAttributes {
        PlatformSpecificWindowBuilderAttributes {
            root_view_class: class!(UIView),
            scale_factor: None,
            valid_orientations: Default::default(),
            prefers_home_indicator_hidden: false,
            prefers_status_bar_hidden: false,
            preferred_screen_edges_deferring_system_gestures: Default::default(),
        }
    }
}
