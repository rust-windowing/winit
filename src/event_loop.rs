//! The [`EventLoop`] struct and assorted supporting types, including
//! [`ControlFlow`].
//!
//! If you want to send custom events to the event loop, use
//! [`EventLoop::create_proxy`] to acquire an [`EventLoopProxy`] and call its
//! [`wake_up`][EventLoopProxy::wake_up] method. Then during handling the wake up
//! you can poll your event sources.
//!
//! See the root-level documentation for information on how to create and use an event loop to
//! handle events.
use std::marker::PhantomData;
#[cfg(any(x11_platform, wayland_platform))]
use std::os::unix::io::{AsFd, AsRawFd, BorrowedFd, RawFd};
use std::sync::atomic::{AtomicBool, Ordering};

use rwh_06::{DisplayHandle, HandleError, HasDisplayHandle};
pub use winit_core::event_loop::*;

use crate::application::ApplicationHandler;
use crate::cursor::{CustomCursor, CustomCursorSource};
use crate::error::{EventLoopError, RequestError};
use crate::platform_impl;

/// Provides a way to retrieve events from the system and from the windows that were registered to
/// the events loop.
///
/// An `EventLoop` can be seen more or less as a "context". Calling [`EventLoop::new`]
/// initializes everything that will be required to create windows. For example on Linux creating
/// an event loop opens a connection to the X or Wayland server.
///
/// To wake up an `EventLoop` from a another thread, see the [`EventLoopProxy`] docs.
///
/// Note that this cannot be shared across threads (due to platform-dependant logic
/// forbidding it), as such it is neither [`Send`] nor [`Sync`]. If you need cross-thread access,
/// the [`Window`] created from this _can_ be sent to an other thread, and the
/// [`EventLoopProxy`] allows you to wake up an `EventLoop` from another thread.
///
/// [`Window`]: crate::window::Window
#[derive(Debug)]
pub struct EventLoop {
    pub(crate) event_loop: platform_impl::EventLoop,
    pub(crate) _marker: PhantomData<*mut ()>, // Not Send nor Sync
}

/// Object that allows building the event loop.
///
/// This is used to make specifying options that affect the whole application
/// easier. But note that constructing multiple event loops is not supported.
///
/// This can be created using [`EventLoop::builder`].
#[derive(Default, Debug, PartialEq, Eq, Hash)]
pub struct EventLoopBuilder {
    pub(crate) platform_specific: platform_impl::PlatformSpecificEventLoopAttributes,
}

static EVENT_LOOP_CREATED: AtomicBool = AtomicBool::new(false);

impl EventLoopBuilder {
    /// Builds a new event loop.
    ///
    /// ***For cross-platform compatibility, the [`EventLoop`] must be created on the main thread,
    /// and only once per application.***
    ///
    /// Calling this function will result in display backend initialisation.
    ///
    /// ## Panics
    ///
    /// Attempting to create the event loop off the main thread will panic. This
    /// restriction isn't strictly necessary on all platforms, but is imposed to
    /// eliminate any nasty surprises when porting to platforms that require it.
    /// `EventLoopBuilderExt::with_any_thread` functions are exposed in the relevant
    /// [`platform`] module if the target platform supports creating an event
    /// loop on any thread.
    ///
    /// ## Platform-specific
    ///
    /// - **Wayland/X11:** to prevent running under `Wayland` or `X11` unset `WAYLAND_DISPLAY` or
    ///   `DISPLAY` respectively when building the event loop.
    /// - **Android:** must be configured with an `AndroidApp` from `android_main()` by calling
    ///   [`.with_android_app(app)`] before calling `.build()`, otherwise it'll panic.
    ///
    /// [`platform`]: crate::platform
    #[cfg_attr(
        android_platform,
        doc = "[`.with_android_app(app)`]: \
               crate::platform::android::EventLoopBuilderExtAndroid::with_android_app"
    )]
    #[cfg_attr(
        not(android_platform),
        doc = "[`.with_android_app(app)`]: #only-available-on-android"
    )]
    #[inline]
    pub fn build(&mut self) -> Result<EventLoop, EventLoopError> {
        let _span = tracing::debug_span!("winit::EventLoopBuilder::build").entered();

        if EVENT_LOOP_CREATED.swap(true, Ordering::Relaxed) {
            return Err(EventLoopError::RecreationAttempt);
        }

        // Certain platforms accept a mutable reference in their API.
        #[allow(clippy::unnecessary_mut_passed)]
        Ok(EventLoop {
            event_loop: platform_impl::EventLoop::new(&mut self.platform_specific)?,
            _marker: PhantomData,
        })
    }

    #[cfg(web_platform)]
    pub(crate) fn allow_event_loop_recreation() {
        EVENT_LOOP_CREATED.store(false, Ordering::Relaxed);
    }
}

