use crate::dpi::{LogicalSize, PhysicalPosition, PhysicalSize, Position, Size};
use crate::error::{ExternalError, NotSupportedError, OsError as RootOE};
use crate::icon::Icon;
use crate::monitor::MonitorHandle as RootMH;
use crate::window::{CursorIcon, Fullscreen, WindowAttributes, WindowId as RootWI};

use raw_window_handle::web::WebHandle;

use super::{backend, monitor, EventLoopWindowTarget};

use std::cell::RefCell;
use std::collections::vec_deque::IntoIter as VecDequeIter;
use std::collections::VecDeque;

pub struct Window {
    canvas: backend::Canvas,
    previous_pointer: RefCell<&'static str>,
    id: Id,
    register_redraw_request: Box<dyn Fn()>,
}

impl Window {
    pub fn new<T>(
        target: &EventLoopWindowTarget<T>,
        attr: WindowAttributes,
        platform_attr: PlatformSpecificBuilderAttributes,
    ) -> Result<Self, RootOE> {
        let runner = target.runner.clone();

        let id = target.generate_id();

        let mut canvas = backend::Canvas::create(platform_attr)?;

        let register_redraw_request = Box::new(move || runner.request_redraw(RootWI(id)));

        target.register(&mut canvas, id);

        let window = Window {
            canvas,
            previous_pointer: RefCell::new("auto"),
            id,
            register_redraw_request,
        };

        window.set_inner_size(attr.inner_size.unwrap_or(Size::Logical(LogicalSize {
            width: 1024.0,
            height: 768.0,
        })));
        window.set_title(&attr.title);
        window.set_maximized(attr.maximized);
        window.set_visible(attr.visible);
        window.set_window_icon(attr.window_icon);

        Ok(window)
    }

    pub fn canvas(&self) -> &backend::Canvas {
        &self.canvas
    }

    pub fn set_title(&self, title: &str) {
        self.canvas.set_attribute("alt", title);
    }

    pub fn set_visible(&self, _visible: bool) {
        // Intentionally a no-op
    }

    pub fn request_redraw(&self) {
        (self.register_redraw_request)();
    }

    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        Ok(self.canvas.position().to_physical(self.scale_factor()))
    }

    pub fn inner_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        // Note: the canvas element has no window decorations, so this is equal to `outer_position`.
        self.outer_position()
    }

    pub fn set_outer_position(&self, position: Position) {
        let position = position.to_logical::<f64>(self.scale_factor());

        self.canvas.set_attribute("position", "fixed");
        self.canvas.set_attribute("left", &position.x.to_string());
        self.canvas.set_attribute("top", &position.y.to_string());
    }

    #[inline]
    pub fn inner_size(&self) -> PhysicalSize<u32> {
        self.canvas.size()
    }

    #[inline]
    pub fn outer_size(&self) -> PhysicalSize<u32> {
        // Note: the canvas element has no window decorations, so this is equal to `inner_size`.
        self.inner_size()
    }

    #[inline]
    pub fn set_inner_size(&self, size: Size) {
        backend::set_canvas_size(self.canvas.raw(), size);
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
        self.canvas
            .set_attribute("style", &format!("cursor: {}", text));
    }

    #[inline]
    pub fn set_cursor_position(&self, _position: Position) -> Result<(), ExternalError> {
        // Intentionally a no-op, as the web does not support setting cursor positions
        Ok(())
    }

    #[inline]
    pub fn set_cursor_grab(&self, _grab: bool) -> Result<(), ExternalError> {
        // Intentionally a no-op, as the web does not (properly) support grabbing the cursor
        Ok(())
    }

    #[inline]
    pub fn set_cursor_visible(&self, visible: bool) {
        if !visible {
            self.canvas.set_attribute("cursor", "none");
        } else {
            self.canvas
                .set_attribute("cursor", *self.previous_pointer.borrow());
        }
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
    pub fn fullscreen(&self) -> Option<Fullscreen> {
        if self.canvas.is_fullscreen() {
            Some(Fullscreen::Borderless(self.current_monitor()))
        } else {
            None
        }
    }

    #[inline]
    pub fn set_fullscreen(&self, monitor: Option<Fullscreen>) {
        if monitor.is_some() {
            self.canvas.request_fullscreen();
        } else if self.canvas.is_fullscreen() {
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
    pub fn current_monitor(&self) -> RootMH {
        RootMH {
            inner: monitor::Handle,
        }
    }

    #[inline]
    pub fn available_monitors(&self) -> VecDequeIter<monitor::Handle> {
        VecDeque::new().into_iter()
    }

    #[inline]
    pub fn primary_monitor(&self) -> monitor::Handle {
        monitor::Handle
    }

    #[inline]
    pub fn id(&self) -> Id {
        return self.id;
    }

    #[inline]
    pub fn raw_window_handle(&self) -> raw_window_handle::RawWindowHandle {
        let handle = WebHandle {
            id: self.id.0,
            ..WebHandle::empty()
        };

        raw_window_handle::RawWindowHandle::Web(handle)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(pub(crate) u32);

impl Id {
    pub unsafe fn dummy() -> Id {
        Id(0)
    }
}

#[derive(Default, Clone)]
pub struct PlatformSpecificBuilderAttributes {
    pub(crate) canvas: Option<backend::RawCanvasType>,
}
