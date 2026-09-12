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
/// Note that this cannot be shared across threads (due to platform-dependent logic
/// forbidding it), as such it is neither [`Send`] nor [`Sync`]. If you need cross-thread access,
/// the [`Window`] created from this _can_ be sent to an other thread, and the
/// [`EventLoopProxy`] allows you to wake up an `EventLoop` from another thread.
///
/// [`Window`]: crate::window::Window
#[derive(Debug)]
pub struct EventLoop {
    pub(crate) event_loop: Box<dyn EventLoopProvider>,
    pub(crate) _marker: PhantomData<*mut ()>, // Not Send nor Sync
}

impl EventLoop {
    /// Create the event loop defaulting to the native backend.
    ///
    /// This is an alias of `EventLoop::builder().build()`.
    #[inline]
    pub fn new() -> Result<EventLoop, EventLoopError> {
        let native_event_loop = platform_impl::EventLoop::new(
            &mut platform_impl::PlatformSpecificEventLoopAttributes::default(),
        )?;
        Ok(Self::new_custom_provider(Box::new(native_event_loop)))
    }

    /// Create the event loop with a specified backend.
    #[inline]
    pub fn new_custom_provider(event_loop: Box<dyn EventLoopProvider>) -> EventLoop {
        Self { event_loop, _marker: PhantomData }
    }

    /// Run the event loop with the given application on the calling thread.
    ///
    /// For details see [`EventLoopProvider`].
    ///
    /// ## Returns
    ///
    /// The semantics of this function can be a bit confusing, because the way different platforms
    /// control their event loop varies significantly.
    ///
    /// On most platforms (Android, macOS, Orbital, X11, Wayland, Windows), this blocks the caller,
    /// runs the event loop internally, and then returns once [`ActiveEventLoop::exit`] is called.
    /// See [`run_app_on_demand`] for more detailed semantics.
    ///
    /// On iOS, this will register the application handler, and then call [`UIApplicationMain`]
    /// (which is the only way to run the system event loop), which never returns to the caller
    /// (the process instead exits after the handler has been dropped). See also
    /// [`run_app_never_return`].
    ///
    /// On the web, this works by registering the application handler, and then immediately
    /// returning to the caller. This is necessary because WebAssembly (and JavaScript) is always
    /// executed in the context of the browser's own (internal) event loop, and thus we need to
    /// return to avoid blocking that and allow events to later be delivered asynchronously. See
    /// also [`register_app`].
    ///
    /// If you call this function inside `fn main`, you usually do not need to think about these
    /// details.
    ///
    /// [`UIApplicationMain`]: https://developer.apple.com/documentation/uikit/uiapplicationmain(_:_:_:_:)-1yub7?language=objc
    /// [`run_app_on_demand`]: crate::event_loop::run_on_demand::EventLoopExtRunOnDemand::run_app_on_demand
    /// [`run_app_never_return`]: crate::event_loop::never_return::EventLoopExtNeverReturn::run_app_never_return
    /// [`register_app`]: crate::event_loop::register::EventLoopExtRegister::register_app
    /// [`EventLoopProvider`]: winit_core::event_loop::EventLoopProvider
    ///
    /// ## Static
    ///
    /// To alleviate the issues noted above, this function requires that you pass in a `'static`
    /// handler, to ensure that any state your application uses will be alive as long as the
    /// application is running.
    ///
    /// To be clear, you should avoid doing e.g. `event_loop.run_app(&mut app)?`, and prefer
    /// `event_loop.run_app(app)?` instead.
    ///
    /// If this requirement is prohibitive for you, consider using [`run_app_on_demand`] instead
    /// (though note that this is not available on iOS and web).
    #[inline]
    pub fn run_app(&mut self, app: Box<dyn ApplicationHandler>) -> Result<(), EventLoopError> {
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
        let _entered = tracing::debug_span!(
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

    /// Returns the platform specific `EventLoopProvider`.
    ///
    /// The `EventLoopProvider` implements `Any`, so this can be used to downcast to platform
    /// specific event loops.
    pub fn raw_event_loop_mut(&mut self) -> &mut dyn EventLoopProvider {
        self.event_loop.as_mut()
    }

    /// Returns the platform specific `EventLoopProvider`.
    ///
    /// The `EventLoopProvider` implements `Any`, so this can be used to downcast to platform
    /// specific event loops.
    pub fn raw_event_loop(&mut self) -> &dyn EventLoopProvider {
        self.event_loop.as_ref()
    }
}

impl EventLoopProvider for EventLoop {
    fn run_app(&mut self, app: Box<dyn ApplicationHandler>) -> Result<(), EventLoopError> {
        self.run_app(app)
    }

    fn create_proxy(&self) -> EventLoopProxy {
        self.create_proxy()
    }

    fn owned_display_handle(&self) -> OwnedDisplayHandle {
        self.owned_display_handle()
    }

    fn listen_device_events(&self, allowed: DeviceEvents) {
        self.listen_device_events(allowed);
    }

    fn set_control_flow(&self, control_flow: ControlFlow) {
        self.set_control_flow(control_flow);
    }

    fn create_custom_cursor(
        &self,
        custom_cursor: CustomCursorSource,
    ) -> Result<CustomCursor, RequestError> {
        self.create_custom_cursor(custom_cursor)
    }

    fn window_target(&self) -> &dyn ActiveEventLoop {
        self.event_loop.window_target()
    }
}

impl HasDisplayHandle for EventLoop {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        HasDisplayHandle::display_handle(self.event_loop.window_target().rwh_06_handle())
    }
}
