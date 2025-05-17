//! The event-loop routines.

use std::cell::{Cell, RefCell};
use std::io::Result as IOResult;
use std::mem;
use std::os::fd::OwnedFd;
use std::os::unix::io::{AsFd, AsRawFd, BorrowedFd, RawFd};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use calloop::ping::Ping;
use rustix::event::{PollFd, PollFlags};
use rustix::pipe::{self, PipeFlags};
use sctk::reexports::calloop_wayland_source::WaylandSource;
use sctk::reexports::client::{globals, Connection, QueueHandle};
use tracing::warn;
use winit_core::event_loop::pump_events::PumpStatus;

use crate::application::ApplicationHandler;
use crate::cursor::{CustomCursor as CoreCustomCursor, CustomCursorSource};
use crate::dpi::LogicalSize;
use crate::error::{EventLoopError, NotSupportedError, OsError, RequestError};
use crate::event::{DeviceEvent, StartCause, SurfaceSizeWriter, WindowEvent};
use crate::event_loop::{
    ActiveEventLoop as RootActiveEventLoop, ControlFlow, DeviceEvents,
    OwnedDisplayHandle as CoreOwnedDisplayHandle,
};
use crate::monitor::MonitorHandle as CoreMonitorHandle;
use crate::platform_impl::platform::min_timeout;
use crate::platform_impl::wayland::types::cursor::WaylandCustomCursor;
use crate::window::Theme;

mod proxy;
pub mod sink;

use proxy::EventLoopProxy;
use sink::EventSink;

use super::output::MonitorHandle;
use super::state::{WindowCompositorUpdate, WinitState};
use super::window::state::FrameCallbackState;
use super::{logical_to_physical_rounded, WindowId};
pub use crate::event_loop::EventLoopProxy as CoreEventLoopProxy;

type WaylandDispatcher = calloop::Dispatcher<'static, WaylandSource<WinitState>, WinitState>;

#[derive(Debug)]
pub(crate) enum Event {
    WindowEvent { window_id: WindowId, event: WindowEvent },
    DeviceEvent { event: DeviceEvent },
}

/// The Wayland event loop.
#[derive(Debug)]
pub struct EventLoop {
    /// Has `run` or `run_on_demand` been called or a call to `pump_events` that starts the loop
    loop_running: bool,

    buffer_sink: EventSink,
    compositor_updates: Vec<WindowCompositorUpdate>,
    window_ids: Vec<WindowId>,

    /// The Wayland dispatcher to has raw access to the queue when needed, such as
    /// when creating a new window.
    wayland_dispatcher: WaylandDispatcher,

    /// Connection to the wayland server.
    handle: Arc<OwnedDisplayHandle>,

    /// Event loop window target.
    active_event_loop: ActiveEventLoop,

    // XXX drop after everything else, just to be safe.
    /// Calloop's event loop.
    event_loop: calloop::EventLoop<'static, WinitState>,

    pump_event_notifier: Option<PumpEventNotifier>,
}

