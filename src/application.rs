//! End user application handling.

use crate::event::{DeviceEvent, DeviceId, StartCause, WindowEvent};
use crate::event_loop::ActiveEventLoop;
use crate::window::WindowId;

/// The handler of the application events.
pub trait ApplicationHandler {
    /// Emitted when new events arrive from the OS to be processed.
    ///
    /// This is a useful place to put code that should be done before you start processing
    /// events, such as updating frame timing information for benchmarking or checking the
    /// [`StartCause`] to see if a timer set by
    /// [`ControlFlow::WaitUntil`][crate::event_loop::ControlFlow::WaitUntil] has elapsed.
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        let _ = (event_loop, cause);
    }

    /// Emitted when the application has been resumed.
    ///
    /// For consistency, all platforms emit a `Resumed` event even if they don't themselves have a
    /// formal suspend/resume lifecycle. For systems without a formal suspend/resume lifecycle
    /// the `Resumed` event is always emitted after the
    /// [`NewEvents(StartCause::Init)`][StartCause::Init] event.
    ///
    /// # Portability
    ///
    /// It's recommended that applications should only initialize their graphics context and create
    /// a window after they have received their first `Resumed` event. Some systems
    /// (specifically Android) won't allow applications to create a render surface until they are
    /// resumed.
    ///
    /// Considering that the implementation of [`Suspended`] and `Resumed` events may be internally
    /// driven by multiple platform-specific events, and that there may be subtle differences across
    /// platforms with how these internal events are delivered, it's recommended that applications
    /// be able to gracefully handle redundant (i.e. back-to-back) [`Suspended`] or `Resumed`
    /// events.
    ///
    /// Also see [`Suspended`] notes.
    ///
    /// ## Android
    ///
    /// On Android, the `Resumed` event is sent when a new [`SurfaceView`] has been created. This is
    /// expected to closely correlate with the [`onResume`] lifecycle event but there may
    /// technically be a discrepancy.
    ///
    /// [`onResume`]: https://developer.android.com/reference/android/app/Activity#onResume()
    ///
    /// Applications that need to run on Android must wait until they have been `Resumed`
    /// before they will be able to create a render surface (such as an `EGLSurface`,
    /// [`VkSurfaceKHR`] or [`wgpu::Surface`]) which depend on having a
    /// [`SurfaceView`]. Applications must also assume that if they are [`Suspended`], then their
    /// render surfaces are invalid and should be dropped.
    ///
    /// Also see [`Suspended`] notes.
    ///
    /// [`SurfaceView`]: https://developer.android.com/reference/android/view/SurfaceView
    /// [Activity lifecycle]: https://developer.android.com/guide/components/activities/activity-lifecycle
    /// [`VkSurfaceKHR`]: https://www.khronos.org/registry/vulkan/specs/1.3-extensions/man/html/VkSurfaceKHR.html
    /// [`wgpu::Surface`]: https://docs.rs/wgpu/latest/wgpu/struct.Surface.html
    ///
    /// ## iOS
    ///
    /// On iOS, the `Resumed` event is emitted in response to an [`applicationDidBecomeActive`]
    /// callback which means the application is "active" (according to the
    /// [iOS application lifecycle]).
    ///
    /// [`applicationDidBecomeActive`]: https://developer.apple.com/documentation/uikit/uiapplicationdelegate/1622956-applicationdidbecomeactive
    /// [iOS application lifecycle]: https://developer.apple.com/documentation/uikit/app_and_environment/managing_your_app_s_life_cycle
    ///
    /// ## Web
    ///
    /// On Web, the `Resumed` event is emitted in response to a [`pageshow`] event
    /// with the property [`persisted`] being true, which means that the page is being
    /// restored from the [`bfcache`] (back/forward cache) - an in-memory cache that
    /// stores a complete snapshot of a page (including the JavaScript heap) as the
    /// user is navigating away.
    ///
    /// [`pageshow`]: https://developer.mozilla.org/en-US/docs/Web/API/Window/pageshow_event
    /// [`persisted`]: https://developer.mozilla.org/en-US/docs/Web/API/PageTransitionEvent/persisted
    /// [`bfcache`]: https://web.dev/bfcache/
    /// [`Suspended`]: Self::suspended
    fn resumed(&mut self, event_loop: &ActiveEventLoop);

    /// Called after a wake up is requested using [`EventLoopProxy::wake_up`].
    ///
    /// Multiple calls to the aforementioned method may result in only a single `proxy_wake_up` call
    /// for performance reasons. Furthermore, this callback may be emitted spuriously. For these
    /// reasons, you should probably use some other tool like [`std::sync::mspc`] if you want to
    /// know the number of times that your application was _actually_ awoken, see the below example.
    ///
    /// The order in which this is emitted in relation to other events is not guaranteed. The time
    /// at which this will be emitted is not guaranteed, only that it will happen "soon". That is,
    /// there may be several executions of the event loop, including multiple redraws to windows,
    /// between [`EventLoopProxy::wake_up`] being called and the event being delivered.
    ///
    /// [`EventLoopProxy::wake_up`]: crate::event_loop::EventLoopProxy::wake_up
    ///
    /// # Example
    ///
    /// Use a [`std::sync::mpsc`] channel to handle events from a different thread.
    ///
    /// ```no_run
    /// use std::sync::mpsc;
    /// use std::thread;
    /// use std::time::Duration;
    ///
    /// use winit::application::ApplicationHandler;
    /// use winit::event_loop::{ActiveEventLoop, EventLoop};
    ///
    /// struct MyApp {
    ///     receiver: mpsc::Receiver<u64>,
    /// }
    ///
    /// impl ApplicationHandler for MyApp {
    ///     # fn window_event(
    ///     #     &mut self,
    ///     #     _event_loop: &ActiveEventLoop,
    ///     #     _window_id: winit::window::WindowId,
    ///     #     _event: winit::event::WindowEvent,
    ///     # ) {
    ///     # }
    ///     #
    ///     # fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}
    ///     #
    ///     fn proxy_wake_up(&mut self, _event_loop: &ActiveEventLoop) {
    ///         // Iterate current events, since wake-ups may have been merged.
    ///         //
    ///         // Note: We take care not to use `recv` or `iter` here, as those are blocking,
    ///         // and that would be bad for performance and might lead to a deadlock.
    ///         for i in self.receiver.try_iter() {
    ///             println!("handling: {i}");
    ///         }
    ///     }
    ///
    ///     // Rest of `ApplicationHandler`
    /// }
    ///
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let event_loop = EventLoop::new()?;
    ///     let proxy = event_loop.create_proxy();
    ///
    ///     let (sender, receiver) = mpsc::channel();
    ///
    ///     let mut app = MyApp { receiver };
    ///
    ///     thread::scope(|s| {
    ///         // Send an event in a loop
    ///         s.spawn(|| {
    ///             let mut i = 0;
    ///             loop {
    ///                 println!("sending: {i}");
    ///                 if sender.send(i).is_err() {
    ///                     // Stop sending once `MyApp` is dropped
    ///                     break;
    ///                 }
    ///                 // Trigger the wake-up _after_ we placed the event in the channel.
    ///                 // Otherwise, `proxy_wake_up` might be triggered prematurely.
    ///                 proxy.wake_up();
    ///                 i += 1;
    ///                 thread::sleep(Duration::from_millis(i));
    ///             }
    ///         });
    ///
    ///         Ok(event_loop.run_app(&mut app)?)
    ///     })
    /// }
    /// ```
    fn proxy_wake_up(&mut self, event_loop: &ActiveEventLoop) {
        let _ = event_loop;
    }

    /// Emitted when the OS sends an event to a winit window.
    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    );

    /// Emitted when the OS sends an event to a device.
    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        device_id: DeviceId,
        event: DeviceEvent,
    ) {
        let _ = (event_loop, device_id, event);
    }

    /// Emitted when the event loop is about to block and wait for new events.
    ///
    /// Most applications shouldn't need to hook into this event since there is no real relationship
    /// between how often the event loop needs to wake up and the dispatching of any specific
    /// events.
    ///
    /// High frequency event sources, such as input devices could potentially lead to lots of wake
    /// ups and also lots of corresponding `AboutToWait` events.
    ///
    /// This is not an ideal event to drive application rendering from and instead applications
    /// should render in response to [`WindowEvent::RedrawRequested`] events.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let _ = event_loop;
    }

    /// Emitted when the application has been suspended.
    ///
    /// # Portability
    ///
    /// Not all platforms support the notion of suspending applications, and there may be no
    /// technical way to guarantee being able to emit a `Suspended` event if the OS has
    /// no formal application lifecycle (currently only Android, iOS, and Web do). For this reason,
    /// Winit does not currently try to emit pseudo `Suspended` events before the application
    /// quits on platforms without an application lifecycle.
    ///
    /// Considering that the implementation of `Suspended` and [`Resumed`] events may be internally
    /// driven by multiple platform-specific events, and that there may be subtle differences across
    /// platforms with how these internal events are delivered, it's recommended that applications
    /// be able to gracefully handle redundant (i.e. back-to-back) `Suspended` or [`Resumed`]
    /// events.
    ///
    /// Also see [`Resumed`] notes.
    ///
    /// ## Android
    ///
    /// On Android, the `Suspended` event is only sent when the application's associated
    /// [`SurfaceView`] is destroyed. This is expected to closely correlate with the [`onPause`]
    /// lifecycle event but there may technically be a discrepancy.
    ///
    /// [`onPause`]: https://developer.android.com/reference/android/app/Activity#onPause()
    ///
    /// Applications that need to run on Android should assume their [`SurfaceView`] has been
    /// destroyed, which indirectly invalidates any existing render surfaces that may have been
    /// created outside of Winit (such as an `EGLSurface`, [`VkSurfaceKHR`] or [`wgpu::Surface`]).
    ///
    /// After being `Suspended` on Android applications must drop all render surfaces before
    /// the event callback completes, which may be re-created when the application is next
    /// [`Resumed`].
    ///
    /// [`SurfaceView`]: https://developer.android.com/reference/android/view/SurfaceView
    /// [Activity lifecycle]: https://developer.android.com/guide/components/activities/activity-lifecycle
    /// [`VkSurfaceKHR`]: https://www.khronos.org/registry/vulkan/specs/1.3-extensions/man/html/VkSurfaceKHR.html
    /// [`wgpu::Surface`]: https://docs.rs/wgpu/latest/wgpu/struct.Surface.html
    ///
    /// ## iOS
    ///
    /// On iOS, the `Suspended` event is currently emitted in response to an
    /// [`applicationWillResignActive`] callback which means that the application is
    /// about to transition from the active to inactive state (according to the
    /// [iOS application lifecycle]).
    ///
    /// [`applicationWillResignActive`]: https://developer.apple.com/documentation/uikit/uiapplicationdelegate/1622950-applicationwillresignactive
    /// [iOS application lifecycle]: https://developer.apple.com/documentation/uikit/app_and_environment/managing_your_app_s_life_cycle
    ///
    /// ## Web
    ///
    /// On Web, the `Suspended` event is emitted in response to a [`pagehide`] event
    /// with the property [`persisted`] being true, which means that the page is being
    /// put in the [`bfcache`] (back/forward cache) - an in-memory cache that stores a
    /// complete snapshot of a page (including the JavaScript heap) as the user is
    /// navigating away.
    ///
    /// [`pagehide`]: https://developer.mozilla.org/en-US/docs/Web/API/Window/pagehide_event
    /// [`persisted`]: https://developer.mozilla.org/en-US/docs/Web/API/PageTransitionEvent/persisted
    /// [`bfcache`]: https://web.dev/bfcache/
    /// [`Resumed`]: Self::resumed
    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        let _ = event_loop;
    }

    /// Emitted when the event loop is being shut down.
    ///
    /// This is irreversible - if this method is called, it is guaranteed that the event loop
    /// will exit right after.
    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        let _ = event_loop;
    }

    /// Emitted when the application has received a memory warning.
    ///
    /// ## Platform-specific
    ///
    /// ### Android
    ///
    /// On Android, the `MemoryWarning` event is sent when [`onLowMemory`] was called. The
    /// application must [release memory] or risk being killed.
    ///
    /// [`onLowMemory`]: https://developer.android.com/reference/android/app/Application.html#onLowMemory()
    /// [release memory]: https://developer.android.com/topic/performance/memory#release
    ///
    /// ### iOS
    ///
    /// On iOS, the `MemoryWarning` event is emitted in response to an
    /// [`applicationDidReceiveMemoryWarning`] callback. The application must free as much
    /// memory as possible or risk being terminated, see [how to respond to memory warnings].
    ///
    /// [`applicationDidReceiveMemoryWarning`]: https://developer.apple.com/documentation/uikit/uiapplicationdelegate/1623063-applicationdidreceivememorywarni
    /// [how to respond to memory warnings]: https://developer.apple.com/documentation/uikit/app_and_environment/managing_your_app_s_life_cycle/responding_to_memory_warnings
    ///
    /// ### Others
    ///
    /// - **macOS / Orbital / Wayland / Web / Windows:** Unsupported.
    fn memory_warning(&mut self, event_loop: &ActiveEventLoop) {
        let _ = event_loop;
    }
}

