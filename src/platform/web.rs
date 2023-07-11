//! The web target does not automatically insert the canvas element object into the web page, to
//! allow end users to determine how the page should be laid out. Use the [`WindowExtWebSys`] trait
//! to retrieve the canvas from the Window. Alternatively, use the [`WindowBuilderExtWebSys`] trait
//! to provide your own canvas.
//!
//! It is recommended **not** to apply certain CSS properties to the canvas:
//! - [`transform`]
//! - [`border`]
//! - [`padding`]
//!
//! The following APIs can't take them into account and will therefore provide inaccurate results:
//! - [`WindowEvent::Resized`] and [`Window::(set_)inner_size()`]
//! - [`WindowEvent::Occluded`]
//! - [`WindowEvent::CursorMoved`], [`WindowEvent::CursorEntered`], [`WindowEvent::CursorLeft`],
//!   and [`WindowEvent::Touch`].
//! - [`Window::set_outer_position()`]
//!
//! [`WindowEvent::Resized`]: crate::event::WindowEvent::Resized
//! [`Window::(set_)inner_size()`]: crate::window::Window::inner_size()
//! [`WindowEvent::Occluded`]: crate::event::WindowEvent::Occluded
//! [`WindowEvent::CursorMoved`]: crate::event::WindowEvent::CursorMoved
//! [`WindowEvent::CursorEntered`]: crate::event::WindowEvent::CursorEntered
//! [`WindowEvent::CursorLeft`]: crate::event::WindowEvent::CursorLeft
//! [`WindowEvent::Touch`]: crate::event::WindowEvent::Touch
//! [`Window::set_outer_position()`]: crate::window::Window::set_outer_position()
//! [`transform`]: https://developer.mozilla.org/en-US/docs/Web/CSS/transform
//! [`border`]: https://developer.mozilla.org/en-US/docs/Web/CSS/border
//! [`padding`]: https://developer.mozilla.org/en-US/docs/Web/CSS/padding

use crate::event::Event;
use crate::event_loop::ControlFlow;
use crate::event_loop::EventLoop;
use crate::event_loop::EventLoopWindowTarget;
use crate::window::WindowBuilder;

use web_sys::HtmlCanvasElement;

pub trait WindowExtWebSys {
    /// Only returns the canvas if called from inside the window.
    fn canvas(&self) -> Option<HtmlCanvasElement>;
}

pub trait WindowBuilderExtWebSys {
    fn with_canvas(self, canvas: Option<HtmlCanvasElement>) -> Self;

    /// Whether `event.preventDefault` should be automatically called to prevent event propagation
    /// when appropriate.
    ///
    /// For example, mouse wheel events are only handled by the canvas by default. This avoids
    /// the default behavior of scrolling the page.
    ///
    /// Some events are impossible to prevent. E.g. Firefox allows to access the native browser
    /// context menu with Shift+Rightclick.
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
    ///
    /// Once the event loop has been destroyed, it's possible to reinitialize another event loop
    /// by calling this function again. This can be useful if you want to recreate the event loop
    /// while the WebAssembly module is still loaded. For example, this can be used to recreate the
    /// event loop when switching between tabs on a single page application.
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