impl EventLoop {
    pub fn new() -> Result<EventLoop, EventLoopError> {
        let connection = Connection::connect_to_env().map_err(|err| os_error!(err))?;

        let (globals, mut event_queue) =
            globals::registry_queue_init(&connection).map_err(|err| os_error!(err))?;
        let queue_handle = event_queue.handle();

        let event_loop =
            calloop::EventLoop::<WinitState>::try_new().map_err(|err| os_error!(err))?;

        let mut winit_state = WinitState::new(&globals, &queue_handle, event_loop.handle())?;

        // NOTE: do a roundtrip after binding the globals to prevent potential
        // races with the server.
        event_queue.roundtrip(&mut winit_state).map_err(|err| os_error!(err))?;

        // Register Wayland source.
        let wayland_source = WaylandSource::new(connection.clone(), event_queue);
        let wayland_dispatcher =
            calloop::Dispatcher::new(wayland_source, |_, queue, winit_state: &mut WinitState| {
                let result = queue.dispatch_pending(winit_state);
                if result.is_ok()
                    && (!winit_state.events_sink.is_empty()
                        || !winit_state.window_compositor_updates.is_empty())
                {
                    winit_state.dispatched_events = true;
                }
                result
            });

        event_loop
            .handle()
            .register_dispatcher(wayland_dispatcher.clone())
            .map_err(|err| os_error!(err))?;

        // Setup the user proxy.
        let (ping, ping_source) = calloop::ping::make_ping().unwrap();
        event_loop
            .handle()
            .insert_source(ping_source, move |_, _, winit_state: &mut WinitState| {
                winit_state.dispatched_events = true;
                winit_state.proxy_wake_up = true;
            })
            .map_err(|err| os_error!(err))?;

        // An event's loop awakener to wake up for window events from winit's windows.
        let (event_loop_awakener, event_loop_awakener_source) =
            calloop::ping::make_ping().map_err(|err| os_error!(err))?;

        event_loop
            .handle()
            .insert_source(event_loop_awakener_source, move |_, _, winit_state: &mut WinitState| {
                // Mark that we have something to dispatch.
                winit_state.dispatched_events = true;
            })
            .map_err(|err| os_error!(err))?;

        let handle = Arc::new(OwnedDisplayHandle::new(connection));
        let active_event_loop = ActiveEventLoop {
            handle: handle.clone(),
            wayland_dispatcher: wayland_dispatcher.clone(),
            event_loop_awakener,
            event_loop_proxy: EventLoopProxy::new(ping).into(),
            queue_handle,
            control_flow: Cell::new(ControlFlow::default()),
            exit: Cell::new(None),
            state: RefCell::new(winit_state),
        };

        let event_loop = Self {
            loop_running: false,
            compositor_updates: Vec::new(),
            buffer_sink: EventSink::default(),
            window_ids: Vec::new(),
            handle,
            wayland_dispatcher,
            event_loop,
            active_event_loop,
            pump_event_notifier: None,
        };

        Ok(event_loop)
    }

    pub fn run_app<A: ApplicationHandler>(mut self, app: A) -> Result<(), EventLoopError> {
        self.run_app_on_demand(app)
    }

    pub fn run_app_on_demand<A: ApplicationHandler>(
        &mut self,
        mut app: A,
    ) -> Result<(), EventLoopError> {
        self.active_event_loop.clear_exit();
        let exit = loop {
            match self.pump_app_events(None, &mut app) {
                PumpStatus::Exit(0) => {
                    break Ok(());
                },
                PumpStatus::Exit(code) => {
                    break Err(EventLoopError::ExitFailure(code));
                },
                _ => {
                    continue;
                },
            }
        };

        // Applications aren't allowed to carry windows between separate
        // `run_on_demand` calls but if they have only just dropped their
        // windows we need to make sure those last requests are sent to the
        // compositor.
        let _ = self.roundtrip().map_err(EventLoopError::Os);

        exit
    }

    pub fn pump_app_events<A: ApplicationHandler>(
        &mut self,
        timeout: Option<Duration>,
        mut app: A,
    ) -> PumpStatus {
        if !self.loop_running {
            self.loop_running = true;

            // Run the initial loop iteration.
            self.single_iteration(&mut app, StartCause::Init);
        }

        // Consider the possibility that the `StartCause::Init` iteration could
        // request to Exit.
        if !self.exiting() {
            self.poll_events_with_timeout(timeout, &mut app);
        }

        if let Some(code) = self.exit_code() {
            self.loop_running = false;

            PumpStatus::Exit(code)
        } else {
            // NOTE: spawn a wake-up thread, thus if we have code reading the wayland connection
            // in parallel to winit, we ensure that the loop itself is marked as having events.
            if timeout.is_some() && self.pump_event_notifier.is_none() {
                self.pump_event_notifier = Some(PumpEventNotifier::spawn(
                    self.active_event_loop.handle.connection.clone(),
                    self.active_event_loop.event_loop_awakener.clone(),
                ));
            }

            if let Some(pump_event_notifier) = self.pump_event_notifier.as_ref() {
                // Notify that we don't have to wait, since we're out of winit.
                *pump_event_notifier.control.0.lock().unwrap() = PumpEventNotifierAction::Monitor;
                pump_event_notifier.control.1.notify_one();
            }

            PumpStatus::Continue
        }
    }

