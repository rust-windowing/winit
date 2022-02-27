//! The web target does not automatically insert the canvas element object into the web page, to
//! allow end users to determine how the page should be laid out. Use the [`WindowExtWebSys`] trait
//! to retrieve the canvas from the Window. Alternatively, use the [`WindowBuilderExtWebSys`] trait
//! to provide your own canvas.
//!
//! # The `css-size` feature
//!
//! By default, the canvas' size is fixed; it can only be resized by calling
//! [`Window::set_inner_size`]. The `css-size` feature changes this, setting the size of the canvas
//! based on CSS. This allows much more easily laying it out within the page.
//!
//! `css-size` relies on `ResizeObserver`, which is still an unstable feature; so, to use it, you
//! have to enable `web_sys_unstable_apis`. For example:
//!
//! ```sh
//! RUSTFLAGS="--cfg=web_sys_unstable_apis" cargo build ...
//! ```
//!
//! ## Initial size handling
//!
//! If the canvas is created by `Window::new` (i.e., isn't passed via [`with_canvas`]), its size
//! isn't initially known, since the canvas hasn't yet been put into the DOM. To work around this,
//! the `Window` doesn't calculate its size until the first call to [`Window::inner_size`], to
//! allow the canvas to be inserted into the page.
//!
//! This has some caveats; if you use a library which gets the size directly from the canvas, it
//! won't trigger this, and will end up with an incorrect initial size. The most reliable method is
//! to create the canvas yourself, insert it into the page, and then pass it to [`with_canvas`].
//!
//! [`Window::new`]: crate::window::Window::new
//! [`Window::inner_size`]: crate::window::Window::inner_size
//! [`Window::set_inner_size`]: crate::window::Window::set_inner_size
//! [`with_canvas`]: crate::platform::web::WindowBuilderExtWebSys::with_canvas

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
