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
use std::fmt;
use std::marker::PhantomData;
#[cfg(any(x11_platform, wayland_platform))]
use std::os::unix::io::{AsFd, AsRawFd, BorrowedFd, RawFd};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
#[cfg(not(web_platform))]
use std::time::{Duration, Instant};

#[cfg(web_platform)]
use web_time::{Duration, Instant};

use crate::application::ApplicationHandler;
use crate::error::{EventLoopError, OsError};
use crate::monitor::MonitorHandle;
use crate::platform_impl;
use crate::window::{CustomCursor, CustomCursorSource, Window, WindowAttributes};

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
pub struct EventLoop {
    pub(crate) event_loop: platform_impl::EventLoop,
    pub(crate) _marker: PhantomData<*mut ()>, // Not Send nor Sync
}

/// Target that associates windows with an [`EventLoop`].
///
/// This type exists to allow you to create new windows while Winit executes
/// your callback.
pub struct ActiveEventLoop {
    pub(crate) p: platform_impl::ActiveEventLoop,
    pub(crate) _marker: PhantomData<*mut ()>, // Not Send nor Sync
}

/// Object that allows building the event loop.
///
/// This is used to make specifying options that affect the whole application
/// easier. But note that constructing multiple event loops is not supported.
///
/// This can be created using [`EventLoop::builder`].
#[derive(Default)]
pub struct EventLoopBuilder {
    pub(crate) platform_specific: platform_impl::PlatformSpecificEventLoopAttributes,
}

static EVENT_LOOP_CREATED: AtomicBool = AtomicBool::new(false);

impl EventLoopBuilder {
    /// Start building a new event loop.
    #[inline]
    #[deprecated = "use `EventLoop::builder` instead"]
    pub fn new() -> Self {
        EventLoop::builder()
    }
}

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
    /// `EventLoopBuilderExt::any_thread` functions are exposed in the relevant
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

impl fmt::Debug for EventLoop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("EventLoop { .. }")
    }
}

impl fmt::Debug for ActiveEventLoop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("ActiveEventLoop { .. }")
    }
}

/// Set through [`ActiveEventLoop::set_control_flow()`].
///
/// Indicates the desired behavior of the event loop after [`about_to_wait`] is called.
///
/// Defaults to [`Wait`].
///
/// [`Wait`]: Self::Wait
/// [`about_to_wait`]: crate::application::ApplicationHandler::about_to_wait
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum ControlFlow {
    /// When the current loop iteration finishes, immediately begin a new iteration regardless of
    /// whether or not new events are available to process.
    Poll,

    /// When the current loop iteration finishes, suspend the thread until another event arrives.
    #[default]
    Wait,

    /// When the current loop iteration finishes, suspend the thread until either another event
    /// arrives or the given time is reached.
    ///
    /// Useful for implementing efficient timers. Applications which want to render at the
    /// display's native refresh rate should instead use [`Poll`] and the VSync functionality
    /// of a graphics API to reduce odds of missed frames.
    ///
    /// [`Poll`]: Self::Poll
    WaitUntil(Instant),
}

