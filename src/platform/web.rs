//! # Web
//!
//! Winit supports running in Browsers by compiling to WebAssembly with
//! [`wasm-bindgen`][wasm_bindgen]. For information on using Rust on WebAssembly, check out the
//! [Rust and WebAssembly book].
//!
//! The officially supported browsers are Chrome, Firefox and Safari 13.1+, though forks of these
//! should work fine.
//!
//! On the Web platform, a Winit [`Window`] is backed by a [`HTMLCanvasElement`][canvas]. Winit will
//! create that canvas for you or you can [provide your own][with_canvas]. Then you can either let
//! Winit [insert it into the DOM for you][insert], or [retrieve the canvas][get] and insert it
//! yourself.
//!
//! [canvas]: https://developer.mozilla.org/en-US/docs/Web/API/HTMLCanvasElement
//! [with_canvas]: WindowAttributesExtWeb::with_canvas
//! [get]: WindowExtWeb::canvas
//! [insert]: WindowAttributesExtWeb::with_append
#![cfg_attr(not(web_platform), doc = "[wasm_bindgen]: https://docs.rs/wasm-bindgen")]
//! [Rust and WebAssembly book]: https://rustwasm.github.io/book
//!
//! ## CSS properties
//!
//! It is recommended **not** to apply certain CSS properties to the canvas:
//! - [`transform`](https://developer.mozilla.org/en-US/docs/Web/CSS/transform)
//! - [`border`](https://developer.mozilla.org/en-US/docs/Web/CSS/border)
//! - [`padding`](https://developer.mozilla.org/en-US/docs/Web/CSS/padding)
//!
//! The following APIs can't take them into account and will therefore provide inaccurate results:
//! - [`WindowEvent::Resized`] and [`Window::(set_)inner_size()`]
//! - [`WindowEvent::Occluded`]
//! - [`WindowEvent::CursorMoved`], [`WindowEvent::CursorEntered`], [`WindowEvent::CursorLeft`], and
//!   [`WindowEvent::Touch`].
//! - [`Window::set_outer_position()`]
//!
//! [`WindowEvent::Resized`]: crate::event::WindowEvent::Resized
//! [`Window::(set_)inner_size()`]: crate::window::Window::inner_size
//! [`WindowEvent::Occluded`]: crate::event::WindowEvent::Occluded
//! [`WindowEvent::CursorMoved`]: crate::event::WindowEvent::CursorMoved
//! [`WindowEvent::CursorEntered`]: crate::event::WindowEvent::CursorEntered
//! [`WindowEvent::CursorLeft`]: crate::event::WindowEvent::CursorLeft
//! [`WindowEvent::Touch`]: crate::event::WindowEvent::Touch
//! [`Window::set_outer_position()`]: crate::window::Window::set_outer_position

use std::cell::Ref;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(web_platform)]
use web_sys::HtmlCanvasElement;

use crate::application::ApplicationHandler;
use crate::cursor::CustomCursorSource;
use crate::error::NotSupportedError;
use crate::event::FingerId;
use crate::event_loop::{ActiveEventLoop, EventLoop};
use crate::monitor::MonitorHandle;
use crate::platform_impl::PlatformCustomCursorSource;
#[cfg(web_platform)]
use crate::platform_impl::{
    CustomCursorFuture as PlatformCustomCursorFuture,
    HasMonitorPermissionFuture as PlatformHasMonitorPermissionFuture,
    MonitorPermissionFuture as PlatformMonitorPermissionFuture,
    OrientationLockFuture as PlatformOrientationLockFuture,
};
use crate::window::{CustomCursor, Window, WindowAttributes};

#[cfg(not(web_platform))]
#[doc(hidden)]
pub struct HtmlCanvasElement;

pub trait WindowExtWeb {
    /// Only returns the canvas if called from inside the window context (the
    /// main thread).
    fn canvas(&self) -> Option<Ref<'_, HtmlCanvasElement>>;

    /// Returns [`true`] if calling `event.preventDefault()` is enabled.
    ///
    /// See [`WindowExtWeb::set_prevent_default()`] for more details.
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

