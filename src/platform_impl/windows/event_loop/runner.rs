use std::{any::Any, cell::RefCell, collections::VecDeque, mem, panic, ptr, rc::Rc, time::Instant};

use winapi::{shared::windef::HWND, um::winuser};

use crate::{
    dpi::PhysicalSize,
    event::{Event, StartCause, WindowEvent},
    event_loop::ControlFlow,
    platform_impl::platform::event_loop::{util, EventLoop},
    window::WindowId,
};

pub(crate) type EventLoopRunnerShared<T> = Rc<ELRShared<T>>;
pub(crate) struct ELRShared<T: 'static> {
    runner: RefCell<Option<EventLoopRunner<T>>>,
    buffer: RefCell<VecDeque<BufferedEvent<T>>>,
}

struct EventLoopRunner<T: 'static> {
    control_flow: ControlFlow,
    runner_state: RunnerState,
    modal_redraw_window: HWND,
    in_modal_loop: bool,
    event_handler: Box<dyn FnMut(Event<'_, T>, &mut ControlFlow)>,
    panic_error: Option<PanicError>,
}

pub type PanicError = Box<dyn Any + Send + 'static>;

pub enum BufferedEvent<T: 'static> {
    Event(Event<'static, T>),
    ScaleFactorChanged(WindowId, f64, PhysicalSize<u32>),
}

impl<T> BufferedEvent<T> {
    pub fn from_event(event: Event<'_, T>) -> BufferedEvent<T> {
        match event {
            Event::WindowEvent {
                event:
                    WindowEvent::ScaleFactorChanged {
                        scale_factor,
                        new_inner_size,
                    },
                window_id,
            } => BufferedEvent::ScaleFactorChanged(window_id, scale_factor, *new_inner_size),
            event => BufferedEvent::Event(event.to_static().unwrap()),
        }
    }

    pub fn dispatch_event(self, dispatch: impl FnOnce(Event<'_, T>)) {
        match self {
            Self::Event(event) => dispatch(event),
            Self::ScaleFactorChanged(window_id, scale_factor, mut new_inner_size) => {
                dispatch(Event::WindowEvent {
                    window_id,
                    event: WindowEvent::ScaleFactorChanged {
                        scale_factor,
                        new_inner_size: &mut new_inner_size,
                    },
                });
                util::set_inner_size_physical(
                    (window_id.0).0,
                    new_inner_size.width as _,
                    new_inner_size.height as _,
                );
            }
        }
    }
}

impl<T> ELRShared<T> {
    pub(crate) fn new() -> ELRShared<T> {
        ELRShared {
            runner: RefCell::new(None),
            buffer: RefCell::new(VecDeque::new()),
        }
    }

    pub(crate) unsafe fn set_runner<F>(&self, event_loop: &EventLoop<T>, f: F)
    where
        F: FnMut(Event<'_, T>, &mut ControlFlow),
    {
        let mut runner = EventLoopRunner::new(event_loop, f);
        {
            let mut runner_ref = self.runner.borrow_mut();
            // Dispatch any events that were buffered during the creation of the window
            self.dispatch_buffered_events(&mut runner);
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
            // Dispatch any events that were buffered during the call `new_events`
            self.dispatch_buffered_events(runner);
        }
    }

    pub(crate) fn send_event(&self, event: Event<'_, T>) {
        if let Err(event) = self.send_event_unbuffered(event) {
            // If the runner is already borrowed, we're in the middle of an event loop invocation.
            // Add the event to a buffer to be processed later.
            if let Event::RedrawRequested(_) = event {
                panic!("buffering RedrawRequested event");
            }
            self.buffer
                .borrow_mut()
                .push_back(BufferedEvent::from_event(event));
        }
    }

    fn send_event_unbuffered<'e>(&self, event: Event<'e, T>) -> Result<(), Event<'e, T>> {
        if let Ok(mut runner_ref) = self.runner.try_borrow_mut() {
            if let Some(ref mut runner) = *runner_ref {
                runner.process_event(event);
                // Dispatch any events that were buffered during the call to `process_event`.
                self.dispatch_buffered_events(runner);
                return Ok(());
            }
        }
        Err(event)
    }

    fn dispatch_buffered_events(&self, runner: &mut EventLoopRunner<T>) {
        // We do this instead of using a `while let` loop because if we use a `while let`
        // loop the reference returned `borrow_mut()` doesn't get dropped until the end
        // of the loop's body and attempts to add events to the event buffer while in
        // `process_event` will fail.
        loop {
            let buffered_event_opt = self.buffer.borrow_mut().pop_front();
            match buffered_event_opt {
                Some(e) => e.dispatch_event(|e| runner.process_event(e)),
                None => break,
            }
        }
    }

    pub(crate) fn main_events_cleared(&self) {
        let mut runner_ref = self.runner.borrow_mut();
        if let Some(ref mut runner) = *runner_ref {
            runner.main_events_cleared();
            if !self.buffer.borrow().is_empty() {
                warn!("Buffered events while dispatching MainEventsCleared");
            }
        }
    }

    pub(crate) fn redraw_events_cleared(&self) {
        let mut runner_ref = self.runner.borrow_mut();
        if let Some(ref mut runner) = *runner_ref {
            runner.redraw_events_cleared();
            if !self.buffer.borrow().is_empty() {
                warn!("Buffered events while dispatching RedrawEventsCleared");
            }
        }
    }