impl ControlFlow {
    /// Creates a [`ControlFlow`] that waits until a timeout has expired.
    ///
    /// In most cases, this is set to [`WaitUntil`]. However, if the timeout overflows, it is
    /// instead set to [`Wait`].
    ///
    /// [`WaitUntil`]: Self::WaitUntil
    /// [`Wait`]: Self::Wait
    pub fn wait_duration(timeout: Duration) -> Self {
        match Instant::now().checked_add(timeout) {
            Some(instant) => Self::WaitUntil(instant),
            None => Self::Wait,
        }
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
}

impl EventLoop {
    /// Run the application with the event loop on the calling thread.
    ///
    /// See the [`set_control_flow()`] docs on how to change the event loop's behavior.
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
        any(web_platform, docsrs),
        doc = "  [`EventLoopExtWeb::spawn_app()`][crate::platform::web::EventLoopExtWeb::spawn_app()]"
    )]
    #[cfg_attr(not(any(web_platform, docsrs)), doc = "  `EventLoopExtWeb::spawn_app()`")]
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
    pub fn run_app<A: ApplicationHandler>(self, app: &mut A) -> Result<(), EventLoopError> {
        self.event_loop.run_app(app)
    }

    /// Creates an [`EventLoopProxy`] that can be used to dispatch user events
    /// to the main event loop, possibly from another thread.
    pub fn create_proxy(&self) -> EventLoopProxy {
        EventLoopProxy { event_loop_proxy: self.event_loop.window_target().p.create_proxy() }
    }

    /// Gets a persistent reference to the underlying platform display.
    ///
    /// See the [`OwnedDisplayHandle`] type for more information.
    pub fn owned_display_handle(&self) -> OwnedDisplayHandle {
        OwnedDisplayHandle { platform: self.event_loop.window_target().p.owned_display_handle() }
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

        self.event_loop.window_target().p.listen_device_events(allowed);
    }

    /// Sets the [`ControlFlow`].
    pub fn set_control_flow(&self, control_flow: ControlFlow) {
        self.event_loop.window_target().p.set_control_flow(control_flow)
    }

    /// Create a window.
    ///
    /// Creating window without event loop running often leads to improper window creation;
    /// use [`ActiveEventLoop::create_window`] instead.
    #[deprecated = "use `ActiveEventLoop::create_window` instead"]
    #[inline]
    pub fn create_window(&self, window_attributes: WindowAttributes) -> Result<Window, OsError> {
        let _span = tracing::debug_span!(
            "winit::EventLoop::create_window",
            window_attributes = ?window_attributes
        )
        .entered();

        let window =
            platform_impl::Window::new(&self.event_loop.window_target().p, window_attributes)?;
        Ok(Window { window })
    }

    /// Create custom cursor.
    pub fn create_custom_cursor(&self, custom_cursor: CustomCursorSource) -> CustomCursor {
        self.event_loop.window_target().p.create_custom_cursor(custom_cursor)
    }
}

#[cfg(feature = "rwh_06")]
impl rwh_06::HasDisplayHandle for EventLoop {
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        rwh_06::HasDisplayHandle::display_handle(self.event_loop.window_target())
    }
}

#[cfg(feature = "rwh_05")]
unsafe impl rwh_05::HasRawDisplayHandle for EventLoop {
    /// Returns a [`rwh_05::RawDisplayHandle`] for the event loop.
    fn raw_display_handle(&self) -> rwh_05::RawDisplayHandle {
        rwh_05::HasRawDisplayHandle::raw_display_handle(self.event_loop.window_target())
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
    /// [`pump_app_events`]: crate::platform::pump_events::EventLoopExtPumpEvents::pump_app_events
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
    /// [`pump_app_events`]: crate::platform::pump_events::EventLoopExtPumpEvents::pump_app_events
    fn as_raw_fd(&self) -> RawFd {
        self.event_loop.as_raw_fd()
    }
}

impl ActiveEventLoop {
    /// Creates an [`EventLoopProxy`] that can be used to dispatch user events
    /// to the main event loop, possibly from another thread.
    pub fn create_proxy(&self) -> EventLoopProxy {
        EventLoopProxy { event_loop_proxy: self.p.create_proxy() }
    }

    /// Create the window.
    ///
    /// Possible causes of error include denied permission, incompatible system, and lack of memory.
    ///
    /// ## Platform-specific
    ///
    /// - **Web:** The window is created but not inserted into the Web page automatically. Please
    ///   see the Web platform module for more information.
    #[inline]
    pub fn create_window(&self, window_attributes: WindowAttributes) -> Result<Window, OsError> {
        let _span = tracing::debug_span!(
            "winit::ActiveEventLoop::create_window",
            window_attributes = ?window_attributes
        )
        .entered();

        let window = platform_impl::Window::new(&self.p, window_attributes)?;
        Ok(Window { window })
    }