    fn poll_events_with_timeout<A: ApplicationHandler>(
        &mut self,
        mut timeout: Option<Duration>,
        app: &mut A,
    ) {
        let cause = loop {
            let start = Instant::now();

            timeout = {
                let control_flow_timeout = match self.control_flow() {
                    ControlFlow::Wait => None,
                    ControlFlow::Poll => Some(Duration::ZERO),
                    ControlFlow::WaitUntil(wait_deadline) => {
                        Some(wait_deadline.saturating_duration_since(start))
                    },
                };
                min_timeout(control_flow_timeout, timeout)
            };

            // NOTE Ideally we should flush as the last thing we do before polling
            // to wait for events, and this should be done by the calloop
            // WaylandSource but we currently need to flush writes manually.
            //
            // Checking for flush error is essential to perform an exit with error, since
            // once we have a protocol error, we could get stuck retrying...
            if self.handle.connection.flush().is_err() {
                self.set_exit_code(1);
                return;
            }

            if let Err(error) = self.loop_dispatch(timeout) {
                // NOTE We exit on errors from dispatches, since if we've got protocol error
                // libwayland-client/wayland-rs will inform us anyway, but crashing downstream is
                // not really an option. Instead we inform that the event loop got
                // destroyed. We may communicate an error that something was
                // terminated, but winit doesn't provide us with an API to do that
                // via some event. Still, we set the exit code to the error's OS
                // error code, or to 1 if not possible.
                let exit_code = error.raw_os_error().unwrap_or(1);
                self.set_exit_code(exit_code);
                return;
            }

            // NB: `StartCause::Init` is handled as a special case and doesn't need
            // to be considered here
            let cause = match self.control_flow() {
                ControlFlow::Poll => StartCause::Poll,
                ControlFlow::Wait => StartCause::WaitCancelled { start, requested_resume: None },
                ControlFlow::WaitUntil(deadline) => {
                    if Instant::now() < deadline {
                        StartCause::WaitCancelled { start, requested_resume: Some(deadline) }
                    } else {
                        StartCause::ResumeTimeReached { start, requested_resume: deadline }
                    }
                },
            };

            // Reduce spurious wake-ups.
            let dispatched_events = self.with_state(|state| state.dispatched_events);
            if matches!(cause, StartCause::WaitCancelled { .. })
                && !dispatched_events
                && timeout.is_none()
            {
                continue;
            }

            break cause;
        };

        self.single_iteration(app, cause);
    }

