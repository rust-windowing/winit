use std::fmt::{self, Debug};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
#[cfg(not(web_platform))]
use std::time::{Duration, Instant};

use rwh_06::{DisplayHandle, HandleError, HasDisplayHandle};
#[cfg(web_platform)]
use web_time::{Duration, Instant};

/// Control the [`EventLoop`], possibly from a different thread, without referencing it directly.
#[derive(Clone, Debug)]
pub struct EventLoopProxy {
    pub(crate) proxy: Arc<dyn EventLoopProxyProvider>,
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
        self.proxy.wake_up();
    }

    pub fn new(proxy: Arc<dyn EventLoopProxyProvider>) -> Self {
        Self { proxy }
    }
}

pub trait EventLoopProxyProvider: Send + Sync + Debug {
    /// See [`EventLoopProxy::wake_up`] for details.
    fn wake_up(&self);
}

/// A proxy for the underlying display handle.
///
/// The purpose of this type is to provide a cheaply cloneable handle to the underlying
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
    pub(crate) handle: Arc<dyn HasDisplayHandle>,
}

impl OwnedDisplayHandle {
    pub fn new(handle: Arc<dyn HasDisplayHandle>) -> Self {
        Self { handle }
    }
}

impl HasDisplayHandle for OwnedDisplayHandle {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        self.handle.display_handle()
    }
}

impl fmt::Debug for OwnedDisplayHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OwnedDisplayHandle").finish_non_exhaustive()
    }
}

impl PartialEq for OwnedDisplayHandle {
    fn eq(&self, other: &Self) -> bool {
        match (self.display_handle(), other.display_handle()) {
            (Ok(lhs), Ok(rhs)) => lhs == rhs,
            _ => false,
        }
    }
}

impl Eq for OwnedDisplayHandle {}

/// Set through [`ActiveEventLoop::set_control_flow()`].
///
/// Indicates the desired behavior of the event loop after [`about_to_wait`] is called.
///
/// Defaults to [`Wait`].
///
/// [`Wait`]: Self::Wait
/// [`about_to_wait`]: crate::application::ApplicationHandler::about_to_wait
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
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

/// Control when device events are captured.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AsyncRequestSerial {
    serial: usize,
}

impl AsyncRequestSerial {
    // TODO(kchibisov): Remove `cfg` when the clipboard will be added.
    #[allow(dead_code)]
    pub fn get() -> Self {
        static CURRENT_SERIAL: AtomicUsize = AtomicUsize::new(0);
        // NOTE: We rely on wrap around here, while the user may just request
        // in the loop usize::MAX times that's issue is considered on them.
        let serial = CURRENT_SERIAL.fetch_add(1, Ordering::Relaxed);
        Self { serial }
    }
}
