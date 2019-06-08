use dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use error::{ExternalError, NotSupportedError, OsError as RootOE};
use event::{Event, WindowEvent};
use icon::Icon;
use platform::web_sys::WindowExtWebSys;
use platform_impl::platform::{document, window};
use monitor::{MonitorHandle as RootMH};
use window::{CursorIcon, Window as RootWindow, WindowAttributes, WindowId as RootWI};
use super::{EventLoopWindowTarget, OsError, register};
use std::collections::VecDeque;
use std::collections::vec_deque::IntoIter as VecDequeIter;
use std::cell::RefCell;
use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::HtmlCanvasElement;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MonitorHandle;

impl MonitorHandle {
    pub fn hidpi_factor(&self) -> f64 {
        1.0
    }

    pub fn position(&self) -> PhysicalPosition {
        unimplemented!();
    }

    pub fn dimensions(&self) -> PhysicalSize {
        unimplemented!();
    }

    pub fn name(&self) -> Option<String> {
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
    pub(crate) canvas: HtmlCanvasElement,
    pub(crate) redraw: Box<dyn Fn()>,
    previous_pointer: RefCell<&'static str>,
    position: RefCell<LogicalPosition>,
}

impl Window {
    pub fn new<T>(target: &EventLoopWindowTarget<T>, attr: WindowAttributes,
                  _: PlatformSpecificWindowBuilderAttributes) -> Result<Self, RootOE> {
        let element = document()
            .create_element("canvas")
            .map_err(|_| os_error!(OsError("Failed to create canvas element".to_owned())))?;
        let canvas: HtmlCanvasElement = element.unchecked_into();
        document().body()
            .ok_or_else(|| os_error!(OsError("Failed to find body node".to_owned())))?
            .append_child(&canvas).map_err(|_| os_error!(OsError("Failed to append canvas".to_owned())))?;

        register(&target.runner, &canvas);

        let runner = target.runner.clone();
        let redraw = Box::new(move || {
            let runner = runner.clone();
            let closure = Closure::once_into_js(move |_: f64| {
                runner.send_event(Event::WindowEvent {
                    window_id: RootWI(WindowId),
                    event: WindowEvent::RedrawRequested
                });
            });
            window().request_animation_frame(closure.as_ref().unchecked_ref());
        });

        let window = Window {
            canvas,
            redraw,
            previous_pointer: RefCell::new("auto"),
            position: RefCell::new(LogicalPosition {
                x: 0.0,
                y: 0.0
            })
        };

        if let Some(inner_size) = attr.inner_size {
            window.set_inner_size(inner_size);
        } else {
            window.set_inner_size(LogicalSize {
                width: 1024.0,
                height: 768.0,
            })
        }
        window.set_min_inner_size(attr.min_inner_size);
        window.set_max_inner_size(attr.max_inner_size);
        window.set_resizable(attr.resizable);
        window.set_title(&attr.title);
        window.set_maximized(attr.maximized);
        window.set_visible(attr.visible);
        //window.set_transparent(attr.transparent);
        window.set_decorations(attr.decorations);
        window.set_always_on_top(attr.always_on_top);
        window.set_window_icon(attr.window_icon);

        Ok(window)
    }

    pub fn set_title(&self, title: &str) {
        document().set_title(title);
    }

    pub fn set_visible(&self, _visible: bool) {
        // Intentionally a no-op
    }

    pub fn request_redraw(&self) {
        (self.redraw)();
    }

    pub fn outer_position(&self) -> Result<LogicalPosition, NotSupportedError> {
        let bounds = self.canvas.get_bounding_client_rect();
        Ok(LogicalPosition {
            x: bounds.x(),
            y: bounds.y(),
        })
    }

    pub fn inner_position(&self) -> Result<LogicalPosition, NotSupportedError> {
        Ok(*self.position.borrow())
    }

    pub fn set_outer_position(&self, position: LogicalPosition) {
        *self.position.borrow_mut() = position;
        self.canvas.set_attribute("position", "fixed")
            .expect("Setting the position for the canvas");
        self.canvas.set_attribute("left", &position.x.to_string())
            .expect("Setting the position for the canvas");
        self.canvas.set_attribute("top", &position.y.to_string())
            .expect("Setting the position for the canvas");
    }

    #[inline]
    pub fn inner_size(&self) -> LogicalSize {
        LogicalSize {
            width: self.canvas.width() as f64,
            height: self.canvas.height() as f64
        }
    }

    #[inline]
    pub fn outer_size(&self) -> LogicalSize {
        LogicalSize {
            width: self.canvas.width() as f64,
            height: self.canvas.height() as f64
        }
    }

    #[inline]
    pub fn set_inner_size(&self, size: LogicalSize) {
        self.canvas.set_width(size.width as u32);
        self.canvas.set_height(size.height as u32);
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
        self.canvas.set_attribute("cursor", text)
            .expect("Setting the cursor on the canvas");
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
            self.canvas.set_attribute("cursor", "none")
                .expect("Setting the cursor on the canvas");
        } else {
            self.canvas.set_attribute("cursor", *self.previous_pointer.borrow())
                .expect("Setting the cursor on the canvas");
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
            inner: MonitorHandle
        }
    }

    #[inline]
    pub fn available_monitors(&self) -> VecDequeIter<MonitorHandle> {
        VecDeque::new().into_iter()
    }

    #[inline]
    pub fn primary_monitor(&self) -> MonitorHandle {
        MonitorHandle
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        // TODO ?
        unsafe { WindowId::dummy() }
    }
}

impl WindowExtWebSys for RootWindow {
    fn canvas(&self) -> HtmlCanvasElement {
        self.window.canvas.clone()
    }
}