    /// Returns whether using [`CursorGrabMode::Locked`] returns raw, un-accelerated mouse input.
    ///
    /// This is the same as [`ActiveEventLoopExtWeb::is_cursor_lock_raw()`], and is provided for
    /// convenience.
    ///
    /// [`CursorGrabMode::Locked`]: crate::window::CursorGrabMode::Locked
    fn is_cursor_lock_raw(&self) -> bool;
}

impl WindowExtWeb for dyn Window + '_ {
    #[inline]
    fn canvas(&self) -> Option<Ref<'_, HtmlCanvasElement>> {
        self.as_any()
            .downcast_ref::<crate::platform_impl::Window>()
            .expect("non Web window on Web")
            .canvas()
    }

    fn prevent_default(&self) -> bool {
        self.as_any()
            .downcast_ref::<crate::platform_impl::Window>()
            .expect("non Web window on Web")
            .prevent_default()
    }

    fn set_prevent_default(&self, prevent_default: bool) {
        self.as_any()
            .downcast_ref::<crate::platform_impl::Window>()
            .expect("non Web window on Web")
            .set_prevent_default(prevent_default)
    }

    fn is_cursor_lock_raw(&self) -> bool {
        self.as_any()
            .downcast_ref::<crate::platform_impl::Window>()
            .expect("non Web window on Web")
            .is_cursor_lock_raw()
    }
}

pub trait WindowAttributesExtWeb {
    /// Pass an [`HtmlCanvasElement`] to be used for this [`Window`]. If [`None`],
    /// [`WindowAttributes::default()`] will create one.
    ///
    /// In any case, the canvas won't be automatically inserted into the Web page.
    ///
    /// [`None`] by default.
    #[cfg_attr(not(web_platform), doc = "", doc = "[`HtmlCanvasElement`]: #only-available-on-wasm")]
    fn with_canvas(self, canvas: Option<HtmlCanvasElement>) -> Self;

    /// Sets whether `event.preventDefault()` should be called on events on the
    /// canvas that have side effects.
    ///
    /// See [`WindowExtWeb::set_prevent_default()`] for more details.
    ///
    /// Enabled by default.
    fn with_prevent_default(self, prevent_default: bool) -> Self;

    /// Whether the canvas should be focusable using the tab key. This is necessary to capture
    /// canvas keyboard events.
    ///
    /// Enabled by default.
    fn with_focusable(self, focusable: bool) -> Self;

    /// On window creation, append the canvas element to the Web page if it isn't already.
    ///
    /// Disabled by default.
    fn with_append(self, append: bool) -> Self;
}

impl WindowAttributesExtWeb for WindowAttributes {
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

/// Additional methods on `EventLoop` that are specific to the Web.
pub trait EventLoopExtWeb {
    /// Initializes the winit event loop.
    ///
    /// Unlike
    #[cfg_attr(all(web_platform, target_feature = "exception-handling"), doc = "`run()`")]
    #[cfg_attr(
        not(all(web_platform, target_feature = "exception-handling")),
        doc = "[`run()`]"
    )]
    /// [^1], this returns immediately, and doesn't throw an exception in order to
    /// satisfy its [`!`] return type.
    ///
    /// Once the event loop has been destroyed, it's possible to reinitialize another event loop
    /// by calling this function again. This can be useful if you want to recreate the event loop
    /// while the WebAssembly module is still loaded. For example, this can be used to recreate the
    /// event loop when switching between tabs on a single page application.
    #[rustfmt::skip]
    ///
    #[cfg_attr(
        not(all(web_platform, target_feature = "exception-handling")),
        doc = "[`run()`]: EventLoop::run()"
    )]
    /// [^1]: `run()` is _not_ available on Wasm when the target supports `exception-handling`.
    fn spawn_app<A: ApplicationHandler + 'static>(self, app: A);

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

    /// Sets the strategy for [`ControlFlow::WaitUntil`].
    ///
    /// See [`WaitUntilStrategy`].
    ///
    /// [`ControlFlow::WaitUntil`]: crate::event_loop::ControlFlow::WaitUntil
    fn set_wait_until_strategy(&self, strategy: WaitUntilStrategy);

    /// Gets the strategy for [`ControlFlow::WaitUntil`].
    ///
    /// See [`WaitUntilStrategy`].
    ///
    /// [`ControlFlow::WaitUntil`]: crate::event_loop::ControlFlow::WaitUntil
    fn wait_until_strategy(&self) -> WaitUntilStrategy;

    /// Returns if the users device has multiple screens. Useful to check before prompting the user
    /// with [`EventLoopExtWeb::request_detailed_monitor_permission()`].
    ///
    /// Browsers might always return [`false`] to reduce fingerprinting.
    fn has_multiple_screens(&self) -> Result<bool, NotSupportedError>;

    /// Prompts the user for permission to query detailed information about available monitors. The
    /// returned [`MonitorPermissionFuture`] can be dropped without aborting the request.
    ///
    /// Check [`EventLoopExtWeb::has_multiple_screens()`] before unnecessarily prompting the user
    /// for such permissions.
    ///
    /// [`MonitorHandle`]s don't automatically make use of this after permission is granted. New
    /// [`MonitorHandle`]s have to be created instead.
    fn request_detailed_monitor_permission(&self) -> MonitorPermissionFuture;

    /// Returns whether the user has given permission to access detailed monitor information.
    ///
    /// [`MonitorHandle`]s don't automatically make use of detailed monitor information after
    /// permission is granted. New [`MonitorHandle`]s have to be created instead.
    fn has_detailed_monitor_permission(&self) -> HasMonitorPermissionFuture;
}