    fn single_iteration<A: ApplicationHandler>(&mut self, app: &mut A, cause: StartCause) {
        // NOTE currently just indented to simplify the diff

        // We retain these grow-only scratch buffers as part of the EventLoop
        // for the sake of avoiding lots of reallocs. We take them here to avoid
        // trying to mutably borrow `self` more than once and we swap them back
        // when finished.
        let mut compositor_updates = std::mem::take(&mut self.compositor_updates);
        let mut buffer_sink = std::mem::take(&mut self.buffer_sink);
        let mut window_ids = std::mem::take(&mut self.window_ids);

        app.new_events(&self.active_event_loop, cause);

        // NB: For consistency all platforms must call `can_create_surfaces` even though Wayland
        // applications don't themselves have a formal surface destroy/create lifecycle.
        if cause == StartCause::Init {
            app.can_create_surfaces(&self.active_event_loop);
        }

        // Indicate user wake up.
        if self.with_state(|state| mem::take(&mut state.proxy_wake_up)) {
            app.proxy_wake_up(&self.active_event_loop);
        }

        // Drain the pending compositor updates.
        self.with_state(|state| compositor_updates.append(&mut state.window_compositor_updates));

        for mut compositor_update in compositor_updates.drain(..) {
            let window_id = compositor_update.window_id;
            if compositor_update.scale_changed {
                let (physical_size, scale_factor) = self.with_state(|state| {
                    let windows = state.windows.get_mut();
                    let window = windows.get(&window_id).unwrap().lock().unwrap();
                    let scale_factor = window.scale_factor();
                    let size = logical_to_physical_rounded(window.surface_size(), scale_factor);
                    (size, scale_factor)
                });

                // Stash the old window size.
                let old_physical_size = physical_size;

                let new_surface_size = Arc::new(Mutex::new(physical_size));
                let event = WindowEvent::ScaleFactorChanged {
                    scale_factor,
                    surface_size_writer: SurfaceSizeWriter::new(Arc::downgrade(&new_surface_size)),
                };

                app.window_event(&self.active_event_loop, window_id, event);

                let physical_size = *new_surface_size.lock().unwrap();
                drop(new_surface_size);

                // Resize the window when user altered the size.
                if old_physical_size != physical_size {
                    self.with_state(|state| {
                        let windows = state.windows.get_mut();
                        let mut window = windows.get(&window_id).unwrap().lock().unwrap();

                        let new_logical_size: LogicalSize<f64> =
                            physical_size.to_logical(scale_factor);
                        window.request_surface_size(new_logical_size.into());
                    });

                    // Make it queue resize.
                    compositor_update.resized = true;
                }
            }

            // NOTE: Rescale changed the physical size which winit operates in, thus we should
            // resize.
            if compositor_update.resized || compositor_update.scale_changed {
                let physical_size = self.with_state(|state| {
                    let windows = state.windows.get_mut();
                    let window = windows.get(&window_id).unwrap().lock().unwrap();

                    let scale_factor = window.scale_factor();
                    let size = logical_to_physical_rounded(window.surface_size(), scale_factor);

                    // Mark the window as needed a redraw.
                    state
                        .window_requests
                        .get_mut()
                        .get_mut(&window_id)
                        .unwrap()
                        .redraw_requested
                        .store(true, Ordering::Relaxed);

                    size
                });

                let event = WindowEvent::SurfaceResized(physical_size);
                app.window_event(&self.active_event_loop, window_id, event);
            }

            if compositor_update.close_window {
                app.window_event(&self.active_event_loop, window_id, WindowEvent::CloseRequested);
            }
        }

        // Push the events directly from the window.
        self.with_state(|state| {
            buffer_sink.append(&mut state.window_events_sink.lock().unwrap());
        });
        for event in buffer_sink.drain() {
            match event {
                Event::WindowEvent { window_id, event } => {
                    app.window_event(&self.active_event_loop, window_id, event)
                },
                Event::DeviceEvent { event } => {
                    app.device_event(&self.active_event_loop, None, event)
                },
            }
        }

        // Handle non-synthetic events.
        self.with_state(|state| {
            buffer_sink.append(&mut state.events_sink);
        });
        for event in buffer_sink.drain() {
            match event {
                Event::WindowEvent { window_id, event } => {
                    app.window_event(&self.active_event_loop, window_id, event)
                },
                Event::DeviceEvent { event } => {
                    app.device_event(&self.active_event_loop, None, event)
                },
            }
        }

        // Collect the window ids
        self.with_state(|state| {
            window_ids.extend(state.window_requests.get_mut().keys());
        });

        for window_id in window_ids.iter() {
            let event = self.with_state(|state| {
                let window_requests = state.window_requests.get_mut();
                if window_requests.get(window_id).unwrap().take_closed() {
                    mem::drop(window_requests.remove(window_id));
                    mem::drop(state.windows.get_mut().remove(window_id));
                    return Some(WindowEvent::Destroyed);
                }

                let mut window =
                    state.windows.get_mut().get_mut(window_id).unwrap().lock().unwrap();

                if window.frame_callback_state() == FrameCallbackState::Requested {
                    return None;
                }

                // Reset the frame callbacks state.
                window.frame_callback_reset();
                let mut redraw_requested =
                    window_requests.get(window_id).unwrap().take_redraw_requested();

                // Redraw the frame while at it.
                redraw_requested |= window.refresh_frame();

                redraw_requested.then_some(WindowEvent::RedrawRequested)
            });

            if let Some(event) = event {
                app.window_event(&self.active_event_loop, *window_id, event);
            }
        }

        // Reset the hint that we've dispatched events.
        self.with_state(|state| {
            state.dispatched_events = false;
        });

        // This is always the last event we dispatch before poll again
        app.about_to_wait(&self.active_event_loop);

        // Update the window frames and schedule redraws.
        let mut wake_up = false;
        for window_id in window_ids.drain(..) {
            wake_up |= self.with_state(|state| match state.windows.get_mut().get_mut(&window_id) {
                Some(window) => {
                    let refresh = window.lock().unwrap().refresh_frame();
                    if refresh {
                        state
                            .window_requests
                            .get_mut()
                            .get_mut(&window_id)
                            .unwrap()
                            .redraw_requested
                            .store(true, Ordering::Relaxed);
                    }

                    refresh
                },
                None => false,
            });
        }

        // Wakeup event loop if needed.
        //
        // If the user draws from the `AboutToWait` this is likely not required, however
        // we can't do much about it.
        if wake_up {
            self.active_event_loop.event_loop_awakener.ping();
        }

        std::mem::swap(&mut self.compositor_updates, &mut compositor_updates);
        std::mem::swap(&mut self.buffer_sink, &mut buffer_sink);
        std::mem::swap(&mut self.window_ids, &mut window_ids);
    }

