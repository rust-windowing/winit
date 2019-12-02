use super::{backend, state::State};
use crate::event::{Event, StartCause};
use crate::event_loop as root;
use crate::window::WindowId;

use instant::{Duration, Instant};
use std::{
    cell::RefCell,
    clone::Clone,
    collections::{HashSet, VecDeque},
    rc::Rc,
};

pub struct Shared<T>(Rc<Execution<T>>);

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Shared(self.0.clone())
    }
}

pub struct Execution<T> {
    runner: RefCell<Option<Runner<T>>>,
    events: RefCell<VecDeque<Event<T>>>,
    id: RefCell<u32>,
    redraw_pending: RefCell<HashSet<WindowId>>,
}

struct Runner<T> {
    state: State,
    is_busy: bool,
    event_handler: Box<dyn FnMut(Event<T>, &mut root::ControlFlow)>,
}

impl<T: 'static> Runner<T> {
    pub fn new(event_handler: Box<dyn FnMut(Event<T>, &mut root::ControlFlow)>) -> Self {
        Runner {
            state: State::Init,
            is_busy: false,
            event_handler,
        }
    }
}

impl<T: 'static> Shared<T> {
    pub fn new() -> Self {
        Shared(Rc::new(Execution {
            runner: RefCell::new(None),
            events: RefCell::new(VecDeque::new()),
            id: RefCell::new(0),
            redraw_pending: RefCell::new(HashSet::new()),
        }))
    }

    // Set the event callback to use for the event loop runner
    // This the event callback is a fairly thin layer over the user-provided callback that closes
    // over a RootEventLoopWindowTarget reference
    pub fn set_listener(&self, event_handler: Box<dyn FnMut(Event<T>, &mut root::ControlFlow)>) {
        self.0.runner.replace(Some(Runner::new(event_handler)));
        self.send_event(Event::NewEvents(StartCause::Init));

        let close_instance = self.clone();
        backend::on_unload(move || close_instance.handle_unload());
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
    }

    // Add an event to the event loop runner
    //
    // It will determine if the event should be immediately sent to the user or buffered for later
    pub fn send_event(&self, event: Event<T>) {
        // If the event loop is closed, it should discard any new events
        if self.is_closed() {
            return;
        }

        // Determine if event handling is in process, and then release the borrow on the runner
        let (start_cause, event_is_start) = match *self.0.runner.borrow() {
            Some(ref runner) if !runner.is_busy => {
                if let Event::NewEvents(cause) = event {
                    (cause, true)
                } else {
                    (
                        match runner.state {
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
                            State::Exit => {
                                return;
                            }
                        },
                        false,
                    )
                }
            }
            _ => {
                // Events are currently being handled, so queue this one and don't try to
                // double-process the event queue
                self.0.events.borrow_mut().push_back(event);
                return;
            }
        };
        let mut control = self.current_control_flow();
        // Handle starting a new batch of events
        //
        // The user is informed via Event::NewEvents that there is a batch of events to process
        // However, there is only one of these per batch of events
        self.handle_event(Event::NewEvents(start_cause), &mut control);
        if !event_is_start {
            self.handle_event(event, &mut control);
        }
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
            self.handle_event(Event::LoopDestroyed, &mut control);
        }
    }

    fn handle_unload(&self) {
        self.apply_control_flow(root::ControlFlow::Exit);
        let mut control = self.current_control_flow();
        self.handle_event(Event::LoopDestroyed, &mut control);
    }

    // handle_event takes in events and either queues them or applies a callback
    //
    // It should only ever be called from send_event
    fn handle_event(&self, event: Event<T>, control: &mut root::ControlFlow) {
        let is_closed = self.is_closed();

        match *self.0.runner.borrow_mut() {
            Some(ref mut runner) => {
                // An event is being processed, so the runner should be marked busy
                runner.is_busy = true;

                (runner.event_handler)(event, control);

                // Maintain closed state, even if the callback changes it
                if is_closed {
                    *control = root::ControlFlow::Exit;
                }

                // An event is no longer being processed
                runner.is_busy = false;
            }
            // If an event is being handled without a runner somehow, add it to the event queue so
            // it will eventually be processed
            _ => self.0.events.borrow_mut().push_back(event),
        }

        // Don't take events out of the queue if the loop is closed or the runner doesn't exist
        // If the runner doesn't exist and this method recurses, it will recurse infinitely
        if !is_closed && self.0.runner.borrow().is_some() {
            // Take an event out of the queue and handle it
            if let Some(event) = self.0.events.borrow_mut().pop_front() {
                self.handle_event(event, control);
            }
        }
    }

    // Apply the new ControlFlow that has been selected by the user
    // Start any necessary timeouts etc
    fn apply_control_flow(&self, control_flow: root::ControlFlow) {
        let new_state = match control_flow {
            root::ControlFlow::Poll => {
                let cloned = self.clone();
                State::Poll {
                    timeout: backend::Timeout::new(
                        move || cloned.send_event(Event::NewEvents(StartCause::Poll)),
                        Duration::from_millis(0),
                    ),
                }
            }
            root::ControlFlow::Wait => State::Wait {
                start: Instant::now(),
            },
            root::ControlFlow::WaitUntil(end) => {
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
                        move || cloned.send_event(Event::NewEvents(StartCause::Poll)),
                        delay,
                    ),
                }
            }
            root::ControlFlow::Exit => State::Exit,
        };

        match *self.0.runner.borrow_mut() {
            Some(ref mut runner) => {
                runner.state = new_state;
            }
            None => (),
        }
    }

    // Check if the event loop is currently closed
    fn is_closed(&self) -> bool {
        match *self.0.runner.borrow() {
            Some(ref runner) => runner.state.is_exit(),
            None => false, // If the event loop is None, it has not been intialised yet, so it cannot be closed
        }
    }

    // Get the current control flow state
    fn current_control_flow(&self) -> root::ControlFlow {
        match *self.0.runner.borrow() {
            Some(ref runner) => runner.state.control_flow(),
            None => root::ControlFlow::Poll,
        }
    }
}
