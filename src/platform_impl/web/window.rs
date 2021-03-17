use crate::dpi::{LogicalSize, PhysicalPosition, PhysicalSize, Position, Size};
use crate::error::{ExternalError, NotSupportedError, OsError as RootOE};
use crate::event;
use crate::icon::Icon;
use crate::monitor::MonitorHandle as RootMH;
use crate::window::{
    CursorIcon, Fullscreen, UserAttentionType, WindowAttributes, WindowId as RootWI,
};

use raw_window_handle::{RawWindowHandle, WebHandle};

use super::{backend, monitor, EventLoopWindowTarget};

use std::cell::{Ref, RefCell};
use std::collections::vec_deque::IntoIter as VecDequeIter;
use std::collections::VecDeque;
use std::rc::Rc;

pub struct Window {
    canvas: Rc<RefCell<backend::Canvas>>,
    previous_pointer: RefCell<&'static str>,
    id: Id,
    register_redraw_request: Box<dyn Fn()>,
    resize_notify_fn: Box<dyn Fn(PhysicalSize<u32>)>,
    destroy_fn: Option<Box<dyn FnOnce()>>,
}

impl Window {
    pub fn new<T>(
        target: &EventLoopWindowTarget<T>,
        attr: WindowAttributes,
        platform_attr: PlatformSpecificBuilderAttributes,
    ) -> Result<Self, RootOE> {
        let runner = target.runner.clone();

        let id = target.generate_id();

        let canvas = backend::Canvas::create(platform_attr)?;
        let mut canvas = Rc::new(RefCell::new(canvas));

        let register_redraw_request = Box::new(move || runner.request_redraw(RootWI(id)));

        target.register(&mut canvas, id);

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

    pub fn canvas<'a>(&'a self) -> Ref<'a, backend::Canvas> {
        self.canvas.borrow()
    }

    pub fn set_title(&self, title: &str) {
        self.canvas.borrow().set_attribute("alt", title);
    }

    pub fn set_visible(&self, _visible: bool) {
        // Intentionally a no-op
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
    pub fn set_resizable(&self, _resizable: bool) {
        // Intentionally a no-op: users can't resize canvas elements
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
    pub fn set_cursor_grab(&self, grab: bool) -> Result<(), ExternalError> {
        self.canvas
            .borrow()
            .set_cursor_grab(grab)
            .map_err(|e| ExternalError::Os(e))
    }

    #[inline]
    pub fn set_cursor_visible(&self, visible: bool) {
        if !visible {
            self.canvas.borrow().set_attribute("cursor", "none");
        } else {
            self.canvas
                .borrow()
                .set_attribute("cursor", *self.previous_pointer.borrow());
        }
    }

    #[inline]
    pub fn drag_window(&self) -> Result<(), ExternalError> {
        Err(ExternalError::NotSupported(NotSupportedError::new()))
    }

    #[inline]
    pub fn set_minimized(&self, _minimized: bool) {
        // Intentionally a no-op, as canvases cannot be 'minimized'
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
    pub fn fullscreen(&self) -> Option<Fullscreen> {
        if self.canvas.borrow().is_fullscreen() {
            Some(Fullscreen::Borderless(Some(self.current_monitor_inner())))
        } else {
            None
        }
    }

    #[inline]
    pub fn set_fullscreen(&self, monitor: Option<Fullscreen>) {
        if monitor.is_some() {
            self.canvas.borrow().request_fullscreen();
        } else if self.canvas.borrow().is_fullscreen() {
            backend::exit_fullscreen();
        }
    }

    #[inline]
    pub fn set_decorations(&self, _decorations: bool) {
        // Intentionally a no-op, no canvas decorations
    }

    #[inline]
    pub fn set_always_on_top(&self, _always_on_top: bool) {
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
    pub fn focus_window(&self) {
        // Currently a no-op as it does not seem there is good support for this on web
    }

    #[inline]
    pub fn request_user_attention(&self, _request_type: Option<UserAttentionType>) {
        // Currently an intentional no-op
    }

    #[inline]
    // Allow directly accessing the current monitor internally without unwrapping.
    fn current_monitor_inner(&self) -> RootMH {
        RootMH {
            inner: monitor::Handle,
        }
    }

    #[inline]
    pub fn current_monitor(&self) -> Option<RootMH> {
        Some(self.current_monitor_inner())
    }

    #[inline]
    pub fn available_monitors(&self) -> VecDequeIter<monitor::Handle> {
        VecDeque::new().into_iter()
    }

    #[inline]
    pub fn primary_monitor(&self) -> Option<RootMH> {
        Some(RootMH {
            inner: monitor::Handle,
        })
    }

    #[inline]
    pub fn id(&self) -> Id {
        return self.id;
    }

    #[inline]
    pub fn raw_window_handle(&self) -> RawWindowHandle {
        let mut handle = WebHandle::empty();
        handle.id = self.id.0;
        RawWindowHandle::Web(handle)
    }

    pub fn reset_dead_keys(&self) {
        // Not supported
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
pub struct Id(pub(crate) u32);

impl Id {
    pub const unsafe fn dummy() -> Id {
        Id(0)
    }
}

#[derive(Default, Clone)]
pub struct PlatformSpecificBuilderAttributes {
    pub(crate) canvas: Option<backend::RawCanvasType>,
}
