//! The [`EventLoop`] struct and assorted supporting types, including
//! [`ControlFlow`].
//!
//! If you want to send custom events to the event loop, use
//! [`EventLoop::create_proxy`] to acquire an [`EventLoopProxy`] and call its
//! [`send_event`](`EventLoopProxy::send_event`) method.
//!
//! See the root-level documentation for information on how to create and use an event loop to
//! handle events.
use std::marker::PhantomData;
#[cfg(any(x11_platform, wayland_platform))]
use std::os::unix::io::{AsFd, AsRawFd, BorrowedFd, RawFd};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::{error, fmt};

#[cfg(not(wasm_platform))]
use std::time::{Duration, Instant};
#[cfg(wasm_platform)]
use web_time::{Duration, Instant};

use crate::error::EventLoopError;
use crate::event::Event;
use crate::event_helper::MapEventHelper;
use crate::handler::ApplicationHandler;
use crate::{monitor::MonitorHandle, platform_impl};

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
/// forbidding it), as such it is neither [`Send`] nor [`Sync`]. If you need cross-thread access, the
/// [`Window`] created from this _can_ be sent to an other thread, and the
/// [`EventLoopProxy`] allows you to wake up an `EventLoop` from another thread.
///
/// [`Window`]: crate::window::Window
// TODO: Don't allow window creation from this, since that is broken on macOS/iOS.
pub struct EventLoop<T: 'static> {
    pub(crate) event_loop: platform_impl::EventLoop<T>,
    pub(crate) _marker: PhantomData<*mut ()>, // Not Send nor Sync
}

/// An active event loop.
///
/// This type exists to differentiate between functionality available when Winit
/// is executing your callback, and outside of it.
#[derive(Copy, Clone)]
pub struct ActiveEventLoop<'a> {
    pub(crate) inner: platform_impl::ActiveEventLoop<'a>,
}

impl fmt::Debug for ActiveEventLoop<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("ActiveEventLoop { .. }")
    }
}

impl ActiveEventLoop<'_> {
    /// Sets the [`ControlFlow`].
    pub fn set_control_flow(&self, control_flow: ControlFlow) {
        self.inner.set_control_flow(control_flow)
    }

    /// Gets the current [`ControlFlow`].
    pub fn control_flow(&self) -> ControlFlow {
        self.inner.control_flow()
    }

    /// This exits the event loop.
    ///
    /// See [`LoopExiting`](Event::LoopExiting).
    pub fn exit(&self) {
        self.inner.exit()
    }

    /// Returns if the [`EventLoop`] is about to stop.
    ///
    /// See [`exit()`](Self::exit).
    pub fn exiting(&self) -> bool {
        self.inner.exiting()
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
        self.inner
            .primary_monitor()
            .map(|inner| MonitorHandle { inner })
    }

    /// Returns the list of all the monitors available on the system.
    #[inline]
    pub fn available_monitors(&self) -> impl Iterator<Item = MonitorHandle> {
        #[allow(clippy::useless_conversion)] // false positive on some platforms
        self.inner
            .available_monitors()
            .into_iter()
            .map(|inner| MonitorHandle { inner })
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
        self.inner.listen_device_events(allowed);
    }
}

/// Object that allows building the event loop.
///
/// This is used to make specifying options that affect the whole application
/// easier. But note that constructing multiple event loops is not supported.
#[derive(Default)]
pub struct EventLoopBuilder<T: 'static> {
    pub(crate) platform_specific: platform_impl::PlatformSpecificEventLoopAttributes,
    _p: PhantomData<T>,
}

impl EventLoopBuilder<()> {
    /// Start building a new event loop.
    #[inline]
    pub fn new() -> Self {
        Self::with_user_event()
    }
}

static EVENT_LOOP_CREATED: AtomicBool = AtomicBool::new(false);

