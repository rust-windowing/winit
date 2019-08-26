use raw_window_handle::{ios::IOSHandle, RawWindowHandle};
use std::{
    cell::RefCell,
    collections::VecDeque,
    ops::{Deref, DerefMut},
    ptr::null_mut,
};

use objc::runtime::{Class, Object, NO, YES};

use crate::{
    dpi::{self, LogicalPosition, LogicalSize},
    error::{ExternalError, NotSupportedError, OsError as RootOsError},
    icon::Icon,
    monitor::MonitorHandle as RootMonitorHandle,
    platform::ios::{MonitorHandleExtIOS, ScreenEdge, ValidOrientations},
    platform_impl::platform::{
        app_state::AppState,
        event_loop,
        ffi::{
            id, CGFloat, CGPoint, CGRect, CGSize, UIEdgeInsets, UIInterfaceOrientationMask,
            UIRectEdge, UIScreenOverscanCompensation,
        },
        monitor, view, EventLoopWindowTarget, MonitorHandle,
    },
    window::{CursorIcon, Fullscreen, WindowAttributes},
};

pub struct Inner {
    pub window_attributes: RefCell<WindowAttributes>,
    pub platform_attributes: RefCell<PlatformSpecificWindowBuilderAttributes>,
    supports_safe_area: bool,
    pub window: id,
    pub view_controller: id,
    pub view: id,
}

impl Drop for Inner {
    fn drop(&mut self) {
        unsafe {
            let () = msg_send![self.view, release];
            if !self.view_controller.is_null() {
                let () = msg_send![self.view_controller, release];
            }
            if !self.window.is_null() {
                let () = msg_send![self.window, release];
            }
        }
    }
}

impl Inner {
    pub fn init(&mut self) {
        debug_assert!(self.window.is_null(), "window already initialized");
        debug_assert!(
            self.view_controller.is_null(),
            "view controller already initialized"
        );
        unsafe {
            let window_attributes = self.window_attributes.borrow();
            let platform_attributes = self.platform_attributes.borrow();
            self.view_controller =
                view::create_view_controller(&window_attributes, &platform_attributes, self.view);
            self.window = view::create_window(
                &window_attributes,
                &platform_attributes,
                self.view_controller,
            );
        }
    }

    pub fn set_title(&self, _title: &str) {
        debug!("`Window::set_title` is ignored on iOS")
    }

    pub fn set_visible(&self, visible: bool) {
        self.window_attributes.borrow_mut().visible = visible;
        if !self.window.is_null() {
            match visible {
                true => unsafe {
                    let () = msg_send![self.window, setHidden: NO];
                },
                false => unsafe {
                    let () = msg_send![self.window, setHidden: YES];
                },
            }
        }
    }

    pub fn request_redraw(&self) {
        unsafe {
            let () = msg_send![self.view, setNeedsDisplay];
        }
    }

    pub fn inner_position(&self) -> Result<LogicalPosition, NotSupportedError> {
        self.outer_position()
    }

    pub fn outer_position(&self) -> Result<LogicalPosition, NotSupportedError> {
        let frame = unsafe {
            if self.window.is_null() {
                view::frame_from_window_attributes(&self.window_attributes.borrow())
            } else {
                msg_send![self.window, frame]
            }
        };
        Ok(LogicalPosition {
            x: frame.origin.x,
            y: frame.origin.y,
        })
    }

    pub fn set_outer_position(&self, _position: LogicalPosition) {
        warn!("`Window::set_outer_position` is ignored on iOS")
    }

    pub fn inner_size(&self) -> LogicalSize {
        self.outer_size()
    }

    pub fn outer_size(&self) -> LogicalSize {
        let frame = unsafe {
            if self.window.is_null() {
                view::frame_from_window_attributes(&self.window_attributes.borrow())
            } else {
                msg_send![self.window, frame]
            }
        };
        LogicalSize {
            width: frame.size.width,
            height: frame.size.height,
        }
    }

