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

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

#[cfg(wasm_platform)]
use web_sys::HtmlCanvasElement;

use crate::cursor::CustomCursorBuilder;
use crate::event::Event;
use crate::event_loop::{EventLoop, EventLoopWindowTarget};
#[cfg(wasm_platform)]
use crate::platform_impl::CustomCursorFuture as PlatformCustomCursorFuture;
use crate::platform_impl::{PlatformCustomCursor, PlatformCustomCursorBuilder};
use crate::window::{CustomCursor, Window, WindowBuilder};

#[cfg(not(wasm_platform))]
#[doc(hidden)]
pub struct HtmlCanvasElement;

pub trait WindowExtWebSys {
    /// Only returns the canvas if called from inside the window context (the
    /// main thread).
    fn canvas(&self) -> Option<HtmlCanvasElement>;

    /// Returns [`true`] if calling `event.preventDefault()` is enabled.
    ///
    /// See [`Window::set_prevent_default()`] for more details.
    fn prevent_default(&self) -> bool;

    /// Sets whether `event.preventDefault()` should be called on events on the
    /// canvas that have side effects.
    ///
    /// For example, by default using the mouse wheel would cause the page to scroll, enabling this
    /// would prevent that.
    ///
    /// Some events are impossible to prevent. E.g. Firefox allows to access the native browser
    /// context menu with Shift+Rightclick.
    fn set_prevent_default(&self, prevent_default: bool);
}

impl WindowExtWebSys for Window {
    #[inline]
    fn canvas(&self) -> Option<HtmlCanvasElement> {
        self.window.canvas()
    }

    fn prevent_default(&self) -> bool {
        self.window.prevent_default()
    }

    fn set_prevent_default(&self, prevent_default: bool) {
        self.window.set_prevent_default(prevent_default)
    }
}

pub trait WindowBuilderExtWebSys {
    /// Pass an [`HtmlCanvasElement`] to be used for this [`Window`]. If [`None`],
    /// [`WindowBuilder::build()`] will create one.
    ///
    /// In any case, the canvas won't be automatically inserted into the web page.
    ///
    /// [`None`] by default.
    #[cfg_attr(
        not(wasm_platform),
        doc = "",
        doc = "[`HtmlCanvasElement`]: #only-available-on-wasm"
    )]
    fn with_canvas(self, canvas: Option<HtmlCanvasElement>) -> Self;

    /// Sets whether `event.preventDefault()` should be called on events on the
    /// canvas that have side effects.
    ///
    /// See [`Window::set_prevent_default()`] for more details.
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
        self.platform_specific.set_canvas(canvas);
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
        F: 'static + FnMut(Event<Self::UserEvent>, &EventLoopWindowTarget);
}

impl<T> EventLoopExtWebSys for EventLoop<T> {
    type UserEvent = T;

    fn spawn<F>(self, event_handler: F)
    where
        F: 'static + FnMut(Event<Self::UserEvent>, &EventLoopWindowTarget),
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

impl EventLoopWindowTargetExtWebSys for EventLoopWindowTarget {
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
    /// Returns if this cursor is an animation.
    fn is_animation(&self) -> bool;

    /// Creates a new cursor from a URL pointing to an image.
    /// It uses the [url css function](https://developer.mozilla.org/en-US/docs/Web/CSS/url),
    /// but browser support for image formats is inconsistent. Using [PNG] is recommended.
    ///
    /// [PNG]: https://en.wikipedia.org/wiki/PNG
    fn from_url(url: String, hotspot_x: u16, hotspot_y: u16) -> CustomCursorBuilder;

    /// Crates a new animated cursor from multiple [`CustomCursor`]s.
    /// Supplied `cursors` can't be empty or other animations.
    fn from_animation(
        duration: Duration,
        cursors: Vec<CustomCursor>,
    ) -> Result<CustomCursorBuilder, BadAnimation>;
}

impl CustomCursorExtWebSys for CustomCursor {
    fn is_animation(&self) -> bool {
        self.inner.animation
    }

    fn from_url(url: String, hotspot_x: u16, hotspot_y: u16) -> CustomCursorBuilder {
        CustomCursorBuilder {
            inner: PlatformCustomCursorBuilder::Url {
                url,
                hotspot_x,
                hotspot_y,
            },
        }
    }

    fn from_animation(
        duration: Duration,
        cursors: Vec<CustomCursor>,
    ) -> Result<CustomCursorBuilder, BadAnimation> {
        if cursors.is_empty() {
            return Err(BadAnimation::Empty);
        }

        if cursors.iter().any(CustomCursor::is_animation) {
            return Err(BadAnimation::Animation);
        }

        Ok(CustomCursorBuilder {
            inner: PlatformCustomCursorBuilder::Animation { duration, cursors },
        })
    }
}

/// An error produced when using [`CustomCursor::from_animation`] with invalid arguments.
#[derive(Debug, Clone)]
pub enum BadAnimation {
    /// Produced when no cursors were supplied.
    Empty,
    /// Produced when a supplied cursor is an animation.
    Animation,
}

impl fmt::Display for BadAnimation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "No cursors supplied"),
            Self::Animation => write!(f, "A supplied cursor is an animtion"),
        }
    }
}

impl Error for BadAnimation {}

pub trait CustomCursorBuilderExtWebSys {
    /// Async version of [`CustomCursorBuilder::build()`] which waits until the
    /// cursor has completely finished loading.
    fn build_async(self, window_target: &EventLoopWindowTarget) -> CustomCursorFuture;
}

impl CustomCursorBuilderExtWebSys for CustomCursorBuilder {
    fn build_async(self, window_target: &EventLoopWindowTarget) -> CustomCursorFuture {
        CustomCursorFuture(PlatformCustomCursor::build_async(
            self.inner,
            &window_target.p,
        ))
    }
}

#[cfg(not(wasm_platform))]
struct PlatformCustomCursorFuture;

#[derive(Debug)]
pub struct CustomCursorFuture(PlatformCustomCursorFuture);

impl Future for CustomCursorFuture {
    type Output = Result<CustomCursor, CustomCursorError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.0)
            .poll(cx)
            .map_ok(|cursor| CustomCursor { inner: cursor })
    }
}

#[derive(Clone, Debug)]
pub enum CustomCursorError {
    Blob,
    Decode(String),
    Animation,
}

impl Display for CustomCursorError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Blob => write!(f, "failed to create `Blob`"),
            Self::Decode(error) => write!(f, "failed to decode image: {error}"),
            Self::Animation => write!(
                f,
                "found `CustomCursor` that is an animation when building an animation"
            ),
        }
    }
}
