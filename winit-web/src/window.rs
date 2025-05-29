use std::cell::Ref;
use std::fmt;
use std::rc::Rc;

use dpi::{
    LogicalInsets, LogicalPosition, LogicalSize, PhysicalInsets, PhysicalPosition, PhysicalSize,
    Position, Size,
};
use web_sys::HtmlCanvasElement;
use winit_core::cursor::Cursor;
use winit_core::error::{NotSupportedError, RequestError};
use winit_core::icon::Icon;
use winit_core::impl_surface_downcast;
use winit_core::monitor::{Fullscreen, MonitorHandle as CoremMonitorHandle};
use winit_core::window::{
    CursorGrabMode, ImePurpose, ResizeDirection, Theme, UserAttentionType, Surface as RootSurface, 
    Window as RootWindow, WindowAttributes, WindowButtons, SurfaceId, WindowLevel,
};

use crate::event_loop::ActiveEventLoop;
use crate::main_thread::MainThreadMarker;
use crate::monitor::MonitorHandler;
use crate::r#async::Dispatcher;
use crate::{backend, lock};

pub struct Window {
    inner: Dispatcher<Inner>,
}

impl fmt::Debug for Window {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Window").finish_non_exhaustive()
    }
}

pub struct Inner {
    id: SurfaceId,
    pub window: web_sys::Window,
    monitor: Rc<MonitorHandler>,
    safe_area: Rc<backend::SafeAreaHandle>,
    canvas: Rc<backend::Canvas>,
    destroy_fn: Option<Box<dyn FnOnce()>>,
}

