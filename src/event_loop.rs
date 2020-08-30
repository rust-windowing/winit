//! The `EventLoop` struct and assorted supporting types, including `ControlFlow`.
//!
//! If you want to send custom events to the event loop, use [`EventLoop::create_proxy()`][create_proxy]
//! to acquire an [`EventLoopProxy`][event_loop_proxy] and call its [`send_event`][send_event] method.
//!
//! See the root-level documentation for information on how to create and use an event loop to
//! handle events.
//!
//! [create_proxy]: crate::event_loop::EventLoop::create_proxy
//! [event_loop_proxy]: crate::event_loop::EventLoopProxy
//! [send_event]: crate::event_loop::EventLoopProxy::send_event
use instant::Instant;
use std::ops::Deref;
use std::{error, fmt};

use crate::{event::Event, monitor::MonitorHandle, platform_impl};

/// Provides a way to retrieve events from the system and from the windows that were registered to
/// the events loop.
///
/// An `EventLoop` can be seen more or less as a "context". Calling `EventLoop::new()`
/// initializes everything that will be required to create windows. For example on Linux creating
/// an event loop opens a connection to the X or Wayland server.
///
/// To wake up an `EventLoop` from a another thread, see the `EventLoopProxy` docs.
///
/// Note that the `EventLoop` cannot be shared across threads (due to platform-dependant logic
/// forbidding it), as such it is neither `Send` nor `Sync`. If you need cross-thread access, the
/// `Window` created from this `EventLoop` _can_ be sent to an other thread, and the
/// `EventLoopProxy` allows you to wake up an `EventLoop` from another thread.
///
pub struct EventLoop<T: 'static> {
    pub(crate) event_loop: platform_impl::EventLoop<T>,
    pub(crate) _marker: ::std::marker::PhantomData<*mut ()>, // Not Send nor Sync
}

/// Target that associates windows with an `EventLoop`.
///
/// This type exists to allow you to create new windows while Winit executes
/// your callback. `EventLoop` will coerce into this type (`impl<T> Deref for
/// EventLoop<T>`), so functions that take this as a parameter can also take
/// `&EventLoop`.
pub struct EventLoopWindowTarget<T: 'static> {
    pub(crate) p: platform_impl::EventLoopWindowTarget<T>,
    pub(crate) _marker: ::std::marker::PhantomData<*mut ()>, // Not Send nor Sync
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

/// Set by the user callback given to the `EventLoop::run` method.
///
/// Indicates the desired behavior of the event loop after [`Event::RedrawEventsCleared`][events_cleared]
/// is emitted. Defaults to `Poll`.
///
/// ## Persistency
/// Almost every change is persistent between multiple calls to the event loop closure within a
/// given run loop. The only exception to this is `Exit` which, once set, cannot be unset. Changes
/// are **not** persistent between multiple calls to `run_return` - issuing a new call will reset
/// the control flow to `Poll`.
///
/// [events_cleared]: crate::event::Event::RedrawEventsCleared
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ControlFlow {
    /// When the current loop iteration finishes, immediately begin a new iteration regardless of
    /// whether or not new events are available to process.
    ///
    /// For web, events are queued and usually sent when `requestAnimationFrame` fires but sometimes
    /// the events in the queue may be sent before the next `requestAnimationFrame` callback, for
    /// example when the scaling of the page has changed.
    Poll,
    /// When the current loop iteration finishes, suspend the thread until another event arrives.
    Wait,
    /// When the current loop iteration finishes, suspend the thread until either another event
    /// arrives or the given time is reached.
    WaitUntil(Instant),
    /// Send a `LoopDestroyed` event and stop the event loop. This variant is *sticky* - once set,
    /// `control_flow` cannot be changed from `Exit`, and any future attempts to do so will result
    /// in the `control_flow` parameter being reset to `Exit`.
    Exit,
}

impl Default for ControlFlow {
    #[inline(always)]
    fn default() -> ControlFlow {
        ControlFlow::Poll
    }
}

impl EventLoop<()> {
    /// Builds a new event loop with a `()` as the user event type.
    ///
    /// ***For cross-platform compatibility, the `EventLoop` must be created on the main thread.***
    /// Attempting to create the event loop on a different thread will panic. This restriction isn't
    /// strictly necessary on all platforms, but is imposed to eliminate any nasty surprises when
    /// porting to platforms that require it. `EventLoopExt::new_any_thread` functions are exposed
    /// in the relevant `platform` module if the target platform supports creating an event loop on
    /// any thread.
    ///
    /// Usage will result in display backend initialisation, this can be controlled on linux
    /// using an environment variable `WINIT_UNIX_BACKEND`. Legal values are `x11` and `wayland`.
    /// If it is not set, winit will try to connect to a wayland connection, and if it fails will
    /// fallback on x11. If this variable is set with any other value, winit will panic.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS:** Can only be called on the main thread.
    pub fn new() -> EventLoop<()> {
        EventLoop::<()>::with_user_event()
    }
}

impl<T> EventLoop<T> {
    /// Builds a new event loop.
    ///
    /// All caveats documented in [`EventLoop::new`] apply to this function.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS:** Can only be called on the main thread.
    pub fn with_user_event() -> EventLoop<T> {
        EventLoop {
            event_loop: platform_impl::EventLoop::new(),
            _marker: ::std::marker::PhantomData,
        }
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
    /// [`ControlFlow`]: crate::event_loop::ControlFlow
    #[inline]
    pub fn run<F>(self, event_handler: F) -> !
    where
        F: 'static + FnMut(Event<'_, T>, &EventLoopWindowTarget<T>, &mut ControlFlow),
    {
        self.event_loop.run(event_handler)
    }

    /// Creates an `EventLoopProxy` that can be used to dispatch user events to the main event loop.
    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy {
            event_loop_proxy: self.event_loop.create_proxy(),
        }
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
    #[inline]
    pub fn primary_monitor(&self) -> MonitorHandle {
        MonitorHandle {
            inner: self.p.primary_monitor(),
        }
    }
}

/// Used to send custom events to `EventLoop`.
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
    /// Send an event to the `EventLoop` from which this proxy was created. This emits a
    /// `UserEvent(event)` event in the event loop, where `event` is the value passed to this
    /// function.
    ///
    /// Returns an `Err` if the associated `EventLoop` no longer exists.
    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed<T>> {
        self.event_loop_proxy.send_event(event)
    }
}

impl<T: 'static> fmt::Debug for EventLoopProxy<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("EventLoopProxy { .. }")
    }
}

/// The error that is returned when an `EventLoopProxy` attempts to wake up an `EventLoop` that
/// no longer exists. Contains the original event given to `send_event`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct EventLoopClosed<T>(pub T);

impl<T> fmt::Display for EventLoopClosed<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Tried to wake up a closed `EventLoop`")
    }
}

impl<T: fmt::Debug> error::Error for EventLoopClosed<T> {}
