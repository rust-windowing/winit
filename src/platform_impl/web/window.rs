use std::cell::Ref;
use std::rc::Rc;
use std::sync::Arc;

use web_sys::HtmlCanvasElement;

use super::main_thread::{MainThreadMarker, MainThreadSafe};
use super::monitor::{MonitorHandle, MonitorHandler};
use super::r#async::Dispatcher;
use super::{backend, lock, ActiveEventLoop, Fullscreen};
use crate::dpi::{PhysicalPosition, PhysicalSize, Position, Size};
use crate::error::{ExternalError, NotSupportedError, OsError as RootOE};
use crate::icon::Icon;
use crate::window::{
    Cursor, CursorGrabMode, ImePurpose, ResizeDirection, Theme, UserAttentionType,
    WindowAttributes, WindowButtons, WindowId as RootWI, WindowLevel,
};

pub struct Window {
    inner: Dispatcher<Inner>,
}

pub struct Inner {
    id: WindowId,
    pub window: web_sys::Window,
    monitor: Rc<MonitorHandler>,
    canvas: Rc<backend::Canvas>,
    destroy_fn: Option<Box<dyn FnOnce()>>,
}

impl Window {
    pub(crate) fn new(target: &ActiveEventLoop, attr: WindowAttributes) -> Result<Self, RootOE> {
        let id = target.generate_id();

        let window = target.runner.window();
        let navigator = target.runner.navigator();
        let document = target.runner.document();
        let canvas = backend::Canvas::create(
            target.runner.main_thread(),
            id,
            window.clone(),
            navigator.clone(),
            document.clone(),
            attr,
        )?;
        let canvas = Rc::new(canvas);

        target.register(&canvas, id);

        let runner = target.runner.clone();
        let destroy_fn = Box::new(move || runner.notify_destroy_window(RootWI(id)));

        let inner = Inner {
            id,
            window: window.clone(),
            monitor: Rc::clone(target.runner.monitor()),
            canvas,
            destroy_fn: Some(destroy_fn),
        };

        let canvas = Rc::downgrade(&inner.canvas);
        let (dispatcher, runner) = Dispatcher::new(target.runner.main_thread(), inner);
        target.runner.add_canvas(RootWI(id), canvas, runner);

        Ok(Window { inner: dispatcher })
    }

    pub(crate) fn maybe_queue_on_main(&self, f: impl FnOnce(&Inner) + Send + 'static) {
        self.inner.dispatch(f)
    }

    pub(crate) fn maybe_wait_on_main<R: Send>(&self, f: impl FnOnce(&Inner) -> R + Send) -> R {
        self.inner.queue(f)
    }

    pub fn canvas(&self) -> Option<Ref<'_, HtmlCanvasElement>> {
        MainThreadMarker::new()
            .map(|main_thread| Ref::map(self.inner.value(main_thread), |inner| inner.canvas.raw()))
    }

    pub(crate) fn prevent_default(&self) -> bool {
        self.inner.queue(|inner| inner.canvas.prevent_default.get())
    }

    pub(crate) fn set_prevent_default(&self, prevent_default: bool) {
        self.inner.dispatch(move |inner| inner.canvas.prevent_default.set(prevent_default))
    }

    pub(crate) fn is_cursor_lock_raw(&self) -> bool {
        self.inner.queue(move |inner| {
            lock::is_cursor_lock_raw(inner.canvas.navigator(), inner.canvas.document())
        })
    }

    #[cfg(feature = "rwh_06")]
    pub fn raw_window_handle_rwh_06(&self) -> Result<rwh_06::RawWindowHandle, rwh_06::HandleError> {
        MainThreadMarker::new()
            .map(|main_thread| {
                let inner = self.inner.value(main_thread);
                // SAFETY: This will only work if the reference to `HtmlCanvasElement` stays valid.
                let canvas: &wasm_bindgen::JsValue = inner.canvas.raw();
                let window_handle =
                    rwh_06::WebCanvasWindowHandle::new(std::ptr::NonNull::from(canvas).cast());
                rwh_06::RawWindowHandle::WebCanvas(window_handle)
            })
            .ok_or(rwh_06::HandleError::Unavailable)
    }

    #[cfg(feature = "rwh_06")]
    pub(crate) fn raw_display_handle_rwh_06(
        &self,
    ) -> Result<rwh_06::RawDisplayHandle, rwh_06::HandleError> {
        Ok(rwh_06::RawDisplayHandle::Web(rwh_06::WebDisplayHandle::new()))
    }
}

