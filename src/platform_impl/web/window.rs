use std::cell::Ref;
use std::rc::Rc;
use std::sync::Arc;

use web_sys::HtmlCanvasElement;

use super::main_thread::{MainThreadMarker, MainThreadSafe};
use super::monitor::MonitorHandler;
use super::r#async::Dispatcher;
use super::{backend, lock, ActiveEventLoop};
use crate::dpi::{PhysicalPosition, PhysicalSize, Position, Size};
use crate::error::{ExternalError, NotSupportedError, OsError as RootOE};
use crate::icon::Icon;
use crate::monitor::MonitorHandle as RootMonitorHandle;
use crate::window::{
    Cursor, CursorGrabMode, Fullscreen as RootFullscreen, ImePurpose, ResizeDirection, Theme,
    UserAttentionType, Window as RootWindow, WindowAttributes, WindowButtons, WindowId as RootWI,
    WindowLevel,
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
}

impl RootWindow for Window {
    fn id(&self) -> RootWI {
        RootWI(self.inner.queue(|inner| inner.id))
    }

    fn scale_factor(&self) -> f64 {
        self.inner.queue(Inner::scale_factor)
    }

    fn request_redraw(&self) {
        self.inner.dispatch(|inner| inner.canvas.request_animation_frame())
    }

    fn pre_present_notify(&self) {}

    fn reset_dead_keys(&self) {
        // Not supported
    }