    pub fn set_inner_size(&self, _size: LogicalSize) {
        warn!("`Window::set_inner_size` is ignored on iOS")
    }

    pub fn set_min_inner_size(&self, _dimensions: Option<LogicalSize>) {
        warn!("`Window::set_min_inner_size` is ignored on iOS")
    }

    pub fn set_max_inner_size(&self, _dimensions: Option<LogicalSize>) {
        warn!("`Window::set_max_inner_size` is ignored on iOS")
    }

    pub fn set_resizable(&self, _resizable: bool) {
        warn!("`Window::set_resizable` is ignored on iOS")
    }

    pub fn hidpi_factor(&self) -> f64 {
        unsafe {
            let hidpi: CGFloat = msg_send![self.view, contentScaleFactor];
            hidpi as _
        }
    }

    pub fn set_cursor_icon(&self, _cursor: CursorIcon) {
        debug!("`Window::set_cursor_icon` ignored on iOS")
    }

    pub fn set_cursor_position(&self, _position: LogicalPosition) -> Result<(), ExternalError> {
        Err(ExternalError::NotSupported(NotSupportedError::new()))
    }

    pub fn set_cursor_grab(&self, _grab: bool) -> Result<(), ExternalError> {
        Err(ExternalError::NotSupported(NotSupportedError::new()))
    }

    pub fn set_cursor_visible(&self, _visible: bool) {
        debug!("`Window::set_cursor_visible` is ignored on iOS")
    }

    pub fn set_maximized(&self, _maximized: bool) {
        warn!("`Window::set_maximized` is ignored on iOS")
    }

    pub fn set_fullscreen(&self, fullscreen: Option<Fullscreen>) {
        self.window_attributes.borrow_mut().fullscreen = fullscreen.clone();
        if !self.window.is_null() {
            unsafe {
                let uiscreen = match fullscreen {
                    Some(Fullscreen::Exclusive(video_mode)) => {
                        let uiscreen = video_mode.video_mode.monitor.ui_screen() as id;
                        let () =
                            msg_send![uiscreen, setCurrentMode: video_mode.video_mode.screen_mode];
                        uiscreen
                    }
                    Some(Fullscreen::Borderless(monitor)) => monitor.ui_screen() as id,
                    None => {
                        warn!("`Window::set_fullscreen(None)` ignored on iOS");
                        return;
                    }
                };

                // this is pretty slow on iOS, so avoid doing it if we can
                let current: id = msg_send![self.window, screen];
                if uiscreen != current {
                    let () = msg_send![self.window, setScreen: uiscreen];
                }

                let bounds: CGRect = msg_send![uiscreen, bounds];
                let () = msg_send![self.window, setFrame: bounds];

                // For external displays, we must disable overscan compensation or the displayed
                // image will have giant black bars surrounding it on each side
                let () = msg_send![
                    uiscreen,
                    setOverscanCompensation: UIScreenOverscanCompensation::None
                ];
            }
        }
    }

    pub fn fullscreen(&self) -> Option<Fullscreen> {
        self.window_attributes.borrow().fullscreen.clone()
    }

    pub fn set_decorations(&self, _decorations: bool) {
        warn!("`Window::set_decorations` is ignored on iOS")
    }

    pub fn set_always_on_top(&self, _always_on_top: bool) {
        warn!("`Window::set_always_on_top` is ignored on iOS")
    }

    pub fn set_window_icon(&self, _icon: Option<Icon>) {
        warn!("`Window::set_window_icon` is ignored on iOS")
    }

    pub fn set_ime_position(&self, _position: LogicalPosition) {
        warn!("`Window::set_ime_position` is ignored on iOS")
    }

