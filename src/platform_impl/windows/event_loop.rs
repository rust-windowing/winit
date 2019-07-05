#![allow(non_snake_case)]
//! An events loop on Win32 is a background thread.
//!
//! Creating an events loop spawns a thread and blocks it in a permanent Win32 events loop.
//! Destroying the events loop stops the thread.
//!
//! You can use the `execute_in_thread` method to execute some code in the background thread.
//! Since Win32 requires you to create a window in the right thread, you must use this method
//! to create a window.
//!
//! If you create a window whose class is set to `callback`, the window's events will be
//! propagated with `run_forever` and `poll_events`.
//! The closure passed to the `execute_in_thread` method takes an `Inserter` that you can use to
//! add a `WindowState` entry to a list of window to be used by the callback.

use parking_lot::Mutex;
use std::{
    any::Any,
    cell::RefCell,
    collections::VecDeque,
    marker::PhantomData,
    mem, panic, ptr,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    time::{Duration, Instant},
};
use winapi::shared::basetsd::{DWORD_PTR, UINT_PTR};

use winapi::{
    ctypes::c_int,
    shared::{
        minwindef::{BOOL, DWORD, HIWORD, INT, LOWORD, LPARAM, LRESULT, UINT, WPARAM},
        windef::{HWND, POINT, RECT},
        windowsx, winerror,
    },
    um::{
        commctrl, libloaderapi, ole2, processthreadsapi, winbase,
        winnt::{LONG, LPCSTR, SHORT},
        winuser,
    },
};

use crate::{
    dpi::{LogicalPosition, LogicalSize, PhysicalSize},
    event::{DeviceEvent, Event, KeyboardInput, StartCause, Touch, TouchPhase, WindowEvent},
    event_loop::{ControlFlow, EventLoopClosed, EventLoopWindowTarget as RootELW},
    platform_impl::platform::{
        dpi::{
            become_dpi_aware, dpi_to_scale_factor, enable_non_client_dpi_scaling, hwnd_scale_factor,
        },
        drop_handler::FileDropHandler,
        event::{self, handle_extended_keys, process_key_params, vkey_to_winit_vkey},
        raw_input::{get_raw_input_data, get_raw_mouse_button_state},
        util,
        window::adjust_size,
        window_state::{CursorFlags, WindowFlags, WindowState},
        wrap_device_id, WindowId, DEVICE_ID,
    },
    window::WindowId as RootWindowId,
};

type GetPointerFrameInfoHistory = unsafe extern "system" fn(
    pointerId: UINT,
    entriesCount: *mut UINT,
    pointerCount: *mut UINT,
    pointerInfo: *mut winuser::POINTER_INFO,
) -> BOOL;

type SkipPointerFrameMessages = unsafe extern "system" fn(pointerId: UINT) -> BOOL;

lazy_static! {
    static ref GET_POINTER_FRAME_INFO_HISTORY: Option<GetPointerFrameInfoHistory> =
        get_function!("user32.dll", GetPointerFrameInfoHistory);
    static ref SKIP_POINTER_FRAME_MESSAGES: Option<SkipPointerFrameMessages> =
        get_function!("user32.dll", SkipPointerFrameMessages);
}

pub(crate) struct SubclassInput<T> {
    pub window_state: Arc<Mutex<WindowState>>,
    pub event_loop_runner: EventLoopRunnerShared<T>,
    pub file_drop_handler: FileDropHandler,
}

impl<T> SubclassInput<T> {
    unsafe fn send_event(&self, event: Event<T>) {
        self.event_loop_runner.send_event(event);
    }
}

struct ThreadMsgTargetSubclassInput<T> {
    event_loop_runner: EventLoopRunnerShared<T>,
    user_event_receiver: Receiver<T>,
}

impl<T> ThreadMsgTargetSubclassInput<T> {
    unsafe fn send_event(&self, event: Event<T>) {
        self.event_loop_runner.send_event(event);
    }
}

pub struct EventLoop<T: 'static> {
    thread_msg_sender: Sender<T>,
    window_target: RootELW<T>,
}

pub struct EventLoopWindowTarget<T> {
    thread_id: DWORD,
    trigger_newevents_on_redraw: Arc<AtomicBool>,
    thread_msg_target: HWND,
    pub(crate) runner_shared: EventLoopRunnerShared<T>,
}

impl<T: 'static> EventLoop<T> {
    pub fn new() -> EventLoop<T> {
        Self::with_dpi_awareness(true)
    }

    pub fn window_target(&self) -> &RootELW<T> {
        &self.window_target
    }

    pub fn with_dpi_awareness(dpi_aware: bool) -> EventLoop<T> {
        become_dpi_aware(dpi_aware);

        let thread_id = unsafe { processthreadsapi::GetCurrentThreadId() };
        let runner_shared = Rc::new(ELRShared {
            runner: RefCell::new(None),
            buffer: RefCell::new(VecDeque::new()),
        });
        let (thread_msg_target, thread_msg_sender) =
            thread_event_target_window(runner_shared.clone());

        EventLoop {
            thread_msg_sender,
            window_target: RootELW {
                p: EventLoopWindowTarget {
                    thread_id,
                    trigger_newevents_on_redraw: Arc::new(AtomicBool::new(true)),
                    thread_msg_target,
                    runner_shared,
                },
                _marker: PhantomData,
            },
        }
    }

    pub fn run<F>(mut self, event_handler: F) -> !
    where
        F: 'static + FnMut(Event<T>, &RootELW<T>, &mut ControlFlow),
    {
        self.run_return(event_handler);
        ::std::process::exit(0);
    }

    pub fn run_return<F>(&mut self, mut event_handler: F)
    where
        F: FnMut(Event<T>, &RootELW<T>, &mut ControlFlow),
    {
        unsafe {
            winuser::IsGUIThread(1);
        }

        let event_loop_windows_ref = &self.window_target;

        let mut runner = unsafe {
            EventLoopRunner::new(self, move |event, control_flow| {
                event_handler(event, event_loop_windows_ref, control_flow)
            })
        };
        {
            let runner_shared = self.window_target.p.runner_shared.clone();
            let mut runner_ref = runner_shared.runner.borrow_mut();
            loop {
                let event = runner_shared.buffer.borrow_mut().pop_front();
                match event {
                    Some(e) => {
                        runner.process_event(e);
                    }
                    None => break,
                }
            }
            *runner_ref = Some(runner);
        }

        macro_rules! runner {
            () => {
                self.window_target
                    .p
                    .runner_shared
                    .runner
                    .borrow_mut()
                    .as_mut()
                    .unwrap()
            };
        }

        unsafe {
            let mut msg = mem::zeroed();
            let mut msg_unprocessed = false;

            'main: loop {
                runner!().new_events();
                loop {
                    if !msg_unprocessed {
                        if 0 == winuser::PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, 1) {
                            break;
                        }
                    }
                    winuser::TranslateMessage(&mut msg);
                    winuser::DispatchMessageW(&mut msg);

                    msg_unprocessed = false;
                }
                runner!().events_cleared();
                if let Some(payload) = runner!().panic_error.take() {
                    panic::resume_unwind(payload);
                }

                if !msg_unprocessed {
                    let control_flow = runner!().control_flow;
                    match control_flow {
                        ControlFlow::Exit => break 'main,
                        ControlFlow::Wait => {
                            if 0 == winuser::GetMessageW(&mut msg, ptr::null_mut(), 0, 0) {
                                break 'main;
                            }
                            msg_unprocessed = true;
                        }
                        ControlFlow::WaitUntil(resume_time) => {
                            wait_until_time_or_msg(resume_time);
                        }
                        ControlFlow::Poll => (),
                    }
                }
            }
        }

        runner!().call_event_handler(Event::LoopDestroyed);
        *self.window_target.p.runner_shared.runner.borrow_mut() = None;
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy {
            target_window: self.window_target.p.thread_msg_target,
            event_send: self.thread_msg_sender.clone(),
        }
    }
}