impl<A: ?Sized + ApplicationHandler> ApplicationHandler for &mut A {
    #[inline]
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        (**self).new_events(event_loop, cause);
    }

    #[inline]
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        (**self).resumed(event_loop);
    }

    #[inline]
    fn proxy_wake_up(&mut self, event_loop: &ActiveEventLoop) {
        (**self).proxy_wake_up(event_loop);
    }

    #[inline]
    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        (**self).window_event(event_loop, window_id, event);
    }

    #[inline]
    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        device_id: DeviceId,
        event: DeviceEvent,
    ) {
        (**self).device_event(event_loop, device_id, event);
    }

    #[inline]
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        (**self).about_to_wait(event_loop);
    }

    #[inline]
    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        (**self).suspended(event_loop);
    }

    #[inline]
    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        (**self).exiting(event_loop);
    }

    #[inline]
    fn memory_warning(&mut self, event_loop: &ActiveEventLoop) {
        (**self).memory_warning(event_loop);
    }
}

impl<A: ?Sized + ApplicationHandler> ApplicationHandler for Box<A> {
    #[inline]
    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        (**self).new_events(event_loop, cause);
    }

    #[inline]
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        (**self).resumed(event_loop);
    }

    #[inline]
    fn proxy_wake_up(&mut self, event_loop: &ActiveEventLoop) {
        (**self).proxy_wake_up(event_loop);
    }

    #[inline]
    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        (**self).window_event(event_loop, window_id, event);
    }

    #[inline]
    fn device_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        device_id: DeviceId,
        event: DeviceEvent,
    ) {
        (**self).device_event(event_loop, device_id, event);
    }

    #[inline]
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        (**self).about_to_wait(event_loop);
    }

    #[inline]
    fn suspended(&mut self, event_loop: &ActiveEventLoop) {
        (**self).suspended(event_loop);
    }

    #[inline]
    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        (**self).exiting(event_loop);
    }

    #[inline]
    fn memory_warning(&mut self, event_loop: &ActiveEventLoop) {
        (**self).memory_warning(event_loop);
    }
}
