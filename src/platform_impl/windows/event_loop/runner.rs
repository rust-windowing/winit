use std::{any::Any, cell::{Cell, RefCell}, collections::{HashSet, VecDeque}, mem, ptr, panic, rc::Rc, time::Instant};

use winapi::{um::winuser, shared::{minwindef::DWORD, windef::HWND}};

use crate::{
    dpi::PhysicalSize,
    event::{Event, StartCause, WindowEvent},
    event_loop::ControlFlow,
    platform_impl::platform::util,
    window::WindowId,
};

pub(crate) type EventLoopRunnerShared<T> = Rc<ELRShared<T>>;
pub(crate) struct ELRShared<T: 'static> {
    thread_msg_target: HWND,
    wait_thread_id: DWORD,
    processing_events: Cell<ProcessingEvents>,
    panic_error: Cell<Option<PanicError>>,
    control_flow: Cell<ControlFlow>,
    last_events_cleared: Cell<Instant>,
    event_handler: Cell<Option<Box<dyn FnMut(Event<'_, T>, &mut ControlFlow)>>>,
    event_buffer: RefCell<VecDeque<BufferedEvent<T>>>,
    owned_windows: Cell<HashSet<HWND>>,
}

pub type PanicError = Box<dyn Any + Send + 'static>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ProcessingEvents {
    Uninitialized,
    NoEvents,
    MainEvents,
    RedrawEvents,
}