impl<T> EventLoopWindowTarget<T> {
    #[inline(always)]
    pub(crate) fn create_thread_executor(&self) -> EventLoopThreadExecutor {
        EventLoopThreadExecutor {
            thread_id: self.thread_id,
            trigger_newevents_on_redraw: self.trigger_newevents_on_redraw.clone(),
            target_window: self.thread_msg_target,
        }
    }
}

pub(crate) type EventLoopRunnerShared<T> = Rc<ELRShared<T>>;
pub(crate) struct ELRShared<T> {
    runner: RefCell<Option<EventLoopRunner<T>>>,
    buffer: RefCell<VecDeque<Event<T>>>,
}
pub(crate) struct EventLoopRunner<T> {
    trigger_newevents_on_redraw: Arc<AtomicBool>,
    control_flow: ControlFlow,
    runner_state: RunnerState,
    modal_redraw_window: HWND,
    in_modal_loop: bool,
    in_repaint: bool,
    event_handler: Box<dyn FnMut(Event<T>, &mut ControlFlow)>,
    panic_error: Option<PanicError>,
}
type PanicError = Box<dyn Any + Send + 'static>;

impl<T> ELRShared<T> {
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
        self.buffer.borrow_mut().push_back(event)
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
}

impl<T> EventLoopRunner<T> {
    unsafe fn new<F>(event_loop: &EventLoop<T>, f: F) -> EventLoopRunner<T>
    where
        F: FnMut(Event<T>, &mut ControlFlow),
    {
        EventLoopRunner {
            trigger_newevents_on_redraw: event_loop
                .window_target
                .p
                .trigger_newevents_on_redraw
                .clone(),
            control_flow: ControlFlow::default(),
            runner_state: RunnerState::New,
            in_modal_loop: false,
            in_repaint: false,
            modal_redraw_window: event_loop.window_target.p.thread_msg_target,
            event_handler: mem::transmute::<
                Box<dyn FnMut(Event<T>, &mut ControlFlow)>,
                Box<dyn FnMut(Event<T>, &mut ControlFlow)>,
            >(Box::new(f)),
            panic_error: None,
        }
    }

