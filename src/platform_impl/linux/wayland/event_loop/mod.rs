//! The event-loop routines.

use std::cell::RefCell;
use std::error::Error;
use std::io::Result as IOResult;
use std::marker::PhantomData;
use std::mem;
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use raw_window_handle::{RawDisplayHandle, WaylandDisplayHandle};

use sctk::reexports::calloop;
use sctk::reexports::client::globals;
use sctk::reexports::client::{Connection, Proxy, QueueHandle, WaylandSource};

use crate::dpi::{LogicalSize, PhysicalSize};
use crate::error::{OsError as RootOsError, RunLoopError};
use crate::event::{Event, StartCause, WindowEvent};
use crate::event_loop::{ControlFlow, EventLoopWindowTarget as RootEventLoopWindowTarget};
use crate::platform::pump_events::PumpStatus;
use crate::platform_impl::platform::min_timeout;
use crate::platform_impl::platform::sticky_exit_callback;
use crate::platform_impl::{EventLoopWindowTarget as PlatformEventLoopWindowTarget, OsError};

mod proxy;
pub mod sink;

pub use proxy::EventLoopProxy;
use sink::EventSink;

use super::state::{WindowCompositorUpdate, WinitState};
use super::{DeviceId, WindowId};

type WaylandDispatcher = calloop::Dispatcher<'static, WaylandSource<WinitState>, WinitState>;

/// The Wayland event loop.
pub struct EventLoop<T: 'static> {
    /// Has `run` or `run_ondemand` been called or a call to `pump_events` that starts the loop
    loop_running: bool,

    /// The application's latest control_flow state
    control_flow: ControlFlow,

    buffer_sink: EventSink,
    compositor_updates: Vec<WindowCompositorUpdate>,
    window_ids: Vec<WindowId>,

    /// Sender of user events.
    user_events_sender: calloop::channel::Sender<T>,

    // XXX can't remove RefCell out of here, unless we can plumb generics into the `Window`, which
    // we don't really want, since it'll break public API by a lot.
    /// Pending events from the user.
    pending_user_events: Rc<RefCell<Vec<T>>>,

    /// The Wayland dispatcher to has raw access to the queue when needed, such as
    /// when creating a new window.
    wayland_dispatcher: WaylandDispatcher,

    /// Connection to the wayland server.
    connection: Connection,

    /// Event loop window target.
    window_target: RootEventLoopWindowTarget<T>,

    // XXX drop after everything else, just to be safe.
    /// Calloop's event loop.
    event_loop: calloop::EventLoop<'static, WinitState>,
}

