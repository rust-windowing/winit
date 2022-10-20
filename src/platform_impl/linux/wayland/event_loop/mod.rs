use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::io::Result as IOResult;
use std::mem;
use std::process;
use std::rc::Rc;
use std::time::{Duration, Instant};

use raw_window_handle::{RawDisplayHandle, WaylandDisplayHandle};

use sctk::reexports::client::protocol::wl_compositor::WlCompositor;
use sctk::reexports::client::protocol::wl_shm::WlShm;
use sctk::reexports::client::Display;

use sctk::reexports::calloop;

use sctk::environment::Environment;
use sctk::seat::pointer::{ThemeManager, ThemeSpec};
use sctk::WaylandSource;

use crate::event::{Event, StartCause, WindowEvent};
use crate::event_loop::{ControlFlow, EventLoopWindowTarget as RootEventLoopWindowTarget};
use crate::platform_impl::platform::sticky_exit_callback;
use crate::platform_impl::EventLoopWindowTarget as PlatformEventLoopWindowTarget;

use super::env::{WindowingFeatures, WinitEnv};
use super::output::OutputManager;
use super::seat::SeatManager;
use super::window::shim::{self, WindowCompositorUpdate, WindowUserRequest};
use super::{DeviceId, WindowId};

mod proxy;
mod sink;
mod state;

pub use proxy::EventLoopProxy;
pub use sink::EventSink;
pub use state::WinitState;

type WinitDispatcher = calloop::Dispatcher<'static, WaylandSource, WinitState>;

pub struct EventLoopWindowTarget<T> {
    /// Wayland display.
    pub display: Display,

    /// Environment to handle object creation, etc.
    pub env: Environment<WinitEnv>,

    /// Event loop handle.
    pub event_loop_handle: calloop::LoopHandle<'static, WinitState>,

    /// Output manager.
    pub output_manager: OutputManager,

    /// State that we share across callbacks.
    pub state: RefCell<WinitState>,

    /// Dispatcher of Wayland events.
    pub wayland_dispatcher: WinitDispatcher,

    /// A proxy to wake up event loop.
    pub event_loop_awakener: calloop::ping::Ping,

    /// The available windowing features.
    pub windowing_features: WindowingFeatures,

    /// Theme manager to manage cursors.
    ///
    /// It's being shared between all windows to avoid loading
    /// multiple similar themes.
    pub theme_manager: ThemeManager,

    _marker: std::marker::PhantomData<T>,
}

impl<T> EventLoopWindowTarget<T> {
    pub fn raw_display_handle(&self) -> RawDisplayHandle {
        let mut display_handle = WaylandDisplayHandle::empty();
        display_handle.display = self.display.get_display_ptr() as *mut _;
        RawDisplayHandle::Wayland(display_handle)
    }
}

pub struct EventLoop<T: 'static> {
    /// Dispatcher of Wayland events.
    pub wayland_dispatcher: WinitDispatcher,

    /// Event loop.
    event_loop: calloop::EventLoop<'static, WinitState>,

    /// Wayland display.
    display: Display,

    /// Pending user events.
    pending_user_events: Rc<RefCell<Vec<T>>>,

    /// Sender of user events.
    user_events_sender: calloop::channel::Sender<T>,

    /// Window target.
    window_target: RootEventLoopWindowTarget<T>,

    /// Output manager.
    _seat_manager: SeatManager,
}

