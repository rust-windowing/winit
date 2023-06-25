use super::super::DeviceId;
use super::{backend, state::State};
use crate::dpi::PhysicalSize;
use crate::event::{
    DeviceEvent, DeviceId as RootDeviceId, ElementState, Event, RawKeyEvent, StartCause,
};
use crate::event_loop::{ControlFlow, DeviceEvents};
use crate::platform_impl::platform::backend::EventListenerHandle;
use crate::window::WindowId;

use std::sync::atomic::Ordering;
use std::{
    cell::{Cell, RefCell},
    clone::Clone,
    collections::{HashSet, VecDeque},
    iter,
    ops::Deref,
    rc::{Rc, Weak},
};
use wasm_bindgen::prelude::Closure;
use web_sys::{KeyboardEvent, PageTransitionEvent, PointerEvent, WheelEvent};
use web_time::{Duration, Instant};

pub struct Shared<T: 'static>(Rc<Execution<T>>);

pub(super) type EventHandler<T> = dyn FnMut(Event<'_, T>, &mut ControlFlow);

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Shared(self.0.clone())
    }
}

type OnEventHandle<T> = RefCell<Option<EventListenerHandle<dyn FnMut(T)>>>;

pub struct Execution<T: 'static> {
    runner: RefCell<RunnerEnum<T>>,
    event_loop_recreation: Cell<bool>,
    events: RefCell<VecDeque<EventWrapper<T>>>,
    id: RefCell<u32>,
    window: web_sys::Window,
    all_canvases: RefCell<Vec<(WindowId, Weak<RefCell<backend::Canvas>>)>>,
    redraw_pending: RefCell<HashSet<WindowId>>,
    destroy_pending: RefCell<VecDeque<WindowId>>,
    page_transition_event_handle: RefCell<Option<backend::PageTransitionEventHandle>>,
    device_events: Cell<DeviceEvents>,
    on_mouse_move: OnEventHandle<PointerEvent>,
    on_wheel: OnEventHandle<WheelEvent>,
    on_mouse_press: OnEventHandle<PointerEvent>,
    on_mouse_release: OnEventHandle<PointerEvent>,
    on_key_press: OnEventHandle<KeyboardEvent>,
    on_key_release: OnEventHandle<KeyboardEvent>,
}

enum RunnerEnum<T: 'static> {
    /// The `EventLoop` is created but not being run.
    Pending,
    /// The `EventLoop` is being run.
    Running(Runner<T>),
    /// The `EventLoop` is exited after being started with `EventLoop::run`. Since
    /// `EventLoop::run` takes ownership of the `EventLoop`, we can be certain
    /// that this event loop will never be run again.
    Destroyed,
}

impl<T: 'static> RunnerEnum<T> {
    fn maybe_runner(&self) -> Option<&Runner<T>> {
        match self {
            RunnerEnum::Running(runner) => Some(runner),
            _ => None,
        }
    }
}

struct Runner<T: 'static> {
    state: State,
    event_handler: Box<EventHandler<T>>,
}

impl<T: 'static> Runner<T> {
    pub fn new(event_handler: Box<EventHandler<T>>) -> Self {
        Runner {
            state: State::Init,
            event_handler,
        }
    }

    /// Returns the corresponding `StartCause` for the current `state`, or `None`
    /// when in `Exit` state.
    fn maybe_start_cause(&self) -> Option<StartCause> {
        Some(match self.state {
            State::Init => StartCause::Init,
            State::Poll { .. } => StartCause::Poll,
            State::Wait { start } => StartCause::WaitCancelled {
                start,
                requested_resume: None,
            },
            State::WaitUntil { start, end, .. } => StartCause::WaitCancelled {
                start,
                requested_resume: Some(end),
            },
            State::Exit => return None,
        })
    }

    fn handle_single_event(
        &mut self,
        runner: &Shared<T>,
        event: impl Into<EventWrapper<T>>,
        control: &mut ControlFlow,
    ) {
        let is_closed = matches!(*control, ControlFlow::ExitWithCode(_));

        match event.into() {
            EventWrapper::Event(event) => (self.event_handler)(event, control),
            EventWrapper::ScaleChange {
                canvas,
                size,
                scale,
            } => {
                if let Some(canvas) = canvas.upgrade() {
                    canvas.borrow().handle_scale_change(
                        runner,
                        |event| (self.event_handler)(event, control),
                        size,
                        scale,
                    )
                }
            }
        }

        // Maintain closed state, even if the callback changes it
        if is_closed {
            *control = ControlFlow::Exit;
        }
    }
}

