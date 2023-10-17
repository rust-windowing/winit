use crate::dpi::{PhysicalPosition, PhysicalSize, Position, Size};
use crate::error::{ExternalError, NotSupportedError, OsError as RootOE};
use crate::icon::Icon;
use crate::window::{
    CursorGrabMode, CursorIcon, ImePurpose, ResizeDirection, Theme, UserAttentionType,
    WindowAttributes, WindowButtons, WindowId as RootWI, WindowLevel,
};
use crate::SendSyncWrapper;

use web_sys::HtmlCanvasElement;

use super::r#async::Dispatcher;
use super::{backend, monitor::MonitorHandle, EventLoopWindowTarget, Fullscreen};

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

pub struct Window {
    inner: Dispatcher<Inner>,
}

pub struct Inner {
    id: WindowId,
    pub window: web_sys::Window,
    canvas: Rc<RefCell<backend::Canvas>>,
    previous_pointer: RefCell<&'static str>,
    destroy_fn: Option<Box<dyn FnOnce()>>,
}

impl Window {
    pub(crate) fn new<T>(
        target: &EventLoopWindowTarget<T>,
        attr: WindowAttributes,
        platform_attr: PlatformSpecificWindowBuilderAttributes,
    ) -> Result<Self, RootOE> {
        let id = target.generate_id();

        let prevent_default = platform_attr.prevent_default;

        let window = target.runner.window();
        let document = target.runner.document();
        let canvas =
            backend::Canvas::create(id, window.clone(), document.clone(), &attr, platform_attr)?;
        let canvas = Rc::new(RefCell::new(canvas));

        target.register(&canvas, id, prevent_default);

        let runner = target.runner.clone();
        let destroy_fn = Box::new(move || runner.notify_destroy_window(RootWI(id)));

        let inner = Inner {
            id,
            window: window.clone(),
            canvas,
            previous_pointer: RefCell::new("auto"),
            destroy_fn: Some(destroy_fn),
        };

        inner.set_title(&attr.title);
        inner.set_maximized(attr.maximized);
        inner.set_visible(attr.visible);
        inner.set_window_icon(attr.window_icon);

        let canvas = Rc::downgrade(&inner.canvas);
        let (dispatcher, runner) = Dispatcher::new(inner).unwrap();
        target.runner.add_canvas(RootWI(id), canvas, runner);

        Ok(Window { inner: dispatcher })
    }

    pub(crate) fn maybe_queue_on_main(&self, f: impl FnOnce(&Inner) + Send + 'static) {
        self.inner.dispatch(f)
    }

    pub(crate) fn maybe_wait_on_main<R: Send>(&self, f: impl FnOnce(&Inner) -> R + Send) -> R {
        self.inner.queue(f)
    }

    pub fn canvas(&self) -> Option<HtmlCanvasElement> {
        self.inner
            .value()
            .map(|inner| inner.canvas.borrow().raw().clone())
    }
}

impl Inner {
    pub fn set_title(&self, title: &str) {
        self.canvas.borrow().set_attribute("alt", title)
    }

    pub fn set_transparent(&self, _transparent: bool) {}

    pub fn set_blur(&self, _blur: bool) {}

    pub fn set_visible(&self, _visible: bool) {
        // Intentionally a no-op
    }

    #[inline]
    pub fn is_visible(&self) -> Option<bool> {
        None
    }

    pub fn request_redraw(&self) {
        self.canvas.borrow().request_animation_frame();
    }