impl EventLoopExtWeb for EventLoop {
    fn spawn_app<A: ApplicationHandler + 'static>(self, app: A) {
        self.event_loop.spawn_app(app);
    }

    fn set_poll_strategy(&self, strategy: PollStrategy) {
        self.event_loop.set_poll_strategy(strategy);
    }

    fn poll_strategy(&self) -> PollStrategy {
        self.event_loop.poll_strategy()
    }

    fn set_wait_until_strategy(&self, strategy: WaitUntilStrategy) {
        self.event_loop.set_wait_until_strategy(strategy);
    }

    fn wait_until_strategy(&self) -> WaitUntilStrategy {
        self.event_loop.wait_until_strategy()
    }

    fn has_multiple_screens(&self) -> Result<bool, NotSupportedError> {
        self.event_loop.has_multiple_screens()
    }

    fn request_detailed_monitor_permission(&self) -> MonitorPermissionFuture {
        MonitorPermissionFuture(self.event_loop.request_detailed_monitor_permission())
    }

    fn has_detailed_monitor_permission(&self) -> HasMonitorPermissionFuture {
        HasMonitorPermissionFuture(self.event_loop.has_detailed_monitor_permission())
    }
}

pub trait ActiveEventLoopExtWeb {
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

    /// Sets the strategy for [`ControlFlow::WaitUntil`].
    ///
    /// See [`WaitUntilStrategy`].
    ///
    /// [`ControlFlow::WaitUntil`]: crate::event_loop::ControlFlow::WaitUntil
    fn set_wait_until_strategy(&self, strategy: WaitUntilStrategy);

    /// Gets the strategy for [`ControlFlow::WaitUntil`].
    ///
    /// See [`WaitUntilStrategy`].
    ///
    /// [`ControlFlow::WaitUntil`]: crate::event_loop::ControlFlow::WaitUntil
    fn wait_until_strategy(&self) -> WaitUntilStrategy;

    /// Async version of [`ActiveEventLoop::create_custom_cursor()`] which waits until the
    /// cursor has completely finished loading.
    fn create_custom_cursor_async(&self, source: CustomCursorSource) -> CustomCursorFuture;

    /// Returns whether using [`CursorGrabMode::Locked`] returns raw, un-accelerated mouse input.
    ///
    /// [`CursorGrabMode::Locked`]: crate::window::CursorGrabMode::Locked
    fn is_cursor_lock_raw(&self) -> bool;

    /// Returns if the users device has multiple screens. Useful to check before prompting the user
    /// with [`EventLoopExtWeb::request_detailed_monitor_permission()`].
    ///
    /// Browsers might always return [`false`] to reduce fingerprinting.
    fn has_multiple_screens(&self) -> Result<bool, NotSupportedError>;

    /// Prompts the user for permission to query detailed information about available monitors. The
    /// returned [`MonitorPermissionFuture`] can be dropped without aborting the request.
    ///
    /// Check [`EventLoopExtWeb::has_multiple_screens()`] before unnecessarily prompting the user
    /// for such permissions.
    ///
    /// [`MonitorHandle`]s don't automatically make use of this after permission is granted. New
    /// [`MonitorHandle`]s have to be created instead.
    fn request_detailed_monitor_permission(&self) -> MonitorPermissionFuture;

