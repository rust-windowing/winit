//! End user application handling.

use crate::event::{DeviceEvent, DeviceId, StartCause, WindowEvent};
use crate::event_loop::ActiveEventLoop;
use crate::window::WindowId;

/// The handler of the application events.
///
/// This is dropped when the event loop is being shut down.
pub trait ApplicationHandler {
    /// Emitted when new events arrive from the OS to be processed.
    ///
    /// This is a useful place to put code that should be done before you start processing
    /// events, such as updating frame timing information for benchmarking or checking the
    /// [`StartCause`] to see if a timer set by
    /// [`ControlFlow::WaitUntil`][crate::event_loop::ControlFlow::WaitUntil] has elapsed.
    fn new_events(&mut self, event_loop: &dyn ActiveEventLoop, cause: StartCause) {
        let _ = (event_loop, cause);
    }

    /// Emitted when the application has been resumed.
    ///
    /// See [`suspended()`][Self::suspended].
    ///
    /// ## Platform-specific
    ///
    /// ### iOS
    ///
    /// On iOS, the [`resumed()`] method is called in response to an [`applicationDidBecomeActive`]
    /// callback which means the application is about to transition from the inactive to active
    /// state (according to the [iOS application lifecycle]).
    ///
    /// [`applicationDidBecomeActive`]: https://developer.apple.com/documentation/uikit/uiapplicationdelegate/1622956-applicationdidbecomeactive
    /// [iOS application lifecycle]: https://developer.apple.com/documentation/uikit/app_and_environment/managing_your_app_s_life_cycle
    ///
    /// ### Web
    ///
    /// On Web, the [`resumed()`] method is called in response to a [`pageshow`] event if the
    /// page is being restored from the [`bfcache`] (back/forward cache) - an in-memory cache
    /// that stores a complete snapshot of a page (including the JavaScript heap) as the user is
    /// navigating away.
    ///
    /// [`pageshow`]: https://developer.mozilla.org/en-US/docs/Web/API/Window/pageshow_event
    /// [`bfcache`]: https://web.dev/bfcache/
    ///
    /// ### Others
    ///
    /// **Android / macOS / Orbital / Wayland / Windows / X11:** Unsupported.
    ///
    /// [`resumed()`]: Self::resumed
    fn resumed(&mut self, event_loop: &dyn ActiveEventLoop) {
        let _ = event_loop;
    }

