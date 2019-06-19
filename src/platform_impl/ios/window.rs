use std::{
    collections::VecDeque,
    ops::{Deref, DerefMut},
};

use objc::runtime::{Class, NO, Object, YES};

use crate::dpi::{self, LogicalPosition, LogicalSize};
use crate::error::{ExternalError, NotSupportedError, OsError as RootOsError};
use crate::icon::Icon;
use crate::monitor::MonitorHandle as RootMonitorHandle;
use crate::platform::ios::{MonitorHandleExtIOS, ValidOrientations};
use crate::window::{
    CursorIcon,
    WindowAttributes,
};
use crate::platform_impl::{
    platform::{
        app_state::AppState,
        event_loop,
        ffi::{
            id,
            CGFloat,
            CGPoint,
            CGRect,
            CGSize,
            UIEdgeInsets,
            UIInterfaceOrientationMask,
        },
        monitor,
        view,
        EventLoopWindowTarget,
        MonitorHandle
    },
};

pub struct Inner {
    pub window: id,
    pub view_controller: id,
    pub view: id,
    supports_safe_area: bool,
}

impl Drop for Inner {
    fn drop(&mut self) {
        unsafe {
            let () = msg_send![self.view, release];
            let () = msg_send![self.view_controller, release];
            let () = msg_send![self.window, release];
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
                let () = msg_send![self.window, setHidden:NO];
            },
            false => unsafe {
                let () = msg_send![self.window, setHidden:YES];
            },
        }
    }

    pub fn request_redraw(&self) {
        unsafe {
            let () = msg_send![self.view, setNeedsDisplay];
        }
    }

    pub fn inner_position(&self) -> Result<LogicalPosition, NotSupportedError> {
        unsafe {
            let safe_area = self.safe_area_screen_space();
            Ok(LogicalPosition {
                x: safe_area.origin.x,
                y: safe_area.origin.y,
            })
        }
    }

    pub fn outer_position(&self) -> Result<LogicalPosition, NotSupportedError> {
        unsafe {
            let screen_frame = self.screen_frame();
            Ok(LogicalPosition {
                x: screen_frame.origin.x,
                y: screen_frame.origin.y,
            })
        }
    }

    pub fn set_outer_position(&self, position: LogicalPosition) {
        unsafe {
            let screen_frame = self.screen_frame();
            let new_screen_frame = CGRect {
                origin: CGPoint {
                    x: position.x as _,
                    y: position.y as _,
                },
                size: screen_frame.size,
            };
            let bounds = self.from_screen_space(new_screen_frame);
            let () = msg_send![self.window, setBounds:bounds];
        }
    }

    pub fn inner_size(&self) -> LogicalSize {
        unsafe {
            let safe_area = self.safe_area_screen_space();
            LogicalSize {
                width: safe_area.size.width,
                height: safe_area.size.height,
            }
        }
    }

    pub fn outer_size(&self) -> LogicalSize {
        unsafe {
            let screen_frame = self.screen_frame();
            LogicalSize {
                width: screen_frame.size.width,
                height: screen_frame.size.height,
            }
        }
    }

    pub fn set_inner_size(&self, _size: LogicalSize) {
        unimplemented!("not clear what `Window::set_inner_size` means on iOS");
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

    pub fn set_fullscreen(&self, monitor: Option<RootMonitorHandle>) {
        unsafe {
            match monitor {
                Some(monitor) => {
                    let uiscreen = monitor.ui_screen() as id;
                    let current: id = msg_send![self.window, screen];
                    let bounds: CGRect = msg_send![uiscreen, bounds];

                    // this is pretty slow on iOS, so avoid doing it if we can
                    if uiscreen != current {
                        let () = msg_send![self.window, setScreen:uiscreen];
                    }
                    let () = msg_send![self.window, setFrame:bounds];
                }
                None => warn!("`Window::set_fullscreen(None)` ignored on iOS"),
            }
        }
    }

    pub fn fullscreen(&self) -> Option<RootMonitorHandle> {
        unsafe {
            let monitor = self.current_monitor();
            let uiscreen = monitor.inner.ui_screen();
            let screen_space_bounds = self.screen_frame();
            let screen_bounds: CGRect = msg_send![uiscreen, bounds];

            // TODO: track fullscreen instead of relying on brittle float comparisons
            if screen_space_bounds.origin.x == screen_bounds.origin.x
                && screen_space_bounds.origin.y == screen_bounds.origin.y
                && screen_space_bounds.size.width == screen_bounds.size.width
                && screen_space_bounds.size.height == screen_bounds.size.height
            {
                Some(monitor)
            } else {
                None
            }
        }
    }

    pub fn set_decorations(&self, decorations: bool) {
        unsafe {
            let status_bar_hidden = if decorations { NO } else { YES };
            let () = msg_send![self.view_controller, setPrefersStatusBarHidden:status_bar_hidden];
        }
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
        unsafe {
            let uiscreen: id = msg_send![self.window, screen];
            RootMonitorHandle { inner: MonitorHandle::retained_new(uiscreen) }
        }
    }

    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        unsafe {
            monitor::uiscreens()
        }
    }

    pub fn primary_monitor(&self) -> MonitorHandle {
        unsafe {
            monitor::main_uiscreen()
        }
    }

    pub fn id(&self) -> WindowId {
        self.window.into()
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
    pub fn new<T>(
        event_loop: &EventLoopWindowTarget<T>,
        window_attributes: WindowAttributes,
        platform_attributes: PlatformSpecificWindowBuilderAttributes,
    ) -> Result<Window, RootOsError> {
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

        unsafe {
            let screen = window_attributes.fullscreen
                .as_ref()
                .map(|screen| screen.ui_screen() as _)
                .unwrap_or_else(|| monitor::main_uiscreen().ui_screen());
            let screen_bounds: CGRect = msg_send![screen, bounds];

            let frame = match window_attributes.inner_size {
                Some(dim) => CGRect {
                    origin: screen_bounds.origin,
                    size: CGSize { width: dim.width, height: dim.height },
                },
                None => screen_bounds,
            };

            let view = view::create_view(&window_attributes, &platform_attributes, frame.clone());
            let view_controller = view::create_view_controller(&window_attributes, &platform_attributes, view);
            let window = view::create_window(&window_attributes, &platform_attributes, frame, view_controller);

            let supports_safe_area = event_loop.capabilities().supports_safe_area;

            let result = Window {
                inner: Inner {
                    window,
                    view_controller,
                    view,
                    supports_safe_area,
                },
            };
            AppState::set_key_window(window);
            Ok(result)
        }
    }
}