    fn new_events(&mut self) {
        self.runner_state = match self.runner_state {
            // If we're already handling events or have deferred `NewEvents`, we don't need to do
            // do any processing.
            RunnerState::HandlingEvents | RunnerState::DeferredNewEvents(..) => self.runner_state,

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

        self.runner_state = RunnerState::HandlingEvents;
        match (self.in_repaint, &event) {
            (
                true,
                Event::WindowEvent {
                    event: WindowEvent::RedrawRequested,
                    ..
                },
            )
            | (false, _) => self.call_event_handler(event),
            (true, _) => {
                self.events_cleared();
                self.new_events();
                self.process_event(event);
            }
        }
    }

    fn events_cleared(&mut self) {
        self.in_repaint = false;

        match self.runner_state {
            // If we were handling events, send the EventsCleared message.
            RunnerState::HandlingEvents => {
                self.call_event_handler(Event::EventsCleared);
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
                        self.call_event_handler(Event::EventsCleared);
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
                            self.call_event_handler(Event::EventsCleared);
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
        match event {
            Event::NewEvents(_) => self
                .trigger_newevents_on_redraw
                .store(true, Ordering::Relaxed),
            Event::EventsCleared => self
                .trigger_newevents_on_redraw
                .store(false, Ordering::Relaxed),
            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                ..
            } => self.in_repaint = true,
            _ => (),
        }

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

// Returns true if the wait time was reached, and false if a message must be processed.
unsafe fn wait_until_time_or_msg(wait_until: Instant) -> bool {
    let mut msg = mem::zeroed();
    let now = Instant::now();
    if now <= wait_until {
        // MsgWaitForMultipleObjects tends to overshoot just a little bit. We subtract 1 millisecond
        // from the requested time and spinlock for the remainder to compensate for that.
        let resume_reason = winuser::MsgWaitForMultipleObjectsEx(
            0,
            ptr::null(),
            dur2timeout(wait_until - now).saturating_sub(1),
            winuser::QS_ALLEVENTS,
            winuser::MWMO_INPUTAVAILABLE,
        );

        if resume_reason == winerror::WAIT_TIMEOUT {
            while Instant::now() < wait_until {
                if 0 != winuser::PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, 0) {
                    return false;
                }
            }
        }
    }

    return true;
}
// Implementation taken from https://github.com/rust-lang/rust/blob/db5476571d9b27c862b95c1e64764b0ac8980e23/src/libstd/sys/windows/mod.rs
fn dur2timeout(dur: Duration) -> DWORD {
    // Note that a duration is a (u64, u32) (seconds, nanoseconds) pair, and the
    // timeouts in windows APIs are typically u32 milliseconds. To translate, we
    // have two pieces to take care of:
    //
    // * Nanosecond precision is rounded up
    // * Greater than u32::MAX milliseconds (50 days) is rounded up to INFINITE
    //   (never time out).
    dur.as_secs()
        .checked_mul(1000)
        .and_then(|ms| ms.checked_add((dur.subsec_nanos() as u64) / 1_000_000))
        .and_then(|ms| {
            ms.checked_add(if dur.subsec_nanos() % 1_000_000 > 0 {
                1
            } else {
                0
            })
        })
        .map(|ms| {
            if ms > DWORD::max_value() as u64 {
                winbase::INFINITE
            } else {
                ms as DWORD
            }
        })
        .unwrap_or(winbase::INFINITE)
}

impl<T> Drop for EventLoop<T> {
    fn drop(&mut self) {
        unsafe {
            winuser::DestroyWindow(self.window_target.p.thread_msg_target);
        }
    }
}

pub(crate) struct EventLoopThreadExecutor {
    thread_id: DWORD,
    trigger_newevents_on_redraw: Arc<AtomicBool>,
    target_window: HWND,
}

unsafe impl Send for EventLoopThreadExecutor {}
unsafe impl Sync for EventLoopThreadExecutor {}

impl EventLoopThreadExecutor {
    /// Check to see if we're in the parent event loop's thread.
    pub(super) fn in_event_loop_thread(&self) -> bool {
        let cur_thread_id = unsafe { processthreadsapi::GetCurrentThreadId() };
        self.thread_id == cur_thread_id
    }

    pub(super) fn trigger_newevents_on_redraw(&self) -> bool {
        !self.in_event_loop_thread() || self.trigger_newevents_on_redraw.load(Ordering::Relaxed)
    }

    /// Executes a function in the event loop thread. If we're already in the event loop thread,
    /// we just call the function directly.
    ///
    /// The `Inserted` can be used to inject a `WindowState` for the callback to use. The state is
    /// removed automatically if the callback receives a `WM_CLOSE` message for the window.
    ///
    /// Note that if you are using this to change some property of a window and updating
    /// `WindowState` then you should call this within the lock of `WindowState`. Otherwise the
    /// events may be sent to the other thread in different order to the one in which you set
    /// `WindowState`, leaving them out of sync.
    ///
    /// Note that we use a FnMut instead of a FnOnce because we're too lazy to create an equivalent
    /// to the unstable FnBox.
    pub(super) fn execute_in_thread<F>(&self, mut function: F)
    where
        F: FnMut() + Send + 'static,
    {
        unsafe {
            if self.in_event_loop_thread() {
                function();
            } else {
                // We double-box because the first box is a fat pointer.
                let boxed = Box::new(function) as Box<dyn FnMut()>;
                let boxed2: ThreadExecFn = Box::new(boxed);

                let raw = Box::into_raw(boxed2);

                let res = winuser::PostMessageW(
                    self.target_window,
                    *EXEC_MSG_ID,
                    raw as *mut () as usize as WPARAM,
                    0,
                );
                assert!(res != 0, "PostMessage failed ; is the messages queue full?");
            }
        }
    }
}

type ThreadExecFn = Box<Box<dyn FnMut()>>;

#[derive(Clone)]
pub struct EventLoopProxy<T: 'static> {
    target_window: HWND,
    event_send: Sender<T>,
}
unsafe impl<T: Send + 'static> Send for EventLoopProxy<T> {}

impl<T: 'static> EventLoopProxy<T> {
    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed> {
        unsafe {
            if winuser::PostMessageW(self.target_window, *USER_EVENT_MSG_ID, 0, 0) != 0 {
                self.event_send.send(event).ok();
                Ok(())
            } else {
                Err(EventLoopClosed)
            }
        }
    }
}

lazy_static! {
    // Message sent by the `EventLoopProxy` when we want to wake up the thread.
    // WPARAM and LPARAM are unused.
    static ref USER_EVENT_MSG_ID: u32 = {
        unsafe {
            winuser::RegisterWindowMessageA("Winit::WakeupMsg\0".as_ptr() as LPCSTR)
        }
    };
    // Message sent when we want to execute a closure in the thread.
    // WPARAM contains a Box<Box<dyn FnMut()>> that must be retrieved with `Box::from_raw`,
    // and LPARAM is unused.
    static ref EXEC_MSG_ID: u32 = {
        unsafe {
            winuser::RegisterWindowMessageA("Winit::ExecMsg\0".as_ptr() as *const i8)
        }
    };
    // Message sent by a `Window` when it wants to be destroyed by the main thread.
    // WPARAM and LPARAM are unused.
    pub static ref DESTROY_MSG_ID: u32 = {
        unsafe {
            winuser::RegisterWindowMessageA("Winit::DestroyMsg\0".as_ptr() as LPCSTR)
        }
    };
    // Message sent by a `Window` after creation if it has a DPI != 96.
    // WPARAM is the the DPI (u32). LOWORD of LPARAM is width, and HIWORD is height.
    pub static ref INITIAL_DPI_MSG_ID: u32 = {
        unsafe {
            winuser::RegisterWindowMessageA("Winit::InitialDpiMsg\0".as_ptr() as LPCSTR)
        }
    };
    // Message sent by a `Window` if it's requesting a redraw without sending a NewEvents.
    pub static ref REQUEST_REDRAW_NO_NEWEVENTS_MSG_ID: u32 = {
        unsafe {
            winuser::RegisterWindowMessageA("Winit::RequestRedrawNoNewevents\0".as_ptr() as LPCSTR)
        }
    };
    // WPARAM is a bool specifying the `WindowFlags::MARKER_RETAIN_STATE_ON_SIZE` flag. See the
    // documentation in the `window_state` module for more information.
    pub static ref SET_RETAIN_STATE_ON_SIZE_MSG_ID: u32 = unsafe {
        winuser::RegisterWindowMessageA("Winit::SetRetainMaximized\0".as_ptr() as LPCSTR)
    };
    static ref THREAD_EVENT_TARGET_WINDOW_CLASS: Vec<u16> = unsafe {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        let class_name: Vec<_> = OsStr::new("Winit Thread Event Target")
            .encode_wide()
            .chain(Some(0).into_iter())
            .collect();

        let class = winuser::WNDCLASSEXW {
            cbSize: mem::size_of::<winuser::WNDCLASSEXW>() as UINT,
            style: 0,
            lpfnWndProc: Some(winuser::DefWindowProcW),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: libloaderapi::GetModuleHandleW(ptr::null()),
            hIcon: ptr::null_mut(),
            hCursor: ptr::null_mut(), // must be null in order for cursor state to work properly
            hbrBackground: ptr::null_mut(),
            lpszMenuName: ptr::null(),
            lpszClassName: class_name.as_ptr(),
            hIconSm: ptr::null_mut(),
        };

        winuser::RegisterClassExW(&class);

        class_name
    };
}

fn thread_event_target_window<T>(event_loop_runner: EventLoopRunnerShared<T>) -> (HWND, Sender<T>) {
    unsafe {
        let window = winuser::CreateWindowExW(
            winuser::WS_EX_NOACTIVATE | winuser::WS_EX_TRANSPARENT | winuser::WS_EX_LAYERED,
            THREAD_EVENT_TARGET_WINDOW_CLASS.as_ptr(),
            ptr::null_mut(),
            0,
            0,
            0,
            0,
            0,
            ptr::null_mut(),
            ptr::null_mut(),
            libloaderapi::GetModuleHandleW(ptr::null()),
            ptr::null_mut(),
        );
        winuser::SetWindowLongPtrW(
            window,
            winuser::GWL_STYLE,
            // The window technically has to be visible to receive WM_PAINT messages (which are used
            // for delivering events during resizes), but it isn't displayed to the user because of
            // the LAYERED style.
            (winuser::WS_VISIBLE | winuser::WS_POPUP) as _,
        );

        let (tx, rx) = mpsc::channel();

        let subclass_input = ThreadMsgTargetSubclassInput {
            event_loop_runner,
            user_event_receiver: rx,
        };
        let input_ptr = Box::into_raw(Box::new(subclass_input));
        let subclass_result = commctrl::SetWindowSubclass(
            window,
            Some(thread_event_target_callback::<T>),
            THREAD_EVENT_TARGET_SUBCLASS_ID,
            input_ptr as DWORD_PTR,
        );
        assert_eq!(subclass_result, 1);

        (window, tx)
    }
}

/// Capture mouse input, allowing `window` to receive mouse events when the cursor is outside of
/// the window.
unsafe fn capture_mouse(window: HWND, window_state: &mut WindowState) {
    window_state.mouse.buttons_down += 1;
    winuser::SetCapture(window);
}

/// Release mouse input, stopping windows on this thread from receiving mouse input when the cursor
/// is outside the window.
unsafe fn release_mouse(window_state: &mut WindowState) {
    window_state.mouse.buttons_down = window_state.mouse.buttons_down.saturating_sub(1);
    if window_state.mouse.buttons_down == 0 {
        winuser::ReleaseCapture();
    }
}

const WINDOW_SUBCLASS_ID: UINT_PTR = 0;
const THREAD_EVENT_TARGET_SUBCLASS_ID: UINT_PTR = 1;
pub(crate) fn subclass_window<T>(window: HWND, subclass_input: SubclassInput<T>) {
    let input_ptr = Box::into_raw(Box::new(subclass_input));
    let subclass_result = unsafe {
        commctrl::SetWindowSubclass(
            window,
            Some(public_window_callback::<T>),
            WINDOW_SUBCLASS_ID,
            input_ptr as DWORD_PTR,
        )
    };
    assert_eq!(subclass_result, 1);
}

/// Any window whose callback is configured to this function will have its events propagated
/// through the events loop of the thread the window was created in.
//
// This is the callback that is called by `DispatchMessage` in the events loop.
//
// Returning 0 tells the Win32 API that the message has been processed.
// FIXME: detect WM_DWMCOMPOSITIONCHANGED and call DwmEnableBlurBehindWindow if necessary
unsafe extern "system" fn public_window_callback<T>(
    window: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
    _: UINT_PTR,
    subclass_input_ptr: DWORD_PTR,
) -> LRESULT {
    let subclass_input = &mut *(subclass_input_ptr as *mut SubclassInput<T>);

    match msg {
        winuser::WM_ENTERSIZEMOVE => {
            let mut runner = subclass_input.event_loop_runner.runner.borrow_mut();
            if let Some(ref mut runner) = *runner {
                runner.in_modal_loop = true;
            }
            0
        }
        winuser::WM_EXITSIZEMOVE => {
            let mut runner = subclass_input.event_loop_runner.runner.borrow_mut();
            if let Some(ref mut runner) = *runner {
                runner.in_modal_loop = false;
            }
            0
        }
        winuser::WM_NCCREATE => {
            enable_non_client_dpi_scaling(window);
            commctrl::DefSubclassProc(window, msg, wparam, lparam)
        }

        winuser::WM_NCLBUTTONDOWN => {
            // jumpstart the modal loop
            winuser::RedrawWindow(
                window,
                ptr::null(),
                ptr::null_mut(),
                winuser::RDW_INTERNALPAINT,
            );
            if wparam == winuser::HTCAPTION as _ {
                winuser::PostMessageW(window, winuser::WM_MOUSEMOVE, 0, 0);
            }
            commctrl::DefSubclassProc(window, msg, wparam, lparam)
        }

        winuser::WM_CLOSE => {
            use crate::event::WindowEvent::CloseRequested;
            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: CloseRequested,
            });
            0
        }