impl Window {
    pub(crate) fn new(
        target: &ActiveEventLoop,
        attr: WindowAttributes,
    ) -> Result<Self, RequestError> {
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
        let destroy_fn = Box::new(move || runner.notify_destroy_window(id));

        let inner = Inner {
            id,
            window: window.clone(),
            monitor: Rc::clone(target.runner.monitor()),
            safe_area: Rc::clone(target.runner.safe_area()),
            canvas,
            destroy_fn: Some(destroy_fn),
        };

        let canvas = Rc::downgrade(&inner.canvas);
        let (dispatcher, runner) = Dispatcher::new(target.runner.main_thread(), inner);
        target.runner.add_canvas(id, canvas, runner);

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

impl RootSurface for Window {
    impl_surface_downcast!(Window);
    
    fn id(&self) -> SurfaceId {
        self.inner.queue(|inner| inner.id)
    }

    fn scale_factor(&self) -> f64 {
        self.inner.queue(Inner::scale_factor)
    }

    fn request_redraw(&self) {
        self.inner.dispatch(|inner| inner.canvas.request_animation_frame())
    }

    fn pre_present_notify(&self) {}

    fn surface_size(&self) -> PhysicalSize<u32> {
        self.inner.queue(|inner| inner.canvas.surface_size())
    }

    fn request_surface_size(&self, size: Size) -> Option<PhysicalSize<u32>> {
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

    fn set_transparent(&self, _: bool) {}

    fn set_cursor(&self, cursor: Cursor) {
        self.inner.dispatch(move |inner| inner.canvas.cursor.set_cursor(cursor))
    }

    fn set_cursor_position(&self, _: Position) -> Result<(), RequestError> {
        Err(NotSupportedError::new("set_cursor_position is not supported").into())
    }

    fn set_cursor_grab(&self, mode: CursorGrabMode) -> Result<(), RequestError> {
        Ok(self.inner.queue(|inner| {
            match mode {
                CursorGrabMode::None => inner.canvas.document().exit_pointer_lock(),
                CursorGrabMode::Locked => lock::request_pointer_lock(
                    inner.canvas.navigator(),
                    inner.canvas.document(),
                    inner.canvas.raw(),
                ),
                CursorGrabMode::Confined => {
                    return Err(NotSupportedError::new("confined cursor mode is not supported"))
                },
            }

            Ok(())
        })?)
    }

    fn set_cursor_visible(&self, visible: bool) {
        self.inner.dispatch(move |inner| inner.canvas.cursor.set_cursor_visible(visible))
    }

    fn set_cursor_hittest(&self, _: bool) -> Result<(), RequestError> {
        Err(NotSupportedError::new("set_cursor_hittest is not supported").into())
    }

    fn current_monitor(&self) -> Option<CoremMonitorHandle> {
        Some(self.inner.queue(|inner| inner.monitor.current_monitor()).into())
    }

    fn available_monitors(&self) -> Box<dyn Iterator<Item = CoremMonitorHandle>> {
        Box::new(
            self.inner
                .queue(|inner| inner.monitor.available_monitors())
                .into_iter()
                .map(CoremMonitorHandle::from),
        )
    }

    fn primary_monitor(&self) -> Option<CoremMonitorHandle> {
        self.inner.queue(|inner| inner.monitor.primary_monitor()).map(CoremMonitorHandle::from)
    }

    fn rwh_06_display_handle(&self) -> &dyn rwh_06::HasDisplayHandle {
        self
    }

    fn rwh_06_window_handle(&self) -> &dyn rwh_06::HasWindowHandle {
        self
    }
}

impl RootWindow for Window {

    fn reset_dead_keys(&self) {
        // Not supported
    }

    fn surface_position(&self) -> PhysicalPosition<i32> {
        // Note: the canvas element has no window decorations.
        (0, 0).into()
    }

    fn outer_position(&self) -> Result<PhysicalPosition<i32>, RequestError> {
        Ok(self.inner.queue(|inner| inner.canvas.position().to_physical(inner.scale_factor())))
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

    fn outer_size(&self) -> PhysicalSize<u32> {
        // Note: the canvas element has no window decorations, so this is equal to `surface_size`.
        self.surface_size()
    }

    fn safe_area(&self) -> PhysicalInsets<u32> {
        self.inner.queue(|inner| {
            let (safe_start_pos, safe_size) = inner.safe_area.get();
            let safe_end_pos = LogicalPosition::new(
                safe_start_pos.x + safe_size.width,
                safe_start_pos.y + safe_size.height,
            );

            let surface_start_pos = inner.canvas.position();
            let surface_size = LogicalSize::new(
                backend::style_size_property(inner.canvas.style(), "width"),
                backend::style_size_property(inner.canvas.style(), "height"),
            );
            let surface_end_pos = LogicalPosition::new(
                surface_start_pos.x + surface_size.width,
                surface_start_pos.y + surface_size.height,
            );

            let top = f64::max(safe_start_pos.y - surface_start_pos.y, 0.);
            let left = f64::max(safe_start_pos.x - surface_start_pos.x, 0.);
            let bottom = f64::max(surface_end_pos.y - safe_end_pos.y, 0.);
            let right = f64::max(surface_end_pos.x - safe_end_pos.x, 0.);

            let insets = LogicalInsets::new(top, left, bottom, right);
            insets.to_physical(inner.scale_factor())
        })
    }

    fn set_min_surface_size(&self, min_size: Option<Size>) {
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

    fn set_max_surface_size(&self, max_size: Option<Size>) {
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

    fn surface_resize_increments(&self) -> Option<PhysicalSize<u32>> {
        None
    }

    fn set_surface_resize_increments(&self, _: Option<Size>) {
        // Intentionally a no-op: users can't resize canvas elements
    }

    fn set_title(&self, title: &str) {
        self.inner.queue(|inner| inner.canvas.set_attribute("alt", title))
    }

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

    fn set_fullscreen(&self, fullscreen: Option<Fullscreen>) {
        self.inner.dispatch(move |inner| {
            if let Some(fullscreen) = fullscreen {
                inner.canvas.request_fullscreen(fullscreen);
            } else {
                inner.canvas.exit_fullscreen()
            }
        })
    }

    fn fullscreen(&self) -> Option<Fullscreen> {
        self.inner.queue(|inner| {
            if inner.canvas.is_fullscreen() {
                Some(Fullscreen::Borderless(None))
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

    fn drag_window(&self) -> Result<(), RequestError> {
        Err(NotSupportedError::new("drag_window is not supported").into())
    }

    fn drag_resize_window(&self, _: ResizeDirection) -> Result<(), RequestError> {
        Err(NotSupportedError::new("drag_resize_window is not supported").into())
    }

    fn show_window_menu(&self, _: Position) {}
}

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
