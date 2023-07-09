use crate::dpi::{PhysicalPosition, PhysicalSize, Position, Size};
use crate::error::{ExternalError, NotSupportedError, OsError as RootOE};
use crate::icon::Icon;
use crate::window::{
    CursorGrabMode, CursorIcon, ImePurpose, ResizeDirection, Theme, UserAttentionType,
    WindowAttributes, WindowButtons, WindowId as RootWI, WindowLevel,
};

use raw_window_handle::{RawDisplayHandle, RawWindowHandle, WebDisplayHandle, WebWindowHandle};
use web_sys::HtmlCanvasElement;

use super::r#async::Dispatcher;
use super::{backend, monitor::MonitorHandle, EventLoopWindowTarget, Fullscreen};

use std::cell::RefCell;
use std::collections::vec_deque::IntoIter as VecDequeIter;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct Window {
    id: WindowId,
    has_focus: Arc<AtomicBool>,
    pub inner: Dispatcher<Inner>,
}

pub struct Inner {
    pub window: web_sys::Window,
    canvas: Rc<RefCell<backend::Canvas>>,
    previous_pointer: RefCell<&'static str>,
    register_redraw_request: Box<dyn Fn()>,
    destroy_fn: Option<Box<dyn FnOnce()>>,
}

impl Window {
    pub(crate) fn new<T>(
        target: &EventLoopWindowTarget<T>,
        attr: WindowAttributes,
        platform_attr: PlatformSpecificWindowBuilderAttributes,
    ) -> Result<Self, RootOE> {
        let runner = target.runner.clone();

        let id = target.generate_id();

        let prevent_default = platform_attr.prevent_default;

        let window = target.runner.window();
        let canvas = backend::Canvas::create(id, window.clone(), &attr, platform_attr)?;
        let canvas = Rc::new(RefCell::new(canvas));

        let register_redraw_request = Box::new(move || runner.request_redraw(RootWI(id)));

        target.register(&canvas, id, prevent_default);

        let runner = target.runner.clone();
        let destroy_fn = Box::new(move || runner.notify_destroy_window(RootWI(id)));

        let has_focus = canvas.borrow().has_focus.clone();
        let window = Window {
            id,
            has_focus,
            inner: Dispatcher::new(Inner {
                window: window.clone(),
                canvas,
                previous_pointer: RefCell::new("auto"),
                register_redraw_request,
                destroy_fn: Some(destroy_fn),
            })
            .unwrap(),
        };

        window.set_title(&attr.title);
        window.set_maximized(attr.maximized);
        window.set_visible(attr.visible);
        window.set_window_icon(attr.window_icon);

        Ok(window)
    }

    pub fn canvas(&self) -> Option<HtmlCanvasElement> {
        self.inner.with(|inner| inner.canvas.borrow().raw().clone())
    }

    pub fn set_title(&self, title: &str) {
        if self
            .inner
            .with(|inner| inner.canvas.borrow().set_attribute("alt", title))
            .is_none()
        {
            let title = title.to_owned();
            self.inner
                .dispatch(move |inner| inner.canvas.borrow().set_attribute("alt", &title));
        }
    }

    pub fn set_transparent(&self, _transparent: bool) {}

    pub fn set_visible(&self, _visible: bool) {
        // Intentionally a no-op
    }

    #[inline]
    pub fn is_visible(&self) -> Option<bool> {
        None
    }