        winuser::WM_DESTROY => {
            use crate::event::WindowEvent::Destroyed;
            ole2::RevokeDragDrop(window);
            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: Destroyed,
            });

            Box::from_raw(subclass_input);
            drop(subclass_input);
            0
        }

        _ if msg == *REQUEST_REDRAW_NO_NEWEVENTS_MSG_ID => {
            use crate::event::WindowEvent::RedrawRequested;
            let mut runner = subclass_input.event_loop_runner.runner.borrow_mut();
            subclass_input.window_state.lock().queued_out_of_band_redraw = false;
            if let Some(ref mut runner) = *runner {
                // This check makes sure that calls to `request_redraw()` during `EventsCleared`
                // handling dispatch `RedrawRequested` immediately after `EventsCleared`, without
                // spinning up a new event loop iteration. We do this because that's what the API
                // says to do.
                let runner_state = runner.runner_state;
                let mut request_redraw = || {
                    runner.call_event_handler(Event::WindowEvent {
                        window_id: RootWindowId(WindowId(window)),
                        event: RedrawRequested,
                    });
                };
                match runner_state {
                    RunnerState::Idle(..) | RunnerState::DeferredNewEvents(..) => request_redraw(),
                    RunnerState::HandlingEvents => {
                        winuser::RedrawWindow(
                            window,
                            ptr::null(),
                            ptr::null_mut(),
                            winuser::RDW_INTERNALPAINT,
                        );
                    }
                    _ => (),
                }
            }
            0
        }
        winuser::WM_PAINT => {
            use crate::event::WindowEvent::RedrawRequested;
            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: RedrawRequested,
            });
            commctrl::DefSubclassProc(window, msg, wparam, lparam)
        }

        // WM_MOVE supplies client area positions, so we send Moved here instead.
        winuser::WM_WINDOWPOSCHANGED => {
            use crate::event::WindowEvent::Moved;

            let windowpos = lparam as *const winuser::WINDOWPOS;
            if (*windowpos).flags & winuser::SWP_NOMOVE != winuser::SWP_NOMOVE {
                let dpi_factor = hwnd_scale_factor(window);
                let logical_position =
                    LogicalPosition::from_physical(((*windowpos).x, (*windowpos).y), dpi_factor);
                subclass_input.send_event(Event::WindowEvent {
                    window_id: RootWindowId(WindowId(window)),
                    event: Moved(logical_position),
                });
            }

            // This is necessary for us to still get sent WM_SIZE.
            commctrl::DefSubclassProc(window, msg, wparam, lparam)
        }

        winuser::WM_SIZE => {
            use crate::event::WindowEvent::Resized;
            let w = LOWORD(lparam as DWORD) as u32;
            let h = HIWORD(lparam as DWORD) as u32;

            let dpi_factor = hwnd_scale_factor(window);
            let logical_size = LogicalSize::from_physical((w, h), dpi_factor);
            let event = Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: Resized(logical_size),
            };

            {
                let mut w = subclass_input.window_state.lock();
                // See WindowFlags::MARKER_RETAIN_STATE_ON_SIZE docs for info on why this `if` check exists.
                if !w
                    .window_flags()
                    .contains(WindowFlags::MARKER_RETAIN_STATE_ON_SIZE)
                {
                    let maximized = wparam == winuser::SIZE_MAXIMIZED;
                    w.set_window_flags_in_place(|f| f.set(WindowFlags::MAXIMIZED, maximized));
                }
            }

            subclass_input.send_event(event);
            0
        }

        winuser::WM_CHAR => {
            use crate::event::WindowEvent::ReceivedCharacter;
            let chr: char = mem::transmute(wparam as u32);
            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: ReceivedCharacter(chr),
            });
            0
        }

        // Prevents default windows menu hotkeys playing unwanted
        // "ding" sounds. Alternatively could check for WM_SYSCOMMAND
        // with wparam being SC_KEYMENU, but this may prevent some
        // other unwanted default hotkeys as well.
        winuser::WM_SYSCHAR => 0,

        winuser::WM_MOUSEMOVE => {
            use crate::event::WindowEvent::{CursorEntered, CursorMoved};
            let mouse_was_outside_window = {
                let mut w = subclass_input.window_state.lock();

                let was_outside_window = !w.mouse.cursor_flags().contains(CursorFlags::IN_WINDOW);
                w.mouse
                    .set_cursor_flags(window, |f| f.set(CursorFlags::IN_WINDOW, true))
                    .ok();
                was_outside_window
            };

            if mouse_was_outside_window {
                subclass_input.send_event(Event::WindowEvent {
                    window_id: RootWindowId(WindowId(window)),
                    event: CursorEntered {
                        device_id: DEVICE_ID,
                    },
                });

                // Calling TrackMouseEvent in order to receive mouse leave events.
                winuser::TrackMouseEvent(&mut winuser::TRACKMOUSEEVENT {
                    cbSize: mem::size_of::<winuser::TRACKMOUSEEVENT>() as DWORD,
                    dwFlags: winuser::TME_LEAVE,
                    hwndTrack: window,
                    dwHoverTime: winuser::HOVER_DEFAULT,
                });
            }

            let x = windowsx::GET_X_LPARAM(lparam) as f64;
            let y = windowsx::GET_Y_LPARAM(lparam) as f64;
            let dpi_factor = hwnd_scale_factor(window);
            let position = LogicalPosition::from_physical((x, y), dpi_factor);

            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: CursorMoved {
                    device_id: DEVICE_ID,
                    position,
                    modifiers: event::get_key_mods(),
                },
            });

            0
        }

        winuser::WM_MOUSELEAVE => {
            use crate::event::WindowEvent::CursorLeft;
            {
                let mut w = subclass_input.window_state.lock();
                w.mouse
                    .set_cursor_flags(window, |f| f.set(CursorFlags::IN_WINDOW, false))
                    .ok();
            }

            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: CursorLeft {
                    device_id: DEVICE_ID,
                },
            });

            0
        }

        winuser::WM_MOUSEWHEEL => {
            use crate::event::MouseScrollDelta::LineDelta;

            let value = (wparam >> 16) as i16;
            let value = value as i32;
            let value = value as f32 / winuser::WHEEL_DELTA as f32;

            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: WindowEvent::MouseWheel {
                    device_id: DEVICE_ID,
                    delta: LineDelta(0.0, value),
                    phase: TouchPhase::Moved,
                    modifiers: event::get_key_mods(),
                },
            });

            0
        }

        winuser::WM_MOUSEHWHEEL => {
            use crate::event::MouseScrollDelta::LineDelta;

            let value = (wparam >> 16) as i16;
            let value = value as i32;
            let value = value as f32 / winuser::WHEEL_DELTA as f32;

            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: WindowEvent::MouseWheel {
                    device_id: DEVICE_ID,
                    delta: LineDelta(value, 0.0),
                    phase: TouchPhase::Moved,
                    modifiers: event::get_key_mods(),
                },
            });

            0
        }

        winuser::WM_KEYDOWN | winuser::WM_SYSKEYDOWN => {
            use crate::event::{ElementState::Pressed, VirtualKeyCode};
            if msg == winuser::WM_SYSKEYDOWN && wparam as i32 == winuser::VK_F4 {
                commctrl::DefSubclassProc(window, msg, wparam, lparam)
            } else {
                if let Some((scancode, vkey)) = process_key_params(wparam, lparam) {
                    subclass_input.send_event(Event::WindowEvent {
                        window_id: RootWindowId(WindowId(window)),
                        event: WindowEvent::KeyboardInput {
                            device_id: DEVICE_ID,
                            input: KeyboardInput {
                                state: Pressed,
                                scancode,
                                virtual_keycode: vkey,
                                modifiers: event::get_key_mods(),
                            },
                        },
                    });
                    // Windows doesn't emit a delete character by default, but in order to make it
                    // consistent with the other platforms we'll emit a delete character here.
                    if vkey == Some(VirtualKeyCode::Delete) {
                        subclass_input.send_event(Event::WindowEvent {
                            window_id: RootWindowId(WindowId(window)),
                            event: WindowEvent::ReceivedCharacter('\u{7F}'),
                        });
                    }
                }
                0
            }
        }

        winuser::WM_KEYUP | winuser::WM_SYSKEYUP => {
            use crate::event::ElementState::Released;
            if let Some((scancode, vkey)) = process_key_params(wparam, lparam) {
                subclass_input.send_event(Event::WindowEvent {
                    window_id: RootWindowId(WindowId(window)),
                    event: WindowEvent::KeyboardInput {
                        device_id: DEVICE_ID,
                        input: KeyboardInput {
                            state: Released,
                            scancode,
                            virtual_keycode: vkey,
                            modifiers: event::get_key_mods(),
                        },
                    },
                });
            }
            0
        }

        winuser::WM_LBUTTONDOWN => {
            use crate::event::{ElementState::Pressed, MouseButton::Left, WindowEvent::MouseInput};

            capture_mouse(window, &mut *subclass_input.window_state.lock());

            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: MouseInput {
                    device_id: DEVICE_ID,
                    state: Pressed,
                    button: Left,
                    modifiers: event::get_key_mods(),
                },
            });
            0
        }

        winuser::WM_LBUTTONUP => {
            use crate::event::{
                ElementState::Released, MouseButton::Left, WindowEvent::MouseInput,
            };

            release_mouse(&mut *subclass_input.window_state.lock());

            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: MouseInput {
                    device_id: DEVICE_ID,
                    state: Released,
                    button: Left,
                    modifiers: event::get_key_mods(),
                },
            });
            0
        }

        winuser::WM_RBUTTONDOWN => {
            use crate::event::{
                ElementState::Pressed, MouseButton::Right, WindowEvent::MouseInput,
            };

            capture_mouse(window, &mut *subclass_input.window_state.lock());

            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: MouseInput {
                    device_id: DEVICE_ID,
                    state: Pressed,
                    button: Right,
                    modifiers: event::get_key_mods(),
                },
            });
            0
        }

        winuser::WM_RBUTTONUP => {
            use crate::event::{
                ElementState::Released, MouseButton::Right, WindowEvent::MouseInput,
            };

            release_mouse(&mut *subclass_input.window_state.lock());

            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: MouseInput {
                    device_id: DEVICE_ID,
                    state: Released,
                    button: Right,
                    modifiers: event::get_key_mods(),
                },
            });
            0
        }

        winuser::WM_MBUTTONDOWN => {
            use crate::event::{
                ElementState::Pressed, MouseButton::Middle, WindowEvent::MouseInput,
            };

            capture_mouse(window, &mut *subclass_input.window_state.lock());

            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: MouseInput {
                    device_id: DEVICE_ID,
                    state: Pressed,
                    button: Middle,
                    modifiers: event::get_key_mods(),
                },
            });
            0
        }

        winuser::WM_MBUTTONUP => {
            use crate::event::{
                ElementState::Released, MouseButton::Middle, WindowEvent::MouseInput,
            };

            release_mouse(&mut *subclass_input.window_state.lock());

            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: MouseInput {
                    device_id: DEVICE_ID,
                    state: Released,
                    button: Middle,
                    modifiers: event::get_key_mods(),
                },
            });
            0
        }

        winuser::WM_XBUTTONDOWN => {
            use crate::event::{
                ElementState::Pressed, MouseButton::Other, WindowEvent::MouseInput,
            };
            let xbutton = winuser::GET_XBUTTON_WPARAM(wparam);

            capture_mouse(window, &mut *subclass_input.window_state.lock());

            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: MouseInput {
                    device_id: DEVICE_ID,
                    state: Pressed,
                    button: Other(xbutton as u8),
                    modifiers: event::get_key_mods(),
                },
            });
            0
        }

        winuser::WM_XBUTTONUP => {
            use crate::event::{
                ElementState::Released, MouseButton::Other, WindowEvent::MouseInput,
            };
            let xbutton = winuser::GET_XBUTTON_WPARAM(wparam);

            release_mouse(&mut *subclass_input.window_state.lock());

            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: MouseInput {
                    device_id: DEVICE_ID,
                    state: Released,
                    button: Other(xbutton as u8),
                    modifiers: event::get_key_mods(),
                },
            });
            0
        }

        winuser::WM_INPUT_DEVICE_CHANGE => {
            let event = match wparam as _ {
                winuser::GIDC_ARRIVAL => DeviceEvent::Added,
                winuser::GIDC_REMOVAL => DeviceEvent::Removed,
                _ => unreachable!(),
            };

            subclass_input.send_event(Event::DeviceEvent {
                device_id: wrap_device_id(lparam as _),
                event,
            });

            0
        }

        winuser::WM_INPUT => {
            use crate::event::{
                DeviceEvent::{Button, Key, Motion, MouseMotion, MouseWheel},
                ElementState::{Pressed, Released},
                MouseScrollDelta::LineDelta,
            };

            if let Some(data) = get_raw_input_data(lparam as _) {
                let device_id = wrap_device_id(data.header.hDevice as _);

                if data.header.dwType == winuser::RIM_TYPEMOUSE {
                    let mouse = data.data.mouse();

                    if util::has_flag(mouse.usFlags, winuser::MOUSE_MOVE_RELATIVE) {
                        let x = mouse.lLastX as f64;
                        let y = mouse.lLastY as f64;

                        if x != 0.0 {
                            subclass_input.send_event(Event::DeviceEvent {
                                device_id,
                                event: Motion { axis: 0, value: x },
                            });
                        }

                        if y != 0.0 {
                            subclass_input.send_event(Event::DeviceEvent {
                                device_id,
                                event: Motion { axis: 1, value: y },
                            });
                        }

                        if x != 0.0 || y != 0.0 {
                            subclass_input.send_event(Event::DeviceEvent {
                                device_id,
                                event: MouseMotion { delta: (x, y) },
                            });
                        }
                    }

                    if util::has_flag(mouse.usButtonFlags, winuser::RI_MOUSE_WHEEL) {
                        let delta = mouse.usButtonData as SHORT / winuser::WHEEL_DELTA;
                        subclass_input.send_event(Event::DeviceEvent {
                            device_id,
                            event: MouseWheel {
                                delta: LineDelta(0.0, delta as f32),
                            },
                        });
                    }

                    let button_state = get_raw_mouse_button_state(mouse.usButtonFlags);
                    // Left, middle, and right, respectively.
                    for (index, state) in button_state.iter().enumerate() {
                        if let Some(state) = *state {
                            // This gives us consistency with X11, since there doesn't
                            // seem to be anything else reasonable to do for a mouse
                            // button ID.
                            let button = (index + 1) as _;
                            subclass_input.send_event(Event::DeviceEvent {
                                device_id,
                                event: Button { button, state },
                            });
                        }
                    }
                } else if data.header.dwType == winuser::RIM_TYPEKEYBOARD {
                    let keyboard = data.data.keyboard();

                    let pressed = keyboard.Message == winuser::WM_KEYDOWN
                        || keyboard.Message == winuser::WM_SYSKEYDOWN;
                    let released = keyboard.Message == winuser::WM_KEYUP
                        || keyboard.Message == winuser::WM_SYSKEYUP;

                    if pressed || released {
                        let state = if pressed { Pressed } else { Released };

                        let scancode = keyboard.MakeCode as _;
                        let extended = util::has_flag(keyboard.Flags, winuser::RI_KEY_E0 as _)
                            | util::has_flag(keyboard.Flags, winuser::RI_KEY_E1 as _);
                        if let Some((vkey, scancode)) =
                            handle_extended_keys(keyboard.VKey as _, scancode, extended)
                        {
                            let virtual_keycode = vkey_to_winit_vkey(vkey);

                            subclass_input.send_event(Event::DeviceEvent {
                                device_id,
                                event: Key(KeyboardInput {
                                    scancode,
                                    state,
                                    virtual_keycode,
                                    modifiers: event::get_key_mods(),
                                }),
                            });
                        }
                    }
                }
            }

            commctrl::DefSubclassProc(window, msg, wparam, lparam)
        }

        winuser::WM_TOUCH => {
            let pcount = LOWORD(wparam as DWORD) as usize;
            let mut inputs = Vec::with_capacity(pcount);
            inputs.set_len(pcount);
            let htouch = lparam as winuser::HTOUCHINPUT;
            if winuser::GetTouchInputInfo(
                htouch,
                pcount as UINT,
                inputs.as_mut_ptr(),
                mem::size_of::<winuser::TOUCHINPUT>() as INT,
            ) > 0
            {
                let dpi_factor = hwnd_scale_factor(window);
                for input in &inputs {
                    let x = (input.x as f64) / 100f64;
                    let y = (input.y as f64) / 100f64;
                    let location = LogicalPosition::from_physical((x, y), dpi_factor);
                    subclass_input.send_event(Event::WindowEvent {
                        window_id: RootWindowId(WindowId(window)),
                        event: WindowEvent::Touch(Touch {
                            phase: if input.dwFlags & winuser::TOUCHEVENTF_DOWN != 0 {
                                TouchPhase::Started
                            } else if input.dwFlags & winuser::TOUCHEVENTF_UP != 0 {
                                TouchPhase::Ended
                            } else if input.dwFlags & winuser::TOUCHEVENTF_MOVE != 0 {
                                TouchPhase::Moved
                            } else {
                                continue;
                            },
                            location,
                            id: input.dwID as u64,
                            device_id: DEVICE_ID,
                        }),
                    });
                }
            }
            winuser::CloseTouchInputHandle(htouch);
            0
        }

        winuser::WM_POINTERDOWN | winuser::WM_POINTERUPDATE | winuser::WM_POINTERUP => {
            if let (Some(GetPointerFrameInfoHistory), Some(SkipPointerFrameMessages)) = (
                *GET_POINTER_FRAME_INFO_HISTORY,
                *SKIP_POINTER_FRAME_MESSAGES,
            ) {
                let pointer_id = LOWORD(wparam as DWORD) as UINT;
                let mut entries_count = 0 as UINT;
                let mut pointers_count = 0 as UINT;
                if GetPointerFrameInfoHistory(
                    pointer_id,
                    &mut entries_count as *mut _,
                    &mut pointers_count as *mut _,
                    std::ptr::null_mut(),
                ) == 0
                {
                    return 0;
                }

                let pointer_info_count = (entries_count * pointers_count) as usize;
                let mut pointer_infos = Vec::with_capacity(pointer_info_count);
                pointer_infos.set_len(pointer_info_count);
                if GetPointerFrameInfoHistory(
                    pointer_id,
                    &mut entries_count as *mut _,
                    &mut pointers_count as *mut _,
                    pointer_infos.as_mut_ptr(),
                ) == 0
                {
                    return 0;
                }

                let dpi_factor = hwnd_scale_factor(window);
                // https://docs.microsoft.com/en-us/windows/desktop/api/winuser/nf-winuser-getpointerframeinfohistory
                // The information retrieved appears in reverse chronological order, with the most recent entry in the first
                // row of the returned array
                for pointer_info in pointer_infos.iter().rev() {
                    let x = pointer_info.ptPixelLocation.x as f64;
                    let y = pointer_info.ptPixelLocation.y as f64;
                    let location = LogicalPosition::from_physical((x, y), dpi_factor);
                    subclass_input.send_event(Event::WindowEvent {
                        window_id: RootWindowId(WindowId(window)),
                        event: WindowEvent::Touch(Touch {
                            phase: if pointer_info.pointerFlags & winuser::POINTER_FLAG_DOWN != 0 {
                                TouchPhase::Started
                            } else if pointer_info.pointerFlags & winuser::POINTER_FLAG_UP != 0 {
                                TouchPhase::Ended
                            } else if pointer_info.pointerFlags & winuser::POINTER_FLAG_UPDATE != 0
                            {
                                TouchPhase::Moved
                            } else {
                                continue;
                            },
                            location,
                            id: pointer_info.pointerId as u64,
                            device_id: DEVICE_ID,
                        }),
                    });
                }

                SkipPointerFrameMessages(pointer_id);
            }
            0
        }

        winuser::WM_SETFOCUS => {
            use crate::event::WindowEvent::Focused;
            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: Focused(true),
            });

            0
        }

        winuser::WM_KILLFOCUS => {
            use crate::event::WindowEvent::Focused;
            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: Focused(false),
            });
            0
        }

        winuser::WM_SETCURSOR => {
            let set_cursor_to = {
                let window_state = subclass_input.window_state.lock();
                if window_state
                    .mouse
                    .cursor_flags()
                    .contains(CursorFlags::IN_WINDOW)
                {
                    Some(window_state.mouse.cursor)
                } else {
                    None
                }
            };

            match set_cursor_to {
                Some(cursor) => {
                    let cursor = winuser::LoadCursorW(ptr::null_mut(), cursor.to_windows_cursor());
                    winuser::SetCursor(cursor);
                    0
                }
                None => winuser::DefWindowProcW(window, msg, wparam, lparam),
            }
        }

        winuser::WM_DROPFILES => {
            // See `FileDropHandler` for implementation.
            0
        }

        winuser::WM_GETMINMAXINFO => {
            let mmi = lparam as *mut winuser::MINMAXINFO;

            let window_state = subclass_input.window_state.lock();

            if window_state.min_size.is_some() || window_state.max_size.is_some() {
                let style = winuser::GetWindowLongA(window, winuser::GWL_STYLE) as DWORD;
                let ex_style = winuser::GetWindowLongA(window, winuser::GWL_EXSTYLE) as DWORD;
                if let Some(min_size) = window_state.min_size {
                    let min_size = min_size.to_physical(window_state.dpi_factor);
                    let (width, height) = adjust_size(min_size, style, ex_style);
                    (*mmi).ptMinTrackSize = POINT {
                        x: width as i32,
                        y: height as i32,
                    };
                }
                if let Some(max_size) = window_state.max_size {
                    let max_size = max_size.to_physical(window_state.dpi_factor);
                    let (width, height) = adjust_size(max_size, style, ex_style);
                    (*mmi).ptMaxTrackSize = POINT {
                        x: width as i32,
                        y: height as i32,
                    };
                }
            }

            0
        }

        // Only sent on Windows 8.1 or newer. On Windows 7 and older user has to log out to change
        // DPI, therefore all applications are closed while DPI is changing.
        winuser::WM_DPICHANGED => {
            use crate::event::WindowEvent::HiDpiFactorChanged;

            // This message actually provides two DPI values - x and y. However MSDN says that
            // "you only need to use either the X-axis or the Y-axis value when scaling your
            // application since they are the same".
            // https://msdn.microsoft.com/en-us/library/windows/desktop/dn312083(v=vs.85).aspx
            let new_dpi_x = u32::from(LOWORD(wparam as DWORD));
            let new_dpi_factor = dpi_to_scale_factor(new_dpi_x);

            let allow_resize = {
                let mut window_state = subclass_input.window_state.lock();
                let old_dpi_factor = window_state.dpi_factor;
                window_state.dpi_factor = new_dpi_factor;

                new_dpi_factor != old_dpi_factor && window_state.fullscreen.is_none()
            };

            // This prevents us from re-applying DPI adjustment to the restored size after exiting
            // fullscreen (the restored size is already DPI adjusted).
            if allow_resize {
                // Resize window to the size suggested by Windows.
                let rect = &*(lparam as *const RECT);
                winuser::SetWindowPos(
                    window,
                    ptr::null_mut(),
                    rect.left,
                    rect.top,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    winuser::SWP_NOZORDER | winuser::SWP_NOACTIVATE,
                );
            }

            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: HiDpiFactorChanged(new_dpi_factor),
            });

            0
        }

        _ => {
            if msg == *DESTROY_MSG_ID {
                winuser::DestroyWindow(window);
                0
            } else if msg == *SET_RETAIN_STATE_ON_SIZE_MSG_ID {
                let mut window_state = subclass_input.window_state.lock();
                window_state.set_window_flags_in_place(|f| {
                    f.set(WindowFlags::MARKER_RETAIN_STATE_ON_SIZE, wparam != 0)
                });
                0
            } else if msg == *INITIAL_DPI_MSG_ID {
                use crate::event::WindowEvent::HiDpiFactorChanged;
                let scale_factor = dpi_to_scale_factor(wparam as u32);
                subclass_input.send_event(Event::WindowEvent {
                    window_id: RootWindowId(WindowId(window)),
                    event: HiDpiFactorChanged(scale_factor),
                });
                // Automatically resize for actual DPI
                let width = LOWORD(lparam as DWORD) as u32;
                let height = HIWORD(lparam as DWORD) as u32;
                let (adjusted_width, adjusted_height): (u32, u32) =
                    PhysicalSize::from_logical((width, height), scale_factor).into();
                // We're not done yet! `SetWindowPos` needs the window size, not the client area size.
                let mut rect = RECT {
                    top: 0,
                    left: 0,
                    bottom: adjusted_height as LONG,
                    right: adjusted_width as LONG,
                };
                let dw_style = winuser::GetWindowLongA(window, winuser::GWL_STYLE) as DWORD;
                let b_menu = !winuser::GetMenu(window).is_null() as BOOL;
                let dw_style_ex = winuser::GetWindowLongA(window, winuser::GWL_EXSTYLE) as DWORD;
                winuser::AdjustWindowRectEx(&mut rect, dw_style, b_menu, dw_style_ex);
                let outer_x = (rect.right - rect.left).abs() as c_int;
                let outer_y = (rect.top - rect.bottom).abs() as c_int;
                winuser::SetWindowPos(
                    window,
                    ptr::null_mut(),
                    0,
                    0,
                    outer_x,
                    outer_y,
                    winuser::SWP_NOMOVE
                        | winuser::SWP_NOREPOSITION
                        | winuser::SWP_NOZORDER
                        | winuser::SWP_NOACTIVATE,
                );
                0
            } else {
                commctrl::DefSubclassProc(window, msg, wparam, lparam)
            }
        }
    }
}

