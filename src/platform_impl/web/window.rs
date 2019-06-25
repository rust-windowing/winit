use crate::dpi::{LogicalPosition, LogicalSize};
use crate::error::{ExternalError, NotSupportedError, OsError as RootOE};
use crate::event::{Event, WindowEvent};
use crate::icon::Icon;
use crate::monitor::MonitorHandle as RootMH;
use crate::window::{CursorIcon, WindowAttributes, WindowId as RootWI};

use super::{backend, monitor, EventLoopWindowTarget};

use std::cell::RefCell;
use std::collections::vec_deque::IntoIter as VecDequeIter;
use std::collections::VecDeque;

pub struct Window {
    canvas: backend::Canvas,
    redraw: Box<dyn Fn()>,
    previous_pointer: RefCell<&'static str>,
    position: RefCell<LogicalPosition>,
}

impl Window {
    pub fn new<T>(
        target: &EventLoopWindowTarget<T>,
        attr: WindowAttributes,
        _: PlatformSpecificBuilderAttributes,
    ) -> Result<Self, RootOE> {
        let canvas = backend::Canvas::create()?;

        target.register(&canvas);

        let runner = target.runner.clone();
        let redraw = Box::new(move || {
            let runner = runner.clone();
            backend::request_animation_frame(move || {
                runner.send_event(Event::WindowEvent {
                    window_id: RootWI(Id),
                    event: WindowEvent::RedrawRequested,
                })
            });
        });

        let window = Window {
            canvas,
            redraw,
            previous_pointer: RefCell::new("auto"),
            position: RefCell::new(LogicalPosition { x: 0.0, y: 0.0 }),
        };

        window.set_inner_size(attr.inner_size.unwrap_or(LogicalSize {
            width: 1024.0,
            height: 768.0,
        }));
        window.set_title(&attr.title);
        window.set_maximized(attr.maximized);
        window.set_visible(attr.visible);
        window.set_window_icon(attr.window_icon);

        Ok(window)
    }

    pub fn set_title(&self, title: &str) {
        backend::Document::set_title(title);
    }

    pub fn set_visible(&self, _visible: bool) {
        // Intentionally a no-op
    }

    pub fn request_redraw(&self) {
        (self.redraw)();
    }

    pub fn outer_position(&self) -> Result<LogicalPosition, NotSupportedError> {
        let (x, y) = self.canvas.position();

        Ok(LogicalPosition { x, y })
    }

    pub fn inner_position(&self) -> Result<LogicalPosition, NotSupportedError> {
        Ok(*self.position.borrow())
    }

    pub fn set_outer_position(&self, position: LogicalPosition) {
        *self.position.borrow_mut() = position;

        self.canvas.set_attribute("position", "fixed");
        self.canvas.set_attribute("left", &position.x.to_string());
        self.canvas.set_attribute("top", &position.y.to_string());
    }

    #[inline]
    pub fn inner_size(&self) -> LogicalSize {
        LogicalSize {
            width: self.canvas.width() as f64,
            height: self.canvas.height() as f64,
        }
    }

    #[inline]
    pub fn outer_size(&self) -> LogicalSize {
        LogicalSize {
            width: self.canvas.width() as f64,
            height: self.canvas.height() as f64,
        }
    }

    #[inline]
    pub fn set_inner_size(&self, size: LogicalSize) {
        self.canvas.set_size(size);
    }

    #[inline]
    pub fn set_min_inner_size(&self, _dimensions: Option<LogicalSize>) {
        // Intentionally a no-op: users can't resize canvas elements
    }

    #[inline]
    pub fn set_max_inner_size(&self, _dimensions: Option<LogicalSize>) {
        // Intentionally a no-op: users can't resize canvas elements
    }

    #[inline]
    pub fn set_resizable(&self, _resizable: bool) {
        // Intentionally a no-op: users can't resize canvas elements
    }

    #[inline]
    pub fn hidpi_factor(&self) -> f64 {
        1.0
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
        self.canvas.set_attribute("cursor", text);
    }

    #[inline]
    pub fn set_cursor_position(&self, _position: LogicalPosition) -> Result<(), ExternalError> {
        // TODO: pointer capture
        Ok(())
    }

    #[inline]
    pub fn set_cursor_grab(&self, _grab: bool) -> Result<(), ExternalError> {
        // TODO: pointer capture
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
    pub fn set_maximized(&self, _maximized: bool) {
        // TODO: should there be a maximization / fullscreen API?
    }

    #[inline]
    pub fn fullscreen(&self) -> Option<RootMH> {
        // TODO: should there be a maximization / fullscreen API?
        None
    }

    #[inline]
    pub fn set_fullscreen(&self, _monitor: Option<RootMH>) {
        // TODO: should there be a maximization / fullscreen API?
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
    pub fn set_ime_position(&self, _position: LogicalPosition) {
        // TODO: what is this?
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
        // TODO ?
        unsafe { Id::dummy() }
    }
}

#[cfg(feature = "stdweb")]
impl WindowExtStdweb for RootWindow {
    fn canvas(&self) -> CanvasElement {
        self.window.canvas.clone()
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id;

impl Id {
    pub unsafe fn dummy() -> Id {
        Id
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlatformSpecificBuilderAttributes;
