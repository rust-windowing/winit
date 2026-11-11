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
use winit_core::error::CustomCursorError;
pub use winit_core::event_loop::*;

use crate::application::ApplicationHandler;
use crate::cursor::{CustomCursor, CustomCursorSource};
use crate::error::EventLoopError;
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
/// Use [`EventLoop::from_backend`] to supply an external [`EventLoopProvider`] instead of
/// the built-in backend selected for the target platform.
///
/// [`Window`]: crate::window::Window
#[derive(Debug)]
pub struct EventLoop {
    pub(crate) event_loop: Box<dyn EventLoopProvider>,
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

impl EventLoopBuilder {
    /// Builds a new event loop.
    ///
    /// ***For cross-platform compatibility, the [`EventLoop`] must be created on the main thread,
    /// and only once per application.***
    ///
    /// Calling this function will result in display backend initialisation.
    ///
    /// Returns [`EventLoopError::NotSupported`] when no built-in backend is available.
    /// Use [`EventLoop::from_backend`] to provide an external backend in that case.
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
        let _entered = tracing::debug_span!("winit::EventLoopBuilder::build").entered();

        Ok(EventLoop {
            event_loop: platform_impl::create_event_loop(&mut self.platform_specific)?,
            _marker: PhantomData,
        })
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

    /// Create an [`EventLoop`] from an external [`EventLoopProvider`] backend.
    ///
    /// On targets without a built-in backend, disable Winit's default features and supply a
    /// backend through this constructor.
    ///
    /// ```no_run
    /// use winit::application::ApplicationHandler;
    /// use winit::error::EventLoopError;
    /// use winit::event_loop::{ControlFlow, EventLoop, EventLoopProvider};
    ///
    /// fn run(
    ///     backend: Box<dyn EventLoopProvider>,
    ///     app: Box<dyn ApplicationHandler>,
    /// ) -> Result<(), EventLoopError> {
    ///     let event_loop = EventLoop::from_backend(backend);
    ///     event_loop.set_control_flow(ControlFlow::Wait);
    ///     event_loop.run_app(app)
    /// }
    /// ```
    #[inline]
    pub fn from_backend(backend: Box<dyn EventLoopProvider>) -> Self {
        Self { event_loop: backend, _marker: PhantomData }
    }

    /// Access the concrete backend using `cast_ref` on the [`EventLoopProvider`] trait object.
    #[inline]
    pub fn backend(&self) -> &dyn EventLoopProvider {
        &*self.event_loop
    }

    /// Mutably access the backend using `cast_mut` on the [`EventLoopProvider`] trait object.
    #[inline]
    pub fn backend_mut(&mut self) -> &mut dyn EventLoopProvider {
        &mut *self.event_loop
    }

    /// Recover the owned [`EventLoopProvider`].
    #[inline]
    pub fn into_backend(self) -> Box<dyn EventLoopProvider> {
        self.event_loop
    }

    /// Access the event-pumping extension, where available.
    ///
    /// ```no_run
    /// use std::time::Duration;
    ///
    /// use winit::application::ApplicationHandler;
    /// use winit::event_loop::EventLoop;
    ///
    /// fn pump(event_loop: &mut EventLoop, app: &mut dyn ApplicationHandler) {
    ///     if let Some(pump) = event_loop.pump_events_ext() {
    ///         let _status = pump.pump_app_events(Some(Duration::ZERO), app);
    ///     }
    /// }
    /// ```
    #[inline(always)]
    pub fn pump_events_ext(&mut self) -> Option<&mut dyn pump_events::EventLoopExtPumpEvents> {
        self.event_loop.pump_events_ext()
    }

    /// Access the run-on-demand extension, where available.
    #[inline(always)]
    pub fn run_on_demand_ext(&mut self) -> Option<&mut dyn run_on_demand::EventLoopExtRunOnDemand> {
        self.event_loop.run_on_demand_ext()
    }