impl EventLoop {
    /// Create the event loop.
    ///
    /// This is an alias of `EventLoop::builder().build()`.
    #[inline]
    pub fn new() -> Result<EventLoop, EventLoopError> {
        Self::builder().build()
    }

    /// Start building a new event loop.
    ///
    /// This returns an [`EventLoopBuilder`], to allow configuring the event loop before creation.
    ///
    /// To get the actual event loop, call [`build`][EventLoopBuilder::build] on that.
    #[inline]
    pub fn builder() -> EventLoopBuilder {
        EventLoopBuilder { platform_specific: Default::default() }
    }

    /// Run the application with the event loop on the calling thread.
    ///
    /// The `app` is dropped when the event loop is shut down.
    ///
    /// ## Event loop flow
    ///
    /// This function internally handles the different parts of a traditional event-handling loop.
    /// You can imagine this method as being implemented like this:
    ///
    /// ```rust,ignore
    /// let mut start_cause = StartCause::Init;
    ///
    /// // Run the event loop.
    /// while !event_loop.exiting() {
    ///     // Wake up.
    ///     app.new_events(event_loop, start_cause);
    ///
    ///     // Indicate that surfaces can now safely be created.
    ///     if start_cause == StartCause::Init {
    ///         app.can_create_surfaces(event_loop);
    ///     }
    ///
    ///     // Handle proxy wake-up event.
    ///     if event_loop.proxy_wake_up_set() {
    ///         event_loop.proxy_wake_up_clear();
    ///         app.proxy_wake_up(event_loop);
    ///     }
    ///
    ///     // Handle actions done by the user / system such as moving the cursor, resizing the
    ///     // window, changing the window theme, etc.
    ///     for event in event_loop.events() {
    ///         match event {
    ///             window event => app.window_event(event_loop, window_id, event),
    ///             device event => app.device_event(event_loop, device_id, event),
    ///         }
    ///     }
    ///
    ///     // Handle redraws.
    ///     for window_id in event_loop.pending_redraws() {
    ///         app.window_event(event_loop, window_id, WindowEvent::RedrawRequested);
    ///     }
    ///
    ///     // Done handling events, wait until we're woken up again.
    ///     app.about_to_wait(event_loop);
    ///     start_cause = event_loop.wait_if_necessary();
    /// }
    ///
    /// // Finished running, drop application state.
    /// drop(app);
    /// ```
    ///
    /// This is of course a very coarse-grained overview, and leaves out timing details like
    /// [`ControlFlow::WaitUntil`] and life-cycle methods like [`ApplicationHandler::resumed`], but
    /// it should give you an idea of how things fit together.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS:** Will never return to the caller and so values not passed to this function will
    ///   *not* be dropped before the process exits.
    /// - **Web:** Will _act_ as if it never returns to the caller by throwing a Javascript
    ///   exception (that Rust doesn't see) that will also mean that the rest of the function is
    ///   never executed and any values not passed to this function will *not* be dropped.
    ///
    ///   Web applications are recommended to use
    #[cfg_attr(
        web_platform,
        doc = "  [`EventLoopExtWeb::spawn_app()`][crate::platform::web::EventLoopExtWeb::spawn_app()]"
    )]
    #[cfg_attr(not(web_platform), doc = "  `EventLoopExtWeb::spawn_app()`")]
    ///   [^1] instead of [`run_app()`] to avoid the need for the Javascript exception trick, and to
    ///   make   it clearer that the event loop runs asynchronously (via the browser's own,
    ///   internal, event   loop) and doesn't block the current thread of execution like it does
    ///   on other platforms.
    ///
    ///   This function won't be available with `target_feature = "exception-handling"`.
    ///
    /// [^1]: `spawn_app()` is only available on the Web platform.
    ///
    /// [`set_control_flow()`]: ActiveEventLoop::set_control_flow()
    /// [`run_app()`]: Self::run_app()
    #[inline]
    #[cfg(not(all(web_platform, target_feature = "exception-handling")))]
    pub fn run_app<A: ApplicationHandler>(self, app: A) -> Result<(), EventLoopError> {
        self.event_loop.run_app(app)
    }

    /// Creates an [`EventLoopProxy`] that can be used to dispatch user events
    /// to the main event loop, possibly from another thread.
    pub fn create_proxy(&self) -> EventLoopProxy {
        self.event_loop.window_target().create_proxy()
    }

    /// Gets a persistent reference to the underlying platform display.
    ///
    /// See the [`OwnedDisplayHandle`] type for more information.
    pub fn owned_display_handle(&self) -> OwnedDisplayHandle {
        self.event_loop.window_target().owned_display_handle()
    }

    /// Change if or when [`DeviceEvent`]s are captured.
    ///
    /// See [`ActiveEventLoop::listen_device_events`] for details.
    ///
    /// [`DeviceEvent`]: crate::event::DeviceEvent
    pub fn listen_device_events(&self, allowed: DeviceEvents) {
        let _span = tracing::debug_span!(
            "winit::EventLoop::listen_device_events",
            allowed = ?allowed
        )
        .entered();
        self.event_loop.window_target().listen_device_events(allowed)
    }

    /// Sets the [`ControlFlow`].
    pub fn set_control_flow(&self, control_flow: ControlFlow) {
        self.event_loop.window_target().set_control_flow(control_flow);
    }

    /// Create custom cursor.
    ///
    /// ## Platform-specific
    ///
    /// **iOS / Android / Orbital:** Unsupported.
    pub fn create_custom_cursor(
        &self,
        custom_cursor: CustomCursorSource,
    ) -> Result<CustomCursor, RequestError> {
        self.event_loop.window_target().create_custom_cursor(custom_cursor)
    }
}