unsafe extern "system" fn thread_event_target_callback<T>(
    window: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
    _: UINT_PTR,
    subclass_input_ptr: DWORD_PTR,
) -> LRESULT {
    let subclass_input = &mut *(subclass_input_ptr as *mut ThreadMsgTargetSubclassInput<T>);
    match msg {
        winuser::WM_DESTROY => {
            Box::from_raw(subclass_input);
            drop(subclass_input);
            0
        }
        // Because WM_PAINT comes after all other messages, we use it during modal loops to detect
        // when the event queue has been emptied. See `process_event` for more details.
        winuser::WM_PAINT => {
            winuser::ValidateRect(window, ptr::null());
            let queue_call_again = || {
                winuser::RedrawWindow(
                    window,
                    ptr::null(),
                    ptr::null_mut(),
                    winuser::RDW_INTERNALPAINT,
                );
            };
            let in_modal_loop = {
                let runner = subclass_input.event_loop_runner.runner.borrow_mut();
                if let Some(ref runner) = *runner {
                    runner.in_modal_loop
                } else {
                    false
                }
            };
            if in_modal_loop {
                let mut msg = mem::zeroed();
                loop {
                    if 0 == winuser::PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, 0) {
                        break;
                    }
                    // Clear all paint/timer messages from the queue before sending the events cleared message.
                    match msg.message {
                        // Flush the event queue of WM_PAINT messages.
                        winuser::WM_PAINT | winuser::WM_TIMER => {
                            // Remove the message from the message queue.
                            winuser::PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, 1);

                            if msg.hwnd != window {
                                winuser::TranslateMessage(&mut msg);
                                winuser::DispatchMessageW(&mut msg);
                            }
                        }
                        // If the message isn't one of those three, it may be handled by the modal
                        // loop so we should return control flow to it.
                        _ => {
                            queue_call_again();
                            return 0;
                        }
                    }
                }

                let mut runner = subclass_input.event_loop_runner.runner.borrow_mut();
                if let Some(ref mut runner) = *runner {
                    runner.events_cleared();
                    match runner.control_flow {
                        // Waiting is handled by the modal loop.
                        ControlFlow::Exit | ControlFlow::Wait => runner.new_events(),
                        ControlFlow::WaitUntil(resume_time) => {
                            wait_until_time_or_msg(resume_time);
                            runner.new_events();
                            queue_call_again();
                        }
                        ControlFlow::Poll => {
                            runner.new_events();
                            queue_call_again();
                        }
                    }
                }
            }
            0
        }
        _ if msg == *USER_EVENT_MSG_ID => {
            if let Ok(event) = subclass_input.user_event_receiver.recv() {
                subclass_input.send_event(Event::UserEvent(event));
            }
            0
        }
        _ if msg == *EXEC_MSG_ID => {
            let mut function: ThreadExecFn = Box::from_raw(wparam as usize as *mut _);
            function();
            0
        }
        _ => commctrl::DefSubclassProc(window, msg, wparam, lparam),
    }
}