    /// Create custom cursor.
    pub fn create_custom_cursor(&self, custom_cursor: CustomCursorSource) -> CustomCursor {
        let _span = tracing::debug_span!("winit::ActiveEventLoop::create_custom_cursor",).entered();

        self.p.create_custom_cursor(custom_cursor)
    }

    /// Returns the list of all the monitors available on the system.
    #[inline]
    pub fn available_monitors(&self) -> impl Iterator<Item = MonitorHandle> {
        let _span = tracing::debug_span!("winit::ActiveEventLoop::available_monitors",).entered();

        #[allow(clippy::useless_conversion)] // false positive on some platforms
        self.p.available_monitors().into_iter().map(|inner| MonitorHandle { inner })
    }

    /// Returns the primary monitor of the system.
    ///
    /// Returns `None` if it can't identify any monitor as a primary one.
    ///
    /// ## Platform-specific
    ///
    /// **Wayland / Web:** Always returns `None`.
    #[inline]
    pub fn primary_monitor(&self) -> Option<MonitorHandle> {
        let _span = tracing::debug_span!("winit::ActiveEventLoop::primary_monitor",).entered();

        self.p.primary_monitor().map(|inner| MonitorHandle { inner })
    }

    /// Change if or when [`DeviceEvent`]s are captured.
    ///
    /// Since the [`DeviceEvent`] capture can lead to high CPU usage for unfocused windows, winit
    /// will ignore them by default for unfocused windows on Linux/BSD. This method allows changing
    /// this at runtime to explicitly capture them again.
    ///
    /// ## Platform-specific
    ///
    /// - **Wayland / macOS / iOS / Android / Orbital:** Unsupported.
    ///
    /// [`DeviceEvent`]: crate::event::DeviceEvent
    pub fn listen_device_events(&self, allowed: DeviceEvents) {
        let _span = tracing::debug_span!(
            "winit::ActiveEventLoop::listen_device_events",
            allowed = ?allowed
        )
        .entered();

        self.p.listen_device_events(allowed);
    }

    /// Sets the [`ControlFlow`].
    pub fn set_control_flow(&self, control_flow: ControlFlow) {
        self.p.set_control_flow(control_flow)
    }

    /// Gets the current [`ControlFlow`].
    pub fn control_flow(&self) -> ControlFlow {
        self.p.control_flow()
    }

    /// This exits the event loop.
    ///
    /// See [`exiting`][crate::application::ApplicationHandler::exiting].
    pub fn exit(&self) {
        let _span = tracing::debug_span!("winit::ActiveEventLoop::exit",).entered();

        self.p.exit()
    }

    /// Returns if the [`EventLoop`] is about to stop.
    ///
    /// See [`exit()`][Self::exit].
    pub fn exiting(&self) -> bool {
        self.p.exiting()
    }

    /// Gets a persistent reference to the underlying platform display.
    ///
    /// See the [`OwnedDisplayHandle`] type for more information.
    pub fn owned_display_handle(&self) -> OwnedDisplayHandle {
        OwnedDisplayHandle { platform: self.p.owned_display_handle() }
    }
}

#[cfg(feature = "rwh_06")]
impl rwh_06::HasDisplayHandle for ActiveEventLoop {
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        let raw = self.p.raw_display_handle_rwh_06()?;
        // SAFETY: The display will never be deallocated while the event loop is alive.
        Ok(unsafe { rwh_06::DisplayHandle::borrow_raw(raw) })
    }
}

#[cfg(feature = "rwh_05")]
unsafe impl rwh_05::HasRawDisplayHandle for ActiveEventLoop {
    /// Returns a [`rwh_05::RawDisplayHandle`] for the event loop.
    fn raw_display_handle(&self) -> rwh_05::RawDisplayHandle {
        self.p.raw_display_handle_rwh_05()
    }
}

