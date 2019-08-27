use std::{
    any::Any,
    cell::RefCell,
    collections::VecDeque,
    mem, panic, ptr,
    rc::Rc,
    time::Instant,
};

use winapi::{
    shared::{
        windef::HWND,
    },
    um::winuser,
};

use crate::{
    event::{Event, StartCause},
    event_loop::ControlFlow,
    platform_impl::platform::{
        event_loop::EventLoop,
    },
    window::WindowId,
};

pub(crate) type EventLoopRunnerShared<T> = Rc<ELRShared<T>>;
pub(crate) struct ELRShared<T> {
    runner: RefCell<Option<EventLoopRunner<T>>>,
    buffer: RefCell<VecDeque<Event<T>>>,
    redraw_buffer: Rc<RefCell<VecDeque<WindowId>>>,
}
struct EventLoopRunner<T> {
    control_flow: ControlFlow,
    runner_state: RunnerState,
    modal_redraw_window: HWND,
    in_modal_loop: bool,
    event_handler: Box<dyn FnMut(Event<T>, &mut ControlFlow)>,
    panic_error: Option<PanicError>,
    redraw_buffer: Rc<RefCell<VecDeque<WindowId>>>,
}
pub type PanicError = Box<dyn Any + Send + 'static>;

impl<T> ELRShared<T> {
    pub(crate) fn new() -> ELRShared<T> {
        ELRShared {
            runner: RefCell::new(None),
            buffer: RefCell::new(VecDeque::new()),
            redraw_buffer: Default::default(),
        }
    }

    pub(crate) unsafe fn set_runner<F>(&self, event_loop: &EventLoop<T>, f: F)
    where
        F: FnMut(Event<T>, &mut ControlFlow),
    {
        let mut runner = EventLoopRunner::new(event_loop, self.redraw_buffer.clone(), f);
        {
            let mut runner_ref = self.runner.borrow_mut();
            loop {
                let event = self.buffer.borrow_mut().pop_front();
                match event {
                    Some(e) => {
                        runner.process_event(e);
                    }
                    None => break,
                }
            }
            *runner_ref = Some(runner);
        }
    }

    pub(crate) fn destroy_runner(&self) {
        *self.runner.borrow_mut() = None;
    }

    pub(crate) fn new_events(&self) {
        let mut runner_ref = self.runner.borrow_mut();
        if let Some(ref mut runner) = *runner_ref {
            runner.new_events();
        }
    }

    pub(crate) unsafe fn send_event(&self, event: Event<T>) {
        if let Ok(mut runner_ref) = self.runner.try_borrow_mut() {
            if let Some(ref mut runner) = *runner_ref {
                runner.process_event(event);

                // Dispatch any events that were buffered during the call to `process_event`.
                loop {
                    // We do this instead of using a `while let` loop because if we use a `while let`
                    // loop the reference returned `borrow_mut()` doesn't get dropped until the end
                    // of the loop's body and attempts to add events to the event buffer while in
                    // `process_event` will fail.
                    let buffered_event_opt = self.buffer.borrow_mut().pop_front();
                    match buffered_event_opt {
                        Some(event) => runner.process_event(event),
                        None => break,
                    }
                }

                return;
            }
        }

        // If the runner is already borrowed, we're in the middle of an event loop invocation. Add
        // the event to a buffer to be processed later.
        self.buffer_event(event);
    }

    pub(crate) unsafe fn call_event_handler(&self, event: Event<T>) {
        if let Ok(mut runner_ref) = self.runner.try_borrow_mut() {
            if let Some(ref mut runner) = *runner_ref {
                runner.call_event_handler(event);
                return;
            }
        }
    }

    pub(crate) fn events_cleared(&self) {
        let mut runner_ref = self.runner.borrow_mut();
        if let Some(ref mut runner) = *runner_ref {
            runner.events_cleared();
        }
    }

    pub(crate) fn take_panic_error(&self) -> Result<(), PanicError> {
        let mut runner_ref = self.runner.borrow_mut();
        if let Some(ref mut runner) = *runner_ref {
            runner.take_panic_error()
        } else {
            Ok(())
        }
    }

    pub(crate) fn set_modal_loop(&self, in_modal_loop: bool) {
        let mut runner_ref = self.runner.borrow_mut();
        if let Some(ref mut runner) = *runner_ref {
            runner.in_modal_loop = in_modal_loop;
        }
    }

    pub(crate) fn in_modal_loop(&self) -> bool {
        let runner = self.runner.borrow();
        if let Some(ref runner) = *runner {
            runner.in_modal_loop
        } else {
            false
        }
    }

    pub fn control_flow(&self) -> ControlFlow {
        let runner_ref = self.runner.borrow();
        if let Some(ref runner) = *runner_ref {
            runner.control_flow
        } else {
            ControlFlow::Exit
        }
    }