    #[inline]
    pub fn window_target(&self) -> &dyn RootActiveEventLoop {
        &self.active_event_loop
    }

    fn with_state<'a, U: 'a, F: FnOnce(&'a mut WinitState) -> U>(&'a mut self, callback: F) -> U {
        let state = self.active_event_loop.state.get_mut();
        callback(state)
    }

    fn loop_dispatch<D: Into<Option<std::time::Duration>>>(&mut self, timeout: D) -> IOResult<()> {
        let state = &mut self.active_event_loop.state.get_mut();

        self.event_loop.dispatch(timeout, state).map_err(|error| {
            tracing::error!("Error dispatching event loop: {}", error);
            error.into()
        })
    }

    fn roundtrip(&mut self) -> Result<usize, OsError> {
        let state = &mut self.active_event_loop.state.get_mut();

        let mut wayland_source = self.wayland_dispatcher.as_source_mut();
        let event_queue = wayland_source.queue();
        event_queue.roundtrip(state).map_err(|err| os_error!(err))
    }

    fn control_flow(&self) -> ControlFlow {
        self.active_event_loop.control_flow()
    }

    fn exiting(&self) -> bool {
        self.active_event_loop.exiting()
    }

    fn set_exit_code(&self, code: i32) {
        self.active_event_loop.set_exit_code(code)
    }

    fn exit_code(&self) -> Option<i32> {
        self.active_event_loop.exit_code()
    }
}

impl AsFd for EventLoop {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.event_loop.as_fd()
    }
}

impl AsRawFd for EventLoop {
    fn as_raw_fd(&self) -> RawFd {
        self.event_loop.as_raw_fd()
    }
}

#[derive(Debug)]
pub struct ActiveEventLoop {
    /// Event loop proxy
    event_loop_proxy: CoreEventLoopProxy,

    /// The event loop wakeup source.
    pub event_loop_awakener: Ping,

    /// The main queue used by the event loop.
    pub queue_handle: QueueHandle<WinitState>,

    /// The application's latest control_flow state
    pub(crate) control_flow: Cell<ControlFlow>,

    /// The application's exit state.
    pub(crate) exit: Cell<Option<i32>>,

    // TODO remove that RefCell once we can pass `&mut` in `Window::new`.
    /// Winit state.
    pub state: RefCell<WinitState>,

    /// Dispatcher of Wayland events.
    pub wayland_dispatcher: WaylandDispatcher,

    /// Handle for the underlying event loop.
    pub handle: Arc<OwnedDisplayHandle>,
}

impl RootActiveEventLoop for ActiveEventLoop {
    fn create_proxy(&self) -> CoreEventLoopProxy {
        self.event_loop_proxy.clone()
    }

    fn set_control_flow(&self, control_flow: ControlFlow) {
        self.control_flow.set(control_flow)
    }

    fn control_flow(&self) -> ControlFlow {
        self.control_flow.get()
    }

    fn exit(&self) {
        self.exit.set(Some(0))
    }

    fn exiting(&self) -> bool {
        self.exit.get().is_some()
    }

    #[inline]
    fn listen_device_events(&self, _allowed: DeviceEvents) {}

    fn create_custom_cursor(
        &self,
        cursor: CustomCursorSource,
    ) -> Result<CoreCustomCursor, RequestError> {
        let cursor_image = match cursor {
            CustomCursorSource::Image(cursor_image) => cursor_image,
            CustomCursorSource::Animation { .. } | CustomCursorSource::Url { .. } => {
                return Err(NotSupportedError::new("unsupported cursor kind").into())
            },
        };

        Ok(CoreCustomCursor(Arc::new(WaylandCustomCursor(cursor_image))))
    }

    #[inline]
    fn system_theme(&self) -> Option<Theme> {
        None
    }

    fn create_window(
        &self,
        window_attributes: crate::window::WindowAttributes,
    ) -> Result<Box<dyn crate::window::Window>, RequestError> {
        let window = crate::platform_impl::wayland::Window::new(self, window_attributes)?;
        Ok(Box::new(window))
    }

    fn available_monitors(&self) -> Box<dyn Iterator<Item = CoreMonitorHandle>> {
        Box::new(
            self.state
                .borrow()
                .output_state
                .outputs()
                .map(MonitorHandle::new)
                .map(|inner| CoreMonitorHandle(Arc::new(inner))),
        )
    }

