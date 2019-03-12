use dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use icon::Icon;
use monitor::{MonitorHandle as RootMH};
use window::{CreationError, MouseCursor, WindowAttributes};
use super::EventLoopWindowTarget;
use std::collections::VecDeque;
use std::collections::vec_deque::IntoIter as VecDequeIter;
use stdweb::{
    traits::*,
    unstable::TryInto
};
use stdweb::web::{
    document,
    html_element::CanvasElement,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MonitorHandle;

impl MonitorHandle {
    pub fn get_hidpi_factor(&self) -> f64 {
        // TODO
        1.0
    }

    pub fn get_position(&self) -> PhysicalPosition {
        unimplemented!();
    }

    pub fn get_dimensions(&self) -> PhysicalSize {
        unimplemented!();
    }

    pub fn get_name(&self) -> Option<String> {
        unimplemented!();
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId;

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlatformSpecificWindowBuilderAttributes;

impl WindowId {
    pub unsafe fn dummy() -> WindowId {
        WindowId
    }
}

pub struct Window {
    pub(crate) canvas: CanvasElement,
}

impl Window {
    pub fn new<T>(target: &EventLoopWindowTarget<T>, attr: WindowAttributes,
                  _: PlatformSpecificWindowBuilderAttributes) -> Result<Self, CreationError> {
        let element = document()
            .create_element("canvas")
            .map_err(|_| CreationError::OsError("Failed to create canvas element".to_owned()))?;
        let canvas: CanvasElement = element.try_into()
            .map_err(|_| CreationError::OsError("Failed to create canvas element".to_owned()))?;
        document().body()
            .ok_or_else(|| CreationError::OsError("Failed to find body node".to_owned()))?
            .append_child(&canvas);
        target.canvases.borrow_mut().push(canvas.clone());
        let window = Window { canvas };
        if let Some(dimensions) = attr.dimensions {
            window.set_inner_size(dimensions);
        } else {
            window.set_inner_size(LogicalSize {
                width: 1024.0,
                height: 768.0,
            })
        }
        // TODO: most of these are no-op, but should they stay here just in case?
        window.set_min_dimensions(attr.min_dimensions);
        window.set_max_dimensions(attr.max_dimensions);
        window.set_resizable(attr.resizable);
        window.set_title(&attr.title);
        window.set_maximized(attr.maximized);
        if attr.visible {
            window.show();
        } else {
            window.hide();
        }
        //window.set_transparent(attr.transparent);
        window.set_decorations(attr.decorations);
        window.set_always_on_top(attr.always_on_top);
        window.set_window_icon(attr.window_icon);
        Ok(window)
    }

    pub fn set_title(&self, title: &str) {
        document().set_title(title);
    }

    pub fn show(&self) {
        // Intentionally a no-op
    }

    pub fn hide(&self) {
        // Intentionally a no-op
    }

    pub fn request_redraw(&self) {
        // TODO: what does this mean? If it's a 'present'-style call then it's not necessary
    }

    pub fn get_position(&self) -> Option<LogicalPosition> {
        let bounds = self.canvas.get_bounding_client_rect();
        Some(LogicalPosition {
            x: bounds.get_x(),
            y: bounds.get_y(),
        })
    }

    pub fn get_inner_position(&self) -> Option<LogicalPosition> {
        // TODO
        None
    }

    pub fn set_position(&self, position: LogicalPosition) {
        // TODO: use CSS?
    }

    #[inline]
    pub fn get_inner_size(&self) -> Option<LogicalSize> {
        Some(LogicalSize {
            width: self.canvas.width() as f64,
            height: self.canvas.height() as f64
        })
    }

    #[inline]
    pub fn get_outer_size(&self) -> Option<LogicalSize> {
        Some(LogicalSize {
            width: self.canvas.width() as f64,
            height: self.canvas.height() as f64
        })
    }

    #[inline]
    pub fn set_inner_size(&self, size: LogicalSize) {
        self.canvas.set_width(size.width as u32);
        self.canvas.set_height(size.height as u32);
    }

    #[inline]
    pub fn set_min_dimensions(&self, _dimensions: Option<LogicalSize>) {
        // Intentionally a no-op: users can't resize canvas elements
    }

    #[inline]
    pub fn set_max_dimensions(&self, _dimensions: Option<LogicalSize>) {
        // Intentionally a no-op: users can't resize canvas elements
    }

    #[inline]
    pub fn set_resizable(&self, _resizable: bool) {
        // Intentionally a no-op: users can't resize canvas elements
    }

    #[inline]
    pub fn get_hidpi_factor(&self) -> f64 {
        // TODO
        1.0
    }

    #[inline]
    pub fn set_cursor(&self, cursor: MouseCursor) {
        let text = match cursor {
            MouseCursor::Default => "auto",
            MouseCursor::Crosshair => "crosshair",
            MouseCursor::Hand => "pointer",
            MouseCursor::Arrow => "default",
            MouseCursor::Move => "move",
            MouseCursor::Text => "text",
            MouseCursor::Wait => "wait",
            MouseCursor::Help => "help",
            MouseCursor::Progress => "progress",

            MouseCursor::NotAllowed => "not-allowed",
            MouseCursor::ContextMenu => "context-menu",
            MouseCursor::Cell => "cell",
            MouseCursor::VerticalText => "vertical-text",
            MouseCursor::Alias => "alias",
            MouseCursor::Copy => "copy",
            MouseCursor::NoDrop => "no-drop",
            MouseCursor::Grab => "grab",
            MouseCursor::Grabbing => "grabbing",
            MouseCursor::AllScroll => "all-scroll",
            MouseCursor::ZoomIn => "zoom-in",
            MouseCursor::ZoomOut => "zoom-out",

            MouseCursor::EResize => "e-resize",
            MouseCursor::NResize => "n-resize",
            MouseCursor::NeResize => "ne-resize",
            MouseCursor::NwResize => "nw-resize",
            MouseCursor::SResize => "s-resize",
            MouseCursor::SeResize => "se-resize",
            MouseCursor::SwResize => "sw-resize",
            MouseCursor::WResize => "w-resize",
            MouseCursor::EwResize => "ew-resize",
            MouseCursor::NsResize => "ns-resize",
            MouseCursor::NeswResize => "nesw-resize",
            MouseCursor::NwseResize => "nwse-resize",
            MouseCursor::ColResize => "col-resize",
            MouseCursor::RowResize => "row-resize",
        };
        self.canvas.set_attribute("cursor", text)
            .expect("Setting the cursor on the canvas");
    }

    #[inline]
    pub fn set_cursor_position(&self, position: LogicalPosition) -> Result<(), String> {
        // TODO: pointer capture
        Ok(())
    }

    #[inline]
    pub fn grab_cursor(&self, grab: bool) -> Result<(), String> {
        // TODO: pointer capture
        Ok(())
    }

    #[inline]
    pub fn hide_cursor(&self, hide: bool) {
        self.canvas.set_attribute("cursor", "none")
            .expect("Setting the cursor on the canvas");
    }

    #[inline]
    pub fn set_maximized(&self, maximized: bool) {
        // TODO: should there be a maximization / fullscreen API?
    }

    #[inline]
    pub fn set_fullscreen(&self, monitor: Option<RootMH>) {
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
    pub fn set_window_icon(&self, window_icon: Option<Icon>) {
        // TODO: should this set the favicon?
    }

    #[inline]
    pub fn set_ime_spot(&self, position: LogicalPosition) {
        // TODO: what is this?
    }

    #[inline]
    pub fn get_current_monitor(&self) -> RootMH {
        RootMH {
            inner: MonitorHandle
        }
    }

    #[inline]
    pub fn get_available_monitors(&self) -> VecDequeIter<MonitorHandle> {
        VecDeque::new().into_iter()
    }

    #[inline]
    pub fn get_primary_monitor(&self) -> MonitorHandle {
        MonitorHandle
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        // TODO ?
        unsafe { WindowId::dummy() }
    }
}