    /// Returns whether the user has given permission to access detailed monitor information.
    ///
    /// [`MonitorHandle`]s don't automatically make use of detailed monitor information after
    /// permission is granted. New [`MonitorHandle`]s have to be created instead.
    fn has_detailed_monitor_permission(&self) -> bool;
}

impl ActiveEventLoopExtWeb for dyn ActiveEventLoop + '_ {
    #[inline]
    fn create_custom_cursor_async(&self, source: CustomCursorSource) -> CustomCursorFuture {
        let event_loop = self
            .as_any()
            .downcast_ref::<crate::platform_impl::ActiveEventLoop>()
            .expect("non Web event loop on Web");
        event_loop.create_custom_cursor_async(source)
    }

    #[inline]
    fn set_poll_strategy(&self, strategy: PollStrategy) {
        let event_loop = self
            .as_any()
            .downcast_ref::<crate::platform_impl::ActiveEventLoop>()
            .expect("non Web event loop on Web");
        event_loop.set_poll_strategy(strategy);
    }

    #[inline]
    fn poll_strategy(&self) -> PollStrategy {
        let event_loop = self
            .as_any()
            .downcast_ref::<crate::platform_impl::ActiveEventLoop>()
            .expect("non Web event loop on Web");
        event_loop.poll_strategy()
    }

    #[inline]
    fn set_wait_until_strategy(&self, strategy: WaitUntilStrategy) {
        let event_loop = self
            .as_any()
            .downcast_ref::<crate::platform_impl::ActiveEventLoop>()
            .expect("non Web event loop on Web");
        event_loop.set_wait_until_strategy(strategy);
    }

    #[inline]
    fn wait_until_strategy(&self) -> WaitUntilStrategy {
        let event_loop = self
            .as_any()
            .downcast_ref::<crate::platform_impl::ActiveEventLoop>()
            .expect("non Web event loop on Web");
        event_loop.wait_until_strategy()
    }

    #[inline]
    fn is_cursor_lock_raw(&self) -> bool {
        let event_loop = self
            .as_any()
            .downcast_ref::<crate::platform_impl::ActiveEventLoop>()
            .expect("non Web event loop on Web");
        event_loop.is_cursor_lock_raw()
    }

    #[inline]
    fn has_multiple_screens(&self) -> Result<bool, NotSupportedError> {
        let event_loop = self
            .as_any()
            .downcast_ref::<crate::platform_impl::ActiveEventLoop>()
            .expect("non Web event loop on Web");
        event_loop.has_multiple_screens()
    }

    #[inline]
    fn request_detailed_monitor_permission(&self) -> MonitorPermissionFuture {
        let event_loop = self
            .as_any()
            .downcast_ref::<crate::platform_impl::ActiveEventLoop>()
            .expect("non Web event loop on Web");
        MonitorPermissionFuture(event_loop.request_detailed_monitor_permission())
    }

    #[inline]
    fn has_detailed_monitor_permission(&self) -> bool {
        let event_loop = self
            .as_any()
            .downcast_ref::<crate::platform_impl::ActiveEventLoop>()
            .expect("non Web event loop on Web");
        event_loop.has_detailed_monitor_permission()
    }
}

/// Strategy used for [`ControlFlow::Poll`][crate::event_loop::ControlFlow::Poll].
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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

/// Strategy used for [`ControlFlow::WaitUntil`][crate::event_loop::ControlFlow::WaitUntil].
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum WaitUntilStrategy {
    /// Uses the [Prioritized Task Scheduling API] to queue the next event loop. If not available
    /// this will fallback to [`setTimeout()`].
    ///
    /// This strategy is commonly not affected by browser throttling unless the window is not
    /// focused.
    ///
    /// This is the default strategy.
    ///
    /// [Prioritized Task Scheduling API]: https://developer.mozilla.org/en-US/docs/Web/API/Prioritized_Task_Scheduling_API
    /// [`setTimeout()`]: https://developer.mozilla.org/en-US/docs/Web/API/setTimeout
    #[default]
    Scheduler,
    /// Equal to [`Scheduler`][Self::Scheduler] but wakes up the event loop from a [worker].
    ///
    /// This strategy is commonly not affected by browser throttling regardless of window focus.
    ///
    /// [worker]: https://developer.mozilla.org/en-US/docs/Web/API/Web_Workers_API
    Worker,
}

pub trait CustomCursorExtWeb {
    /// Returns if this cursor is an animation.
    fn is_animation(&self) -> bool;

    /// Creates a new cursor from a URL pointing to an image.
    /// It uses the [url css function](https://developer.mozilla.org/en-US/docs/Web/CSS/url),
    /// but browser support for image formats is inconsistent. Using [PNG] is recommended.
    ///
    /// [PNG]: https://en.wikipedia.org/wiki/PNG
    fn from_url(url: String, hotspot_x: u16, hotspot_y: u16) -> CustomCursorSource;

    /// Crates a new animated cursor from multiple [`CustomCursor`]s.
    /// Supplied `cursors` can't be empty or other animations.
    fn from_animation(
        duration: Duration,
        cursors: Vec<CustomCursor>,
    ) -> Result<CustomCursorSource, BadAnimation>;
}

impl CustomCursorExtWeb for CustomCursor {
    fn is_animation(&self) -> bool {
        self.inner.animation
    }

    fn from_url(url: String, hotspot_x: u16, hotspot_y: u16) -> CustomCursorSource {
        CustomCursorSource { inner: PlatformCustomCursorSource::Url { url, hotspot_x, hotspot_y } }
    }

    fn from_animation(
        duration: Duration,
        cursors: Vec<CustomCursor>,
    ) -> Result<CustomCursorSource, BadAnimation> {
        if cursors.is_empty() {
            return Err(BadAnimation::Empty);
        }

        if cursors.iter().any(CustomCursor::is_animation) {
            return Err(BadAnimation::Animation);
        }

        Ok(CustomCursorSource {
            inner: PlatformCustomCursorSource::Animation { duration, cursors },
        })
    }
}

/// An error produced when using [`CustomCursor::from_animation`] with invalid arguments.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
            Self::Animation => write!(f, "A supplied cursor is an animation"),
        }
    }
}

impl Error for BadAnimation {}

#[cfg(not(web_platform))]
struct PlatformCustomCursorFuture;

#[derive(Debug)]
pub struct CustomCursorFuture(pub(crate) PlatformCustomCursorFuture);

impl Future for CustomCursorFuture {
    type Output = Result<CustomCursor, CustomCursorError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.0).poll(cx).map_ok(|cursor| CustomCursor { inner: cursor })
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum CustomCursorError {
    Blob,
    Decode(String),
}

impl Display for CustomCursorError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Blob => write!(f, "failed to create `Blob`"),
            Self::Decode(error) => write!(f, "failed to decode image: {error}"),
        }
    }
}

impl Error for CustomCursorError {}

#[cfg(not(web_platform))]
struct PlatformMonitorPermissionFuture;

/// Can be dropped without aborting the request for detailed monitor permissions.
#[derive(Debug)]
pub struct MonitorPermissionFuture(pub(crate) PlatformMonitorPermissionFuture);

impl Future for MonitorPermissionFuture {
    type Output = Result<(), MonitorPermissionError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.0).poll(cx)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MonitorPermissionError {
    /// User has explicitly denied permission to query detailed monitor information.
    Denied,
    /// User has not decided to give permission to query detailed monitor information.
    Prompt,
    /// Browser does not support detailed monitor information.
    Unsupported,
}

impl Display for MonitorPermissionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            MonitorPermissionError::Denied => write!(
                f,
                "User has explicitly denied permission to query detailed monitor information"
            ),
            MonitorPermissionError::Prompt => write!(
                f,
                "User has not decided to give permission to query detailed monitor information"
            ),
            MonitorPermissionError::Unsupported => {
                write!(f, "Browser does not support detailed monitor information")
            },
        }
    }
}

impl Error for MonitorPermissionError {}

#[cfg(not(web_platform))]
struct PlatformHasMonitorPermissionFuture;

#[derive(Debug)]
pub struct HasMonitorPermissionFuture(PlatformHasMonitorPermissionFuture);

impl Future for HasMonitorPermissionFuture {
    type Output = bool;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.0).poll(cx)
    }
}