impl<T: 'static> Shared<T> {
    pub fn new() -> Self {
        Shared(Rc::new(Execution {
            runner: RefCell::new(RunnerEnum::Pending),
            event_loop_recreation: Cell::new(false),
            events: RefCell::new(VecDeque::new()),
            #[allow(clippy::disallowed_methods)]
            window: web_sys::window().expect("only callable from inside the `Window`"),
            id: RefCell::new(0),
            all_canvases: RefCell::new(Vec::new()),
            redraw_pending: RefCell::new(HashSet::new()),
            destroy_pending: RefCell::new(VecDeque::new()),
            page_transition_event_handle: RefCell::new(None),
            device_events: Cell::default(),
            on_mouse_move: RefCell::new(None),
            on_wheel: RefCell::new(None),
            on_mouse_press: RefCell::new(None),
            on_mouse_release: RefCell::new(None),
            on_key_press: RefCell::new(None),
            on_key_release: RefCell::new(None),
        }))
    }

    pub fn window(&self) -> &web_sys::Window {
        &self.0.window
    }

    pub fn add_canvas(&self, id: WindowId, canvas: &Rc<RefCell<backend::Canvas>>) {
        self.0
            .all_canvases
            .borrow_mut()
            .push((id, Rc::downgrade(canvas)));
    }

    pub fn notify_destroy_window(&self, id: WindowId) {
        self.0.destroy_pending.borrow_mut().push_back(id);
    }

    // Set the event callback to use for the event loop runner
    // This the event callback is a fairly thin layer over the user-provided callback that closes
    // over a RootEventLoopWindowTarget reference
    pub fn set_listener(&self, event_handler: Box<EventHandler<T>>) {
        {
            let mut runner = self.0.runner.borrow_mut();
            assert!(matches!(*runner, RunnerEnum::Pending));
            *runner = RunnerEnum::Running(Runner::new(event_handler));
        }
        self.init();

        *self.0.page_transition_event_handle.borrow_mut() = Some(backend::on_page_transition(
            self.window(),
            {
                let runner = self.clone();
                move |event: PageTransitionEvent| {
                    if event.persisted() {
                        runner.send_event(Event::Resumed);
                    }
                }
            },
            {
                let runner = self.clone();
                move |event: PageTransitionEvent| {
                    if event.persisted() {
                        runner.send_event(Event::Suspended);
                    } else {
                        runner.handle_unload();
                    }
                }
            },
        ));

        let runner = self.clone();
        let window = self.window().clone();
        *self.0.on_mouse_move.borrow_mut() = Some(EventListenerHandle::new(
            self.window(),
            "pointermove",
            Closure::new(move |event: PointerEvent| {
                if !runner.device_events() {
                    return;
                }

                let pointer_type = event.pointer_type();

                if pointer_type != "mouse" {
                    return;
                }

                // chorded button event
                let device_id = RootDeviceId(DeviceId(event.pointer_id()));

                if let Some(button) = backend::event::mouse_button(&event) {
                    debug_assert_eq!(
                        pointer_type, "mouse",
                        "expect pointer type of a chorded button event to be a mouse"
                    );

                    let state = if backend::event::mouse_buttons(&event).contains(button.into()) {
                        ElementState::Pressed
                    } else {
                        ElementState::Released
                    };

                    runner.send_event(Event::DeviceEvent {
                        device_id,
                        event: DeviceEvent::Button {
                            button: button.to_id(),
                            state,
                        },
                    });

                    return;
                }

                // pointer move event
                let mut delta = backend::event::MouseDelta::init(&window, &event);
                runner.send_events(backend::event::pointer_move_event(event).flat_map(|event| {
                    let delta = delta
                        .delta(&event)
                        .to_physical(backend::scale_factor(&window));

                    let x_motion = (delta.x != 0.0).then_some(Event::DeviceEvent {
                        device_id,
                        event: DeviceEvent::Motion {
                            axis: 0,
                            value: delta.x,
                        },
                    });

                    let y_motion = (delta.y != 0.0).then_some(Event::DeviceEvent {
                        device_id,
                        event: DeviceEvent::Motion {
                            axis: 1,
                            value: delta.y,
                        },
                    });

                    x_motion
                        .into_iter()
                        .chain(y_motion)
                        .chain(iter::once(Event::DeviceEvent {
                            device_id,
                            event: DeviceEvent::MouseMotion {
                                delta: (delta.x, delta.y),
                            },
                        }))
                }));
            }),
        ));
        let runner = self.clone();
        let window = self.window().clone();
        *self.0.on_wheel.borrow_mut() = Some(EventListenerHandle::new(
            self.window(),
            "wheel",
            Closure::new(move |event: WheelEvent| {
                if !runner.device_events() {
                    return;
                }

                if let Some(delta) = backend::event::mouse_scroll_delta(&window, &event) {
                    runner.send_event(Event::DeviceEvent {
                        device_id: RootDeviceId(DeviceId(0)),
                        event: DeviceEvent::MouseWheel { delta },
                    });
                }
            }),
        ));
        let runner = self.clone();
        *self.0.on_mouse_press.borrow_mut() = Some(EventListenerHandle::new(
            self.window(),
            "pointerdown",
            Closure::new(move |event: PointerEvent| {
                if !runner.device_events() {
                    return;
                }

                if event.pointer_type() != "mouse" {
                    return;
                }

                let button = backend::event::mouse_button(&event).expect("no mouse button pressed");
                runner.send_event(Event::DeviceEvent {
                    device_id: RootDeviceId(DeviceId(event.pointer_id())),
                    event: DeviceEvent::Button {
                        button: button.to_id(),
                        state: ElementState::Pressed,
                    },
                });
            }),
        ));
        let runner = self.clone();
        *self.0.on_mouse_release.borrow_mut() = Some(EventListenerHandle::new(
            self.window(),
            "pointerup",
            Closure::new(move |event: PointerEvent| {
                if !runner.device_events() {
                    return;
                }

                if event.pointer_type() != "mouse" {
                    return;
                }

                let button = backend::event::mouse_button(&event).expect("no mouse button pressed");
                runner.send_event(Event::DeviceEvent {
                    device_id: RootDeviceId(DeviceId(event.pointer_id())),
                    event: DeviceEvent::Button {
                        button: button.to_id(),
                        state: ElementState::Released,
                    },
                });
            }),
        ));
        let runner = self.clone();
        *self.0.on_key_press.borrow_mut() = Some(EventListenerHandle::new(
            self.window(),
            "keydown",
            Closure::new(move |event: KeyboardEvent| {
                if !runner.device_events() {
                    return;
                }

                runner.send_event(Event::DeviceEvent {
                    device_id: RootDeviceId(unsafe { DeviceId::dummy() }),
                    event: DeviceEvent::Key(RawKeyEvent {
                        physical_key: backend::event::key_code(&event),
                        state: ElementState::Pressed,
                    }),
                });
            }),
        ));
        let runner = self.clone();
        *self.0.on_key_release.borrow_mut() = Some(EventListenerHandle::new(
            self.window(),
            "keyup",
            Closure::new(move |event: KeyboardEvent| {
                if !runner.device_events() {
                    return;
                }

                runner.send_event(Event::DeviceEvent {
                    device_id: RootDeviceId(unsafe { DeviceId::dummy() }),
                    event: DeviceEvent::Key(RawKeyEvent {
                        physical_key: backend::event::key_code(&event),
                        state: ElementState::Released,
                    }),
                });
            }),
        ));
    }

    // Generate a strictly increasing ID
    // This is used to differentiate windows when handling events
    pub fn generate_id(&self) -> u32 {
        let mut id = self.0.id.borrow_mut();
        *id += 1;

        *id
    }

    pub fn request_redraw(&self, id: WindowId) {
        self.0.redraw_pending.borrow_mut().insert(id);
        self.send_events::<EventWrapper<T>>(iter::empty());
    }

    pub fn init(&self) {
        // NB: For consistency all platforms must emit a 'resumed' event even though web
        // applications don't themselves have a formal suspend/resume lifecycle.
        self.run_until_cleared([Event::NewEvents(StartCause::Init), Event::Resumed].into_iter());
    }

    // Run the polling logic for the Poll ControlFlow, which involves clearing the queue
    pub fn poll(&self) {
        let start_cause = Event::NewEvents(StartCause::Poll);
        self.run_until_cleared(iter::once(start_cause));
    }

    // Run the logic for waking from a WaitUntil, which involves clearing the queue
    // Generally there shouldn't be events built up when this is called
    pub fn resume_time_reached(&self, start: Instant, requested_resume: Instant) {
        let start_cause = Event::NewEvents(StartCause::ResumeTimeReached {
            start,
            requested_resume,
        });
        self.run_until_cleared(iter::once(start_cause));
    }

    // Add an event to the event loop runner, from the user or an event handler
    //
    // It will determine if the event should be immediately sent to the user or buffered for later
    pub(crate) fn send_event<E: Into<EventWrapper<T>>>(&self, event: E) {
        self.send_events(iter::once(event));
    }

    // Add a series of events to the event loop runner
    //
    // It will determine if the event should be immediately sent to the user or buffered for later
    pub(crate) fn send_events<E: Into<EventWrapper<T>>>(
        &self,
        events: impl IntoIterator<Item = E>,
    ) {
        // If the event loop is closed, it should discard any new events
        if self.is_closed() {
            return;
        }
        // If we can run the event processing right now, or need to queue this and wait for later
        let mut process_immediately = true;
        match self.0.runner.try_borrow().as_ref().map(Deref::deref) {
            Ok(RunnerEnum::Running(ref runner)) => {
                // If we're currently polling, queue this and wait for the poll() method to be called
                if let State::Poll { .. } = runner.state {
                    process_immediately = false;
                }
            }
            Ok(RunnerEnum::Pending) => {
                // The runner still hasn't been attached: queue this event and wait for it to be
                process_immediately = false;
            }
            // Some other code is mutating the runner, which most likely means
            // the event loop is running and busy. So we queue this event for
            // it to be processed later.
            Err(_) => {
                process_immediately = false;
            }
            // This is unreachable since `self.is_closed() == true`.
            Ok(RunnerEnum::Destroyed) => unreachable!(),
        }
        if !process_immediately {
            // Queue these events to look at later
            self.0
                .events
                .borrow_mut()
                .extend(events.into_iter().map(Into::into));
            return;
        }
        // At this point, we know this is a fresh set of events
        // Now we determine why new events are incoming, and handle the events
        let start_cause = match (self.0.runner.borrow().maybe_runner())
            .unwrap_or_else(|| {
                unreachable!("The runner cannot process events when it is not attached")
            })
            .maybe_start_cause()
        {
            Some(c) => c,
            // If we're in the exit state, don't do event processing
            None => return,
        };
        // Take the start event, then the events provided to this function, and run an iteration of
        // the event loop
        let start_event = Event::NewEvents(start_cause);
        let events =
            iter::once(EventWrapper::from(start_event)).chain(events.into_iter().map(Into::into));
        self.run_until_cleared(events);
    }

    // Process the destroy-pending windows. This should only be called from
    // `run_until_cleared`, somewhere between emitting `NewEvents` and `MainEventsCleared`.
    fn process_destroy_pending_windows(&self, control: &mut ControlFlow) {
        while let Some(id) = self.0.destroy_pending.borrow_mut().pop_front() {
            self.0
                .all_canvases
                .borrow_mut()
                .retain(|&(item_id, _)| item_id != id);
            self.handle_event(
                Event::WindowEvent {
                    window_id: id,
                    event: crate::event::WindowEvent::Destroyed,
                },
                control,
            );
            self.0.redraw_pending.borrow_mut().remove(&id);
        }
    }

    // Given the set of new events, run the event loop until the main events and redraw events are
    // cleared
    //
    // This will also process any events that have been queued or that are queued during processing
    fn run_until_cleared<E: Into<EventWrapper<T>>>(&self, events: impl Iterator<Item = E>) {
        let mut control = self.current_control_flow();
        for event in events {
            self.handle_event(event.into(), &mut control);
        }
        self.process_destroy_pending_windows(&mut control);
        self.handle_event(Event::MainEventsCleared, &mut control);

        // Collect all of the redraw events to avoid double-locking the RefCell
        let redraw_events: Vec<WindowId> = self.0.redraw_pending.borrow_mut().drain().collect();
        for window_id in redraw_events {
            self.handle_event(Event::RedrawRequested(window_id), &mut control);
        }
        self.handle_event(Event::RedrawEventsCleared, &mut control);

        self.apply_control_flow(control);
        // If the event loop is closed, it has been closed this iteration and now the closing
        // event should be emitted
        if self.is_closed() {
            self.handle_loop_destroyed(&mut control);
        }
    }

    fn handle_unload(&self) {
        self.apply_control_flow(ControlFlow::Exit);
        let mut control = self.current_control_flow();
        // We don't call `handle_loop_destroyed` here because we don't need to
        // perform cleanup when the web browser is going to destroy the page.
        self.handle_event(Event::LoopDestroyed, &mut control);
    }

    // handle_event takes in events and either queues them or applies a callback
    //
    // It should only ever be called from `run_until_cleared`.
    fn handle_event(&self, event: impl Into<EventWrapper<T>>, control: &mut ControlFlow) {
        if self.is_closed() {
            *control = ControlFlow::Exit;
        }
        match *self.0.runner.borrow_mut() {
            RunnerEnum::Running(ref mut runner) => {
                runner.handle_single_event(self, event, control);
            }
            // If an event is being handled without a runner somehow, add it to the event queue so
            // it will eventually be processed
            RunnerEnum::Pending => self.0.events.borrow_mut().push_back(event.into()),
            // If the Runner has been destroyed, there is nothing to do.
            RunnerEnum::Destroyed => return,
        }

        let is_closed = matches!(*control, ControlFlow::ExitWithCode(_));

        // Don't take events out of the queue if the loop is closed or the runner doesn't exist
        // If the runner doesn't exist and this method recurses, it will recurse infinitely
        if !is_closed && self.0.runner.borrow().maybe_runner().is_some() {
            // Take an event out of the queue and handle it
            // Make sure not to let the borrow_mut live during the next handle_event
            let event = { self.0.events.borrow_mut().pop_front() };
            if let Some(event) = event {
                self.handle_event(event, control);
            }
        }
    }

    // Apply the new ControlFlow that has been selected by the user
    // Start any necessary timeouts etc
    fn apply_control_flow(&self, control_flow: ControlFlow) {
        let new_state = match control_flow {
            ControlFlow::Poll => {
                let cloned = self.clone();
                State::Poll {
                    request: backend::IdleCallback::new(self.window().clone(), move || {
                        cloned.poll()
                    }),
                }
            }
            ControlFlow::Wait => State::Wait {
                start: Instant::now(),
            },
            ControlFlow::WaitUntil(end) => {
                let start = Instant::now();

                let delay = if end <= start {
                    Duration::from_millis(0)
                } else {
                    end - start
                };

                let cloned = self.clone();

                State::WaitUntil {
                    start,
                    end,
                    timeout: backend::Timeout::new(
                        self.window().clone(),
                        move || cloned.resume_time_reached(start, end),
                        delay,
                    ),
                }
            }
            ControlFlow::ExitWithCode(_) => State::Exit,
        };

        if let RunnerEnum::Running(ref mut runner) = *self.0.runner.borrow_mut() {
            runner.state = new_state;
        }
    }

    fn handle_loop_destroyed(&self, control: &mut ControlFlow) {
        self.handle_event(Event::LoopDestroyed, control);
        let all_canvases = std::mem::take(&mut *self.0.all_canvases.borrow_mut());
        *self.0.page_transition_event_handle.borrow_mut() = None;
        *self.0.on_mouse_move.borrow_mut() = None;
        *self.0.on_wheel.borrow_mut() = None;
        *self.0.on_mouse_press.borrow_mut() = None;
        *self.0.on_mouse_release.borrow_mut() = None;
        *self.0.on_key_press.borrow_mut() = None;
        *self.0.on_key_release.borrow_mut() = None;
        // Dropping the `Runner` drops the event handler closure, which will in
        // turn drop all `Window`s moved into the closure.
        *self.0.runner.borrow_mut() = RunnerEnum::Destroyed;
        for (_, canvas) in all_canvases {
            // In case any remaining `Window`s are still not dropped, we will need
            // to explicitly remove the event handlers associated with their canvases.
            if let Some(canvas) = canvas.upgrade() {
                let mut canvas = canvas.borrow_mut();
                canvas.remove_listeners();
            }
        }
        // At this point, the `self.0` `Rc` should only be strongly referenced
        // by the following:
        // * `self`, i.e. the item which triggered this event loop wakeup, which
        //   is usually a `wasm-bindgen` `Closure`, which will be dropped after
        //   returning to the JS glue code.
        // * The `EventLoopWindowTarget` leaked inside `EventLoop::run` due to the
        //   JS exception thrown at the end.
        // * For each undropped `Window`:
        //     * The `register_redraw_request` closure.
        //     * The `destroy_fn` closure.
        if self.0.event_loop_recreation.get() {
            crate::event_loop::EventLoopBuilder::<T>::allow_event_loop_recreation();
        }
    }

    // Check if the event loop is currently closed
    fn is_closed(&self) -> bool {
        match self.0.runner.try_borrow().as_ref().map(Deref::deref) {
            Ok(RunnerEnum::Running(runner)) => runner.state.is_exit(),
            // The event loop is not closed since it is not initialized.
            Ok(RunnerEnum::Pending) => false,
            // The event loop is closed since it has been destroyed.
            Ok(RunnerEnum::Destroyed) => true,
            // Some other code is mutating the runner, which most likely means
            // the event loop is running and busy.
            Err(_) => false,
        }
    }

    // Get the current control flow state
    fn current_control_flow(&self) -> ControlFlow {
        match *self.0.runner.borrow() {
            RunnerEnum::Running(ref runner) => runner.state.control_flow(),
            RunnerEnum::Pending => ControlFlow::Poll,
            RunnerEnum::Destroyed => ControlFlow::Exit,
        }
    }

    pub fn listen_device_events(&self, allowed: DeviceEvents) {
        self.0.device_events.set(allowed)
    }

    pub fn device_events(&self) -> bool {
        match self.0.device_events.get() {
            DeviceEvents::Always => true,
            DeviceEvents::WhenFocused => self.0.all_canvases.borrow().iter().any(|(_, canvas)| {
                if let Some(canvas) = canvas.upgrade() {
                    canvas.borrow().has_focus.load(Ordering::Relaxed)
                } else {
                    false
                }
            }),
            DeviceEvents::Never => false,
        }
    }

    pub fn event_loop_recreation(&self, allow: bool) {
        self.0.event_loop_recreation.set(allow)
    }
}

pub(crate) enum EventWrapper<T: 'static> {
    Event(Event<'static, T>),
    ScaleChange {
        canvas: Weak<RefCell<backend::Canvas>>,
        size: PhysicalSize<u32>,
        scale: f64,
    },
}

impl<T> From<Event<'static, T>> for EventWrapper<T> {
    fn from(value: Event<'static, T>) -> Self {
        Self::Event(value)
    }
}
