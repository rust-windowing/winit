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
use std::ops::Deref;
use std::{error, fmt};

use instant::{Duration, Instant};
use once_cell::sync::OnceCell;
use raw_window_handle::{HasRawDisplayHandle, RawDisplayHandle};

use crate::{event::Event, monitor::MonitorHandle, platform_impl};

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
pub struct EventLoop<T: 'static> {
    pub(crate) event_loop: platform_impl::EventLoop<T>,
    pub(crate) _marker: PhantomData<*mut ()>, // Not Send nor Sync
}

/// Target that associates windows with an [`EventLoop`].
///
/// This type exists to allow you to create new windows while Winit executes
/// your callback. [`EventLoop`] will coerce into this type (`impl<T> Deref for
/// EventLoop<T>`), so functions that take this as a parameter can also take
/// `&EventLoop`.
pub struct EventLoopWindowTarget<T: 'static> {
    pub(crate) p: platform_impl::EventLoopWindowTarget<T>,
    pub(crate) _marker: PhantomData<*mut ()>, // Not Send nor Sync
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
    /// Attempting to create the event loop on a different thread, or multiple event loops in
    /// the same application, will panic. This restriction isn't
    /// strictly necessary on all platforms, but is imposed to eliminate any nasty surprises when
    /// porting to platforms that require it. `EventLoopBuilderExt::any_thread` functions are exposed
    /// in the relevant [`platform`] module if the target platform supports creating an event loop on
    /// any thread.
    ///
    /// Calling this function will result in display backend initialisation.
    ///
    /// ## Platform-specific
    ///
    /// - **Linux:** Backend type can be controlled using an environment variable
    ///   `WINIT_UNIX_BACKEND`. Legal values are `x11` and `wayland`.
    ///   If it is not set, winit will try to connect to a Wayland connection, and if that fails,
    ///   will fall back on X11. If this variable is set with any other value, winit will panic.
    /// - **Android:** Must be configured with an `AndroidApp` from `android_main()` by calling
    ///     [`.with_android_app(app)`] before calling `.build()`.
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
    pub fn build(&mut self) -> EventLoop<T> {
        static EVENT_LOOP_CREATED: OnceCell<()> = OnceCell::new();
        if EVENT_LOOP_CREATED.set(()).is_err() {
            panic!("Creating EventLoop multiple times is not supported.");
        }
        // Certain platforms accept a mutable reference in their API.
        #[allow(clippy::unnecessary_mut_passed)]
        EventLoop {
            event_loop: platform_impl::EventLoop::new(&mut self.platform_specific),
            _marker: PhantomData,
        }
    }
}

impl<T> fmt::Debug for EventLoop<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("EventLoop { .. }")
    }
}

impl<T> fmt::Debug for EventLoopWindowTarget<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("EventLoopWindowTarget { .. }")
    }
}

/// Set by the user callback given to the [`EventLoop::run`] method.
///
/// Indicates the desired behavior of the event loop after [`Event::RedrawEventsCleared`] is emitted.
///
/// Defaults to [`Poll`].
///
/// ## Persistency
///
/// Almost every change is persistent between multiple calls to the event loop closure within a
/// given run loop. The only exception to this is [`ExitWithCode`] which, once set, cannot be unset.
/// Changes are **not** persistent between multiple calls to `run_return` - issuing a new call will
/// reset the control flow to [`Poll`].
///
/// [`ExitWithCode`]: Self::ExitWithCode
/// [`Poll`]: Self::Poll
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ControlFlow {
    /// When the current loop iteration finishes, immediately begin a new iteration regardless of
    /// whether or not new events are available to process.
    ///
    /// ## Platform-specific
    ///
    /// - **Web:** Events are queued and usually sent when `requestAnimationFrame` fires but sometimes
    ///   the events in the queue may be sent before the next `requestAnimationFrame` callback, for
    ///   example when the scaling of the page has changed. This should be treated as an implementation
    ///   detail which should not be relied on.
    Poll,

    /// When the current loop iteration finishes, suspend the thread until another event arrives.
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

    /// Send a [`LoopDestroyed`] event and stop the event loop. This variant is *sticky* - once set,
    /// `control_flow` cannot be changed from `ExitWithCode`, and any future attempts to do so will
    /// result in the `control_flow` parameter being reset to `ExitWithCode`.
    ///
    /// The contained number will be used as exit code. The [`Exit`] constant is a shortcut for this
    /// with exit code 0.
    ///
    /// ## Platform-specific
    ///
    /// - **Android / iOS / WASM:** The supplied exit code is unused.
    /// - **Unix:** On most Unix-like platforms, only the 8 least significant bits will be used,
    ///   which can cause surprises with negative exit values (`-42` would end up as `214`). See
    ///   [`std::process::exit`].
    ///
    /// [`LoopDestroyed`]: Event::LoopDestroyed
    /// [`Exit`]: ControlFlow::Exit
    ExitWithCode(i32),
}

impl ControlFlow {
    /// Alias for [`ExitWithCode`]`(0)`.
    ///
    /// [`ExitWithCode`]: Self::ExitWithCode
    #[allow(non_upper_case_globals)]
    pub const Exit: Self = Self::ExitWithCode(0);

    /// Sets this to [`Poll`].
    ///
    /// [`Poll`]: Self::Poll
    pub fn set_poll(&mut self) {
        *self = Self::Poll;
    }

    /// Sets this to [`Wait`].
    ///
    /// [`Wait`]: Self::Wait
    pub fn set_wait(&mut self) {
        *self = Self::Wait;
    }