impl HasDisplayHandle for EventLoop {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        HasDisplayHandle::display_handle(self.event_loop.window_target().rwh_06_handle())
    }
}

#[cfg(any(x11_platform, wayland_platform))]
impl AsFd for EventLoop {
    /// Get the underlying [EventLoop]'s `fd` which you can register
    /// into other event loop, like [`calloop`] or [`mio`]. When doing so, the
    /// loop must be polled with the [`pump_app_events`] API.
    ///
    /// [`calloop`]: https://crates.io/crates/calloop
    /// [`mio`]: https://crates.io/crates/mio
    /// [`pump_app_events`]: crate::event_loop::pump_events::EventLoopExtPumpEvents::pump_app_events
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.event_loop.as_fd()
    }
}

#[cfg(any(x11_platform, wayland_platform))]
impl AsRawFd for EventLoop {
    /// Get the underlying [EventLoop]'s raw `fd` which you can register
    /// into other event loop, like [`calloop`] or [`mio`]. When doing so, the
    /// loop must be polled with the [`pump_app_events`] API.
    ///
    /// [`calloop`]: https://crates.io/crates/calloop
    /// [`mio`]: https://crates.io/crates/mio
    /// [`pump_app_events`]: crate::event_loop::pump_events::EventLoopExtPumpEvents::pump_app_events
    fn as_raw_fd(&self) -> RawFd {
        self.event_loop.as_raw_fd()
    }
}

#[cfg(any(
    windows_platform,
    macos_platform,
    android_platform,
    x11_platform,
    wayland_platform,
    docsrs,
))]
impl winit_core::event_loop::pump_events::EventLoopExtPumpEvents for EventLoop {
    fn pump_app_events<A: ApplicationHandler>(
        &mut self,
        timeout: Option<std::time::Duration>,
        app: A,
    ) -> winit_core::event_loop::pump_events::PumpStatus {
        self.event_loop.pump_app_events(timeout, app)
    }
}

#[allow(unused_imports)]
#[cfg(any(
    windows_platform,
    macos_platform,
    android_platform,
    x11_platform,
    wayland_platform,
    docsrs,
))]
impl winit_core::event_loop::run_on_demand::EventLoopExtRunOnDemand for EventLoop {
    fn run_app_on_demand<A: ApplicationHandler>(&mut self, app: A) -> Result<(), EventLoopError> {
        self.event_loop.run_app_on_demand(app)
    }
}

/// ```compile_error
/// use winit::event_loop::run_on_demand::EventLoopExtRunOnDemand;
/// use winit::event_loop::EventLoop;
///
/// let mut event_loop = EventLoop::new().unwrap();
/// event_loop.run_app_on_demand(|_, _| {
///     // Attempt to run the event loop re-entrantly; this must fail.
///     event_loop.run_app_on_demand(|_, _| {});
/// });
/// ```
#[allow(dead_code)]
fn test_run_on_demand_cannot_access_event_loop() {}