    fn inner_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        // Note: the canvas element has no window decorations, so this is equal to `outer_position`.
        self.outer_position()
    }

    fn outer_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        self.inner.queue(|inner| Ok(inner.canvas.position().to_physical(inner.scale_factor())))
    }

    fn set_outer_position(&self, position: Position) {
        self.inner.dispatch(move |inner| {
            let position = position.to_logical::<f64>(inner.scale_factor());
            backend::set_canvas_position(
                inner.canvas.document(),
                inner.canvas.raw(),
                inner.canvas.style(),
                position,
            )
        })
    }

    fn inner_size(&self) -> PhysicalSize<u32> {
        self.inner.queue(|inner| inner.canvas.inner_size())
    }

    fn request_inner_size(&self, size: Size) -> Option<PhysicalSize<u32>> {
        self.inner.queue(|inner| {
            let size = size.to_logical(self.scale_factor());
            backend::set_canvas_size(
                inner.canvas.document(),
                inner.canvas.raw(),
                inner.canvas.style(),
                size,
            );
            None
        })
    }

    fn outer_size(&self) -> PhysicalSize<u32> {
        // Note: the canvas element has no window decorations, so this is equal to `inner_size`.
        self.inner_size()
    }

    fn set_min_inner_size(&self, min_size: Option<Size>) {
        self.inner.dispatch(move |inner| {
            let dimensions = min_size.map(|min_size| min_size.to_logical(inner.scale_factor()));
            backend::set_canvas_min_size(
                inner.canvas.document(),
                inner.canvas.raw(),
                inner.canvas.style(),
                dimensions,
            )
        })
    }

    fn set_max_inner_size(&self, max_size: Option<Size>) {
        self.inner.dispatch(move |inner| {
            let dimensions = max_size.map(|dimensions| dimensions.to_logical(inner.scale_factor()));
            backend::set_canvas_max_size(
                inner.canvas.document(),
                inner.canvas.raw(),
                inner.canvas.style(),
                dimensions,
            )
        })
    }

    fn resize_increments(&self) -> Option<PhysicalSize<u32>> {
        None
    }

    fn set_resize_increments(&self, _: Option<Size>) {
        // Intentionally a no-op: users can't resize canvas elements
    }

    fn set_title(&self, title: &str) {
        self.inner.queue(|inner| inner.canvas.set_attribute("alt", title))
    }

    fn set_transparent(&self, _: bool) {}

    fn set_blur(&self, _: bool) {}

    fn set_visible(&self, _: bool) {
        // Intentionally a no-op
    }

    fn is_visible(&self) -> Option<bool> {
        None
    }

    fn set_resizable(&self, _: bool) {
        // Intentionally a no-op: users can't resize canvas elements
    }

    fn is_resizable(&self) -> bool {
        true
    }

    fn set_enabled_buttons(&self, _: WindowButtons) {}

    fn enabled_buttons(&self) -> WindowButtons {
        WindowButtons::all()
    }

    fn set_minimized(&self, _: bool) {
        // Intentionally a no-op, as canvases cannot be 'minimized'
    }

    fn is_minimized(&self) -> Option<bool> {
        // Canvas cannot be 'minimized'
        Some(false)
    }

    fn set_maximized(&self, _: bool) {
        // Intentionally a no-op, as canvases cannot be 'maximized'
    }

    fn is_maximized(&self) -> bool {
        // Canvas cannot be 'maximized'
        false
    }

    fn set_fullscreen(&self, fullscreen: Option<RootFullscreen>) {
        self.inner.dispatch(move |inner| {
            if let Some(fullscreen) = fullscreen {
                inner.canvas.request_fullscreen(fullscreen.into());
            } else {
                inner.canvas.exit_fullscreen()
            }
        })
    }

    fn fullscreen(&self) -> Option<RootFullscreen> {
        self.inner.queue(|inner| {
            if inner.canvas.is_fullscreen() {
                Some(RootFullscreen::Borderless(None))
            } else {
                None
            }
        })
    }

    fn set_decorations(&self, _: bool) {
        // Intentionally a no-op, no canvas decorations
    }

    fn is_decorated(&self) -> bool {
        true
    }

    fn set_window_level(&self, _: WindowLevel) {
        // Intentionally a no-op, no window ordering
    }

    fn set_window_icon(&self, _: Option<Icon>) {
        // Currently an intentional no-op
    }

    fn set_ime_cursor_area(&self, _: Position, _: Size) {
        // Currently not implemented
    }

    fn set_ime_allowed(&self, _: bool) {
        // Currently not implemented
    }

    fn set_ime_purpose(&self, _: ImePurpose) {
        // Currently not implemented
    }

    fn focus_window(&self) {
        self.inner.dispatch(|inner| {
            let _ = inner.canvas.raw().focus();
        })
    }

    fn has_focus(&self) -> bool {
        self.inner.queue(|inner| inner.canvas.has_focus.get())
    }

    fn request_user_attention(&self, _: Option<UserAttentionType>) {
        // Currently an intentional no-op
    }

    fn set_theme(&self, _: Option<Theme>) {}

    fn theme(&self) -> Option<Theme> {
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

    fn set_content_protected(&self, _: bool) {}

    fn title(&self) -> String {
        String::new()
    }

    fn set_cursor(&self, cursor: Cursor) {
        self.inner.dispatch(move |inner| inner.canvas.cursor.set_cursor(cursor))
    }

    fn set_cursor_position(&self, _: Position) -> Result<(), ExternalError> {
        Err(ExternalError::NotSupported(NotSupportedError::new()))
    }

    fn set_cursor_grab(&self, mode: CursorGrabMode) -> Result<(), ExternalError> {
        self.inner.queue(|inner| {
            match mode {
                CursorGrabMode::None => inner.canvas.document().exit_pointer_lock(),
                CursorGrabMode::Locked => lock::request_pointer_lock(
                    inner.canvas.navigator(),
                    inner.canvas.document(),
                    inner.canvas.raw(),
                ),
                CursorGrabMode::Confined => {
                    return Err(ExternalError::NotSupported(NotSupportedError::new()))
                },
            }

            Ok(())
        })
    }

    fn set_cursor_visible(&self, visible: bool) {
        self.inner.dispatch(move |inner| inner.canvas.cursor.set_cursor_visible(visible))
    }

    fn drag_window(&self) -> Result<(), ExternalError> {
        Err(ExternalError::NotSupported(NotSupportedError::new()))
    }

    fn drag_resize_window(&self, _: ResizeDirection) -> Result<(), ExternalError> {
        Err(ExternalError::NotSupported(NotSupportedError::new()))
    }

    fn show_window_menu(&self, _: Position) {}

    fn set_cursor_hittest(&self, _: bool) -> Result<(), ExternalError> {
        Err(ExternalError::NotSupported(NotSupportedError::new()))
    }

    fn current_monitor(&self) -> Option<RootMonitorHandle> {
        Some(self.inner.queue(|inner| inner.monitor.current_monitor()).into())
    }

    fn available_monitors(&self) -> Box<dyn Iterator<Item = RootMonitorHandle>> {
        Box::new(
            self.inner
                .queue(|inner| inner.monitor.available_monitors())
                .into_iter()
                .map(RootMonitorHandle::from),
        )
    }

    fn primary_monitor(&self) -> Option<RootMonitorHandle> {
        self.inner.queue(|inner| inner.monitor.primary_monitor()).map(RootMonitorHandle::from)
    }

    #[cfg(feature = "rwh_06")]
    fn rwh_06_display_handle(&self) -> &dyn rwh_06::HasDisplayHandle {
        self
    }

    #[cfg(feature = "rwh_06")]
    fn rwh_06_window_handle(&self) -> &dyn rwh_06::HasWindowHandle {
        self
    }
}

#[cfg(feature = "rwh_06")]
impl rwh_06::HasWindowHandle for Window {
    fn window_handle(&self) -> Result<rwh_06::WindowHandle<'_>, rwh_06::HandleError> {
        MainThreadMarker::new()
            .map(|main_thread| {
                let inner = self.inner.value(main_thread);
                // SAFETY: This will only work if the reference to `HtmlCanvasElement` stays valid.
                let canvas: &wasm_bindgen::JsValue = inner.canvas.raw();
                let window_handle =
                    rwh_06::WebCanvasWindowHandle::new(std::ptr::NonNull::from(canvas).cast());
                // SAFETY: The pointer won't be invalidated as long as `Window` lives, which the
                // lifetime is bound to.
                unsafe {
                    rwh_06::WindowHandle::borrow_raw(rwh_06::RawWindowHandle::WebCanvas(
                        window_handle,
                    ))
                }
            })
            .ok_or(rwh_06::HandleError::Unavailable)
    }
}

#[cfg(feature = "rwh_06")]
impl rwh_06::HasDisplayHandle for Window {
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        Ok(rwh_06::DisplayHandle::web())
    }
}

impl Inner {
    #[inline]
    pub fn scale_factor(&self) -> f64 {
        super::backend::scale_factor(&self.window)
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