/// Additional methods on [`MonitorHandle`] that are specific to the Web.
pub trait MonitorHandleExtWeb {
    /// Returns whether the screen is internal to the device or external.
    ///
    /// External devices are generally manufactured separately from the device they are attached to
    /// and can be connected and disconnected as needed, whereas internal screens are part of
    /// the device and not intended to be disconnected.
    fn is_internal(&self) -> Option<bool>;

    /// Returns screen orientation data for this monitor.
    fn orientation(&self) -> OrientationData;

    /// Lock the screen orientation. The returned [`OrientationLockFuture`] can be dropped without
    /// aborting the request.
    ///
    /// Will fail if another locking call is in progress.
    fn request_lock(&self, orientation: OrientationLock) -> OrientationLockFuture;

    /// Unlock the screen orientation.
    ///
    /// Will fail if a locking call is in progress.
    fn unlock(&self) -> Result<(), OrientationLockError>;

    /// Returns whether this [`MonitorHandle`] was created using detailed monitor permissions. If
    /// [`false`] will always represent the current monitor the browser window is in instead of a
    /// specific monitor.
    ///
    /// See [`ActiveEventLoopExtWeb::request_detailed_monitor_permission()`].
    fn is_detailed(&self) -> bool;
}

impl MonitorHandleExtWeb for MonitorHandle {
    fn is_internal(&self) -> Option<bool> {
        self.inner.is_internal()
    }

    fn orientation(&self) -> OrientationData {
        self.inner.orientation()
    }

    fn request_lock(&self, orientation_lock: OrientationLock) -> OrientationLockFuture {
        OrientationLockFuture(self.inner.request_lock(orientation_lock))
    }

    fn unlock(&self) -> Result<(), OrientationLockError> {
        self.inner.unlock()
    }

    fn is_detailed(&self) -> bool {
        self.inner.is_detailed()
    }
}

/// Screen orientation data.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct OrientationData {
    /// The orientation.
    pub orientation: Orientation,
    /// [`true`] if the [`orientation`](Self::orientation) is flipped upside down.
    pub flipped: bool,
    /// The most natural orientation for the screen. Computer monitors are commonly naturally
    /// landscape mode, while mobile phones are commonly naturally portrait mode.
    pub natural: Orientation,
}

/// Screen orientation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Orientation {
    /// The screen's aspect ratio has a width greater than the height.
    Landscape,
    /// The screen's aspect ratio has a height greater than the width.
    Portrait,
}

/// Screen orientation lock options. Reoresents which orientations a user can use.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum OrientationLock {
    /// User is free to use any orientation.
    Any,
    /// User is locked to the most upright natural orientation for the screen. Computer monitors
    /// are commonly naturally landscape mode, while mobile phones are commonly
    /// naturally portrait mode.
    Natural,
    /// User is locked to landscape mode.
    Landscape {
        /// - [`None`]: User is locked to both upright or upside down landscape mode.
        /// - [`true`]: User is locked to upright landscape mode.
        /// - [`false`]: User is locked to upside down landscape mode.
        flipped: Option<bool>,
    },
    /// User is locked to portrait mode.
    Portrait {
        /// - [`None`]: User is locked to both upright or upside down portrait mode.
        /// - [`true`]: User is locked to upright portrait mode.
        /// - [`false`]: User is locked to upside down portrait mode.
        flipped: Option<bool>,
    },
}

#[cfg(not(web_platform))]
struct PlatformOrientationLockFuture;

/// Can be dropped without aborting the request to lock the screen.
#[derive(Debug)]
pub struct OrientationLockFuture(PlatformOrientationLockFuture);

impl Future for OrientationLockFuture {
    type Output = Result<(), OrientationLockError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.0).poll(cx)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum OrientationLockError {
    Unsupported,
    Busy,
}

impl Display for OrientationLockError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported => write!(f, "Locking the screen orientation is not supported"),
            Self::Busy => write!(f, "Another locking call is in progress"),
        }
    }
}

impl Error for OrientationLockError {}

/// Additional methods on [`FingerId`] that are specific to Web.
pub trait FingerIdExtWeb {
    /// Indicates if the finger represents the first contact in a multi-touch interaction.
    #[allow(clippy::wrong_self_convention)]
    fn is_primary(self) -> bool;
}

impl FingerIdExtWeb for FingerId {
    fn is_primary(self) -> bool {
        self.0.is_primary()
    }
}