    pub fn current_monitor(&self) -> RootMonitorHandle {
        if self.window.is_null() {
            match self.window_attributes.borrow().fullscreen {
                Some(Fullscreen::Exclusive(ref video_mode)) => video_mode.video_mode.monitor(),
                Some(Fullscreen::Borderless(ref monitor)) => monitor.clone(),
                None => RootMonitorHandle {
                    inner: self.primary_monitor(),
                },
            }
        } else {
            unsafe {
                let uiscreen: id = msg_send![self.window, screen];
                RootMonitorHandle {
                    inner: MonitorHandle::retained_new(uiscreen),
                }
            }
        }
    }

    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        unsafe { monitor::uiscreens() }
    }

    pub fn primary_monitor(&self) -> MonitorHandle {
        unsafe { monitor::main_uiscreen() }
    }

    pub fn id(&self) -> WindowId {
        self.window.into()
    }

    pub fn raw_window_handle(&self) -> RawWindowHandle {
        // Note that only `ui_view` will be available if this is called before the event loop is
        // running! On iOS windows and view controllers must not be created before
        // `UIApplicationMain` (called by winit in `EventLoop::run`). See also the comment regarding
        // deferred initialization in `Window::new`
        let handle = IOSHandle {
            ui_window: self.window as _,
            ui_view: self.view as _,
            ui_view_controller: self.view_controller as _,
            ..IOSHandle::empty()
        };
        RawWindowHandle::IOS(handle)
    }
}

pub struct Window {
    pub inner: Box<Inner>,
}

impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            assert_main_thread!("`Window::drop` can only be run on the main thread on iOS");
            AppState::cancel_deferred_window_init(&mut *self.inner);
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
    pub fn new<T>(
        event_loop: &EventLoopWindowTarget<T>,
        window_attributes: WindowAttributes,
        platform_attributes: PlatformSpecificWindowBuilderAttributes,
    ) -> Result<Window, RootOsError> {
        if let Some(_) = window_attributes.inner_size {
            warn!("`WindowAttributes::inner_size` is ignored on iOS");
        }
        if let Some(_) = window_attributes.min_inner_size {
            warn!("`WindowAttributes::min_inner_size` is ignored on iOS");
        }
        if let Some(_) = window_attributes.max_inner_size {
            warn!("`WindowAttributes::max_inner_size` is ignored on iOS");
        }
        if window_attributes.always_on_top {
            warn!("`WindowAttributes::always_on_top` is unsupported on iOS");
        }
        // TODO: transparency, visible
        let mut inner = Box::new(Inner {
            supports_safe_area: event_loop.capabilities().supports_safe_area,
            view: unsafe { view::create_view(&window_attributes, &platform_attributes) },
            window_attributes: RefCell::new(window_attributes),
            platform_attributes: RefCell::new(platform_attributes),
            // Window creation is (and must be) deferred until after `UIApplicationMain` is called,
            // because this is what UIKit expects internally, and not doing so will result in
            // serious problems, such as the window having an incorrect position, size, and
            // orientation. Additionally, the iOS 13.0 Simulator will crash upon receiving a touch
            // event.
            window: null_mut(),
            view_controller: null_mut(),
        });
        unsafe {
            AppState::defer_window_init(&mut *inner);
        }
        Ok(Window { inner })
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

    pub fn set_hidpi_factor(&self, hidpi_factor: f64) {
        unsafe {
            assert!(
                dpi::validate_hidpi_factor(hidpi_factor),
                "`WindowExtIOS::set_hidpi_factor` received an invalid hidpi factor"
            );
            let hidpi_factor = hidpi_factor as CGFloat;
            let () = msg_send![self.view, setContentScaleFactor: hidpi_factor];
        }
    }

    pub fn set_valid_orientations(&self, valid_orientations: ValidOrientations) {
        self.platform_attributes.borrow_mut().valid_orientations = valid_orientations;
        if !self.view_controller.is_null() {
            unsafe {
                let idiom = event_loop::get_idiom();
                let supported_orientations =
                    UIInterfaceOrientationMask::from_valid_orientations_idiom(
                        valid_orientations,
                        idiom,
                    );
                msg_send![
                    self.view_controller,
                    setSupportedInterfaceOrientations: supported_orientations
                ];
            }
        }
    }

    pub fn set_prefers_home_indicator_hidden(&self, hidden: bool) {
        self.platform_attributes
            .borrow_mut()
            .prefers_home_indicator_hidden = hidden;
        if !self.view_controller.is_null() {
            unsafe {
                let prefers_home_indicator_hidden = if hidden { YES } else { NO };
                let () = msg_send![
                    self.view_controller,
                    setPrefersHomeIndicatorAutoHidden: prefers_home_indicator_hidden
                ];
            }
        }
    }

    pub fn set_preferred_screen_edges_deferring_system_gestures(&self, edges: ScreenEdge) {
        self.platform_attributes
            .borrow_mut()
            .preferred_screen_edges_deferring_system_gestures = edges;
        if !self.view_controller.is_null() {
            let edges: UIRectEdge = edges.into();
            unsafe {
                let () = msg_send![
                    self.view_controller,
                    setPreferredScreenEdgesDeferringSystemGestures: edges
                ];
            }
        }
    }

    pub fn set_prefers_status_bar_hidden(&self, hidden: bool) {
        self.platform_attributes
            .borrow_mut()
            .prefers_status_bar_hidden = hidden;
        if !self.view_controller.is_null() {
            unsafe {
                let status_bar_hidden = if hidden { YES } else { NO };
                let () = msg_send![
                    self.view_controller,
                    setPrefersStatusBarHidden: status_bar_hidden
                ];
            }
        }
    }

    pub fn safe_area_screen_space(&self) -> (LogicalPosition, LogicalSize) {
        assert!(
            !self.window.is_null(),
            "`safe_area_screen_space` cannot be called before `EventLoop::run` on iOS"
        );
        let rect = unsafe {
            if self.supports_safe_area {
                let window_frame: CGRect = msg_send![self.window, frame];
                let safe_area: UIEdgeInsets = msg_send![self.window, safeAreaInsets];
                CGRect {
                    origin: CGPoint {
                        x: window_frame.origin.x + safe_area.left,
                        y: window_frame.origin.y + safe_area.top,
                    },
                    size: CGSize {
                        width: window_frame.size.width - safe_area.left - safe_area.right,
                        height: window_frame.size.height - safe_area.top - safe_area.bottom,
                    },
                }
            } else {
                let mut window_frame: CGRect = msg_send![self.window, frame];
                let status_bar_frame: CGRect = {
                    let app: id = msg_send![class!(UIApplication), sharedApplication];
                    msg_send![app, statusBarFrame]
                };
                if window_frame.origin.y <= status_bar_frame.size.height {
                    window_frame.origin.y = status_bar_frame.size.height;
                    window_frame.size.height -=
                        status_bar_frame.size.height - window_frame.origin.y;
                }
                window_frame
            }
        };

        (
            LogicalPosition {
                x: rect.origin.x,
                y: rect.origin.y,
            },
            LogicalSize {
                width: rect.size.width,
                height: rect.size.height,
            },
        )
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId {
    window: id,
}

impl WindowId {
    pub unsafe fn dummy() -> Self {
        WindowId {
            window: std::ptr::null_mut(),
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
    pub hidpi_factor: Option<f64>,
    pub valid_orientations: ValidOrientations,
    pub prefers_home_indicator_hidden: bool,
    pub prefers_status_bar_hidden: bool,
    pub preferred_screen_edges_deferring_system_gestures: ScreenEdge,
}

impl Default for PlatformSpecificWindowBuilderAttributes {
    fn default() -> PlatformSpecificWindowBuilderAttributes {
        PlatformSpecificWindowBuilderAttributes {
            root_view_class: class!(UIView),
            hidpi_factor: None,
            valid_orientations: Default::default(),
            prefers_home_indicator_hidden: false,
            prefers_status_bar_hidden: false,
            preferred_screen_edges_deferring_system_gestures: Default::default(),
        }
    }
}