    /// Sets this to [`WaitUntil`]`(instant)`.
    ///
    /// [`WaitUntil`]: Self::WaitUntil
    pub fn set_wait_until(&mut self, instant: Instant) {
        *self = Self::WaitUntil(instant);
    }

    /// Sets this to wait until a timeout has expired.
    ///
    /// In most cases, this is set to [`WaitUntil`]. However, if the timeout overflows, it is
    /// instead set to [`Wait`].
    ///
    /// [`WaitUntil`]: Self::WaitUntil
    /// [`Wait`]: Self::Wait
    pub fn set_wait_timeout(&mut self, timeout: Duration) {
        match Instant::now().checked_add(timeout) {
            Some(instant) => self.set_wait_until(instant),
            None => self.set_wait(),
        }
    }

    /// Sets this to [`ExitWithCode`]`(code)`.
    ///
    /// [`ExitWithCode`]: Self::ExitWithCode
    pub fn set_exit_with_code(&mut self, code: i32) {
        *self = Self::ExitWithCode(code);
    }

    /// Sets this to [`Exit`].
    ///
    /// [`Exit`]: Self::Exit
    pub fn set_exit(&mut self) {
        *self = Self::Exit;
    }
}

impl Default for ControlFlow {
    #[inline(always)]
    fn default() -> Self {
        Self::Poll
    }
}

impl EventLoop<()> {
    /// Alias for [`EventLoopBuilder::new().build()`].
    ///
    /// [`EventLoopBuilder::new().build()`]: EventLoopBuilder::build
    #[inline]
    pub fn new() -> EventLoop<()> {
        EventLoopBuilder::new().build()
    }
}

impl Default for EventLoop<()> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> EventLoop<T> {
    #[deprecated = "Use `EventLoopBuilder::<T>::with_user_event().build()` instead."]
    pub fn with_user_event() -> EventLoop<T> {
        EventLoopBuilder::<T>::with_user_event().build()
    }

    /// Hijacks the calling thread and initializes the winit event loop with the provided
    /// closure. Since the closure is `'static`, it must be a `move` closure if it needs to
    /// access any data from the calling context.
    ///
    /// See the [`ControlFlow`] docs for information on how changes to `&mut ControlFlow` impact the
    /// event loop's behavior.
    ///
    /// Any values not passed to this function will *not* be dropped.
    ///
    /// ## Platform-specific
    ///
    /// - **X11 / Wayland:** The program terminates with exit code 1 if the display server
    ///   disconnects.
    ///
    /// [`ControlFlow`]: crate::event_loop::ControlFlow
    #[inline]
    pub fn run<F>(self, event_handler: F) -> !
    where
        F: 'static + FnMut(Event<'_, T>, &EventLoopWindowTarget<T>, &mut ControlFlow),
    {
        self.event_loop.run(event_handler)
    }

    /// Creates an [`EventLoopProxy`] that can be used to dispatch user events to the main event loop.
    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy {
            event_loop_proxy: self.event_loop.create_proxy(),
        }
    }
}

unsafe impl<T> HasRawDisplayHandle for EventLoop<T> {
    /// Returns a [`raw_window_handle::RawDisplayHandle`] for the event loop.
    fn raw_display_handle(&self) -> RawDisplayHandle {
        self.event_loop.window_target().p.raw_display_handle()
    }
}

impl<T> Deref for EventLoop<T> {
    type Target = EventLoopWindowTarget<T>;
    fn deref(&self) -> &EventLoopWindowTarget<T> {
        self.event_loop.window_target()
    }
}

impl<T> EventLoopWindowTarget<T> {
    /// Returns the list of all the monitors available on the system.
    #[inline]
    pub fn available_monitors(&self) -> impl Iterator<Item = MonitorHandle> {
        self.p
            .available_monitors()
            .into_iter()
            .map(|inner| MonitorHandle { inner })
    }

    /// Returns the primary monitor of the system.
    ///
    /// Returns `None` if it can't identify any monitor as a primary one.
    ///
    /// ## Platform-specific
    ///
    /// **Wayland:** Always returns `None`.
    #[inline]
    pub fn primary_monitor(&self) -> Option<MonitorHandle> {
        self.p
            .primary_monitor()
            .map(|inner| MonitorHandle { inner })
    }

    /// Change [`DeviceEvent`] filter mode.
    ///
    /// Since the [`DeviceEvent`] capture can lead to high CPU usage for unfocused windows, winit
    /// will ignore them by default for unfocused windows on Linux/BSD. This method allows changing
    /// this filter at runtime to explicitly capture them again.
    ///
    /// ## Platform-specific
    ///
    /// - **Wayland / macOS / iOS / Android / Web / Orbital:** Unsupported.
    ///
    /// [`DeviceEvent`]: crate::event::DeviceEvent
    pub fn set_device_event_filter(&self, _filter: DeviceEventFilter) {
        #[cfg(any(x11_platform, wayland_platform, windows))]
        self.p.set_device_event_filter(_filter);
    }
}

unsafe impl<T> HasRawDisplayHandle for EventLoopWindowTarget<T> {
    /// Returns a [`raw_window_handle::RawDisplayHandle`] for the event loop.
    fn raw_display_handle(&self) -> RawDisplayHandle {
        self.p.raw_display_handle()
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

/// Filter controlling the propagation of device events.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum DeviceEventFilter {
    /// Always filter out device events.
    Always,
    /// Filter out device events while the window is not focused.
    Unfocused,
    /// Report all device events regardless of window focus.
    Never,
}

impl Default for DeviceEventFilter {
    fn default() -> Self {
        Self::Unfocused
    }
}