/// A proxy for the underlying display handle.
///
/// The purpose of this type is to provide a cheaply clonable handle to the underlying
/// display handle. This is often used by graphics APIs to connect to the underlying APIs.
/// It is difficult to keep a handle to the [`EventLoop`] type or the [`ActiveEventLoop`]
/// type. In contrast, this type involves no lifetimes and can be persisted for as long as
/// needed.
///
/// For all platforms, this is one of the following:
///
/// - A zero-sized type that is likely optimized out.
/// - A reference-counted pointer to the underlying type.
#[derive(Clone)]
pub struct OwnedDisplayHandle {
    #[cfg_attr(not(any(feature = "rwh_05", feature = "rwh_06")), allow(dead_code))]
    platform: platform_impl::OwnedDisplayHandle,
}

impl fmt::Debug for OwnedDisplayHandle {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OwnedDisplayHandle").finish_non_exhaustive()
    }
}

#[cfg(feature = "rwh_06")]
impl rwh_06::HasDisplayHandle for OwnedDisplayHandle {
    #[inline]
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        let raw = self.platform.raw_display_handle_rwh_06()?;

        // SAFETY: The underlying display handle should be safe.
        let handle = unsafe { rwh_06::DisplayHandle::borrow_raw(raw) };

        Ok(handle)
    }
}

#[cfg(feature = "rwh_05")]
unsafe impl rwh_05::HasRawDisplayHandle for OwnedDisplayHandle {
    #[inline]
    fn raw_display_handle(&self) -> rwh_05::RawDisplayHandle {
        self.platform.raw_display_handle_rwh_05()
    }
}

/// Control the [`EventLoop`], possibly from a different thread, without referencing it directly.
#[derive(Clone)]
pub struct EventLoopProxy {
    event_loop_proxy: platform_impl::EventLoopProxy,
}

impl EventLoopProxy {
    /// Wake up the [`EventLoop`], resulting in [`ApplicationHandler::proxy_wake_up()`] being
    /// called.
    ///
    /// Calls to this method are coalesced into a single call to [`proxy_wake_up`], see the
    /// documentation on that for details.
    ///
    /// If the event loop is no longer running, this is a no-op.
    ///
    /// [`proxy_wake_up`]: ApplicationHandler::proxy_wake_up
    ///
    /// # Platform-specific
    ///
    /// - **Windows**: The wake-up may be ignored under high contention, see [#3687].
    ///
    /// [#3687]: https://github.com/rust-windowing/winit/pull/3687
    pub fn wake_up(&self) {
        self.event_loop_proxy.wake_up();
    }
}

impl fmt::Debug for EventLoopProxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("EventLoopProxy { .. }")
    }
}

/// Control when device events are captured.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub enum DeviceEvents {
    /// Report device events regardless of window focus.
    Always,
    /// Only capture device events while the window is focused.
    #[default]
    WhenFocused,
    /// Never capture device events.
    Never,
}

/// A unique identifier of the winit's async request.
///
/// This could be used to identify the async request once it's done
/// and a specific action must be taken.
///
/// One of the handling scenarios could be to maintain a working list
/// containing [`AsyncRequestSerial`] and some closure associated with it.
/// Then once event is arriving the working list is being traversed and a job
/// executed and removed from the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsyncRequestSerial {
    serial: usize,
}

impl AsyncRequestSerial {
    // TODO(kchibisov): Remove `cfg` when the clipboard will be added.
    #[allow(dead_code)]
    pub(crate) fn get() -> Self {
        static CURRENT_SERIAL: AtomicUsize = AtomicUsize::new(0);
        // NOTE: We rely on wrap around here, while the user may just request
        // in the loop usize::MAX times that's issue is considered on them.
        let serial = CURRENT_SERIAL.fetch_add(1, Ordering::Relaxed);
        Self { serial }
    }
}