    pub(crate) fn destroy_loop(&self) {
        if let Ok(mut runner_ref) = self.runner.try_borrow_mut() {
            if let Some(ref mut runner) = *runner_ref {
                runner.call_event_handler(Event::LoopDestroyed);
            }
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
            if in_modal_loop {
                // jumpstart the modal loop
                unsafe {
                    winuser::RedrawWindow(
                        runner.modal_redraw_window,
                        ptr::null(),
                        ptr::null_mut(),
                        winuser::RDW_INTERNALPAINT,
                    );
                }
            }
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
    /// `NewEvents` has been sent, and `MainEventsCleared` hasn't.
    HandlingEvents,
    /// The event loop is handling the redraw events and sending them to the user's callback.
    /// `MainEventsCleared` has been sent, and `RedrawEventsCleared` hasn't.
    HandlingRedraw,
}

impl<T> EventLoopRunner<T> {
    unsafe fn new<F>(event_loop: &EventLoop<T>, f: F) -> EventLoopRunner<T>
    where
        F: FnMut(Event<'_, T>, &mut ControlFlow),
    {
        EventLoopRunner {
            control_flow: ControlFlow::default(),
            runner_state: RunnerState::New,
            in_modal_loop: false,
            modal_redraw_window: event_loop.window_target.p.thread_msg_target,
            event_handler: mem::transmute::<
                Box<dyn FnMut(Event<'_, T>, &mut ControlFlow)>,
                Box<dyn FnMut(Event<'_, T>, &mut ControlFlow)>,
            >(Box::new(f)),
            panic_error: None,
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
            RunnerState::HandlingEvents
            | RunnerState::HandlingRedraw
            | RunnerState::DeferredNewEvents(..) => self.runner_state,

            // Send the `Init` `NewEvents` and immediately move into event processing.
            RunnerState::New => {
                self.call_event_handler(Event::NewEvents(StartCause::Init));
                RunnerState::HandlingEvents
            }

            // When `NewEvents` gets sent after an idle depends on the control flow...
            // Some `NewEvents` are deferred because not all Windows messages trigger an event_loop event.
            // So we defer the `NewEvents` to when we actually process an event.
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

    fn process_event(&mut self, event: Event<'_, T>) {
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
                // that was sent after `MainEventsCleared`.
                ControlFlow::Poll => self.call_event_handler(Event::NewEvents(StartCause::Poll)),
            }
            self.runner_state = RunnerState::HandlingEvents;
        }

        match (self.runner_state, &event) {
            (RunnerState::HandlingEvents, Event::RedrawRequested(window_id)) => {
                self.call_event_handler(Event::MainEventsCleared);
                self.runner_state = RunnerState::HandlingRedraw;
                self.call_event_handler(Event::RedrawRequested(*window_id));
            }
            (RunnerState::HandlingRedraw, Event::RedrawRequested(window_id)) => {
                self.call_event_handler(Event::RedrawRequested(*window_id));
            }
            (RunnerState::HandlingRedraw, _) => {
                warn!(
                    "non-redraw event in redraw phase: {:?}",
                    event.map_nonuser_event::<()>().ok()
                );
            }
            (_, _) => {
                self.runner_state = RunnerState::HandlingEvents;
                self.call_event_handler(event);
            }
        }
    }

    fn main_events_cleared(&mut self) {
        match self.runner_state {
            // If we were handling events, send the MainEventsCleared message.
            RunnerState::HandlingEvents => {
                self.call_event_handler(Event::MainEventsCleared);
                self.runner_state = RunnerState::HandlingRedraw;
            }

            // We already cleared the main events, we don't have to do anything.
            // This happens when process_events() processed a RedrawRequested event.
            RunnerState::HandlingRedraw => {}

            // If we *weren't* handling events, we don't have to do anything.
            RunnerState::New | RunnerState::Idle(..) => (),

            // Some control flows require a NewEvents call even if no events were received. This
            // branch handles those.
            RunnerState::DeferredNewEvents(wait_start) => {
                match self.control_flow {
                    // If we had deferred a Poll, send the Poll NewEvents and MainEventsCleared.
                    ControlFlow::Poll => {
                        self.call_event_handler(Event::NewEvents(StartCause::Poll));
                        self.runner_state = RunnerState::HandlingEvents;
                        self.call_event_handler(Event::MainEventsCleared);
                        self.runner_state = RunnerState::HandlingRedraw;
                    }
                    // If we had deferred a WaitUntil and the resume time has since been reached,
                    // send the resume notification and MainEventsCleared event.
                    ControlFlow::WaitUntil(resume_time) => {
                        if Instant::now() >= resume_time {
                            self.call_event_handler(Event::NewEvents(
                                StartCause::ResumeTimeReached {
                                    start: wait_start,
                                    requested_resume: resume_time,
                                },
                            ));
                            self.runner_state = RunnerState::HandlingEvents;
                            self.call_event_handler(Event::MainEventsCleared);
                            self.runner_state = RunnerState::HandlingRedraw;
                        }
                    }
                    // If we deferred a wait and no events were received, the user doesn't have to
                    // get an event.
                    ControlFlow::Wait | ControlFlow::Exit => (),
                }
            }
        }
    }

    fn redraw_events_cleared(&mut self) {
        match self.runner_state {
            // If we were handling redraws, send the RedrawEventsCleared message.
            RunnerState::HandlingRedraw => {
                self.call_event_handler(Event::RedrawEventsCleared);
                self.runner_state = RunnerState::Idle(Instant::now());
            }
            // No event was processed, we don't have to do anything.
            RunnerState::DeferredNewEvents(_) => (),
            // Should not happen.
            _ => warn!(
                "unexpected state in redraw_events_cleared: {:?}",
                self.runner_state
            ),
        }
    }

    fn call_event_handler(&mut self, event: Event<'_, T>) {
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