    fn primary_monitor(&self) -> Option<CoreMonitorHandle> {
        // There's no primary monitor on Wayland.
        None
    }

    fn owned_display_handle(&self) -> CoreOwnedDisplayHandle {
        CoreOwnedDisplayHandle::new(self.handle.clone())
    }

    fn rwh_06_handle(&self) -> &dyn rwh_06::HasDisplayHandle {
        self
    }
}

impl ActiveEventLoop {
    fn clear_exit(&self) {
        self.exit.set(None)
    }

    fn set_exit_code(&self, code: i32) {
        self.exit.set(Some(code))
    }

    fn exit_code(&self) -> Option<i32> {
        self.exit.get()
    }
}

impl rwh_06::HasDisplayHandle for ActiveEventLoop {
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        self.handle.display_handle()
    }
}

#[derive(Debug)]
pub struct OwnedDisplayHandle {
    pub(crate) connection: Connection,
}

impl OwnedDisplayHandle {
    fn new(connection: Connection) -> Self {
        Self { connection }
    }
}

impl rwh_06::HasDisplayHandle for OwnedDisplayHandle {
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        use sctk::reexports::client::Proxy;

        let raw = rwh_06::WaylandDisplayHandle::new({
            let ptr = self.connection.display().id().as_ptr();
            std::ptr::NonNull::new(ptr as *mut _).expect("wl_display should never be null")
        });

        Ok(unsafe { rwh_06::DisplayHandle::borrow_raw(raw.into()) })
    }
}

#[derive(Debug)]
struct PumpEventNotifier {
    /// Whether we're in winit or not.
    control: Arc<(Mutex<PumpEventNotifierAction>, Condvar)>,
    /// Waker handle for the working thread.
    worker_waker: Option<OwnedFd>,
    /// Thread handle.
    handle: Option<JoinHandle<()>>,
}

impl Drop for PumpEventNotifier {
    fn drop(&mut self) {
        // Wake-up the thread.
        if let Some(worker_waker) = self.worker_waker.as_ref() {
            let _ = rustix::io::write(worker_waker.as_fd(), &[0u8]);
        }
        *self.control.0.lock().unwrap() = PumpEventNotifierAction::Monitor;
        self.control.1.notify_one();

        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl PumpEventNotifier {
    fn spawn(connection: Connection, awakener: Ping) -> Self {
        // Start from the waiting state.
        let control = Arc::new((Mutex::new(PumpEventNotifierAction::Pause), Condvar::new()));
        let control_thread = Arc::clone(&control);

        let (read, write) = match pipe::pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK) {
            Ok((read, write)) => (read, write),
            Err(_) => return Self { control, handle: None, worker_waker: None },
        };

        let handle =
            std::thread::Builder::new().name(String::from("pump_events mon")).spawn(move || {
                let (lock, cvar) = &*control_thread;
                'outer: loop {
                    let mut wait = lock.lock().unwrap();
                    while *wait == PumpEventNotifierAction::Pause {
                        wait = cvar.wait(wait).unwrap();
                    }
                    // Wake-up the main loop and put this one back to sleep.
                    *wait = PumpEventNotifierAction::Pause;
                    drop(wait);

                    while let Some(read_guard) = connection.prepare_read() {
                        let _ = connection.flush();
                        let poll_fd = PollFd::from_borrowed_fd(connection.as_fd(), PollFlags::IN);
                        let pipe_poll_fd = PollFd::from_borrowed_fd(read.as_fd(), PollFlags::IN);
                        // Read from the `fd` before going back to poll.
                        if Ok(1) == rustix::io::read(read.as_fd(), &mut [0u8; 1]) {
                            break 'outer;
                        }
                        let _ = rustix::event::poll(&mut [poll_fd, pipe_poll_fd], -1);
                        // Non-blocking read the connection.
                        let _ = read_guard.read_without_dispatch();
                    }

                    awakener.ping();
                }
            });

        if let Some(err) = handle.as_ref().err() {
            warn!("failed to spawn pump_events wake-up thread: {err}");
        }

        PumpEventNotifier { control, handle: handle.ok(), worker_waker: Some(write) }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum PumpEventNotifierAction {
    /// Monitor the wayland queue.
    Monitor,
    /// Pause monitoring.
    Pause,
}
