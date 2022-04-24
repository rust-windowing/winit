//! The web target does not automatically insert the canvas element object into the web page, to
//! allow end users to determine how the page should be laid out. Use the [`WindowExtWebSys`] trait
//! to retrieve the canvas from the Window. Alternatively, use the [`WindowBuilderExtWebSys`] trait
//! to provide your own canvas.

use crate::event::Event;
use crate::event_loop::ControlFlow;
use crate::event_loop::EventLoop;
use crate::event_loop::EventLoopWindowTarget;
use crate::window::WindowBuilder;

use web_sys::HtmlCanvasElement;

pub trait WindowExtWebSys {
    fn canvas(&self) -> HtmlCanvasElement;

    /// Whether the browser reports the preferred color scheme to be "dark".
    fn is_dark_mode(&self) -> bool;
}

pub trait WindowBuilderExtWebSys {
    /// Enable scrolling of the web page the canvas is in when the canvas is focused.
    ///
    /// Scrolling is disabled by default because the scroll input on many mobile devices
    /// is the same as click and dragging which is a very common input method for many applications.
    ///
    /// So only call this method if you know that you will never need to handle mouse wheel inputs
    /// or click and dragging.
    fn enable_web_page_scroll(self) -> Self;

    fn with_canvas(self, canvas: Option<HtmlCanvasElement>) -> Self;

    /// Whether `event.preventDefault` should be automatically called to prevent event propagation
    /// when appropriate.
    ///
    /// For example, mouse wheel events are only handled by the canvas by default. This avoids
    /// the default behavior of scrolling the page.
    fn with_prevent_default(self, prevent_default: bool) -> Self;

    /// Whether the canvas should be focusable using the tab key. This is necessary to capture
    /// canvas keyboard events.
    fn with_focusable(self, focusable: bool) -> Self;
}

impl WindowBuilderExtWebSys for WindowBuilder {
    fn enable_web_page_scroll(mut self) -> Self {
        self.platform_specific.enable_web_page_scroll = true;

        self
    }

    fn with_canvas(mut self, canvas: Option<HtmlCanvasElement>) -> Self {
        self.platform_specific.canvas = canvas;

        self
    }

    fn with_prevent_default(mut self, prevent_default: bool) -> Self {
        self.platform_specific.prevent_default = prevent_default;

        self
    }

    fn with_focusable(mut self, focusable: bool) -> Self {
        self.platform_specific.focusable = focusable;

        self
    }
}

/// Additional methods on `EventLoop` that are specific to the web.
pub trait EventLoopExtWebSys {
    /// A type provided by the user that can be passed through `Event::UserEvent`.
    type UserEvent;

    /// Initializes the winit event loop.
    ///
    /// Unlike `run`, this returns immediately, and doesn't throw an exception in order to
    /// satisfy its `!` return type.
    fn spawn<F>(self, event_handler: F)
    where
        F: 'static
            + FnMut(
                Event<'_, Self::UserEvent>,
                &EventLoopWindowTarget<Self::UserEvent>,
                &mut ControlFlow,
            );
}

impl<T> EventLoopExtWebSys for EventLoop<T> {
    type UserEvent = T;

    fn spawn<F>(self, event_handler: F)
    where
        F: 'static
            + FnMut(
                Event<'_, Self::UserEvent>,
                &EventLoopWindowTarget<Self::UserEvent>,
                &mut ControlFlow,
            ),
    {
        self.event_loop.spawn(event_handler)
    }
}