    pub fn request_redraw(&self) {
        self.inner
            .dispatch(|inner| (inner.register_redraw_request)());
    }

    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        self.inner.queue(|inner| {
            Ok(inner
                .canvas
                .borrow()
                .position()
                .to_physical(inner.scale_factor()))
        })
    }

    pub fn inner_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        // Note: the canvas element has no window decorations, so this is equal to `outer_position`.
        self.outer_position()
    }

    pub fn set_outer_position(&self, position: Position) {
        self.inner.dispatch(move |inner| {
            let position = position.to_logical::<f64>(inner.scale_factor());

            let canvas = inner.canvas.borrow();
            canvas.set_attribute("position", "fixed");
            canvas.set_attribute("left", &position.x.to_string());
            canvas.set_attribute("top", &position.y.to_string());
        });
    }

    #[inline]
    pub fn inner_size(&self) -> PhysicalSize<u32> {
        self.inner.queue(|inner| inner.canvas.borrow().inner_size())
    }

    #[inline]
    pub fn outer_size(&self) -> PhysicalSize<u32> {
        // Note: the canvas element has no window decorations, so this is equal to `inner_size`.
        self.inner_size()
    }

    #[inline]
    pub fn set_inner_size(&self, size: Size) {
        self.inner.dispatch(move |inner| {
            let size = size.to_logical(inner.scale_factor());
            let canvas = inner.canvas.borrow();
            backend::set_canvas_size(canvas.window(), canvas.raw(), size);
        });
    }

    #[inline]
    pub fn set_min_inner_size(&self, _dimensions: Option<Size>) {
        // Intentionally a no-op: users can't resize canvas elements
    }

    #[inline]
    pub fn set_max_inner_size(&self, _dimensions: Option<Size>) {
        // Intentionally a no-op: users can't resize canvas elements
    }

    #[inline]
    pub fn resize_increments(&self) -> Option<PhysicalSize<u32>> {
        None
    }

    #[inline]
    pub fn set_resize_increments(&self, _increments: Option<Size>) {
        // Intentionally a no-op: users can't resize canvas elements
    }

    #[inline]
    pub fn set_resizable(&self, _resizable: bool) {
        // Intentionally a no-op: users can't resize canvas elements
    }

    pub fn is_resizable(&self) -> bool {
        true
    }

    #[inline]
    pub fn set_enabled_buttons(&self, _buttons: WindowButtons) {}

    #[inline]
    pub fn enabled_buttons(&self) -> WindowButtons {
        WindowButtons::all()
    }

    #[inline]
    pub fn scale_factor(&self) -> f64 {
        self.inner.queue(|inner| inner.scale_factor())
    }

    #[inline]
    pub fn set_cursor_icon(&self, cursor: CursorIcon) {
        self.inner.dispatch(move |inner| {
            *inner.previous_pointer.borrow_mut() = cursor.name();
            backend::set_canvas_style_property(
                inner.canvas.borrow().raw(),
                "cursor",
                cursor.name(),
            );
        });
    }

    #[inline]
    pub fn set_cursor_position(&self, _position: Position) -> Result<(), ExternalError> {
        Err(ExternalError::NotSupported(NotSupportedError::new()))
    }

    #[inline]
    pub fn set_cursor_grab(&self, mode: CursorGrabMode) -> Result<(), ExternalError> {
        self.inner.queue(move |inner| {
            let lock = match mode {
                CursorGrabMode::None => false,
                CursorGrabMode::Locked => true,
                CursorGrabMode::Confined => {
                    return Err(ExternalError::NotSupported(NotSupportedError::new()))
                }
            };

            inner
                .canvas
                .borrow()
                .set_cursor_lock(lock)
                .map_err(ExternalError::Os)
        })
    }

    #[inline]
    pub fn set_cursor_visible(&self, visible: bool) {
        self.inner.dispatch(move |inner| {
            if !visible {
                inner.canvas.borrow().set_attribute("cursor", "none");
            } else {
                inner
                    .canvas
                    .borrow()
                    .set_attribute("cursor", &inner.previous_pointer.borrow());
            }
        });
    }

    #[inline]
    pub fn drag_window(&self) -> Result<(), ExternalError> {
        Err(ExternalError::NotSupported(NotSupportedError::new()))
    }

    #[inline]
    pub fn drag_resize_window(&self, _direction: ResizeDirection) -> Result<(), ExternalError> {
        Err(ExternalError::NotSupported(NotSupportedError::new()))
    }

    #[inline]
    pub fn set_cursor_hittest(&self, _hittest: bool) -> Result<(), ExternalError> {
        Err(ExternalError::NotSupported(NotSupportedError::new()))
    }

    #[inline]
    pub fn set_minimized(&self, _minimized: bool) {
        // Intentionally a no-op, as canvases cannot be 'minimized'
    }

    #[inline]
    pub fn is_minimized(&self) -> Option<bool> {
        // Canvas cannot be 'minimized'
        Some(false)
    }

    #[inline]
    pub fn set_maximized(&self, _maximized: bool) {
        // Intentionally a no-op, as canvases cannot be 'maximized'
    }

    #[inline]
    pub fn is_maximized(&self) -> bool {
        // Canvas cannot be 'maximized'
        false
    }

    #[inline]
    pub(crate) fn fullscreen(&self) -> Option<Fullscreen> {
        self.inner.queue(|inner| {
            if inner.canvas.borrow().is_fullscreen() {
                Some(Fullscreen::Borderless(Some(MonitorHandle)))
            } else {
                None
            }
        })
    }

    #[inline]
    pub(crate) fn set_fullscreen(&self, fullscreen: Option<Fullscreen>) {
        self.inner.dispatch(move |inner| {
            if fullscreen.is_some() {
                inner.canvas.borrow().request_fullscreen();
            } else if inner.canvas.borrow().is_fullscreen() {
                backend::exit_fullscreen(&inner.window);
            }
        });
    }

    #[inline]
    pub fn set_decorations(&self, _decorations: bool) {
        // Intentionally a no-op, no canvas decorations
    }

    pub fn is_decorated(&self) -> bool {
        true
    }

    #[inline]
    pub fn set_window_level(&self, _level: WindowLevel) {
        // Intentionally a no-op, no window ordering
    }

    #[inline]
    pub fn set_window_icon(&self, _window_icon: Option<Icon>) {
        // Currently an intentional no-op
    }

    #[inline]
    pub fn set_ime_cursor_area(&self, _position: Position, _size: Size) {
        // Currently a no-op as it does not seem there is good support for this on web
    }

    #[inline]
    pub fn set_ime_allowed(&self, _allowed: bool) {
        // Currently not implemented
    }

    #[inline]
    pub fn set_ime_purpose(&self, _purpose: ImePurpose) {
        // Currently not implemented
    }

    #[inline]
    pub fn focus_window(&self) {
        // Currently a no-op as it does not seem there is good support for this on web
    }

    #[inline]
    pub fn request_user_attention(&self, _request_type: Option<UserAttentionType>) {
        // Currently an intentional no-op
    }

    #[inline]
    pub fn current_monitor(&self) -> Option<MonitorHandle> {
        Some(MonitorHandle)
    }

    #[inline]
    pub fn available_monitors(&self) -> VecDequeIter<MonitorHandle> {
        VecDeque::new().into_iter()
    }

    #[inline]
    pub fn primary_monitor(&self) -> Option<MonitorHandle> {
        Some(MonitorHandle)
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        self.id
    }

    #[inline]
    pub fn raw_window_handle(&self) -> RawWindowHandle {
        let mut window_handle = WebWindowHandle::empty();
        window_handle.id = self.id.0;
        RawWindowHandle::Web(window_handle)
    }

    #[inline]
    pub fn raw_display_handle(&self) -> RawDisplayHandle {
        RawDisplayHandle::Web(WebDisplayHandle::empty())
    }

    #[inline]
    pub fn set_theme(&self, _theme: Option<Theme>) {}

    #[inline]
    pub fn theme(&self) -> Option<Theme> {
        self.inner.queue(|inner| {
            backend::is_dark_mode(&inner.window).map(|is_dark_mode| {
                if is_dark_mode {
                    Theme::Dark
                } else {
                    Theme::Light
                }
            })
        })
    }

    #[inline]
    pub fn has_focus(&self) -> bool {
        self.has_focus.load(Ordering::Relaxed)
    }

    pub fn title(&self) -> String {
        String::new()
    }

    pub fn reset_dead_keys(&self) {
        // Not supported
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        self.inner.dispatch_mut(|inner| {
            if let Some(destroy_fn) = inner.destroy_fn.take() {
                destroy_fn();
            }
        });
    }
}

impl Inner {
    #[inline]
    pub fn scale_factor(&self) -> f64 {
        super::backend::scale_factor(&self.window)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(pub(crate) u32);

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
        Self(raw_id as u32)
    }
}

#[derive(Clone)]
pub struct PlatformSpecificWindowBuilderAttributes {
    pub(crate) canvas: Option<backend::RawCanvasType>,
    pub(crate) prevent_default: bool,
    pub(crate) focusable: bool,
}

impl Default for PlatformSpecificWindowBuilderAttributes {
    fn default() -> Self {
        Self {
            canvas: None,
            prevent_default: true,
            focusable: true,
        }
    }
}