// WindowExtIOS
impl Inner {
    pub fn ui_window(&self) -> id { self.window }
    pub fn ui_view_controller(&self) -> id { self.view_controller }
    pub fn ui_view(&self) -> id { self.view }

    pub fn set_hidpi_factor(&self, hidpi_factor: f64) {
        unsafe {
            assert!(dpi::validate_hidpi_factor(hidpi_factor), "`WindowExtIOS::set_hidpi_factor` received an invalid hidpi factor");
            let hidpi_factor = hidpi_factor as CGFloat;
            let () = msg_send![self.view, setContentScaleFactor:hidpi_factor];
        }
    }

    pub fn set_valid_orientations(&self, valid_orientations: ValidOrientations) {
        unsafe {
            let idiom = event_loop::get_idiom();
            let supported_orientations = UIInterfaceOrientationMask::from_valid_orientations_idiom(valid_orientations, idiom);
            msg_send![self.view_controller, setSupportedInterfaceOrientations:supported_orientations];
        }
    }
}

impl Inner {
    // requires main thread
    unsafe fn screen_frame(&self) -> CGRect {
        self.to_screen_space(msg_send![self.window, bounds])
    }

    // requires main thread
    unsafe fn to_screen_space(&self, rect: CGRect) -> CGRect {
        let screen: id = msg_send![self.window, screen];
        if !screen.is_null() {
            let screen_space: id = msg_send![screen, coordinateSpace];
            msg_send![self.window, convertRect:rect toCoordinateSpace:screen_space]
        } else {
            rect
        }
    }

    // requires main thread
    unsafe fn from_screen_space(&self, rect: CGRect) -> CGRect {
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
        if self.supports_safe_area {
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
            self.to_screen_space(safe_bounds)
        } else {
            let screen_frame = self.to_screen_space(bounds);
            let status_bar_frame: CGRect = {
                let app: id = msg_send![class!(UIApplication), sharedApplication];
                assert!(!app.is_null(), "`Window::get_inner_position` cannot be called before `EventLoop::run` on iOS");
                msg_send![app, statusBarFrame]
            };
            let (y, height) = if screen_frame.origin.y > status_bar_frame.size.height {
                (screen_frame.origin.y, screen_frame.size.height)
            } else {
                let y = status_bar_frame.size.height;
                let height = screen_frame.size.height - (status_bar_frame.size.height - screen_frame.origin.y);
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
                }
            }
        }
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
        WindowId { window: window as *const _ as _ }
    }
}

impl From<&mut Object> for WindowId {
    fn from(window: &mut Object) -> WindowId {
        WindowId { window: window as _ }
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
}

impl Default for PlatformSpecificWindowBuilderAttributes {
    fn default() -> PlatformSpecificWindowBuilderAttributes {
        PlatformSpecificWindowBuilderAttributes {
            root_view_class: class!(UIView),
            hidpi_factor: None,
            valid_orientations: Default::default(),
        }
    }
}