impl<T> EventLoopBuilder<T> {
    /// Start building a new event loop, with the given type as the user event
    /// type.
    #[inline]
    pub fn with_user_event() -> Self {
        Self {
            platform_specific: Default::default(),
            _p: PhantomData,
        }
    }

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
    /// - **Wayland/X11:** to prevent running under `Wayland` or `X11` unset `WAYLAND_DISPLAY`
    ///                    or `DISPLAY` respectively when building the event loop.
    /// - **Android:** must be configured with an `AndroidApp` from `android_main()` by calling
    ///     [`.with_android_app(app)`] before calling `.build()`, otherwise it'll panic.
    ///
    /// [`platform`]: crate::platform
    #[cfg_attr(
        android,
        doc = "[`.with_android_app(app)`]: crate::platform::android::EventLoopBuilderExtAndroid::with_android_app"
    )]
    #[cfg_attr(
        not(android),
        doc = "[`.with_android_app(app)`]: #only-available-on-android"
    )]
    #[inline]
    pub fn build(&mut self) -> Result<EventLoop<T>, EventLoopError> {
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

    #[cfg(wasm_platform)]
    pub(crate) fn allow_event_loop_recreation() {
        EVENT_LOOP_CREATED.store(false, Ordering::Relaxed);
    }
}

impl<T> fmt::Debug for EventLoop<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("EventLoop { .. }")
    }
}

/// Set through [`ActiveEventLoop::set_control_flow()`].
///
/// Indicates the desired behavior of the event loop after [`Event::AboutToWait`] is emitted.
///
/// Defaults to [`Wait`].
///
/// [`Wait`]: Self::Wait
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
    /// Useful for implementing efficient timers. Applications which want to render at the display's
    /// native refresh rate should instead use [`Poll`] and the VSync functionality of a graphics API
    /// to reduce odds of missed frames.
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

impl EventLoop<()> {
    /// Alias for [`EventLoopBuilder::new().build()`].
    ///
    /// [`EventLoopBuilder::new().build()`]: EventLoopBuilder::build
    #[inline]
    pub fn new() -> Result<EventLoop<()>, EventLoopError> {
        EventLoopBuilder::new().build()
    }
}

impl<T> EventLoop<T> {
    #[deprecated = "Use `EventLoopBuilder::<T>::with_user_event().build()` instead."]
    pub fn with_user_event() -> Result<EventLoop<T>, EventLoopError> {
        EventLoopBuilder::<T>::with_user_event().build()
    }