    /// Convert into an event loop that registers callbacks with the system event loop.
    ///
    /// Returns `Err(self)` when unsupported.
    ///
    /// ```no_run
    /// use winit::application::ApplicationHandler;
    /// use winit::error::EventLoopError;
    /// use winit::event_loop::EventLoop;
    ///
    /// fn start(
    ///     event_loop: EventLoop,
    ///     app: Box<dyn ApplicationHandler>,
    /// ) -> Result<(), EventLoopError> {
    ///     match event_loop.into_register() {
    ///         Ok(registration) => {
    ///             registration.register_app(app);
    ///             Ok(())
    ///         },
    ///         Err(event_loop) => event_loop.run_app(app),
    ///     }
    /// }
    /// ```
    #[inline]
    pub fn into_register(self) -> Result<Box<dyn register::EventLoopExtRegister>, Self> {
        self.event_loop
            .into_register()
            .map_err(|event_loop| Self { event_loop, _marker: self._marker })
    }

    /// Convert into an event loop whose execution never returns.
    ///
    /// Returns `Err(self)` when unsupported.
    #[inline]
    pub fn into_never_return(self) -> Result<Box<dyn never_return::EventLoopExtNeverReturn>, Self> {
        self.event_loop
            .into_never_return()
            .map_err(|event_loop| Self { event_loop, _marker: self._marker })
    }

    /// Access the underlying pollable file descriptor, where available.
    #[cfg(unix)]
    #[inline(always)]
    pub fn as_fd_ext(&self) -> Option<&dyn std::os::fd::AsFd> {
        self.event_loop.as_fd_ext()
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
    /// To alleviate the issues noted above, this function requires a `'static` handler, to ensure
    /// that any state your application uses will be alive as long as the application is running.
    ///
    /// If this requirement is prohibitive for you, consider using [`run_app_on_demand`] via
    /// [`Self::run_on_demand_ext`] instead (though note that this is not available on iOS and web).
    #[inline]
    pub fn run_app(self, app: Box<dyn ApplicationHandler>) -> Result<(), EventLoopError> {
        self.event_loop.run_app(app)
    }

    /// Creates an [`EventLoopProxy`] that can be used to dispatch user events
    /// to the main event loop, possibly from another thread.
    pub fn create_proxy(&self) -> EventLoopProxy {
        self.event_loop.create_proxy()
    }

    /// Gets a persistent reference to the underlying platform display.
    ///
    /// See the [`OwnedDisplayHandle`] type for more information.
    pub fn owned_display_handle(&self) -> OwnedDisplayHandle {
        self.event_loop.owned_display_handle()
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
        self.event_loop.listen_device_events(allowed)
    }

    /// Sets the [`ControlFlow`].
    pub fn set_control_flow(&self, control_flow: ControlFlow) {
        self.event_loop.set_control_flow(control_flow);
    }

    /// Create custom cursor.
    ///
    /// ## Platform-specific
    ///
    /// **iOS / Android / Orbital:** Unsupported.
    pub fn create_custom_cursor(
        &self,
        custom_cursor: CustomCursorSource,
    ) -> Result<CustomCursor, CustomCursorError> {
        self.event_loop.create_custom_cursor(custom_cursor)
    }
}

impl EventLoopProvider for EventLoop {
    fn run_app(self: Box<Self>, app: Box<dyn ApplicationHandler>) -> Result<(), EventLoopError> {
        self.event_loop.run_app(app)
    }

    #[inline(always)]
    fn pump_events_ext(&mut self) -> Option<&mut dyn pump_events::EventLoopExtPumpEvents> {
        self.event_loop.pump_events_ext()
    }

    #[inline(always)]
    fn run_on_demand_ext(&mut self) -> Option<&mut dyn run_on_demand::EventLoopExtRunOnDemand> {
        self.event_loop.run_on_demand_ext()
    }

    fn into_register(
        self: Box<Self>,
    ) -> Result<Box<dyn register::EventLoopExtRegister>, Box<dyn EventLoopProvider>> {
        (*self)
            .into_register()
            .map_err(|event_loop| Box::new(event_loop) as Box<dyn EventLoopProvider>)
    }