    pub fn pre_present_notify(&self) {}

    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        Ok(self
            .canvas
            .borrow()
            .position()
            .to_physical(self.scale_factor()))
    }

    pub fn inner_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        // Note: the canvas element has no window decorations, so this is equal to `outer_position`.
        self.outer_position()
    }

    pub fn set_outer_position(&self, position: Position) {
        let canvas = self.canvas.borrow();
        let position = position.to_logical::<f64>(self.scale_factor());

        backend::set_canvas_position(canvas.document(), canvas.raw(), canvas.style(), position)
    }

    #[inline]
    pub fn inner_size(&self) -> PhysicalSize<u32> {
        self.canvas.borrow().inner_size()
    }

    #[inline]
    pub fn outer_size(&self) -> PhysicalSize<u32> {
        // Note: the canvas element has no window decorations, so this is equal to `inner_size`.
        self.inner_size()
    }

    #[inline]
    pub fn request_inner_size(&self, size: Size) -> Option<PhysicalSize<u32>> {
        let size = size.to_logical(self.scale_factor());
        let canvas = self.canvas.borrow();
        backend::set_canvas_size(canvas.document(), canvas.raw(), canvas.style(), size);
        None
    }

    #[inline]
    pub fn set_min_inner_size(&self, dimensions: Option<Size>) {
        let dimensions = dimensions.map(|dimensions| dimensions.to_logical(self.scale_factor()));
        let canvas = self.canvas.borrow();
        backend::set_canvas_min_size(canvas.document(), canvas.raw(), canvas.style(), dimensions)
    }

    #[inline]
    pub fn set_max_inner_size(&self, dimensions: Option<Size>) {
        let dimensions = dimensions.map(|dimensions| dimensions.to_logical(self.scale_factor()));
        let canvas = self.canvas.borrow();
        backend::set_canvas_max_size(canvas.document(), canvas.raw(), canvas.style(), dimensions)
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
        super::backend::scale_factor(&self.window)
    }

    #[inline]
    pub fn set_cursor_icon(&self, cursor: CursorIcon) {
        *self.previous_pointer.borrow_mut() = cursor.name();
        backend::set_canvas_style_property(self.canvas.borrow().raw(), "cursor", cursor.name());
    }

    #[inline]
    pub fn set_cursor_position(&self, _position: Position) -> Result<(), ExternalError> {
        Err(ExternalError::NotSupported(NotSupportedError::new()))
    }

    #[inline]
    pub fn set_cursor_grab(&self, mode: CursorGrabMode) -> Result<(), ExternalError> {
        let lock = match mode {
            CursorGrabMode::None => false,
            CursorGrabMode::Locked => true,
            CursorGrabMode::Confined => {
                return Err(ExternalError::NotSupported(NotSupportedError::new()))
            }
        };

        self.canvas
            .borrow()
            .set_cursor_lock(lock)
            .map_err(ExternalError::Os)
    }

    #[inline]
    pub fn set_cursor_visible(&self, visible: bool) {
        if !visible {
            backend::set_canvas_style_property(self.canvas.borrow().raw(), "cursor", "none");
        } else {
            backend::set_canvas_style_property(
                self.canvas.borrow().raw(),
                "cursor",
                &self.previous_pointer.borrow(),
            );
        }
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
    pub fn show_window_menu(&self, _position: Position) {}

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
        if self.canvas.borrow().is_fullscreen() {
            Some(Fullscreen::Borderless(None))
        } else {
            None
        }
    }

    #[inline]
    pub(crate) fn set_fullscreen(&self, fullscreen: Option<Fullscreen>) {
        let canvas = &self.canvas.borrow();

        if fullscreen.is_some() {
            canvas.request_fullscreen();
        } else {
            canvas.exit_fullscreen()
        }
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
        let _ = self.canvas.borrow().raw().focus();
    }

    #[inline]
    pub fn request_user_attention(&self, _request_type: Option<UserAttentionType>) {
        // Currently an intentional no-op
    }

    #[inline]
    pub fn current_monitor(&self) -> Option<MonitorHandle> {
        None
    }

    #[inline]
    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        VecDeque::new()
    }

    #[inline]
    pub fn primary_monitor(&self) -> Option<MonitorHandle> {
        None
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        self.id
    }

    #[cfg(feature = "rwh_04")]
    #[inline]
    pub fn raw_window_handle_rwh_04(&self) -> rwh_04::RawWindowHandle {
        let mut window_handle = rwh_04::WebHandle::empty();
        window_handle.id = self.id.0;
        rwh_04::RawWindowHandle::Web(window_handle)
    }

    #[cfg(feature = "rwh_05")]
    #[inline]
    pub fn raw_window_handle_rwh_05(&self) -> rwh_05::RawWindowHandle {
        let mut window_handle = rwh_05::WebWindowHandle::empty();
        window_handle.id = self.id.0;
        rwh_05::RawWindowHandle::Web(window_handle)
    }

    #[cfg(feature = "rwh_05")]
    #[inline]
    pub fn raw_display_handle_rwh_05(&self) -> rwh_05::RawDisplayHandle {
        rwh_05::RawDisplayHandle::Web(rwh_05::WebDisplayHandle::empty())
    }

    #[cfg(feature = "rwh_06")]
    #[inline]
    pub fn raw_window_handle_rwh_06(&self) -> Result<rwh_06::RawWindowHandle, rwh_06::HandleError> {
        let window_handle = rwh_06::WebWindowHandle::new(self.id.0);
        Ok(rwh_06::RawWindowHandle::Web(window_handle))
    }

    #[cfg(feature = "rwh_06")]
    #[inline]
    pub fn raw_display_handle_rwh_06(
        &self,
    ) -> Result<rwh_06::RawDisplayHandle, rwh_06::HandleError> {
        Ok(rwh_06::RawDisplayHandle::Web(
            rwh_06::WebDisplayHandle::new(),
        ))
    }

    #[inline]
    pub fn set_theme(&self, _theme: Option<Theme>) {}

    #[inline]
    pub fn theme(&self) -> Option<Theme> {
        backend::is_dark_mode(&self.window).map(|is_dark_mode| {
            if is_dark_mode {
                Theme::Dark
            } else {
                Theme::Light
            }
        })
    }

    pub fn set_content_protected(&self, _protected: bool) {}

    #[inline]
    pub fn has_focus(&self) -> bool {
        self.canvas.borrow().has_focus.get()
    }

    pub fn title(&self) -> String {
        String::new()
    }

    pub fn reset_dead_keys(&self) {
        // Not supported
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        if let Some(destroy_fn) = self.destroy_fn.take() {
            destroy_fn();
        }
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
    pub(crate) canvas: SendSyncWrapper<Option<backend::RawCanvasType>>,
    pub(crate) prevent_default: bool,
    pub(crate) focusable: bool,
    pub(crate) append: bool,
}

impl Default for PlatformSpecificWindowBuilderAttributes {
    fn default() -> Self {
        Self {
            canvas: SendSyncWrapper(None),
            prevent_default: true,
            focusable: true,
            append: false,
        }
    }
}