    /// Runs the event loop in the calling thread and calls the given `event_handler` closure
    /// to dispatch any pending events.
    ///
    /// Since the closure is `'static`, it must be a `move` closure if it needs to
    /// access any data from the calling context.
    ///
    /// See the [`set_control_flow()`] docs on how to change the event loop's behavior.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS:** Will never return to the caller and so values not passed to this function will
    ///   *not* be dropped before the process exits.
    /// - **Web:** Will _act_ as if it never returns to the caller by throwing a Javascript exception
    ///   (that Rust doesn't see) that will also mean that the rest of the function is never executed
    ///   and any values not passed to this function will *not* be dropped.
    ///
    ///   Web applications are recommended to use
    #[cfg_attr(
        wasm_platform,
        doc = "[`EventLoopExtWebSys::spawn()`][crate::platform::web::EventLoopExtWebSys::spawn()]"
    )]
    #[cfg_attr(not(wasm_platform), doc = "`EventLoopExtWebSys::spawn()`")]
    ///   [^1] instead of [`run()`] to avoid the need
    ///   for the Javascript exception trick, and to make it clearer that the event loop runs
    ///   asynchronously (via the browser's own, internal, event loop) and doesn't block the
    ///   current thread of execution like it does on other platforms.
    ///
    ///   This function won't be available with `target_feature = "exception-handling"`.
    ///
    /// [`set_control_flow()`]: ActiveEventLoop::set_control_flow()
    /// [`run()`]: Self::run()
    /// [^1]: `EventLoopExtWebSys::spawn()` is only available on WASM.
    #[inline]
    #[cfg(not(all(wasm_platform, target_feature = "exception-handling")))]
    pub fn run<F>(self, handler: F) -> Result<(), EventLoopError>
    where
        F: FnMut(Event<T>, ActiveEventLoop<'_>),
    {
        self.run_with(MapEventHelper::new(handler))
    }

    #[cfg(not(all(wasm_platform, target_feature = "exception-handling")))]
    pub fn run_with<A: ApplicationHandler<T>>(self, mut handler: A) -> Result<(), EventLoopError> {
        #[cfg(any(macos_platform, orbital_platform))]
        {
            self.event_loop.run(handler)
        }
        #[cfg(not(any(macos_platform, orbital_platform)))]
        {
            self.event_loop.run(|event, inner| {
                crate::event_helper::map_event(&mut handler, event, ActiveEventLoop { inner })
            })
        }
    }

    /// Set the initial [`ControlFlow`].
    pub fn set_control_flow(&self, control_flow: ControlFlow) {
        self.event_loop
            .window_target()
            .set_control_flow(control_flow)
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
        self.event_loop
            .window_target()
            .primary_monitor()
            .map(|inner| MonitorHandle { inner })
    }

    /// Returns the list of all the monitors available on the system.
    #[inline]
    pub fn available_monitors(&self) -> impl Iterator<Item = MonitorHandle> {
        #[allow(clippy::useless_conversion)] // false positive on some platforms
        self.event_loop
            .window_target()
            .available_monitors()
            .into_iter()
            .map(|inner| MonitorHandle { inner })
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
        self.event_loop
            .window_target()
            .listen_device_events(allowed);
    }

    /// Creates an [`EventLoopProxy`] that can be used to dispatch user events to the main event loop.
    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy {
            event_loop_proxy: self.event_loop.create_proxy(),
        }
    }
}

#[cfg(feature = "rwh_06")]
impl<T> rwh_06::HasDisplayHandle for EventLoop<T> {
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        let raw = self
            .event_loop
            .window_target()
            .raw_display_handle_rwh_06()?;
        // SAFETY: The display will never be deallocated while the event loop is alive.
        Ok(unsafe { rwh_06::DisplayHandle::borrow_raw(raw) })
    }
}

#[cfg(feature = "rwh_05")]
unsafe impl<T> rwh_05::HasRawDisplayHandle for EventLoop<T> {
    /// Returns a [`rwh_05::RawDisplayHandle`] for the event loop.
    fn raw_display_handle(&self) -> rwh_05::RawDisplayHandle {
        self.event_loop.window_target().raw_display_handle_rwh_05()
    }
}

#[cfg(any(x11_platform, wayland_platform))]
impl<T> AsFd for EventLoop<T> {
    /// Get the underlying [EventLoop]'s `fd` which you can register
    /// into other event loop, like [`calloop`] or [`mio`]. When doing so, the
    /// loop must be polled with the [`pump_events`] API.
    ///
    /// [`calloop`]: https://crates.io/crates/calloop
    /// [`mio`]: https://crates.io/crates/mio
    /// [`pump_events`]: crate::platform::pump_events::EventLoopExtPumpEvents::pump_events
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.event_loop.as_fd()
    }
}

#[cfg(any(x11_platform, wayland_platform))]
impl<T> AsRawFd for EventLoop<T> {
    /// Get the underlying [EventLoop]'s raw `fd` which you can register
    /// into other event loop, like [`calloop`] or [`mio`]. When doing so, the
    /// loop must be polled with the [`pump_events`] API.
    ///
    /// [`calloop`]: https://crates.io/crates/calloop
    /// [`mio`]: https://crates.io/crates/mio
    /// [`pump_events`]: crate::platform::pump_events::EventLoopExtPumpEvents::pump_events
    fn as_raw_fd(&self) -> RawFd {
        self.event_loop.as_raw_fd()
    }
}

