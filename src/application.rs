// TMP: For showcase
#![allow(unreachable_code)]
#![allow(unused_variables)]
#![allow(dead_code)]
use std::fmt;
#[cfg(not(wasm_platform))]
use std::time::Instant;

#[cfg(wasm_platform)]
use web_time::Instant;

use crate::{
    error::EventLoopError,
    event::{DeviceEvent, DeviceId, Event, StartCause, WindowEvent},
    event_loop::{EventLoop, EventLoopWindowTarget},
    window::WindowId,
};

/// TODO
///
/// Unknown: Should the user event be an associated type instead?
#[allow(missing_docs)]
pub trait ApplicationHandler<T = ()> {
    fn resume(&mut self, elwt: &EventLoopWindowTarget<T>) {
        let _ = elwt;
    }

    fn suspend(&mut self, elwt: &EventLoopWindowTarget<T>) {
        let _ = elwt;
    }

    fn window_event(
        &mut self,
        elwt: &EventLoopWindowTarget<T>,
        window_id: WindowId,
        event: WindowEvent,
    );

    // Default noop events

    fn device_event(
        &mut self,
        elwt: &EventLoopWindowTarget<T>,
        device_id: DeviceId,
        event: DeviceEvent,
    ) {
        let _ = elwt;
        let _ = device_id;
        let _ = event;
    }

    fn user_event(&mut self, elwt: &EventLoopWindowTarget<T>, event: T) {
        let _ = elwt;
        let _ = event;
    }

    // Unsure about these, we should probably figure out better timer support

    fn start_wait_cancelled(
        &mut self,
        elwt: &EventLoopWindowTarget<T>,
        start: Instant,
        requested_resume: Option<Instant>,
    ) {
        let _ = elwt;
        let _ = start;
        let _ = requested_resume;
    }

    fn start_resume_time_reached(
        &mut self,
        elwt: &EventLoopWindowTarget<T>,
        start: Instant,
        requested_resume: Instant,
    ) {
        let _ = elwt;
        let _ = start;
        let _ = requested_resume;
    }

    fn start_poll(&mut self, elwt: &EventLoopWindowTarget<T>) {
        let _ = elwt;
    }

    fn about_to_wait(&mut self, elwt: &EventLoopWindowTarget<T>) {
        let _ = elwt;
    }
}

enum State<A, I> {
    /// Stores an initialization closure.
    Uninitialized(I),
    /// Stores the application while the event loop is running.
    Running(A),
    /// Stores nothing, the application has been dropped at this point.
    Exited,
}

impl<A, I> State<A, I> {
    // Handle the event, and possibly transition to another state
    fn next<T>(self, event: Event<T>, elwt: &EventLoopWindowTarget<T>) -> Self
    where
        A: ApplicationHandler<T>,
        I: FnOnce(&EventLoopWindowTarget<T>) -> A,
    {
        match event {
            Event::NewEvents(StartCause::Init) => match self {
                State::Uninitialized(init) => State::Running(init(elwt)),
                state => unreachable!("invalid initialization: state was {state:?}"),
            },
            Event::LoopExiting => match self {
                // Don't forward the event; users should just overwrite `Drop` for their type if they want to do something on exit.
                State::Running(_app_to_drop) => State::Exited,
                state => unreachable!("invalid exit: state was {state:?}"),
            },
            Event::Suspended => match self {
                State::Running(mut app) => {
                    app.suspend(elwt);
                    State::Running(app)
                }
                state => unreachable!("invalid suspend: state was {state:?}"),
            },
            Event::Resumed => match self {
                State::Running(mut app) => {
                    app.resume(elwt);
                    State::Running(app)
                }
                state => unreachable!("invalid resume: state was {state:?}"),
            },
            Event::WindowEvent { window_id, event } => match self {
                State::Running(mut app) => {
                    app.window_event(elwt, window_id, event);
                    State::Running(app)
                }
                state => unreachable!("invalid window event: state was {state:?}"),
            },
            Event::DeviceEvent { device_id, event } => match self {
                State::Running(mut app) => {
                    app.device_event(elwt, device_id, event);
                    State::Running(app)
                }
                state => unreachable!("invalid device event: state was {state:?}"),
            },
            Event::UserEvent(event) => match self {
                State::Running(mut app) => {
                    app.user_event(elwt, event);
                    State::Running(app)
                }
                state => unreachable!("invalid user event: state was {state:?}"),
            },
            Event::NewEvents(StartCause::ResumeTimeReached {
                start,
                requested_resume,
            }) => match self {
                State::Running(mut app) => {
                    app.start_resume_time_reached(elwt, start, requested_resume);
                    State::Running(app)
                }
                state => unreachable!("invalid resume time reached event: state was {state:?}"),
            },
            Event::NewEvents(StartCause::WaitCancelled {
                start,
                requested_resume,
            }) => match self {
                State::Running(mut app) => {
                    app.start_wait_cancelled(elwt, start, requested_resume);
                    State::Running(app)
                }
                state => unreachable!("invalid wait cancelled event: state was {state:?}"),
            },
            Event::NewEvents(StartCause::Poll) => match self {
                State::Running(mut app) => {
                    app.start_poll(elwt);
                    State::Running(app)
                }
                state => unreachable!("invalid poll event: state was {state:?}"),
            },
            Event::AboutToWait => match self {
                State::Running(mut app) => {
                    app.about_to_wait(elwt);
                    State::Running(app)
                }
                state => unreachable!("invalid about to wait event: state was {state:?}"),
            },
        }
    }
}

impl<A, I> fmt::Debug for State<A, I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Uninitialized(_) => f.write_str("Uninitialized"),
            Self::Running(_) => f.write_str("Running"),
            Self::Exited => f.write_str("Exited"),
        }
    }
}

impl<T> EventLoop<T> {
    pub fn run_with<A: ApplicationHandler<T>>(
        self,
        init: impl FnOnce(&EventLoopWindowTarget<T>) -> A,
    ) -> Result<(), EventLoopError> {
        let mut state_storage: Option<State<A, _>> = Some(State::Uninitialized(init));

        self.run(move |event, elwt| {
            let state = state_storage
                .take()
                .expect("failed extracting state, either due to re-entrancy or because a a panic occurred previously");

            state_storage = Some(state.next(event, elwt));
        })
    }
}

// Extensions

// Simpler version of the State enum above, used for communicating the current
// state to the user when using `pump_events`.
//
// The intention is that the application is always dropped by the user
// themselves, and the application is never returned in a suspended state.
enum PumpEventStatus<A, I> {
    Uninitialized(I),
    Running(A),
}

struct ShouldExit(pub bool);

impl<T> EventLoop<T> {
    fn pump_events_with<A: ApplicationHandler<T>>(
        self,
        status: &mut PumpEventStatus<A, impl FnOnce(&EventLoopWindowTarget<T>) -> A>,
    ) -> Result<ShouldExit, EventLoopError> {
        *status = PumpEventStatus::Running(todo!());
        Ok(ShouldExit(false))
    }

    // Same signature and semantics as `run_with`, except for taking `&mut self`.
    fn run_ondemand_with<A: ApplicationHandler<T>>(
        &mut self,
        init: impl FnOnce(&EventLoopWindowTarget<T>) -> A,
    ) -> Result<(), EventLoopError> {
        todo!()
    }
}