impl<T: 'static> EventLoop<T> {
    pub fn new() -> Result<EventLoop<T>, Box<dyn Error>> {
        let connection = Connection::connect_to_env()?;

        let (globals, mut event_queue) = globals::registry_queue_init(&connection)?;
        let queue_handle = event_queue.handle();

        let event_loop = calloop::EventLoop::<WinitState>::try_new()?;

        let mut winit_state = WinitState::new(&globals, &queue_handle, event_loop.handle())?;

        // NOTE: do a roundtrip after binding the globals to prevent potential
        // races with the server.
        event_queue.roundtrip(&mut winit_state)?;

        // Register Wayland source.
        let wayland_source = WaylandSource::new(event_queue)?;
        let wayland_dispatcher =
            calloop::Dispatcher::new(wayland_source, |_, queue, winit_state| {
                queue.dispatch_pending(winit_state)
            });

        event_loop
            .handle()
            .register_dispatcher(wayland_dispatcher.clone())?;

        // Setup the user proxy.
        let pending_user_events = Rc::new(RefCell::new(Vec::new()));
        let pending_user_events_clone = pending_user_events.clone();
        let (user_events_sender, user_events_channel) = calloop::channel::channel();
        event_loop
            .handle()
            .insert_source(user_events_channel, move |event, _, _| {
                if let calloop::channel::Event::Msg(msg) = event {
                    pending_user_events_clone.borrow_mut().push(msg);
                }
            })?;

        // An event's loop awakener to wake up for window events from winit's windows.
        let (event_loop_awakener, event_loop_awakener_source) = calloop::ping::make_ping()?;
        event_loop
            .handle()
            .insert_source(event_loop_awakener_source, move |_, _, _| {
                // No extra handling is required, we just need to wake-up.
            })?;

        let window_target = EventLoopWindowTarget {
            connection: connection.clone(),
            wayland_dispatcher: wayland_dispatcher.clone(),
            event_loop_awakener,
            queue_handle,
            state: RefCell::new(winit_state),
            _marker: PhantomData,
        };

        let event_loop = Self {
            loop_running: false,
            control_flow: ControlFlow::default(),
            compositor_updates: Vec::new(),
            buffer_sink: EventSink::default(),
            window_ids: Vec::new(),
            connection,
            wayland_dispatcher,
            user_events_sender,
            pending_user_events,
            event_loop,
            window_target: RootEventLoopWindowTarget {
                p: PlatformEventLoopWindowTarget::Wayland(window_target),
                _marker: PhantomData,
            },
        };

        Ok(event_loop)
    }

    pub fn run_ondemand<F>(&mut self, mut event_handler: F) -> Result<(), RunLoopError>
    where
        F: FnMut(Event<'_, T>, &RootEventLoopWindowTarget<T>, &mut ControlFlow),
    {
        if self.loop_running {
            return Err(RunLoopError::AlreadyRunning);
        }

        let exit = loop {
            match self.pump_events(None, &mut event_handler) {
                PumpStatus::Exit(0) => {
                    break Ok(());
                }
                PumpStatus::Exit(code) => {
                    break Err(RunLoopError::ExitFailure(code));
                }
                _ => {
                    continue;
                }
            }
        };

        // Applications aren't allowed to carry windows between separate
        // `run_ondemand` calls but if they have only just dropped their
        // windows we need to make sure those last requests are sent to the
        // compositor.
        let _ = self.roundtrip().map_err(RunLoopError::Os);

        exit
    }

    pub fn pump_events<F>(&mut self, timeout: Option<Duration>, mut callback: F) -> PumpStatus
    where
        F: FnMut(Event<'_, T>, &RootEventLoopWindowTarget<T>, &mut ControlFlow),
    {
        if !self.loop_running {
            self.loop_running = true;

            // Reset the internal state for the loop as we start running to
            // ensure consistent behaviour in case the loop runs and exits more
            // than once.
            self.control_flow = ControlFlow::Poll;

            // Run the initial loop iteration.
            self.single_iteration(&mut callback, StartCause::Init);
        }

        // Consider the possibility that the `StartCause::Init` iteration could
        // request to Exit.
        if !matches!(self.control_flow, ControlFlow::ExitWithCode(_)) {
            self.poll_events_with_timeout(timeout, &mut callback);
        }
        if let ControlFlow::ExitWithCode(code) = self.control_flow {
            self.loop_running = false;

            let mut dummy = self.control_flow;
            sticky_exit_callback(
                Event::LoopExiting,
                self.window_target(),
                &mut dummy,
                &mut callback,
            );

            PumpStatus::Exit(code)
        } else {
            PumpStatus::Continue
        }
    }

    pub fn poll_events_with_timeout<F>(&mut self, mut timeout: Option<Duration>, mut callback: F)
    where
        F: FnMut(Event<'_, T>, &RootEventLoopWindowTarget<T>, &mut ControlFlow),
    {
        let start = Instant::now();

        // TODO(rib): remove this workaround and instead make sure that the calloop
        // WaylandSource correctly implements the cooperative prepare_read protocol
        // that support multithreaded wayland clients that may all read from the
        // same socket.
        //
        // During the run of the user callback, some other code monitoring and reading the
        // Wayland socket may have been run (mesa for example does this with vsync), if that
        // is the case, some events may have been enqueued in our event queue.
        //
        // If some messages are there, the event loop needs to behave as if it was instantly
        // woken up by messages arriving from the Wayland socket, to avoid delaying the
        // dispatch of these events until we're woken up again.
        let instant_wakeup = {
            let mut wayland_source = self.wayland_dispatcher.as_source_mut();
            let queue = wayland_source.queue();
            let state = match &mut self.window_target.p {
                PlatformEventLoopWindowTarget::Wayland(window_target) => {
                    window_target.state.get_mut()
                }
                #[cfg(x11_platform)]
                _ => unreachable!(),
            };

            match queue.dispatch_pending(state) {
                Ok(dispatched) => dispatched > 0,
                Err(error) => {
                    error!("Error dispatching wayland queue: {}", error);
                    self.control_flow = ControlFlow::ExitWithCode(1);
                    return;
                }
            }
        };

        timeout = if instant_wakeup {
            Some(Duration::ZERO)
        } else {
            let control_flow_timeout = match self.control_flow {
                ControlFlow::Wait => None,
                ControlFlow::Poll => Some(Duration::ZERO),
                ControlFlow::WaitUntil(wait_deadline) => {
                    Some(wait_deadline.saturating_duration_since(start))
                }
                // This function shouldn't have to handle any requests to exit
                // the application (there should be no need to poll for events
                // if the application has requested to exit) so we consider
                // it a bug in the backend if we ever see `ExitWithCode` here.
                ControlFlow::ExitWithCode(_code) => unreachable!(),
            };
            min_timeout(control_flow_timeout, timeout)
        };

        // NOTE Ideally we should flush as the last thing we do before polling
        // to wait for events, and this should be done by the calloop
        // WaylandSource but we currently need to flush writes manually.
        let _ = self.connection.flush();

        if let Err(error) = self.loop_dispatch(timeout) {
            // NOTE We exit on errors from dispatches, since if we've got protocol error
            // libwayland-client/wayland-rs will inform us anyway, but crashing downstream is not
            // really an option. Instead we inform that the event loop got destroyed. We may
            // communicate an error that something was terminated, but winit doesn't provide us
            // with an API to do that via some event.
            // Still, we set the exit code to the error's OS error code, or to 1 if not possible.
            let exit_code = error.raw_os_error().unwrap_or(1);
            self.control_flow = ControlFlow::ExitWithCode(exit_code);
            return;
        }

        // NB: `StartCause::Init` is handled as a special case and doesn't need
        // to be considered here
        let cause = match self.control_flow {
            ControlFlow::Poll => StartCause::Poll,
            ControlFlow::Wait => StartCause::WaitCancelled {
                start,
                requested_resume: None,
            },
            ControlFlow::WaitUntil(deadline) => {
                if Instant::now() < deadline {
                    StartCause::WaitCancelled {
                        start,
                        requested_resume: Some(deadline),
                    }
                } else {
                    StartCause::ResumeTimeReached {
                        start,
                        requested_resume: deadline,
                    }
                }
            }
            // This function shouldn't have to handle any requests to exit
            // the application (there should be no need to poll for events
            // if the application has requested to exit) so we consider
            // it a bug in the backend if we ever see `ExitWithCode` here.
            ControlFlow::ExitWithCode(_code) => unreachable!(),
        };

        self.single_iteration(&mut callback, cause);
    }

    fn single_iteration<F>(&mut self, mut callback: &mut F, cause: StartCause)
    where
        F: FnMut(Event<'_, T>, &RootEventLoopWindowTarget<T>, &mut ControlFlow),
    {
        // NOTE currently just indented to simplify the diff

        let mut control_flow = self.control_flow;

        // We retain these grow-only scratch buffers as part of the EventLoop
        // for the sake of avoiding lots of reallocs. We take them here to avoid
        // trying to mutably borrow `self` more than once and we swap them back
        // when finished.
        let mut compositor_updates = std::mem::take(&mut self.compositor_updates);
        let mut buffer_sink = std::mem::take(&mut self.buffer_sink);
        let mut window_ids = std::mem::take(&mut self.window_ids);

        sticky_exit_callback(
            Event::NewEvents(cause),
            &self.window_target,
            &mut control_flow,
            callback,
        );

        // NB: For consistency all platforms must emit a 'resumed' event even though Wayland
        // applications don't themselves have a formal suspend/resume lifecycle.
        if cause == StartCause::Init {
            sticky_exit_callback(
                Event::Resumed,
                &self.window_target,
                &mut control_flow,
                callback,
            );
        }

        // Handle pending user events. We don't need back buffer, since we can't dispatch
        // user events indirectly via callback to the user.
        for user_event in self.pending_user_events.borrow_mut().drain(..) {
            sticky_exit_callback(
                Event::UserEvent(user_event),
                &self.window_target,
                &mut control_flow,
                &mut callback,
            );
        }

        // Drain the pending compositor updates.
        self.with_state(|state| compositor_updates.append(&mut state.window_compositor_updates));

        for mut compositor_update in compositor_updates.drain(..) {
            let window_id = compositor_update.window_id;
            if let Some(scale_factor) = compositor_update.scale_factor {
                let mut physical_size = self.with_state(|state| {
                    let windows = state.windows.get_mut();
                    let mut window = windows.get(&window_id).unwrap().lock().unwrap();

                    // Set the new scale factor.
                    window.set_scale_factor(scale_factor);
                    let window_size = compositor_update.size.unwrap_or(window.inner_size());
                    logical_to_physical_rounded(window_size, scale_factor)
                });

                // Stash the old window size.
                let old_physical_size = physical_size;

                sticky_exit_callback(
                    Event::WindowEvent {
                        window_id: crate::window::WindowId(window_id),
                        event: WindowEvent::ScaleFactorChanged {
                            scale_factor,
                            new_inner_size: &mut physical_size,
                        },
                    },
                    &self.window_target,
                    &mut control_flow,
                    &mut callback,
                );

                let new_logical_size = physical_size.to_logical(scale_factor);

                // Resize the window when user altered the size.
                if old_physical_size != physical_size {
                    self.with_state(|state| {
                        let windows = state.windows.get_mut();
                        let mut window = windows.get(&window_id).unwrap().lock().unwrap();
                        window.resize(new_logical_size);
                    });
                }

                // Make it queue resize.
                compositor_update.size = Some(new_logical_size);
            }

            if let Some(size) = compositor_update.size.take() {
                let physical_size = self.with_state(|state| {
                    let windows = state.windows.get_mut();
                    let window = windows.get(&window_id).unwrap().lock().unwrap();

                    let scale_factor = window.scale_factor();
                    let physical_size = logical_to_physical_rounded(size, scale_factor);

                    // TODO could probably bring back size reporting optimization.

                    // Mark the window as needed a redraw.
                    state
                        .window_requests
                        .get_mut()
                        .get_mut(&window_id)
                        .unwrap()
                        .redraw_requested
                        .store(true, Ordering::Relaxed);

                    physical_size
                });

                sticky_exit_callback(
                    Event::WindowEvent {
                        window_id: crate::window::WindowId(window_id),
                        event: WindowEvent::Resized(physical_size),
                    },
                    &self.window_target,
                    &mut control_flow,
                    &mut callback,
                );
            }

            if compositor_update.close_window {
                sticky_exit_callback(
                    Event::WindowEvent {
                        window_id: crate::window::WindowId(window_id),
                        event: WindowEvent::CloseRequested,
                    },
                    &self.window_target,
                    &mut control_flow,
                    &mut callback,
                );
            }
        }

        // Push the events directly from the window.
        self.with_state(|state| {
            buffer_sink.append(&mut state.window_events_sink.lock().unwrap());
        });
        for event in buffer_sink.drain() {
            let event = event.map_nonuser_event().unwrap();
            sticky_exit_callback(event, &self.window_target, &mut control_flow, &mut callback);
        }

        // Handle non-synthetic events.
        self.with_state(|state| {
            buffer_sink.append(&mut state.events_sink);
        });
        for event in buffer_sink.drain() {
            let event = event.map_nonuser_event().unwrap();
            sticky_exit_callback(event, &self.window_target, &mut control_flow, &mut callback);
        }

        // Send events cleared.
        sticky_exit_callback(
            Event::MainEventsCleared,
            &self.window_target,
            &mut control_flow,
            &mut callback,
        );

        // Collect the window ids
        self.with_state(|state| {
            window_ids.extend(state.window_requests.get_mut().keys());
        });

        for window_id in window_ids.drain(..) {
            let request_redraw = self.with_state(|state| {
                let window_requests = state.window_requests.get_mut();
                if window_requests.get(&window_id).unwrap().take_closed() {
                    mem::drop(window_requests.remove(&window_id));
                    mem::drop(state.windows.get_mut().remove(&window_id));
                    false
                } else {
                    let mut redraw_requested = window_requests
                        .get(&window_id)
                        .unwrap()
                        .take_redraw_requested();

                    // Redraw the frames while at it.
                    redraw_requested |= state
                        .windows
                        .get_mut()
                        .get_mut(&window_id)
                        .unwrap()
                        .lock()
                        .unwrap()
                        .refresh_frame();

                    redraw_requested
                }
            });

            if request_redraw {
                sticky_exit_callback(
                    Event::RedrawRequested(crate::window::WindowId(window_id)),
                    &self.window_target,
                    &mut control_flow,
                    &mut callback,
                );
            }
        }

        // Send RedrawEventCleared.
        sticky_exit_callback(
            Event::RedrawEventsCleared,
            &self.window_target,
            &mut control_flow,
            &mut callback,
        );

        self.control_flow = control_flow;
        std::mem::swap(&mut self.compositor_updates, &mut compositor_updates);
        std::mem::swap(&mut self.buffer_sink, &mut buffer_sink);
        std::mem::swap(&mut self.window_ids, &mut window_ids);
    }

    #[inline]
    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy::new(self.user_events_sender.clone())
    }

    #[inline]
    pub fn window_target(&self) -> &RootEventLoopWindowTarget<T> {
        &self.window_target
    }

    fn with_state<'a, U: 'a, F: FnOnce(&'a mut WinitState) -> U>(&'a mut self, callback: F) -> U {
        let state = match &mut self.window_target.p {
            PlatformEventLoopWindowTarget::Wayland(window_target) => window_target.state.get_mut(),
            #[cfg(x11_platform)]
            _ => unreachable!(),
        };

        callback(state)
    }

    fn loop_dispatch<D: Into<Option<std::time::Duration>>>(&mut self, timeout: D) -> IOResult<()> {
        let state = match &mut self.window_target.p {
            PlatformEventLoopWindowTarget::Wayland(window_target) => window_target.state.get_mut(),
            #[cfg(feature = "x11")]
            _ => unreachable!(),
        };

        self.event_loop.dispatch(timeout, state).map_err(|error| {
            error!("Error dispatching event loop: {}", error);
            error.into()
        })
    }

    fn roundtrip(&mut self) -> Result<usize, RootOsError> {
        let state = match &mut self.window_target.p {
            PlatformEventLoopWindowTarget::Wayland(window_target) => window_target.state.get_mut(),
            #[cfg(feature = "x11")]
            _ => unreachable!(),
        };

        let mut wayland_source = self.wayland_dispatcher.as_source_mut();
        let event_queue = wayland_source.queue();
        event_queue.roundtrip(state).map_err(|_| {
            os_error!(OsError::WaylandMisc(
                "failed to do a final roundtrip before exiting the loop."
            ))
        })
    }
}

pub struct EventLoopWindowTarget<T> {
    /// The event loop wakeup source.
    pub event_loop_awakener: calloop::ping::Ping,

    /// The main queue used by the event loop.
    pub queue_handle: QueueHandle<WinitState>,

    // TODO remove that RefCell once we can pass `&mut` in `Window::new`.
    /// Winit state.
    pub state: RefCell<WinitState>,

    /// Dispatcher of Wayland events.
    pub wayland_dispatcher: WaylandDispatcher,

    /// Connection to the wayland server.
    pub connection: Connection,

    _marker: std::marker::PhantomData<T>,
}

impl<T> EventLoopWindowTarget<T> {
    pub fn raw_display_handle(&self) -> RawDisplayHandle {
        let mut display_handle = WaylandDisplayHandle::empty();
        display_handle.display = self.connection.display().id().as_ptr() as *mut _;
        RawDisplayHandle::Wayland(display_handle)
    }
}

// The default routine does floor, but we need round on Wayland.
fn logical_to_physical_rounded(size: LogicalSize<u32>, scale_factor: f64) -> PhysicalSize<u32> {
    let width = size.width as f64 * scale_factor;
    let height = size.height as f64 * scale_factor;
    (width.round(), height.round()).into()
}