impl<T: 'static> EventLoop<T> {
    pub fn new() -> Result<EventLoop<T>, Box<dyn Error>> {
        // Connect to wayland server and setup event queue.
        let display = Display::connect_to_env()?;
        let mut event_queue = display.create_event_queue();
        let display_proxy = display.attach(event_queue.token());

        // Setup environment.
        let env = Environment::new(&display_proxy, &mut event_queue, WinitEnv::new())?;

        // Create event loop.
        let event_loop = calloop::EventLoop::<'static, WinitState>::try_new()?;
        // Build windowing features.
        let windowing_features = WindowingFeatures::new(&env);

        // Create a theme manager.
        let compositor = env.require_global::<WlCompositor>();
        let shm = env.require_global::<WlShm>();
        let theme_manager = ThemeManager::init(ThemeSpec::System, compositor, shm);

        // Setup theme seat and output managers.
        let seat_manager = SeatManager::new(&env, event_loop.handle(), theme_manager.clone());
        let output_manager = OutputManager::new(&env);

        // A source of events that we plug into our event loop.
        let wayland_source = WaylandSource::new(event_queue);
        let wayland_dispatcher =
            calloop::Dispatcher::new(wayland_source, |_, queue, winit_state| {
                queue.dispatch_pending(winit_state, |event, object, _| {
                    panic!(
                        "[calloop] Encountered an orphan event: {}@{} : {}",
                        event.interface,
                        object.as_ref().id(),
                        event.name
                    );
                })
            });

        let _wayland_source_dispatcher = event_loop
            .handle()
            .register_dispatcher(wayland_dispatcher.clone())?;

        // A source of user events.
        let pending_user_events = Rc::new(RefCell::new(Vec::new()));
        let pending_user_events_clone = pending_user_events.clone();
        let (user_events_sender, user_events_channel) = calloop::channel::channel();

        // User events channel.
        event_loop
            .handle()
            .insert_source(user_events_channel, move |event, _, _| {
                if let calloop::channel::Event::Msg(msg) = event {
                    pending_user_events_clone.borrow_mut().push(msg);
                }
            })?;

        // An event's loop awakener to wake up for window events from winit's windows.
        let (event_loop_awakener, event_loop_awakener_source) = calloop::ping::make_ping()?;

        // Handler of window requests.
        event_loop
            .handle()
            .insert_source(event_loop_awakener_source, move |_, _, state| {
                // Drain events here as well to account for application doing batch event processing
                // on RedrawEventsCleared.
                shim::handle_window_requests(state);
            })?;

        let event_loop_handle = event_loop.handle();
        let window_map = HashMap::new();
        let event_sink = EventSink::new();
        let window_user_requests = HashMap::new();
        let window_compositor_updates = HashMap::new();

        // Create event loop window target.
        let event_loop_window_target = EventLoopWindowTarget {
            display: display.clone(),
            env,
            state: RefCell::new(WinitState {
                window_map,
                event_sink,
                window_user_requests,
                window_compositor_updates,
            }),
            event_loop_handle,
            output_manager,
            event_loop_awakener,
            wayland_dispatcher: wayland_dispatcher.clone(),
            windowing_features,
            theme_manager,
            _marker: std::marker::PhantomData,
        };

        // Create event loop itself.
        let event_loop = Self {
            event_loop,
            display,
            pending_user_events,
            wayland_dispatcher,
            _seat_manager: seat_manager,
            user_events_sender,
            window_target: RootEventLoopWindowTarget {
                p: PlatformEventLoopWindowTarget::Wayland(event_loop_window_target),
                _marker: std::marker::PhantomData,
            },
        };

        Ok(event_loop)
    }

    pub fn run<F>(mut self, callback: F) -> !
    where
        F: FnMut(Event<'_, T>, &RootEventLoopWindowTarget<T>, &mut ControlFlow) + 'static,
    {
        let exit_code = self.run_return(callback);
        process::exit(exit_code);
    }

    pub fn run_return<F>(&mut self, mut callback: F) -> i32
    where
        F: FnMut(Event<'_, T>, &RootEventLoopWindowTarget<T>, &mut ControlFlow),
    {
        let mut control_flow = ControlFlow::Poll;
        let pending_user_events = self.pending_user_events.clone();

        callback(
            Event::NewEvents(StartCause::Init),
            &self.window_target,
            &mut control_flow,
        );

        // NB: For consistency all platforms must emit a 'resumed' event even though Wayland
        // applications don't themselves have a formal suspend/resume lifecycle.
        callback(Event::Resumed, &self.window_target, &mut control_flow);

        let mut window_compositor_updates: Vec<(WindowId, WindowCompositorUpdate)> = Vec::new();
        let mut window_user_requests: Vec<(WindowId, WindowUserRequest)> = Vec::new();
        let mut event_sink_back_buffer = Vec::new();

        // NOTE We break on errors from dispatches, since if we've got protocol error
        // libwayland-client/wayland-rs will inform us anyway, but crashing downstream is not
        // really an option. Instead we inform that the event loop got destroyed. We may
        // communicate an error that something was terminated, but winit doesn't provide us
        // with an API to do that via some event.
        // Still, we set the exit code to the error's OS error code, or to 1 if not possible.
        let exit_code = loop {
            // Send pending events to the server.
            let _ = self.display.flush();

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
                    #[cfg(feature = "x11")]
                    _ => unreachable!(),
                };

                match queue.dispatch_pending(state, |_, _, _| unimplemented!()) {
                    Ok(dispatched) => dispatched > 0,
                    Err(error) => break error.raw_os_error().unwrap_or(1),
                }
            };

            match control_flow {
                ControlFlow::ExitWithCode(code) => break code,
                ControlFlow::Poll => {
                    // Non-blocking dispatch.
                    let timeout = Duration::from_millis(0);
                    if let Err(error) = self.loop_dispatch(Some(timeout)) {
                        break error.raw_os_error().unwrap_or(1);
                    }

                    callback(
                        Event::NewEvents(StartCause::Poll),
                        &self.window_target,
                        &mut control_flow,
                    );
                }
                ControlFlow::Wait => {
                    let timeout = if instant_wakeup {
                        Some(Duration::from_millis(0))
                    } else {
                        None
                    };

                    if let Err(error) = self.loop_dispatch(timeout) {
                        break error.raw_os_error().unwrap_or(1);
                    }

                    callback(
                        Event::NewEvents(StartCause::WaitCancelled {
                            start: Instant::now(),
                            requested_resume: None,
                        }),
                        &self.window_target,
                        &mut control_flow,
                    );
                }
                ControlFlow::WaitUntil(deadline) => {
                    let start = Instant::now();

                    // Compute the amount of time we'll block for.
                    let duration = if deadline > start && !instant_wakeup {
                        deadline - start
                    } else {
                        Duration::from_millis(0)
                    };

                    if let Err(error) = self.loop_dispatch(Some(duration)) {
                        break error.raw_os_error().unwrap_or(1);
                    }

                    let now = Instant::now();

                    if now < deadline {
                        callback(
                            Event::NewEvents(StartCause::WaitCancelled {
                                start,
                                requested_resume: Some(deadline),
                            }),
                            &self.window_target,
                            &mut control_flow,
                        )
                    } else {
                        callback(
                            Event::NewEvents(StartCause::ResumeTimeReached {
                                start,
                                requested_resume: deadline,
                            }),
                            &self.window_target,
                            &mut control_flow,
                        )
                    }
                }
            }

            // Handle pending user events. We don't need back buffer, since we can't dispatch
            // user events indirectly via callback to the user.
            for user_event in pending_user_events.borrow_mut().drain(..) {
                sticky_exit_callback(
                    Event::UserEvent(user_event),
                    &self.window_target,
                    &mut control_flow,
                    &mut callback,
                );
            }

            // Process 'new' pending updates from compositor.
            self.with_state(|state| {
                window_compositor_updates.clear();
                window_compositor_updates.extend(
                    state
                        .window_compositor_updates
                        .iter_mut()
                        .map(|(wid, window_update)| (*wid, mem::take(window_update))),
                );
            });

            for (window_id, window_compositor_update) in window_compositor_updates.iter_mut() {
                if let Some(scale_factor) = window_compositor_update.scale_factor.map(|f| f as f64)
                {
                    let mut physical_size = self.with_state(|state| {
                        let window_handle = state.window_map.get(window_id).unwrap();
                        let mut size = window_handle.size.lock().unwrap();

                        // Update the new logical size if it was changed.
                        let window_size = window_compositor_update.size.unwrap_or(*size);
                        *size = window_size;

                        window_size.to_physical(scale_factor)
                    });

                    sticky_exit_callback(
                        Event::WindowEvent {
                            window_id: crate::window::WindowId(*window_id),
                            event: WindowEvent::ScaleFactorChanged {
                                scale_factor,
                                new_inner_size: &mut physical_size,
                            },
                        },
                        &self.window_target,
                        &mut control_flow,
                        &mut callback,
                    );

                    // We don't update size on a window handle since we'll do that later
                    // when handling size update.
                    let new_logical_size = physical_size.to_logical(scale_factor);
                    window_compositor_update.size = Some(new_logical_size);
                }

                if let Some(size) = window_compositor_update.size.take() {
                    let physical_size = self.with_state(|state| {
                        let window_handle = state.window_map.get_mut(window_id).unwrap();
                        let mut window_size = window_handle.size.lock().unwrap();

                        // Always issue resize event on scale factor change.
                        let physical_size = if window_compositor_update.scale_factor.is_none()
                            && *window_size == size
                        {
                            // The size hasn't changed, don't inform downstream about that.
                            None
                        } else {
                            *window_size = size;
                            let scale_factor =
                                sctk::get_surface_scale_factor(window_handle.window.surface());
                            let physical_size = size.to_physical(scale_factor as f64);
                            Some(physical_size)
                        };

                        // We still perform all of those resize related logic even if the size
                        // hasn't changed, since GNOME relies on `set_geometry` calls after
                        // configures.
                        window_handle.window.resize(size.width, size.height);
                        window_handle.window.refresh();

                        // Mark that refresh isn't required, since we've done it right now.
                        state
                            .window_user_requests
                            .get_mut(window_id)
                            .unwrap()
                            .refresh_frame = false;

                        physical_size
                    });

                    if let Some(physical_size) = physical_size {
                        sticky_exit_callback(
                            Event::WindowEvent {
                                window_id: crate::window::WindowId(*window_id),
                                event: WindowEvent::Resized(physical_size),
                            },
                            &self.window_target,
                            &mut control_flow,
                            &mut callback,
                        );
                    }
                }

                // If the close is requested, send it here.
                if window_compositor_update.close_window {
                    sticky_exit_callback(
                        Event::WindowEvent {
                            window_id: crate::window::WindowId(*window_id),
                            event: WindowEvent::CloseRequested,
                        },
                        &self.window_target,
                        &mut control_flow,
                        &mut callback,
                    );
                }
            }

            // The purpose of the back buffer and that swap is to not hold borrow_mut when
            // we're doing callback to the user, since we can double borrow if the user decides
            // to create a window in one of those callbacks.
            self.with_state(|state| {
                std::mem::swap(
                    &mut event_sink_back_buffer,
                    &mut state.event_sink.window_events,
                )
            });

            // Handle pending window events.
            for event in event_sink_back_buffer.drain(..) {
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

            // Apply user requests, so every event required resize and latter surface commit will
            // be applied right before drawing. This will also ensure that every `RedrawRequested`
            // event will be delivered in time.
            self.with_state(|state| {
                shim::handle_window_requests(state);
            });

            // Process 'new' pending updates from compositor.
            self.with_state(|state| {
                window_user_requests.clear();
                window_user_requests.extend(
                    state
                        .window_user_requests
                        .iter_mut()
                        .map(|(wid, window_request)| (*wid, mem::take(window_request))),
                );
            });

            // Handle RedrawRequested events.
            for (window_id, mut window_request) in window_user_requests.iter() {
                // Handle refresh of the frame.
                if window_request.refresh_frame {
                    self.with_state(|state| {
                        let window_handle = state.window_map.get_mut(window_id).unwrap();
                        window_handle.window.refresh();
                    });

                    // In general refreshing the frame requires surface commit, those force user
                    // to redraw.
                    window_request.redraw_requested = true;
                }

                // Handle redraw request.
                if window_request.redraw_requested {
                    sticky_exit_callback(
                        Event::RedrawRequested(crate::window::WindowId(*window_id)),
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
        };

        callback(Event::LoopDestroyed, &self.window_target, &mut control_flow);
        exit_code
    }

    #[inline]
    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy::new(self.user_events_sender.clone())
    }

    #[inline]
    pub fn window_target(&self) -> &RootEventLoopWindowTarget<T> {
        &self.window_target
    }

    fn with_state<U, F: FnOnce(&mut WinitState) -> U>(&mut self, f: F) -> U {
        let state = match &mut self.window_target.p {
            PlatformEventLoopWindowTarget::Wayland(window_target) => window_target.state.get_mut(),
            #[cfg(feature = "x11")]
            _ => unreachable!(),
        };

        f(state)
    }

    fn loop_dispatch<D: Into<Option<std::time::Duration>>>(&mut self, timeout: D) -> IOResult<()> {
        let state = match &mut self.window_target.p {
            PlatformEventLoopWindowTarget::Wayland(window_target) => window_target.state.get_mut(),
            #[cfg(feature = "x11")]
            _ => unreachable!(),
        };

        self.event_loop
            .dispatch(timeout, state)
            .map_err(|error| error.into())
    }
}