    /// Called after a wake up is requested using [`EventLoopProxy::wake_up()`].
    ///
    /// Multiple calls to the aforementioned method will be merged, and will only wake the event
    /// loop once; however, due to the nature of multi-threading some wake ups may appear
    /// spuriously. For these reasons, you should not rely on the number of times that this was
    /// called.
    ///
    /// The order in which this is emitted in relation to other events is not guaranteed. The time
    /// at which this will be emitted is not guaranteed, only that it will happen "soon". That is,
    /// there may be several executions of the event loop, including multiple redraws to windows,
    /// between [`EventLoopProxy::wake_up()`] being called and the event being delivered.
    ///
    /// [`EventLoopProxy::wake_up()`]: crate::event_loop::EventLoopProxy::wake_up
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
    ///     #     _event_loop: &dyn ActiveEventLoop,
    ///     #     _window_id: winit::window::WindowId,
    ///     #     _event: winit::event::WindowEvent,
    ///     # ) {
    ///     # }
    ///     #
    ///     fn proxy_wake_up(&mut self, _event_loop: &dyn ActiveEventLoop) {
    ///         // Iterate current events, since wake-ups may have been merged.
    ///         //
    ///         // Note: We take care not to use `recv` or `iter` here, as those are blocking,
    ///         // and that would be bad for performance and might lead to a deadlock.
    ///         for i in self.receiver.try_iter() {
    ///             println!("received: {i}");
    ///         }
    ///     }
    ///
    ///     // Rest of `ApplicationHandler`
    /// }
    ///
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let event_loop = EventLoop::new()?;
    ///
    ///     let (sender, receiver) = mpsc::channel();
    ///
    ///     // Send an event in a loop
    ///     let proxy = event_loop.create_proxy();
    ///     let background_thread = thread::spawn(move || {
    ///         let mut i = 0;
    ///         loop {
    ///             println!("sending: {i}");
    ///             if sender.send(i).is_err() {
    ///                 // Stop sending once `MyApp` is dropped
    ///                 break;
    ///             }
    ///             // Trigger the wake-up _after_ we placed the event in the channel.
    ///             // Otherwise, `proxy_wake_up` might be triggered prematurely.
    ///             proxy.wake_up();
    ///             i += 1;
    ///             thread::sleep(Duration::from_secs(1));
    ///         }
    ///     });
    ///
    ///     event_loop.run(|_event_loop| MyApp { receiver })?;
    ///
    ///     background_thread.join().unwrap();
    ///
    ///     Ok(())
    /// }
    /// ```
    fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
        let _ = event_loop;
    }

    /// Emitted when the OS sends an event to a winit window.
    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    );

    /// Emitted when the OS sends an event to a device.
    fn device_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
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
    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        let _ = event_loop;
    }

    /// Emitted when the application has been suspended.
    ///
    /// See [`resumed()`][Self::resumed].
    ///
    /// ## Platform-specific
    ///
    /// ### iOS
    ///
    /// On iOS, the [`suspended()`] method is called in response to an
    /// [`applicationWillResignActive`] callback which means that the application is about to
    /// transition from the active to inactive state (according to the [iOS application lifecycle]).
    ///
    /// [`applicationWillResignActive`]: https://developer.apple.com/documentation/uikit/uiapplicationdelegate/1622950-applicationwillresignactive
    /// [iOS application lifecycle]: https://developer.apple.com/documentation/uikit/app_and_environment/managing_your_app_s_life_cycle
    ///
    /// ### Web
    ///
    /// On Web, the [`suspended()`] method is called in response to a [`pagehide`] event if the
    /// page is being restored from the [`bfcache`] (back/forward cache) - an in-memory cache that
    /// stores a complete snapshot of a page (including the JavaScript heap) as the user is
    /// navigating away.
    ///
    /// [`pagehide`]: https://developer.mozilla.org/en-US/docs/Web/API/Window/pagehide_event
    /// [`bfcache`]: https://web.dev/bfcache/
    ///
    /// ### Others
    ///
    /// **Android / macOS / Orbital / Wayland / Windows / X11:** Unsupported.
    ///
    /// [`suspended()`]: Self::suspended
    fn suspended(&mut self, event_loop: &dyn ActiveEventLoop) {
        let _ = event_loop;
    }

    /// Emitted when the application must destroy its render surfaces.
    ///
    /// [`recreate_surfaces()`] is emitted after this when it's safe to re-create surfaces.
    ///
    /// ## Platform-specific
    ///
    /// ### Android
    ///
    /// Applications that need to run on Android must be able to handle their underlying
    /// [`SurfaceView`] being destroyed, which in turn indirectly invalidates any existing
    /// render surfaces that may have been created outside of Winit (such as an `EGLSurface`,
    /// [`VkSurfaceKHR`] or [`wgpu::Surface`]).
    ///
    /// This means that in this method, you must drop all render surfaces before the event callback
    /// completes, and only re-create them in [`recreate_surfaces()`].
    ///
    /// This is expected to closely correlate with the [`onPause`] lifecycle event but there may
    /// technically be a discrepancy.
    ///
    /// [`recreate_surfaces()`]: Self::recreate_surfaces
    /// [`SurfaceView`]: https://developer.android.com/reference/android/view/SurfaceView
    /// [`VkSurfaceKHR`]: https://www.khronos.org/registry/vulkan/specs/1.3-extensions/man/html/VkSurfaceKHR.html
    /// [`wgpu::Surface`]: https://docs.rs/wgpu/latest/wgpu/struct.Surface.html
    /// [`onPause`]: https://developer.android.com/reference/android/app/Activity#onPause()
    fn destroy_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        let _ = event_loop;
    }

    /// Emitted when the application should re-create render surfaces, after destroying them in
    /// [`destroy_surfaces()`], see that for details.
    ///
    /// ## Platform-specific
    ///
    /// ### Android
    ///
    /// On Android, surfaces should be created in the initialization closure given to
    /// [`EventLoop::run()`], just like on all other platforms. When they need to be re-created
    /// after being destroyed by the system, because a new [`SurfaceView`] has been created, you
    /// will need to use this callback.
    ///
    /// This is expected to closely correlate with the [`onResume`] lifecycle event but there may
    /// technically be a discrepancy.
    ///
    /// [`destroy_surfaces()`]: Self::destroy_surfaces
    /// [`EventLoop::run()`]: crate::event_loop::EventLoop::run
    /// [`SurfaceView`]: https://developer.android.com/reference/android/view/SurfaceView
    /// [`onResume`]: https://developer.android.com/reference/android/app/Activity#onResume()
    fn recreate_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
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
    fn memory_warning(&mut self, event_loop: &dyn ActiveEventLoop) {
        let _ = event_loop;
    }
}