    fn into_never_return(
        self: Box<Self>,
    ) -> Result<Box<dyn never_return::EventLoopExtNeverReturn>, Box<dyn EventLoopProvider>> {
        (*self)
            .into_never_return()
            .map_err(|event_loop| Box::new(event_loop) as Box<dyn EventLoopProvider>)
    }

    #[cfg(unix)]
    #[inline(always)]
    fn as_fd_ext(&self) -> Option<&dyn std::os::fd::AsFd> {
        self.event_loop.as_fd_ext()
    }

    fn create_proxy(&self) -> EventLoopProxy {
        self.create_proxy()
    }

    fn owned_display_handle(&self) -> OwnedDisplayHandle {
        self.owned_display_handle()
    }

    fn rwh_06_handle(&self) -> &dyn HasDisplayHandle {
        self.event_loop.rwh_06_handle()
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
    ) -> Result<CustomCursor, CustomCursorError> {
        self.create_custom_cursor(custom_cursor)
    }
}

impl HasDisplayHandle for EventLoop {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        self.event_loop.display_handle()
    }
}

#[cfg(android_platform)]
impl winit_android::EventLoopExtAndroid for EventLoop {
    fn android_app(&self) -> &winit_android::activity::AndroidApp {
        &self
            .backend()
            .cast_ref::<winit_android::EventLoop>()
            .expect("Not an Android backend")
            .android_app
    }
}

#[cfg(android_platform)]
impl winit_android::EventLoopBuilderExtAndroid for EventLoopBuilder {
    fn with_android_app(&mut self, app: winit_android::activity::AndroidApp) -> &mut Self {
        self.platform_specific.android_app = Some(app);
        self
    }

    fn handle_volume_keys(&mut self) -> &mut Self {
        self.platform_specific.ignore_volume_keys = false;
        self
    }
}

#[cfg(macos_platform)]
impl winit_appkit::EventLoopBuilderExtMacOS for EventLoopBuilder {
    #[inline]
    fn with_activation_policy(
        &mut self,
        activation_policy: winit_appkit::ActivationPolicy,
    ) -> &mut Self {
        self.platform_specific.activation_policy = Some(activation_policy);
        self
    }

    #[inline]
    fn with_default_menu(&mut self, enable: bool) -> &mut Self {
        self.platform_specific.default_menu = enable;
        self
    }

    #[inline]
    fn with_activate_ignoring_other_apps(&mut self, ignore: bool) -> &mut Self {
        self.platform_specific.activate_ignoring_other_apps = ignore;
        self
    }
}

#[cfg(wayland_platform)]
impl winit_wayland::EventLoopExtWayland for EventLoop {
    #[inline]
    fn is_wayland(&self) -> bool {
        matches!(
            self.display_handle().map(|handle| handle.as_raw()),
            Ok(rwh_06::RawDisplayHandle::Wayland(_))
        )
    }
}

#[cfg(wayland_platform)]
impl winit_wayland::EventLoopBuilderExtWayland for EventLoopBuilder {
    #[inline]
    fn with_wayland(&mut self) -> &mut Self {
        self.platform_specific.forced_backend = Some(crate::platform_impl::Backend::Wayland);
        self
    }

    #[inline]
    fn with_any_thread(&mut self, any_thread: bool) -> &mut Self {
        self.platform_specific.any_thread = any_thread;
        self
    }
}

#[cfg(web_platform)]
impl winit_web::EventLoopExtWeb for EventLoop {
    fn set_poll_strategy(&self, strategy: winit_web::PollStrategy) {
        self.backend()
            .cast_ref::<winit_web::EventLoop>()
            .expect("Not a Web backend")
            .set_poll_strategy(strategy);
    }

    fn poll_strategy(&self) -> winit_web::PollStrategy {
        self.backend()
            .cast_ref::<winit_web::EventLoop>()
            .expect("Not a Web backend")
            .poll_strategy()
    }

    fn set_wait_until_strategy(&self, strategy: winit_web::WaitUntilStrategy) {
        self.backend()
            .cast_ref::<winit_web::EventLoop>()
            .expect("Not a Web backend")
            .set_wait_until_strategy(strategy);
    }