    fn buffer_event(&self, event: Event<T>) {
        match event {
            Event::RedrawRequested(window_id) => self.redraw_buffer.borrow_mut().push_back(window_id),
            _ => self.buffer.borrow_mut().push_back(event),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunnerState {
    /// The event loop has just been created, and an `Init` event must be sent.
    New,
    /// The event loop is idling, and began idling at the given instant.
    Idle(Instant),
    /// The event loop has received a signal from the OS that the loop may resume, but no winit
    /// events have been generated yet. We're waiting for an event to be processed or the events
    /// to be marked as cleared to send `NewEvents`, depending on the current `ControlFlow`.
    DeferredNewEvents(Instant),
    /// The event loop is handling the OS's events and sending them to the user's callback.
    /// `NewEvents` has been sent, and `EventsCleared` hasn't.
    HandlingEvents,
    HandlingRedraw,
}

impl<T> EventLoopRunner<T> {
    unsafe fn new<F>(event_loop: &EventLoop<T>, redraw_buffer: Rc<RefCell<VecDeque<WindowId>>>, f: F) -> EventLoopRunner<T>
    where
        F: FnMut(Event<T>, &mut ControlFlow),
    {
        EventLoopRunner {
            control_flow: ControlFlow::default(),
            runner_state: RunnerState::New,
            in_modal_loop: false,
            modal_redraw_window: event_loop.window_target.p.thread_msg_target,
            event_handler: mem::transmute::<
                Box<dyn FnMut(Event<T>, &mut ControlFlow)>,
                Box<dyn FnMut(Event<T>, &mut ControlFlow)>,
            >(Box::new(f)),
            panic_error: None,
            redraw_buffer,
        }
    }

    fn take_panic_error(&mut self) -> Result<(), PanicError> {
        match self.panic_error.take() {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    fn new_events(&mut self) {
        self.runner_state = match self.runner_state {
            // If we're already handling events or have deferred `NewEvents`, we don't need to do
            // do any processing.
            RunnerState::HandlingEvents | RunnerState::HandlingRedraw | RunnerState::DeferredNewEvents(..) => self.runner_state,

            // Send the `Init` `NewEvents` and immediately move into event processing.
            RunnerState::New => {
                self.call_event_handler(Event::NewEvents(StartCause::Init));
                RunnerState::HandlingEvents
            }

            // When `NewEvents` gets sent after an idle depends on the control flow...
            RunnerState::Idle(wait_start) => {
                match self.control_flow {
                    // If we're polling, send `NewEvents` and immediately move into event processing.
                    ControlFlow::Poll => {
                        self.call_event_handler(Event::NewEvents(StartCause::Poll));
                        RunnerState::HandlingEvents
                    },
                    // If the user was waiting until a specific time, the `NewEvents` call gets sent
                    // at varying times depending on the current time.
                    ControlFlow::WaitUntil(resume_time) => {
                        match Instant::now() >= resume_time {
                            // If the current time is later than the requested resume time, we can tell the
                            // user that the resume time has been reached with `NewEvents` and immdiately move
                            // into event processing.
                            true => {
                                self.call_event_handler(Event::NewEvents(StartCause::ResumeTimeReached {
                                    start: wait_start,
                                    requested_resume: resume_time,
                                }));
                                RunnerState::HandlingEvents
                            },
                            // However, if the current time is EARLIER than the requested resume time, we
                            // don't want to send the `WaitCancelled` event until we know an event is being
                            // sent. Defer.
                            false => RunnerState::DeferredNewEvents(wait_start)
                        }
                    },
                    // If we're waiting, `NewEvents` doesn't get sent until winit gets an event, so
                    // we defer.
                    ControlFlow::Wait |
                    // `Exit` shouldn't really ever get sent here, but if it does do something somewhat sane.
                    ControlFlow::Exit => RunnerState::DeferredNewEvents(wait_start),
                }
            }
        };
    }

    fn process_event(&mut self, event: Event<T>) {
        // If we're in the modal loop, we need to have some mechanism for finding when the event
        // queue has been cleared so we can call `events_cleared`. Windows doesn't give any utilities
        // for doing this, but it DOES guarantee that WM_PAINT will only occur after input events have
        // been processed. So, we send WM_PAINT to a dummy window which calls `events_cleared` when
        // the events queue has been emptied.
        if self.in_modal_loop {
            unsafe {
                winuser::RedrawWindow(
                    self.modal_redraw_window,
                    ptr::null(),
                    ptr::null_mut(),
                    winuser::RDW_INTERNALPAINT,
                );
            }
        }

        // If new event processing has to be done (i.e. call NewEvents or defer), do it. If we're
        // already in processing nothing happens with this call.
        self.new_events();

        // Now that an event has been received, we have to send any `NewEvents` calls that were
        // deferred.
        if let RunnerState::DeferredNewEvents(wait_start) = self.runner_state {
            match self.control_flow {
                ControlFlow::Exit | ControlFlow::Wait => {
                    self.call_event_handler(Event::NewEvents(StartCause::WaitCancelled {
                        start: wait_start,
                        requested_resume: None,
                    }))
                }
                ControlFlow::WaitUntil(resume_time) => {
                    let start_cause = match Instant::now() >= resume_time {
                        // If the current time is later than the requested resume time, the resume time
                        // has been reached.
                        true => StartCause::ResumeTimeReached {
                            start: wait_start,
                            requested_resume: resume_time,
                        },
                        // Otherwise, the requested resume time HASN'T been reached and we send a WaitCancelled.
                        false => StartCause::WaitCancelled {
                            start: wait_start,
                            requested_resume: Some(resume_time),
                        },
                    };
                    self.call_event_handler(Event::NewEvents(start_cause));
                }
                // This can be reached if the control flow is changed to poll during a `RedrawRequested`
                // that was sent after `EventsCleared`.
                ControlFlow::Poll => self.call_event_handler(Event::NewEvents(StartCause::Poll)),
            }
        }

        match (self.runner_state, &event) {
            (RunnerState::HandlingRedraw, Event::RedrawRequested(_)) => self.call_event_handler(event),
            (_, Event::RedrawRequested(_)) => {
                self.call_event_handler(Event::MainEventsCleared);
                self.runner_state = RunnerState::HandlingRedraw;
                self.call_event_handler(event);
            },
            (RunnerState::HandlingRedraw, _) => {
                warn!("Non-redraw event dispatched durning redraw phase");
                self.events_cleared();
                self.new_events();
                self.call_event_handler(event);
            }
            (_, _) => {
                self.runner_state = RunnerState::HandlingEvents;
                self.call_event_handler(event);
            }
        }
    }

    fn flush_redraws(&mut self) {
        loop {
            let redraw_window_opt = self.redraw_buffer.borrow_mut().pop_front();
            match redraw_window_opt {
                Some(window_id) => self.process_event(Event::RedrawRequested(window_id)),
                None => break,
            }
        }
    }

    fn events_cleared(&mut self) {
        match self.runner_state {
            // If we were handling events, send the EventsCleared message.
            RunnerState::HandlingEvents => {
                self.call_event_handler(Event::MainEventsCleared);
                self.flush_redraws();
                self.call_event_handler(Event::RedrawEventsCleared);
                self.runner_state = RunnerState::Idle(Instant::now());
            }

            RunnerState::HandlingRedraw => {
                self.call_event_handler(Event::RedrawEventsCleared);
                self.runner_state = RunnerState::Idle(Instant::now());
            }

            // If we *weren't* handling events, we don't have to do anything.
            RunnerState::New | RunnerState::Idle(..) => (),

            // Some control flows require a NewEvents call even if no events were received. This
            // branch handles those.
            RunnerState::DeferredNewEvents(wait_start) => {
                match self.control_flow {
                    // If we had deferred a Poll, send the Poll NewEvents and EventsCleared.
                    ControlFlow::Poll => {
                        self.call_event_handler(Event::NewEvents(StartCause::Poll));
                        self.call_event_handler(Event::MainEventsCleared);
                        self.flush_redraws();
                        self.call_event_handler(Event::RedrawEventsCleared);
                    }
                    // If we had deferred a WaitUntil and the resume time has since been reached,
                    // send the resume notification and EventsCleared event.
                    ControlFlow::WaitUntil(resume_time) => {
                        if Instant::now() >= resume_time {
                            self.call_event_handler(Event::NewEvents(
                                StartCause::ResumeTimeReached {
                                    start: wait_start,
                                    requested_resume: resume_time,
                                },
                            ));
                            self.call_event_handler(Event::MainEventsCleared);
                            self.flush_redraws();
                            self.call_event_handler(Event::RedrawEventsCleared);
                        }
                    }
                    // If we deferred a wait and no events were received, the user doesn't have to
                    // get an event.
                    ControlFlow::Wait | ControlFlow::Exit => (),
                }
                // Mark that we've entered an idle state.
                self.runner_state = RunnerState::Idle(wait_start)
            }
        }
    }

    fn call_event_handler(&mut self, event: Event<T>) {
        if self.panic_error.is_none() {
            let EventLoopRunner {
                ref mut panic_error,
                ref mut event_handler,
                ref mut control_flow,
                ..
            } = self;
            *panic_error = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                if *control_flow != ControlFlow::Exit {
                    (*event_handler)(event, control_flow);
                } else {
                    (*event_handler)(event, &mut ControlFlow::Exit);
                }
            }))
            .err();
        }
    }
}