#[cfg(feature = "rwh_06")]
impl rwh_06::HasDisplayHandle for ActiveEventLoop<'_> {
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        let raw = self.inner.raw_display_handle_rwh_06()?;
        // SAFETY: The display will never be deallocated while the event loop is alive.
        Ok(unsafe { rwh_06::DisplayHandle::borrow_raw(raw) })
    }
}

#[cfg(feature = "rwh_05")]
unsafe impl rwh_05::HasRawDisplayHandle for ActiveEventLoop<'_> {
    /// Returns a [`rwh_05::RawDisplayHandle`] for the event loop.
    fn raw_display_handle(&self) -> rwh_05::RawDisplayHandle {
        self.inner.raw_display_handle_rwh_05()
    }
}

mod private {
    pub trait Sealed {}
}

/// Trait to allow functions to be generic over [`EventLoop`] and [`ActiveEventLoop`].
pub trait MaybeActiveEventLoop<'a>: private::Sealed {
    #[doc(hidden)]
    fn __inner(self) -> &'a platform_impl::EventLoopWindowTarget;
}

impl<T: 'static> private::Sealed for &EventLoop<T> {}
impl<'a, T: 'static> MaybeActiveEventLoop<'a> for &'a EventLoop<T> {
    fn __inner(self) -> &'a platform_impl::EventLoopWindowTarget {
        self.event_loop.window_target()
    }
}

impl private::Sealed for ActiveEventLoop<'_> {}
impl<'a> MaybeActiveEventLoop<'a> for ActiveEventLoop<'a> {
    fn __inner(self) -> &'a platform_impl::EventLoopWindowTarget {
        self.inner
    }
}

/// Used to send custom events to [`EventLoop`].
pub struct EventLoopProxy<T: 'static> {
    event_loop_proxy: platform_impl::EventLoopProxy<T>,
}

impl<T: 'static> Clone for EventLoopProxy<T> {
    fn clone(&self) -> Self {
        Self {
            event_loop_proxy: self.event_loop_proxy.clone(),
        }
    }
}

impl<T: 'static> EventLoopProxy<T> {
    /// Send an event to the [`EventLoop`] from which this proxy was created. This emits a
    /// `UserEvent(event)` event in the event loop, where `event` is the value passed to this
    /// function.
    ///
    /// Returns an `Err` if the associated [`EventLoop`] no longer exists.
    ///
    /// [`UserEvent(event)`]: Event::UserEvent
    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed<T>> {
        self.event_loop_proxy.send_event(event)
    }
}

impl<T: 'static> fmt::Debug for EventLoopProxy<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("EventLoopProxy { .. }")
    }
}

/// The error that is returned when an [`EventLoopProxy`] attempts to wake up an [`EventLoop`] that
/// no longer exists.
///
/// Contains the original event given to [`EventLoopProxy::send_event`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct EventLoopClosed<T>(pub T);

impl<T> fmt::Display for EventLoopClosed<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Tried to wake up a closed `EventLoop`")
    }
}

impl<T: fmt::Debug> error::Error for EventLoopClosed<T> {}

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
/// One of the handling scenarious could be to maintain a working list
/// containing [`AsyncRequestSerial`] and some closure associated with it.
/// Then once event is arriving the working list is being traversed and a job
/// executed and removed from the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsyncRequestSerial {
    serial: u64,
}

impl AsyncRequestSerial {
    // TODO(kchibisov) remove `cfg` when the clipboard will be added.
    #[allow(dead_code)]
    pub(crate) fn get() -> Self {
        static CURRENT_SERIAL: AtomicU64 = AtomicU64::new(0);
        // NOTE: we rely on wrap around here, while the user may just request
        // in the loop u64::MAX times that's issue is considered on them.
        let serial = CURRENT_SERIAL.fetch_add(1, Ordering::Relaxed);
        Self { serial }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(unused)]
    fn assert_active_event_loop_covariance<'b>(
        event_loop: ActiveEventLoop<'static>,
    ) -> ActiveEventLoop<'b> {
        event_loop
    }
}