    fn wait_until_strategy(&self) -> winit_web::WaitUntilStrategy {
        self.backend()
            .cast_ref::<winit_web::EventLoop>()
            .expect("Not a Web backend")
            .wait_until_strategy()
    }

    fn has_multiple_screens(&self) -> Result<bool, winit_core::error::NotSupportedError> {
        self.backend()
            .cast_ref::<winit_web::EventLoop>()
            .ok_or_else(|| crate::error::NotSupportedError::new("Not a Web backend"))?
            .has_multiple_screens()
    }

    fn request_detailed_monitor_permission(&self) -> winit_web::MonitorPermissionFuture {
        self.backend()
            .cast_ref::<winit_web::EventLoop>()
            .expect("Not a Web backend")
            .request_detailed_monitor_permission()
    }

    fn has_detailed_monitor_permission(&self) -> winit_web::HasMonitorPermissionFuture {
        self.backend()
            .cast_ref::<winit_web::EventLoop>()
            .expect("Not a Web backend")
            .has_detailed_monitor_permission()
    }
}

#[cfg(windows_platform)]
impl winit_win32::EventLoopBuilderExtWindows for EventLoopBuilder {
    #[inline]
    fn with_any_thread(&mut self, any_thread: bool) -> &mut Self {
        self.platform_specific.any_thread = any_thread;
        self
    }

    #[inline]
    fn with_dpi_aware(&mut self, dpi_aware: bool) -> &mut Self {
        self.platform_specific.dpi_aware = dpi_aware;
        self
    }

    #[inline]
    fn with_msg_hook<F>(&mut self, callback: F) -> &mut Self
    where
        F: FnMut(*const core::ffi::c_void) -> bool + 'static,
    {
        self.platform_specific.msg_hook = Some(Box::new(callback));
        self
    }
}

#[cfg(x11_platform)]
impl winit_x11::EventLoopExtX11 for EventLoop {
    #[inline]
    fn is_x11(&self) -> bool {
        matches!(
            self.display_handle().map(|handle| handle.as_raw()),
            Ok(rwh_06::RawDisplayHandle::Xlib(_) | rwh_06::RawDisplayHandle::Xcb(_))
        )
    }
}

#[cfg(x11_platform)]
impl winit_x11::EventLoopBuilderExtX11 for EventLoopBuilder {
    #[inline]
    fn with_x11(&mut self) -> &mut Self {
        self.platform_specific.forced_backend = Some(crate::platform_impl::Backend::X);
        self
    }

    #[inline]
    fn with_any_thread(&mut self, any_thread: bool) -> &mut Self {
        self.platform_specific.any_thread = any_thread;
        self
    }
}

/// ```compile_fail,E0499
/// use winit::application::ApplicationHandler;
/// use winit::event::WindowEvent;
/// use winit::event_loop::{ActiveEventLoop, EventLoop};
/// use winit::window::WindowId;
///
/// struct App<'a>(&'a mut EventLoop);
/// impl ApplicationHandler for App<'_> {
///     fn can_create_surfaces(&mut self, _: &dyn ActiveEventLoop) {}
///     fn window_event(&mut self, _: &dyn ActiveEventLoop, _: WindowId, _: WindowEvent) {}
/// }
///
/// let mut event_loop = EventLoop::new().unwrap();
/// let mut app = App(&mut event_loop);
/// // Attempt to access `event_loop` while `app` borrows it; this must fail.
/// event_loop.run_on_demand_ext().unwrap().run_app_on_demand(&mut app).unwrap();
/// ```
#[allow(dead_code)]
fn test_run_on_demand_cannot_access_event_loop() {}

#[cfg(all(test, not(builtin_platform)))]
mod tests {
    use super::EventLoop;
    use crate::error::EventLoopError;

    #[test]
    fn new_returns_not_supported_without_builtin_backend() {
        assert!(matches!(EventLoop::new(), Err(EventLoopError::NotSupported(_))));
    }
}