impl Inner {
    pub fn set_title(&self, title: &str) {
        self.canvas.set_attribute("alt", title)
    }

    pub fn set_transparent(&self, _transparent: bool) {}

    pub fn set_blur(&self, _blur: bool) {}

    pub fn set_visible(&self, _visible: bool) {
        // Intentionally a no-op
    }

    pub fn is_visible(&self) -> Option<bool> {
        None
    }

    pub fn request_redraw(&self) {
        self.canvas.request_animation_frame();
    }

    pub fn pre_present_notify(&self) {}

    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        Ok(self.canvas.position().to_physical(self.scale_factor()))
    }

    pub fn inner_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        // Note: the canvas element has no window decorations, so this is equal to `outer_position`.
        self.outer_position()
    }

    pub fn set_outer_position(&self, position: Position) {
        let position = position.to_logical::<f64>(self.scale_factor());

        backend::set_canvas_position(
            self.canvas.document(),
            self.canvas.raw(),
            self.canvas.style(),
            position,
        )
    }

    pub fn inner_size(&self) -> PhysicalSize<u32> {
        self.canvas.inner_size()
    }

    pub fn outer_size(&self) -> PhysicalSize<u32> {
        // Note: the canvas element has no window decorations, so this is equal to `inner_size`.
        self.inner_size()
    }

    pub fn request_inner_size(&self, size: Size) -> Option<PhysicalSize<u32>> {
        let size = size.to_logical(self.scale_factor());
        backend::set_canvas_size(
            self.canvas.document(),
            self.canvas.raw(),
            self.canvas.style(),
            size,
        );
        None
    }

    pub fn set_min_inner_size(&self, dimensions: Option<Size>) {
        let dimensions = dimensions.map(|dimensions| dimensions.to_logical(self.scale_factor()));
        backend::set_canvas_min_size(
            self.canvas.document(),
            self.canvas.raw(),
            self.canvas.style(),
            dimensions,
        )
    }

    pub fn set_max_inner_size(&self, dimensions: Option<Size>) {
        let dimensions = dimensions.map(|dimensions| dimensions.to_logical(self.scale_factor()));
        backend::set_canvas_max_size(
            self.canvas.document(),
            self.canvas.raw(),
            self.canvas.style(),
            dimensions,
        )
    }

    pub fn resize_increments(&self) -> Option<PhysicalSize<u32>> {
        None
    }

    pub fn set_resize_increments(&self, _increments: Option<Size>) {
        // Intentionally a no-op: users can't resize canvas elements
    }

    pub fn set_resizable(&self, _resizable: bool) {
        // Intentionally a no-op: users can't resize canvas elements
    }

    pub fn is_resizable(&self) -> bool {
        true
    }

    pub fn set_enabled_buttons(&self, _buttons: WindowButtons) {}

    pub fn enabled_buttons(&self) -> WindowButtons {
        WindowButtons::all()
    }

    pub fn scale_factor(&self) -> f64 {
        super::backend::scale_factor(&self.window)
    }

    pub fn set_cursor(&self, cursor: Cursor) {
        self.canvas.cursor.set_cursor(cursor)
    }

    pub fn set_cursor_position(&self, _position: Position) -> Result<(), ExternalError> {
        Err(ExternalError::NotSupported(NotSupportedError::new()))
    }

    pub fn set_cursor_grab(&self, mode: CursorGrabMode) -> Result<(), ExternalError> {
        match mode {
            CursorGrabMode::None => self.canvas.document().exit_pointer_lock(),
            CursorGrabMode::Locked => lock::request_pointer_lock(
                self.canvas.navigator(),
                self.canvas.document(),
                self.canvas.raw(),
            ),
            CursorGrabMode::Confined => {
                return Err(ExternalError::NotSupported(NotSupportedError::new()))
            },
        }

        Ok(())
    }

    pub fn set_cursor_visible(&self, visible: bool) {
        self.canvas.cursor.set_cursor_visible(visible)
    }

    pub fn drag_window(&self) -> Result<(), ExternalError> {
        Err(ExternalError::NotSupported(NotSupportedError::new()))
    }

    pub fn drag_resize_window(&self, _direction: ResizeDirection) -> Result<(), ExternalError> {
        Err(ExternalError::NotSupported(NotSupportedError::new()))
    }

    pub fn show_window_menu(&self, _position: Position) {}

    pub fn set_cursor_hittest(&self, _hittest: bool) -> Result<(), ExternalError> {
        Err(ExternalError::NotSupported(NotSupportedError::new()))
    }

    pub fn set_minimized(&self, _minimized: bool) {
        // Intentionally a no-op, as canvases cannot be 'minimized'
    }

    pub fn is_minimized(&self) -> Option<bool> {
        // Canvas cannot be 'minimized'
        Some(false)
    }

    pub fn set_maximized(&self, _maximized: bool) {
        // Intentionally a no-op, as canvases cannot be 'maximized'
    }

    pub fn is_maximized(&self) -> bool {
        // Canvas cannot be 'maximized'
        false
    }

    pub(crate) fn fullscreen(&self) -> Option<Fullscreen> {
        if self.canvas.is_fullscreen() {
            Some(Fullscreen::Borderless(Some(self.monitor.current_monitor())))
        } else {
            None
        }
    }

    pub(crate) fn set_fullscreen(&self, fullscreen: Option<Fullscreen>) {
        if let Some(fullscreen) = fullscreen {
            self.canvas.request_fullscreen(fullscreen);
        } else {
            self.canvas.exit_fullscreen()
        }
    }

    pub fn set_decorations(&self, _decorations: bool) {
        // Intentionally a no-op, no canvas decorations
    }

    pub fn is_decorated(&self) -> bool {
        true
    }

    pub fn set_window_level(&self, _level: WindowLevel) {
        // Intentionally a no-op, no window ordering
    }

    pub fn set_window_icon(&self, _window_icon: Option<Icon>) {
        // Currently an intentional no-op
    }

    pub fn set_ime_cursor_area(&self, _position: Position, _size: Size) {
        // Currently not implemented
    }

    pub fn set_ime_allowed(&self, _allowed: bool) {
        // Currently not implemented
    }

    pub fn set_ime_purpose(&self, _purpose: ImePurpose) {
        // Currently not implemented
    }

    pub fn focus_window(&self) {
        let _ = self.canvas.raw().focus();
    }

    pub fn request_user_attention(&self, _request_type: Option<UserAttentionType>) {
        // Currently an intentional no-op
    }

    pub fn current_monitor(&self) -> Option<MonitorHandle> {
        Some(self.monitor.current_monitor())
    }

    pub fn available_monitors(&self) -> Vec<MonitorHandle> {
        self.monitor.available_monitors()
    }

    pub fn primary_monitor(&self) -> Option<MonitorHandle> {
        self.monitor.primary_monitor()
    }

    pub fn id(&self) -> WindowId {
        self.id
    }

    pub fn set_theme(&self, _theme: Option<Theme>) {}

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

    pub fn has_focus(&self) -> bool {
        self.canvas.has_focus.get()
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
    pub const fn dummy() -> Self {
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

#[derive(Clone, Debug)]
pub struct PlatformSpecificWindowAttributes {
    pub(crate) canvas: Option<Arc<MainThreadSafe<backend::RawCanvasType>>>,
    pub(crate) prevent_default: bool,
    pub(crate) focusable: bool,
    pub(crate) append: bool,
}

impl PartialEq for PlatformSpecificWindowAttributes {
    fn eq(&self, other: &Self) -> bool {
        (match (&self.canvas, &other.canvas) {
            (Some(this), Some(other)) => Arc::ptr_eq(this, other),
            (None, None) => true,
            _ => false,
        }) && self.prevent_default.eq(&other.prevent_default)
            && self.focusable.eq(&other.focusable)
            && self.append.eq(&other.append)
    }
}

impl PlatformSpecificWindowAttributes {
    pub(crate) fn set_canvas(&mut self, canvas: Option<backend::RawCanvasType>) {
        let Some(canvas) = canvas else {
            self.canvas = None;
            return;
        };

        let main_thread = MainThreadMarker::new()
            .expect("received a `HtmlCanvasElement` outside the window context");

        self.canvas = Some(Arc::new(MainThreadSafe::new(main_thread, canvas)));
    }
}

impl Default for PlatformSpecificWindowAttributes {
    fn default() -> Self {
        Self { canvas: None, prevent_default: true, focusable: true, append: false }
    }
}