#[deny(clippy::missing_trait_methods)]
impl<A: ?Sized + ApplicationHandler> ApplicationHandler for &mut A {
    #[inline]
    fn new_events(&mut self, event_loop: &dyn ActiveEventLoop, cause: StartCause) {
        (**self).new_events(event_loop, cause);
    }

    #[inline]
    fn resumed(&mut self, event_loop: &dyn ActiveEventLoop) {
        (**self).resumed(event_loop);
    }

    #[inline]
    fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
        (**self).proxy_wake_up(event_loop);
    }

    #[inline]
    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        (**self).window_event(event_loop, window_id, event);
    }

    #[inline]
    fn device_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        device_id: DeviceId,
        event: DeviceEvent,
    ) {
        (**self).device_event(event_loop, device_id, event);
    }

    #[inline]
    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        (**self).about_to_wait(event_loop);
    }

    #[inline]
    fn suspended(&mut self, event_loop: &dyn ActiveEventLoop) {
        (**self).suspended(event_loop);
    }

    #[inline]
    fn destroy_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        (**self).destroy_surfaces(event_loop);
    }

    #[inline]
    fn recreate_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        (**self).recreate_surfaces(event_loop);
    }

    #[inline]
    fn memory_warning(&mut self, event_loop: &dyn ActiveEventLoop) {
        (**self).memory_warning(event_loop);
    }
}

#[deny(clippy::missing_trait_methods)]
impl<A: ?Sized + ApplicationHandler> ApplicationHandler for Box<A> {
    #[inline]
    fn new_events(&mut self, event_loop: &dyn ActiveEventLoop, cause: StartCause) {
        (**self).new_events(event_loop, cause);
    }

    #[inline]
    fn resumed(&mut self, event_loop: &dyn ActiveEventLoop) {
        (**self).resumed(event_loop);
    }

    #[inline]
    fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
        (**self).proxy_wake_up(event_loop);
    }

    #[inline]
    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        (**self).window_event(event_loop, window_id, event);
    }

    #[inline]
    fn device_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        device_id: DeviceId,
        event: DeviceEvent,
    ) {
        (**self).device_event(event_loop, device_id, event);
    }

    #[inline]
    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        (**self).about_to_wait(event_loop);
    }

    #[inline]
    fn suspended(&mut self, event_loop: &dyn ActiveEventLoop) {
        (**self).suspended(event_loop);
    }

    #[inline]
    fn destroy_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        (**self).destroy_surfaces(event_loop);
    }

    #[inline]
    fn recreate_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        (**self).recreate_surfaces(event_loop);
    }

    #[inline]
    fn memory_warning(&mut self, event_loop: &dyn ActiveEventLoop) {
        (**self).memory_warning(event_loop);
    }
}