enum BufferedEvent<T: 'static> {
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
    pub(crate) fn new(thread_msg_target: HWND, wait_thread_id: DWORD) -> ELRShared<T> {
        ELRShared {
            thread_msg_target,
            wait_thread_id,
            processing_events: Cell::new(ProcessingEvents::Uninitialized),
            control_flow: Cell::new(ControlFlow::Poll),
            panic_error: Cell::new(None),
            last_events_cleared: Cell::new(Instant::now()),
            event_handler: Cell::new(None),
            event_buffer: RefCell::new(VecDeque::new()),
            owned_windows: Cell::new(HashSet::new())
        }
    }

    pub fn thread_msg_target(&self) -> HWND {
        self.thread_msg_target
    }

    pub fn wait_thread_id(&self) -> DWORD {
        self.wait_thread_id
    }

    pub(crate) unsafe fn set_event_handler<F>(&self, f: F)
    where
        F: FnMut(Event<'_, T>, &mut ControlFlow),
    {
        let old_event_handler = self.event_handler.replace(
            mem::transmute::<
                Option<Box<dyn FnMut(Event<'_, T>, &mut ControlFlow)>>,
                Option<Box<dyn FnMut(Event<'_, T>, &mut ControlFlow)>>,
            >(Some(Box::new(f))),
        );
        assert!(old_event_handler.is_none());
    }

    pub(crate) fn reset_runner(&self) {
        let ELRShared {
            thread_msg_target: _,
            wait_thread_id: _,
            processing_events,
            panic_error,
            control_flow,
            last_events_cleared: _,
            event_handler,
            event_buffer: _,
            owned_windows: _,
        } = self;
        processing_events.set(ProcessingEvents::Uninitialized);
        panic_error.set(None);
        control_flow.set(ControlFlow::Poll);
        event_handler.set(None);
    }

    pub(crate) unsafe fn poll(&self) {
        self.move_state_to(ProcessingEvents::MainEvents);
    }

    pub(crate) unsafe fn send_event(&self, event: Event<'_, T>) {
        if let Event::RedrawRequested(_) = event {
            self.move_state_to(ProcessingEvents::RedrawEvents);
            self.call_event_handler(event);
        } else {
            if self.should_buffer() {
                // If the runner is already borrowed, we're in the middle of an event loop invocation. Add
                // the event to a buffer to be processed later.
                self.event_buffer
                    .borrow_mut()
                    .push_back(BufferedEvent::from_event(event))
            } else {
                self.move_state_to(ProcessingEvents::MainEvents);
                self.call_event_handler(event);
                self.dispatch_buffered_events();
            }
        }
    }

    pub(crate) unsafe fn main_events_cleared(&self) {
        self.move_state_to(ProcessingEvents::RedrawEvents);
    }

    pub(crate) unsafe fn redraw_events_cleared(&self) {
        self.move_state_to(ProcessingEvents::NoEvents);
    }

    pub fn redrawing(&self) -> bool {
        self.processing_events.get() == ProcessingEvents::RedrawEvents
    }

    pub(crate) unsafe fn call_event_handler(&self, event: Event<'_, T>) {
        let mut panic_error = self.panic_error.take();
        if panic_error.is_none() {
            let mut control_flow = self.control_flow.take();
            let mut event_handler = self.event_handler.take()
                .expect("either event handler is re-entrant (likely), or no event handler is registered (very unlikely)");

            panic_error = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                if control_flow != ControlFlow::Exit {
                    event_handler(event, &mut control_flow);
                } else {
                    event_handler(event, &mut ControlFlow::Exit);
                }
            })).err();

            assert!(self.event_handler.replace(Some(event_handler)).is_none());
            self.control_flow.set(control_flow);
        }
        self.panic_error.set(panic_error);
    }

    pub(crate) fn take_panic_error(&self) -> Result<(), PanicError> {
        match self.panic_error.take() {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    pub fn register_window(&self, window: HWND) {
        let mut owned_windows = self.owned_windows.take();
        owned_windows.insert(window);
        self.owned_windows.set(owned_windows);
    }

    pub fn remove_window(&self, window: HWND) {
        let mut owned_windows = self.owned_windows.take();
        owned_windows.remove(&window);
        self.owned_windows.set(owned_windows);
    }

    pub fn control_flow(&self) -> ControlFlow {
        self.control_flow.get()
    }

    pub fn handling_events(&self) -> bool {
        self.processing_events.get() != ProcessingEvents::NoEvents
    }

    pub fn owned_windows(&self, mut f: impl FnMut(HWND)) {
        let mut owned_windows = self.owned_windows.take();
        for hwnd in &owned_windows {
            f(*hwnd);
        }
        let new_owned_windows = self.owned_windows.take();
        owned_windows.extend(&new_owned_windows);
        self.owned_windows.set(owned_windows);
    }

    pub fn should_buffer(&self) -> bool {
        let handler = self.event_handler.take();
        let should_buffer = handler.is_none();
        self.event_handler.set(handler);
        should_buffer
    }

    unsafe fn dispatch_buffered_events(&self) {
        loop {
            // We do this instead of using a `while let` loop because if we use a `while let`
            // loop the reference returned `borrow_mut()` doesn't get dropped until the end
            // of the loop's body and attempts to add events to the event buffer while in
            // `process_event` will fail.
            let buffered_event_opt = self.event_buffer.borrow_mut().pop_front();
            match buffered_event_opt {
                Some(e) => e.dispatch_event(|e| self.call_event_handler(e)),
                None => break,
            }
        }
    }

    unsafe fn move_state_to(&self, processing_events: ProcessingEvents) {
        use ProcessingEvents::{Uninitialized, NoEvents, MainEvents, RedrawEvents};

        let probably_wrong = || warn!("Given winit's current design, the fact that this branch is getting hit \
                                       is probably indicates a bug. Please open an issue at \
                                       https://github.com/rust-windowing/winit");

        match (self.processing_events.replace(processing_events), processing_events) {
            (Uninitialized, Uninitialized) |
            (NoEvents, NoEvents) |
            (MainEvents, MainEvents) |
            (RedrawEvents, RedrawEvents) => (),

            (Uninitialized, MainEvents) => {
                self.call_new_events(true);
                self.call_event_handler(Event::NewEvents(StartCause::Init));
            },
            (Uninitialized, RedrawEvents) => {
                self.call_new_events(true);
                self.call_event_handler(Event::MainEventsCleared);
            },
            (Uninitialized, NoEvents) => {
                self.call_new_events(true);
                self.call_event_handler(Event::MainEventsCleared);
                self.call_event_handler(Event::RedrawEventsCleared);
            },
            (_, Uninitialized) => panic!("cannot move state to Uninitialized"),

            (NoEvents, MainEvents) => {
                self.call_new_events(false);
            },
            (NoEvents, RedrawEvents) => {
                self.call_new_events(false);
                self.call_event_handler(Event::MainEventsCleared);
            },
            (MainEvents, RedrawEvents) => {
                self.call_event_handler(Event::MainEventsCleared);
            },
            (MainEvents, NoEvents) => {
                probably_wrong();
                self.call_event_handler(Event::MainEventsCleared);
                self.call_event_handler(Event::RedrawEventsCleared);
            },
            (RedrawEvents, NoEvents) => {
                self.call_event_handler(Event::RedrawEventsCleared);
            },
            (RedrawEvents, MainEvents) => {
                probably_wrong();
                self.call_event_handler(Event::RedrawEventsCleared);
                self.call_new_events(false);
            }
        }
    }

    unsafe fn call_new_events(&self, init: bool) {
        let start_cause = match (init, self.control_flow()) {
            (true, _) => StartCause::Init,
            (false, ControlFlow::Poll) => StartCause::Poll,
            (false, ControlFlow::Exit) |
            (false, ControlFlow::Wait) => StartCause::WaitCancelled{ requested_resume: None, start: self.last_events_cleared.get() },
            (false, ControlFlow::WaitUntil(requested_resume)) => StartCause::WaitCancelled{ requested_resume: Some(requested_resume), start: self.last_events_cleared.get() },
        };
        self.call_event_handler(Event::NewEvents(start_cause));
        self.dispatch_buffered_events();
        winuser::RedrawWindow(self.thread_msg_target, ptr::null(), ptr::null_mut(), winuser::RDW_INTERNALPAINT);
    }
}
