use crate::dpi::{LogicalSize, PhysicalPosition, PhysicalSize, Position, Size};
use crate::error::{ExternalError, NotSupportedError, OsError as RootOE};
use crate::event;
use crate::icon::Icon;
use crate::window::{
    CursorGrabMode, CursorIcon, ImePurpose, ResizeDirection, Theme, UserAttentionType,
    WindowAttributes, WindowButtons, WindowId as RootWI, WindowLevel,
};

use raw_window_handle::{RawDisplayHandle, RawWindowHandle, WebDisplayHandle, WebWindowHandle};

use super::{backend, monitor::MonitorHandle, EventLoopWindowTarget, Fullscreen};

use std::cell::{Ref, RefCell};
use std::collections::vec_deque::IntoIter as VecDequeIter;
use std::collections::VecDeque;
use std::rc::Rc;

pub struct Window {
    canvas: Rc<RefCell<backend::Canvas>>,
    previous_pointer: RefCell<&'static str>,
    id: WindowId,
    register_redraw_request: Box<dyn Fn()>,
    resize_notify_fn: Box<dyn Fn(PhysicalSize<u32>)>,
    destroy_fn: Option<Box<dyn FnOnce()>>,
    has_focus: Rc<RefCell<bool>>,
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

        let canvas = backend::Canvas::create(platform_attr)?;
        let canvas = Rc::new(RefCell::new(canvas));

        let register_redraw_request = Box::new(move || runner.request_redraw(RootWI(id)));

        let has_focus = Rc::new(RefCell::new(false));
        target.register(&canvas, id, prevent_default, has_focus.clone());

        let runner = target.runner.clone();
        let resize_notify_fn = Box::new(move |new_size| {
            runner.send_event(event::Event::WindowEvent {
                window_id: RootWI(id),
                event: event::WindowEvent::Resized(new_size),
            });
        });

        let runner = target.runner.clone();
        let destroy_fn = Box::new(move || runner.notify_destroy_window(RootWI(id)));

        let window = Window {
            canvas,
            previous_pointer: RefCell::new("auto"),
            id,
            register_redraw_request,
            resize_notify_fn,
            destroy_fn: Some(destroy_fn),
            has_focus,
        };

        backend::set_canvas_size(
            window.canvas.borrow().raw(),
            attr.inner_size.unwrap_or(Size::Logical(LogicalSize {
                width: 1024.0,
                height: 768.0,
            })),
        );
        window.set_title(&attr.title);
        window.set_maximized(attr.maximized);
        window.set_visible(attr.visible);
        window.set_window_icon(attr.window_icon);

        Ok(window)
    }

    pub fn canvas(&self) -> Ref<'_, backend::Canvas> {
        self.canvas.borrow()
    }

    pub fn set_title(&self, title: &str) {
        self.canvas.borrow().set_attribute("alt", title);
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
        (self.register_redraw_request)();
    }

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
        let position = position.to_logical::<f64>(self.scale_factor());

        let canvas = self.canvas.borrow();
        canvas.set_attribute("position", "fixed");
        canvas.set_attribute("left", &position.x.to_string());
        canvas.set_attribute("top", &position.y.to_string());
    }

    #[inline]
    pub fn inner_size(&self) -> PhysicalSize<u32> {
        self.canvas.borrow().size()
    }

    #[inline]
    pub fn outer_size(&self) -> PhysicalSize<u32> {
        // Note: the canvas element has no window decorations, so this is equal to `inner_size`.
        self.inner_size()
    }

    #[inline]
    pub fn set_inner_size(&self, size: Size) {
        let old_size = self.inner_size();
        backend::set_canvas_size(self.canvas.borrow().raw(), size);
        let new_size = self.inner_size();
        if old_size != new_size {
            (self.resize_notify_fn)(new_size);
        }
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
        super::backend::scale_factor()
    }

    #[inline]
    pub fn set_cursor_icon(&self, cursor: CursorIcon) {
        let text = match cursor {
            CursorIcon::Default => "auto",
            CursorIcon::Crosshair => "crosshair",
            CursorIcon::Hand => "pointer",
            CursorIcon::Arrow => "default",
            CursorIcon::Move => "move",
            CursorIcon::Text => "text",
            CursorIcon::Wait => "wait",
            CursorIcon::Help => "help",
            CursorIcon::Progress => "progress",

            CursorIcon::NotAllowed => "not-allowed",
            CursorIcon::ContextMenu => "context-menu",
            CursorIcon::Cell => "cell",
            CursorIcon::VerticalText => "vertical-text",
            CursorIcon::Alias => "alias",
            CursorIcon::Copy => "copy",
            CursorIcon::NoDrop => "no-drop",
            CursorIcon::Grab => "grab",
            CursorIcon::Grabbing => "grabbing",
            CursorIcon::AllScroll => "all-scroll",
            CursorIcon::ZoomIn => "zoom-in",
            CursorIcon::ZoomOut => "zoom-out",

            CursorIcon::EResize => "e-resize",
            CursorIcon::NResize => "n-resize",
            CursorIcon::NeResize => "ne-resize",
            CursorIcon::NwResize => "nw-resize",
            CursorIcon::SResize => "s-resize",
            CursorIcon::SeResize => "se-resize",
            CursorIcon::SwResize => "sw-resize",
            CursorIcon::WResize => "w-resize",
            CursorIcon::EwResize => "ew-resize",
            CursorIcon::NsResize => "ns-resize",
            CursorIcon::NeswResize => "nesw-resize",
            CursorIcon::NwseResize => "nwse-resize",
            CursorIcon::ColResize => "col-resize",
            CursorIcon::RowResize => "row-resize",
        };
        *self.previous_pointer.borrow_mut() = text;
        backend::set_canvas_style_property(self.canvas.borrow().raw(), "cursor", text);
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
            self.canvas.borrow().set_attribute("cursor", "none");
        } else {
            self.canvas
                .borrow()
                .set_attribute("cursor", &self.previous_pointer.borrow());
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
            Some(Fullscreen::Borderless(Some(self.current_monitor_inner())))
        } else {
            None
        }
    }

    #[inline]
    pub(crate) fn set_fullscreen(&self, fullscreen: Option<Fullscreen>) {
        if fullscreen.is_some() {
            self.canvas.borrow().request_fullscreen();
        } else if self.canvas.borrow().is_fullscreen() {
            backend::exit_fullscreen();
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
    pub fn set_ime_position(&self, _position: Position) {
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
    // Allow directly accessing the current monitor internally without unwrapping.
    fn current_monitor_inner(&self) -> MonitorHandle {
        MonitorHandle
    }

    #[inline]
    pub fn current_monitor(&self) -> Option<MonitorHandle> {
        Some(self.current_monitor_inner())
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
        web_sys::window()
            .and_then(|window| {
                window
                    .match_media("(prefers-color-scheme: dark)")
                    .ok()
                    .flatten()
            })
            .map(|media_query_list| {
                if media_query_list.matches() {
                    Theme::Dark
                } else {
                    Theme::Light
                }
            })
    }

    #[inline]
    pub fn has_focus(&self) -> bool {
        *self.has_focus.borrow()
    }

    pub fn title(&self) -> String {
        String::new()
    }
}

impl Drop for Window {
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
