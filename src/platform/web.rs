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

use crate::cursor::CustomCursor;
use crate::event::Event;
use crate::event_loop::EventLoop;
use crate::event_loop::EventLoopWindowTarget;
use crate::platform_impl::PlatformCustomCursor;
use crate::window::{Window, WindowBuilder};
use crate::SendSyncWrapper;

use web_sys::HtmlCanvasElement;

pub trait WindowExtWebSys {
    /// Only returns the canvas if called from inside the window.
    fn canvas(&self) -> Option<HtmlCanvasElement>;
}

impl WindowExtWebSys for Window {
    #[inline]
    fn canvas(&self) -> Option<HtmlCanvasElement> {
        self.window.canvas()
    }
}

pub trait WindowBuilderExtWebSys {
    /// Pass an [`HtmlCanvasElement`] to be used for this [`Window`]. If [`None`],
    /// [`WindowBuilder::build()`] will create one.
    ///
    /// In any case, the canvas won't be automatically inserted into the web page.
    ///
    /// [`None`] by default.
    fn with_canvas(self, canvas: Option<HtmlCanvasElement>) -> Self;

    /// Whether `event.preventDefault` should be automatically called to prevent event propagation
    /// when appropriate.
    ///
    /// For example, mouse wheel events are only handled by the canvas by default. This avoids
    /// the default behavior of scrolling the page.
    ///
    /// Some events are impossible to prevent. E.g. Firefox allows to access the native browser
    /// context menu with Shift+Rightclick.
    ///
    /// Enabled by default.
    fn with_prevent_default(self, prevent_default: bool) -> Self;

    /// Whether the canvas should be focusable using the tab key. This is necessary to capture
    /// canvas keyboard events.
    ///
    /// Enabled by default.
    fn with_focusable(self, focusable: bool) -> Self;

    /// On window creation, append the canvas element to the web page if it isn't already.
    ///
    /// Disabled by default.
    fn with_append(self, append: bool) -> Self;
}

impl WindowBuilderExtWebSys for WindowBuilder {
    fn with_canvas(mut self, canvas: Option<HtmlCanvasElement>) -> Self {
        self.platform_specific.canvas = SendSyncWrapper(canvas);
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

    fn with_append(mut self, append: bool) -> Self {
        self.platform_specific.append = append;
        self
    }
}

/// Additional methods on `EventLoop` that are specific to the web.
pub trait EventLoopExtWebSys {
    /// A type provided by the user that can be passed through `Event::UserEvent`.
    type UserEvent;

    /// Initializes the winit event loop.
    ///
    /// Unlike
    #[cfg_attr(
        all(wasm_platform, target_feature = "exception-handling"),
        doc = "`run()`"
    )]
    #[cfg_attr(
        not(all(wasm_platform, target_feature = "exception-handling")),
        doc = "[`run()`]"
    )]
    /// [^1], this returns immediately, and doesn't throw an exception in order to
    /// satisfy its [`!`] return type.
    ///
    /// Once the event loop has been destroyed, it's possible to reinitialize another event loop
    /// by calling this function again. This can be useful if you want to recreate the event loop
    /// while the WebAssembly module is still loaded. For example, this can be used to recreate the
    /// event loop when switching between tabs on a single page application.
    ///
    #[cfg_attr(
        not(all(wasm_platform, target_feature = "exception-handling")),
        doc = "[`run()`]: EventLoop::run()"
    )]
    /// [^1]: `run()` is _not_ available on WASM when the target supports `exception-handling`.
    fn spawn<F>(self, event_handler: F)
    where
        F: 'static + FnMut(Event<Self::UserEvent>, &EventLoopWindowTarget<Self::UserEvent>);
}

impl<T> EventLoopExtWebSys for EventLoop<T> {
    type UserEvent = T;

    fn spawn<F>(self, event_handler: F)
    where
        F: 'static + FnMut(Event<Self::UserEvent>, &EventLoopWindowTarget<Self::UserEvent>),
    {
        self.event_loop.spawn(event_handler)
    }
}

pub trait EventLoopWindowTargetExtWebSys {
    /// Sets the strategy for [`ControlFlow::Poll`].
    ///
    /// See [`PollStrategy`].
    ///
    /// [`ControlFlow::Poll`]: crate::event_loop::ControlFlow::Poll
    fn set_poll_strategy(&self, strategy: PollStrategy);

    /// Gets the strategy for [`ControlFlow::Poll`].
    ///
    /// See [`PollStrategy`].
    ///
    /// [`ControlFlow::Poll`]: crate::event_loop::ControlFlow::Poll
    fn poll_strategy(&self) -> PollStrategy;
}

impl<T> EventLoopWindowTargetExtWebSys for EventLoopWindowTarget<T> {
    #[inline]
    fn set_poll_strategy(&self, strategy: PollStrategy) {
        self.p.set_poll_strategy(strategy);
    }

    #[inline]
    fn poll_strategy(&self) -> PollStrategy {
        self.p.poll_strategy()
    }
}

/// Strategy used for [`ControlFlow::Poll`](crate::event_loop::ControlFlow::Poll).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PollStrategy {
    /// Uses [`Window.requestIdleCallback()`] to queue the next event loop. If not available
    /// this will fallback to [`setTimeout()`].
    ///
    /// This strategy will wait for the browser to enter an idle period before running and might
    /// be affected by browser throttling.
    ///
    /// [`Window.requestIdleCallback()`]: https://developer.mozilla.org/en-US/docs/Web/API/Window/requestIdleCallback
    /// [`setTimeout()`]: https://developer.mozilla.org/en-US/docs/Web/API/setTimeout
    IdleCallback,
    /// Uses the [Prioritized Task Scheduling API] to queue the next event loop. If not available
    /// this will fallback to [`setTimeout()`].
    ///
    /// This strategy will run as fast as possible without disturbing users from interacting with
    /// the page and is not affected by browser throttling.
    ///
    /// This is the default strategy.
    ///
    /// [Prioritized Task Scheduling API]: https://developer.mozilla.org/en-US/docs/Web/API/Prioritized_Task_Scheduling_API
    /// [`setTimeout()`]: https://developer.mozilla.org/en-US/docs/Web/API/setTimeout
    #[default]
    Scheduler,
}

pub trait CustomCursorExtWebSys {
    fn from_url(url: String, hotspot_x: u32, hotspot_y: u32) -> Self;
}

impl CustomCursorExtWebSys for CustomCursor {
    fn from_url(url: String, hotspot_x: u32, hotspot_y: u32) -> Self {
        Self {
            inner: PlatformCustomCursor::Url {
                url,
                hotspot_x,
                hotspot_y,
            }
            .into(),
        }
    }
}
