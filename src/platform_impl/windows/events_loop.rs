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

use winapi::shared::basetsd::DWORD_PTR;
use winapi::shared::basetsd::UINT_PTR;
use std::{mem, ptr};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use std::rc::Rc;
use std::cell::RefCell;
use parking_lot::Mutex;
use crossbeam_channel::{self, Sender, Receiver};

use winapi::ctypes::c_int;
use winapi::shared::minwindef::{
    BOOL,
    DWORD,
    HIWORD,
    INT,
    LOWORD,
    LPARAM,
    LRESULT,
    UINT,
    WPARAM,
};
use winapi::shared::windef::{HWND, POINT, RECT};
use winapi::shared::windowsx;
use winapi::um::{winuser, winbase, ole2, processthreadsapi, commctrl, libloaderapi};
use winapi::um::winnt::{LONG, LPCSTR, SHORT};

use {
    ControlFlow,
    Event,
    EventLoopClosed,
    KeyboardInput,
    LogicalPosition,
    LogicalSize,
    PhysicalSize,
    WindowEvent,
    WindowId as SuperWindowId,
};
use events::{DeviceEvent, Touch, TouchPhase, StartCause};
use platform_impl::platform::{event, Cursor, WindowId, DEVICE_ID, wrap_device_id, util};
use platform_impl::platform::dpi::{
    become_dpi_aware,
    dpi_to_scale_factor,
    enable_non_client_dpi_scaling,
    get_hwnd_scale_factor,
};
use platform_impl::platform::drop_handler::FileDropHandler;
use platform_impl::platform::event::{handle_extended_keys, process_key_params, vkey_to_winit_vkey};
use platform_impl::platform::icon::WinIcon;
use platform_impl::platform::raw_input::{get_raw_input_data, get_raw_mouse_button_state};
use platform_impl::platform::window::adjust_size;

/// Contains saved window info for switching between fullscreen
#[derive(Clone)]
pub struct SavedWindowInfo {
    /// Window style
    pub style: LONG,
    /// Window ex-style
    pub ex_style: LONG,
    /// Window position and size
    pub rect: RECT,
    // Since a window can be fullscreened to a different monitor, a DPI change can be triggered. This could result in
    // the window being automitcally resized to smaller/larger than it was supposed to be restored to, so we thus must
    // check if the post-fullscreen DPI matches the pre-fullscreen DPI.
    pub is_fullscreen: bool,
    pub dpi_factor: Option<f64>,
}

/// Contains information about states and the window that the callback is going to use.
#[derive(Clone)]
pub struct WindowState {
    /// Cursor to set at the next `WM_SETCURSOR` event received.
    pub cursor: Cursor,
    pub cursor_grabbed: bool,
    pub cursor_hidden: bool,
    /// Used by `WM_GETMINMAXINFO`.
    pub max_size: Option<PhysicalSize>,
    pub min_size: Option<PhysicalSize>,
    /// Will contain `true` if the mouse is hovering the window.
    pub mouse_in_window: bool,
    /// Saved window info for fullscreen restored
    pub saved_window_info: Option<SavedWindowInfo>,
    // This is different from the value in `SavedWindowInfo`! That one represents the DPI saved upon entering
    // fullscreen. This will always be the most recent DPI for the window.
    pub dpi_factor: f64,
    pub fullscreen: Option<::MonitorId>,
    pub window_icon: Option<WinIcon>,
    pub taskbar_icon: Option<WinIcon>,
    pub decorations: bool,
    pub always_on_top: bool,
    pub maximized: bool,
    pub resizable: bool,
    pub mouse_buttons_down: u32,
    pub modal_timer_handle: UINT_PTR
}

impl WindowState {
    pub fn update_min_max(&mut self, old_dpi_factor: f64, new_dpi_factor: f64) {
        let scale_factor = new_dpi_factor / old_dpi_factor;
        let dpi_adjuster = |mut physical_size: PhysicalSize| -> PhysicalSize {
            physical_size.width *= scale_factor;
            physical_size.height *= scale_factor;
            physical_size
        };
        self.max_size = self.max_size.map(&dpi_adjuster);
        self.min_size = self.min_size.map(&dpi_adjuster);
    }
}

pub(crate) struct SubclassInput<T> {
    pub window_state: Arc<Mutex<WindowState>>,
    pub event_loop_runner: EventLoopRunnerShared<T>,
    pub file_drop_handler: FileDropHandler,
}

impl<T> SubclassInput<T> {
    unsafe fn send_event(&self, event: Event<T>) {
        let mut runner = self.event_loop_runner.borrow_mut();
        match *runner {
            ELRSharedOption::Runner(runner) => (*runner).process_event(event),
            ELRSharedOption::Buffer(ref mut buffer) => buffer.push(event)
        }
    }
}

struct ThreadMsgTargetSubclassInput<T> {
    event_loop_runner: EventLoopRunnerShared<T>,
    user_event_receiver: Receiver<T>
}

impl<T> ThreadMsgTargetSubclassInput<T> {
    unsafe fn send_event(&self, event: Event<T>) {
        let mut runner = self.event_loop_runner.borrow_mut();
        match *runner {
            ELRSharedOption::Runner(runner) => (*runner).process_event(event),
            ELRSharedOption::Buffer(ref mut buffer) => buffer.push(event)
        }
    }
}

pub struct EventLoop<T> {
    // Id of the background thread from the Win32 API.
    thread_id: DWORD,
    thread_msg_target: HWND,
    thread_msg_sender: Sender<T>,
    trigger_newevents_on_redraw: Arc<AtomicBool>,
    pub(crate) runner_shared: EventLoopRunnerShared<T>,
}

impl<T> EventLoop<T> {
    pub fn new() -> EventLoop<T> {
        Self::with_dpi_awareness(true)
    }

    pub fn with_dpi_awareness(dpi_aware: bool) -> EventLoop<T> {
        become_dpi_aware(dpi_aware);

        let thread_id = unsafe { processthreadsapi::GetCurrentThreadId() };
        let runner_shared = Rc::new(RefCell::new(ELRSharedOption::Buffer(vec![])));
        let (thread_msg_target, thread_msg_sender) = thread_event_target_window(runner_shared.clone());

        EventLoop {
            thread_id,
            thread_msg_target, thread_msg_sender,
            trigger_newevents_on_redraw: Arc::new(AtomicBool::new(true)),
            runner_shared
        }
    }

    pub fn run<F>(self, mut event_handler: F) -> !
        where F: 'static + FnMut(Event<T>, &::EventLoop<T>, &mut ControlFlow)
    {
        unsafe {
            winuser::IsGUIThread(1);

            let mut runner = EventLoopRunner {
                event_loop: ::EventLoop {
                    events_loop: self,
                    _marker: ::std::marker::PhantomData
                },
                control_flow: ControlFlow::default(),
                runner_state: RunnerState::New,
                modal_loop_data: None,
                event_handler: &mut event_handler
            };
            {
                let runner_shared = runner.event_loop.events_loop.runner_shared.clone();
                let mut runner_shared = runner_shared.borrow_mut();
                let mut event_buffer = vec![];
                if let ELRSharedOption::Buffer(ref mut buffer) = *runner_shared {
                    mem::swap(buffer, &mut event_buffer);
                }
                for event in event_buffer.drain(..) {
                    runner.process_event(event);
                }
                *runner_shared = ELRSharedOption::Runner(&mut runner);
            }

            let timer_handle = winuser::SetTimer(ptr::null_mut(), 0, 0x7FFFFFFF, None);

            let mut msg = mem::uninitialized();
            let mut msg_unprocessed = false;

            'main: loop {
                runner.new_events();
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
                runner.events_cleared();

                match runner.control_flow {
                    ControlFlow::Exit => break 'main,
                    ControlFlow::Wait => {
                        if 0 == winuser::GetMessageW(&mut msg, ptr::null_mut(), 0, 0) {
                            break 'main
                        }
                        msg_unprocessed = true;
                    }
                    ControlFlow::WaitUntil(resume_time) => {
                        let now = Instant::now();
                        if now <= resume_time {
                            let duration = resume_time - now;
                            winuser::SetTimer(ptr::null_mut(), timer_handle, dur2timeout(duration), None);
                            if 0 == winuser::GetMessageW(&mut msg, ptr::null_mut(), 0, 0) {
                                break 'main
                            }
                            winuser::SetTimer(ptr::null_mut(), timer_handle, 0x7FFFFFFF, None);
                            msg_unprocessed = true;
                        }
                    },
                    ControlFlow::Poll => ()
                }
            }

            runner.call_event_handler(Event::LoopDestroyed);
            *runner.event_loop.events_loop.runner_shared.borrow_mut() = ELRSharedOption::Buffer(vec![]);
        }

        drop(event_handler);
        ::std::process::exit(0);
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy {
            target_window: self.thread_msg_target,
            event_send: self.thread_msg_sender.clone()
        }
    }

    #[inline(always)]
    pub(crate) fn create_thread_executor(&self) -> EventLoopThreadExecutor {
        EventLoopThreadExecutor {
            thread_id: self.thread_id,
            trigger_newevents_on_redraw: self.trigger_newevents_on_redraw.clone()
        }
    }
}

pub(crate) type EventLoopRunnerShared<T> = Rc<RefCell<ELRSharedOption<T>>>;
pub(crate) enum ELRSharedOption<T> {
    Runner(*mut EventLoopRunner<T>),
    Buffer(Vec<Event<T>>)
}
pub(crate) struct EventLoopRunner<T> {
    event_loop: ::EventLoop<T>,
    control_flow: ControlFlow,
    runner_state: RunnerState,
    modal_loop_data: Option<ModalLoopData>,
    event_handler: *mut FnMut(Event<T>, &::EventLoop<T>, &mut ControlFlow)
}

struct ModalLoopData {
    hwnd: HWND,
    timer_handle: UINT_PTR
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
    unsafe fn new_events(&mut self) {
        self.runner_state = match self.runner_state {
            // If we're already handling events or have deferred `NewEvents`, we don't need to do
            // do any processing.
            RunnerState::HandlingEvents |
            RunnerState::DeferredNewEvents(..) => self.runner_state,

            // Send the `Init` `NewEvents` and immediately move into event processing.
            RunnerState::New => {
                self.call_event_handler(Event::NewEvents(StartCause::Init));
                RunnerState::HandlingEvents
            },

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
                                    requested_resume: resume_time
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

    unsafe fn process_event(&mut self, event: Event<T>) {
        // If we're in the middle of a modal loop, only set the timer for zero if it hasn't been
        // reset in a prior call to `process_event`.
        if let Some(ModalLoopData{hwnd, timer_handle}) = self.modal_loop_data {
            if self.runner_state != RunnerState::HandlingEvents {
                winuser::SetTimer(hwnd, timer_handle, 0, None);
            }
        }

        // If new event processing has to be done (i.e. call NewEvents or defer), do it. If we're
        // already in processing nothing happens with this call.
        self.new_events();

        // Now that an event has been received, we have to send any `NewEvents` calls that were
        // deferred.
        if let RunnerState::DeferredNewEvents(wait_start) = self.runner_state {
            match self.control_flow {
                ControlFlow::Wait => self.call_event_handler(
                    Event::NewEvents(StartCause::WaitCancelled {
                        start: wait_start,
                        requested_resume: None
                    })
                ),
                ControlFlow::WaitUntil(resume_time) => {
                    let start_cause = match Instant::now() >= resume_time {
                        // If the current time is later than the requested resume time, the resume time
                        // has been reached.
                        true => StartCause::ResumeTimeReached {
                            start: wait_start,
                            requested_resume: resume_time
                        },
                        // Otherwise, the requested resume time HASN'T been reached and we send a WaitCancelled.
                        false => StartCause::WaitCancelled {
                            start: wait_start,
                            requested_resume: Some(resume_time)
                        },
                    };
                    self.call_event_handler(Event::NewEvents(start_cause));
                },
                ControlFlow::Poll |
                ControlFlow::Exit => unreachable!()
            }
        }

        self.runner_state = RunnerState::HandlingEvents;
        self.call_event_handler(event);
    }

    unsafe fn events_cleared(&mut self) {
        match self.runner_state {
            // If we were handling events, send the EventsCleared message.
            RunnerState::HandlingEvents => {
                self.call_event_handler(Event::EventsCleared);
                self.runner_state = RunnerState::Idle(Instant::now());
            },

            // If we *weren't* handling events, we don't have to do anything.
            RunnerState::New |
            RunnerState::Idle(..) => (),

            // Some control flows require a NewEvents call even if no events were received. This
            // branch handles those.
            RunnerState::DeferredNewEvents(wait_start) => {
                match self.control_flow {
                    // If we had deferred a Poll, send the Poll NewEvents and EventsCleared.
                    ControlFlow::Poll => {
                        self.call_event_handler(Event::NewEvents(StartCause::Poll));
                        self.call_event_handler(Event::EventsCleared);
                    },
                    // If we had deferred a WaitUntil and the resume time has since been reached,
                    // send the resume notification and EventsCleared event.
                    ControlFlow::WaitUntil(resume_time) => {
                        if Instant::now() >= resume_time {
                            self.call_event_handler(Event::NewEvents(StartCause::ResumeTimeReached {
                                start: wait_start,
                                requested_resume: resume_time
                            }));
                            self.call_event_handler(Event::EventsCleared);
                        }
                    },
                    // If we deferred a wait and no events were received, the user doesn't have to
                    // get an event.
                    ControlFlow::Wait |
                    ControlFlow::Exit => ()
                }
                // Mark that we've entered an idle state.
                self.runner_state = RunnerState::Idle(wait_start)
            },
        }
    }

    unsafe fn call_event_handler(&mut self, event: Event<T>) {
        if self.event_handler != mem::zeroed() {
            match event {
                Event::NewEvents(_) => self.event_loop.events_loop.trigger_newevents_on_redraw.store(true, Ordering::Relaxed),
                Event::EventsCleared => self.event_loop.events_loop.trigger_newevents_on_redraw.store(false, Ordering::Relaxed),
                _ => ()
            }

            if self.control_flow != ControlFlow::Exit {
                (*self.event_handler)(event, &self.event_loop, &mut self.control_flow);
            } else {
                (*self.event_handler)(event, &self.event_loop, &mut ControlFlow::Exit);
            }
        } else {
            panic!("Tried to call event handler with null handler");
        }
    }
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
    dur.as_secs().checked_mul(1000).and_then(|ms| {
        ms.checked_add((dur.subsec_nanos() as u64) / 1_000_000)
    }).and_then(|ms| {
        ms.checked_add(if dur.subsec_nanos() % 1_000_000 > 0 {1} else {0})
    }).map(|ms| {
        if ms > DWORD::max_value() as u64 {
            winbase::INFINITE
        } else {
            ms as DWORD
        }
    }).unwrap_or(winbase::INFINITE)
}

impl<T> Drop for EventLoop<T> {
    fn drop(&mut self) {
        unsafe {
            winuser::DestroyWindow(self.thread_msg_target);
            // Posting `WM_QUIT` will cause `GetMessage` to stop.
            winuser::PostThreadMessageA(self.thread_id, winuser::WM_QUIT, 0, 0);
        }
    }
}

pub(crate) struct EventLoopThreadExecutor {
    thread_id: DWORD,
    trigger_newevents_on_redraw: Arc<AtomicBool>
}

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
        where F: FnMut() + Send + 'static
    {
        unsafe {
            if self.in_event_loop_thread() {
                function();
            } else {
                // We double-box because the first box is a fat pointer.
                let boxed = Box::new(function) as Box<FnMut()>;
                let boxed2 = Box::new(boxed);

                let raw = Box::into_raw(boxed2);

                let res = winuser::PostThreadMessageA(self.thread_id, *EXEC_MSG_ID,
                                                     raw as *mut () as usize as WPARAM, 0);
                // PostThreadMessage can only fail if the thread ID is invalid (which shouldn't happen
                // as the events loop is still alive) or if the queue is full.
                assert!(res != 0, "PostThreadMessage failed ; is the messages queue full?");
            }
        }
    }
}

#[derive(Clone)]
pub struct EventLoopProxy<T> {
    target_window: HWND,
    event_send: Sender<T>
}
unsafe impl<T: Send> Send for EventLoopProxy<T> {}

impl<T> EventLoopProxy<T> {
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
    // WPARAM contains a Box<Box<FnMut()>> that must be retrieved with `Box::from_raw`,
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
            0,
            THREAD_EVENT_TARGET_WINDOW_CLASS.as_ptr(),
            ptr::null_mut(),
            0,
            0, 0,
            0, 0,
            ptr::null_mut(),
            ptr::null_mut(),
            libloaderapi::GetModuleHandleW(ptr::null()),
            ptr::null_mut()
        );

        let (tx, rx) = crossbeam_channel::unbounded();

        let subclass_input = ThreadMsgTargetSubclassInput {
            event_loop_runner,
            user_event_receiver: rx
        };
        let input_ptr = Box::into_raw(Box::new(subclass_input));
        let subclass_result = commctrl::SetWindowSubclass(
            window,
            Some(thread_event_target_callback::<T>),
            THREAD_EVENT_TARGET_SUBCLASS_ID,
            input_ptr as DWORD_PTR
        );
        assert_eq!(subclass_result, 1);

        (window, tx)
    }
}

/// Capture mouse input, allowing `window` to receive mouse events when the cursor is outside of
/// the window.
unsafe fn capture_mouse(window: HWND, window_state: &mut WindowState) {
    window_state.mouse_buttons_down += 1;
    winuser::SetCapture(window);
}

/// Release mouse input, stopping windows on this thread from receiving mouse input when the cursor
/// is outside the window.
unsafe fn release_mouse(window_state: &mut WindowState) {
    window_state.mouse_buttons_down = window_state.mouse_buttons_down.saturating_sub(1);
    if window_state.mouse_buttons_down == 0 {
        winuser::ReleaseCapture();
    }
}

const WINDOW_SUBCLASS_ID: UINT_PTR = 0;
const THREAD_EVENT_TARGET_SUBCLASS_ID: UINT_PTR = 1;
pub(crate) fn subclass_window<T>(window: HWND, subclass_input: SubclassInput<T>) {
    let input_ptr = Box::into_raw(Box::new(subclass_input));
    let subclass_result = unsafe{ commctrl::SetWindowSubclass(
        window,
        Some(public_window_callback::<T>),
        WINDOW_SUBCLASS_ID,
        input_ptr as DWORD_PTR
    ) };
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
    subclass_input_ptr: DWORD_PTR
) -> LRESULT {
    let subclass_input = &mut*(subclass_input_ptr as *mut SubclassInput<T>);

    match msg {
        winuser::WM_SYSCOMMAND => {
            {
                let mut window_state = subclass_input.window_state.lock();
                if window_state.modal_timer_handle == 0 {
                    window_state.modal_timer_handle = winuser::SetTimer(window, 0, 0x7FFFFFFF, None);
                }
            }
            commctrl::DefSubclassProc(window, msg, wparam, lparam)
        }
        winuser::WM_ENTERSIZEMOVE => {
            let modal_timer_handle = subclass_input.window_state.lock().modal_timer_handle;
            if let ELRSharedOption::Runner(runner) = *subclass_input.event_loop_runner.borrow_mut() {
                (*runner).modal_loop_data = Some(ModalLoopData {
                    hwnd: window,
                    timer_handle: modal_timer_handle
                });
            }
            winuser::SetTimer(window, modal_timer_handle, 0, None);
            0
        },
        winuser::WM_EXITSIZEMOVE => {
            let modal_timer_handle = subclass_input.window_state.lock().modal_timer_handle;
            if let ELRSharedOption::Runner(runner) = *subclass_input.event_loop_runner.borrow_mut() {
                (*runner).modal_loop_data = None;
            }
            winuser::SetTimer(window, modal_timer_handle, 0x7FFFFFFF, None);
            0
        },
        winuser::WM_TIMER => {
            let modal_timer_handle = subclass_input.window_state.lock().modal_timer_handle;
            if wparam == modal_timer_handle {
                let runner = subclass_input.event_loop_runner.borrow_mut();
                if let ELRSharedOption::Runner(runner) = *runner {
                    let runner = &mut *runner;
                    if runner.modal_loop_data.is_some() {
                        runner.events_cleared();
                        match runner.control_flow {
                            ControlFlow::Exit => (),
                            ControlFlow::Wait => {
                                winuser::SetTimer(window, modal_timer_handle, 0x7FFFFFFF, None);
                            },
                            ControlFlow::WaitUntil(resume_time) => {
                                let now = Instant::now();
                                let duration = match now <= resume_time {
                                    true => dur2timeout(resume_time - now),
                                    false => 0
                                };
                                winuser::SetTimer(window, modal_timer_handle, duration, None);
                            },
                            ControlFlow::Poll => {
                                winuser::SetTimer(window, modal_timer_handle, 0, None);
                            }
                        }

                        runner.new_events();
                    }
                }
            }
            0
        }

        winuser::WM_NCCREATE => {
            enable_non_client_dpi_scaling(window);
            commctrl::DefSubclassProc(window, msg, wparam, lparam)
        },

        winuser::WM_CLOSE => {
            use events::WindowEvent::CloseRequested;
            subclass_input.send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: CloseRequested
            });
            0
        },

        winuser::WM_DESTROY => {
            use events::WindowEvent::Destroyed;
            ole2::RevokeDragDrop(window);
            {
                let window_state = subclass_input.window_state.lock();
                if window_state.modal_timer_handle != 0 {
                    winuser::KillTimer(window, window_state.modal_timer_handle);
                }
            }
            subclass_input.send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: Destroyed
            });

            Box::from_raw(subclass_input);
            drop(subclass_input);
            0
        },

        _ if msg == *REQUEST_REDRAW_NO_NEWEVENTS_MSG_ID => {
            use events::WindowEvent::RedrawRequested;
            let runner = subclass_input.event_loop_runner.borrow_mut();
            if let ELRSharedOption::Runner(runner) = *runner {
                let runner = &mut *runner;
                match runner.runner_state {
                    RunnerState::Idle(..) |
                    RunnerState::DeferredNewEvents(..) => runner.call_event_handler(Event::WindowEvent {
                        window_id: SuperWindowId(WindowId(window)),
                        event: RedrawRequested,
                    }),
                    _ => ()
                }
            }
            0
        },
        winuser::WM_PAINT => {
            use events::WindowEvent::RedrawRequested;
            let event = || Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: RedrawRequested,
            };

            let mut send_event = false;
            {
                let runner = subclass_input.event_loop_runner.borrow_mut();
                if let ELRSharedOption::Runner(runner) = *runner {
                    let runner = &mut *runner;
                    match runner.runner_state {
                        RunnerState::Idle(..) |
                        RunnerState::DeferredNewEvents(..) => runner.call_event_handler(event()),
                        _ => send_event = true
                    }
                }
            }
            if send_event {
                subclass_input.send_event(event());
            }
            commctrl::DefSubclassProc(window, msg, wparam, lparam)
        },

        // WM_MOVE supplies client area positions, so we send Moved here instead.
        winuser::WM_WINDOWPOSCHANGED => {
            use events::WindowEvent::Moved;

            let windowpos = lparam as *const winuser::WINDOWPOS;
            if (*windowpos).flags & winuser::SWP_NOMOVE != winuser::SWP_NOMOVE {
                let dpi_factor = get_hwnd_scale_factor(window);
                let logical_position = LogicalPosition::from_physical(
                    ((*windowpos).x, (*windowpos).y),
                    dpi_factor,
                );
                subclass_input.send_event(Event::WindowEvent {
                    window_id: SuperWindowId(WindowId(window)),
                    event: Moved(logical_position),
                });
            }

            // This is necessary for us to still get sent WM_SIZE.
            commctrl::DefSubclassProc(window, msg, wparam, lparam)
        },

        winuser::WM_SIZE => {
            use events::WindowEvent::Resized;
            let w = LOWORD(lparam as DWORD) as u32;
            let h = HIWORD(lparam as DWORD) as u32;

            let dpi_factor = get_hwnd_scale_factor(window);
            let logical_size = LogicalSize::from_physical((w, h), dpi_factor);
            let event = Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: Resized(logical_size),
            };

            subclass_input.send_event(event);
            0
        },

        winuser::WM_CHAR => {
            use std::mem;
            use events::WindowEvent::ReceivedCharacter;
            let chr: char = mem::transmute(wparam as u32);
            subclass_input.send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: ReceivedCharacter(chr),
            });
            0
        },

        // Prevents default windows menu hotkeys playing unwanted
        // "ding" sounds. Alternatively could check for WM_SYSCOMMAND
        // with wparam being SC_KEYMENU, but this may prevent some
        // other unwanted default hotkeys as well.
        winuser::WM_SYSCHAR => {
            0
        }

        winuser::WM_MOUSEMOVE => {
            use events::WindowEvent::{CursorEntered, CursorMoved};
            let mouse_outside_window = {
                let mut window = subclass_input.window_state.lock();
                if !window.mouse_in_window {
                    window.mouse_in_window = true;
                    true
                } else {
                    false
                }
            };

            if mouse_outside_window {
                subclass_input.send_event(Event::WindowEvent {
                    window_id: SuperWindowId(WindowId(window)),
                    event: CursorEntered { device_id: DEVICE_ID },
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
            let dpi_factor = get_hwnd_scale_factor(window);
            let position = LogicalPosition::from_physical((x, y), dpi_factor);

            subclass_input.send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: CursorMoved { device_id: DEVICE_ID, position, modifiers: event::get_key_mods() },
            });

            0
        },

        winuser::WM_MOUSELEAVE => {
            use events::WindowEvent::CursorLeft;
            let mouse_in_window = {
                let mut window = subclass_input.window_state.lock();
                if window.mouse_in_window {
                    window.mouse_in_window = false;
                    true
                } else {
                    false
                }
            };

            if mouse_in_window {
                subclass_input.send_event(Event::WindowEvent {
                    window_id: SuperWindowId(WindowId(window)),
                    event: CursorLeft { device_id: DEVICE_ID }
                });
            }

            0
        },

        winuser::WM_MOUSEWHEEL => {
            use events::MouseScrollDelta::LineDelta;
            use events::TouchPhase;

            let value = (wparam >> 16) as i16;
            let value = value as i32;
            let value = value as f32 / winuser::WHEEL_DELTA as f32;

            subclass_input.send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: WindowEvent::MouseWheel { device_id: DEVICE_ID, delta: LineDelta(0.0, value), phase: TouchPhase::Moved, modifiers: event::get_key_mods() },
            });

            0
        },

        winuser::WM_KEYDOWN | winuser::WM_SYSKEYDOWN => {
            use events::ElementState::Pressed;
            use events::VirtualKeyCode;
            if msg == winuser::WM_SYSKEYDOWN && wparam as i32 == winuser::VK_F4 {
                commctrl::DefSubclassProc(window, msg, wparam, lparam)
            } else {
                if let Some((scancode, vkey)) = process_key_params(wparam, lparam) {
                    subclass_input.send_event(Event::WindowEvent {
                        window_id: SuperWindowId(WindowId(window)),
                        event: WindowEvent::KeyboardInput {
                            device_id: DEVICE_ID,
                            input: KeyboardInput {
                                state: Pressed,
                                scancode: scancode,
                                virtual_keycode: vkey,
                                modifiers: event::get_key_mods(),
                            }
                        }
                    });
                    // Windows doesn't emit a delete character by default, but in order to make it
                    // consistent with the other platforms we'll emit a delete character here.
                    if vkey == Some(VirtualKeyCode::Delete) {
                        subclass_input.send_event(Event::WindowEvent {
                            window_id: SuperWindowId(WindowId(window)),
                            event: WindowEvent::ReceivedCharacter('\u{7F}'),
                        });
                    }
                }
                0
            }
        },

        winuser::WM_KEYUP | winuser::WM_SYSKEYUP => {
            use events::ElementState::Released;
            if let Some((scancode, vkey)) = process_key_params(wparam, lparam) {
                subclass_input.send_event(Event::WindowEvent {
                    window_id: SuperWindowId(WindowId(window)),
                    event: WindowEvent::KeyboardInput {
                        device_id: DEVICE_ID,
                        input: KeyboardInput {
                            state: Released,
                            scancode: scancode,
                            virtual_keycode: vkey,
                            modifiers: event::get_key_mods(),
                        },
                    }
                });
            }
            0
        },

        winuser::WM_LBUTTONDOWN => {
            use events::WindowEvent::MouseInput;
            use events::MouseButton::Left;
            use events::ElementState::Pressed;

            capture_mouse(window, &mut *subclass_input.window_state.lock());

            subclass_input.send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: MouseInput { device_id: DEVICE_ID, state: Pressed, button: Left, modifiers: event::get_key_mods() }
            });
            0
        },

        winuser::WM_LBUTTONUP => {
            use events::WindowEvent::MouseInput;
            use events::MouseButton::Left;
            use events::ElementState::Released;

            release_mouse(&mut *subclass_input.window_state.lock());

            subclass_input.send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: MouseInput { device_id: DEVICE_ID, state: Released, button: Left, modifiers: event::get_key_mods() }
            });
            0
        },

        winuser::WM_RBUTTONDOWN => {
            use events::WindowEvent::MouseInput;
            use events::MouseButton::Right;
            use events::ElementState::Pressed;

            capture_mouse(window, &mut *subclass_input.window_state.lock());

            subclass_input.send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: MouseInput { device_id: DEVICE_ID, state: Pressed, button: Right, modifiers: event::get_key_mods() }
            });
            0
        },

        winuser::WM_RBUTTONUP => {
            use events::WindowEvent::MouseInput;
            use events::MouseButton::Right;
            use events::ElementState::Released;

            release_mouse(&mut *subclass_input.window_state.lock());

            subclass_input.send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: MouseInput { device_id: DEVICE_ID, state: Released, button: Right, modifiers: event::get_key_mods() }
            });
            0
        },

        winuser::WM_MBUTTONDOWN => {
            use events::WindowEvent::MouseInput;
            use events::MouseButton::Middle;
            use events::ElementState::Pressed;

            capture_mouse(window, &mut *subclass_input.window_state.lock());

            subclass_input.send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: MouseInput { device_id: DEVICE_ID, state: Pressed, button: Middle, modifiers: event::get_key_mods() }
            });
            0
        },

        winuser::WM_MBUTTONUP => {
            use events::WindowEvent::MouseInput;
            use events::MouseButton::Middle;
            use events::ElementState::Released;

            release_mouse(&mut *subclass_input.window_state.lock());

            subclass_input.send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: MouseInput { device_id: DEVICE_ID, state: Released, button: Middle, modifiers: event::get_key_mods() }
            });
            0
        },

        winuser::WM_XBUTTONDOWN => {
            use events::WindowEvent::MouseInput;
            use events::MouseButton::Other;
            use events::ElementState::Pressed;
            let xbutton = winuser::GET_XBUTTON_WPARAM(wparam);

            capture_mouse(window, &mut *subclass_input.window_state.lock());

            subclass_input.send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: MouseInput { device_id: DEVICE_ID, state: Pressed, button: Other(xbutton as u8), modifiers: event::get_key_mods() }
            });
            0
        },

        winuser::WM_XBUTTONUP => {
            use events::WindowEvent::MouseInput;
            use events::MouseButton::Other;
            use events::ElementState::Released;
            let xbutton = winuser::GET_XBUTTON_WPARAM(wparam);

            release_mouse(&mut *subclass_input.window_state.lock());

            subclass_input.send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: MouseInput { device_id: DEVICE_ID, state: Released, button: Other(xbutton as u8), modifiers: event::get_key_mods() }
            });
            0
        },

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

            commctrl::DefSubclassProc(window, msg, wparam, lparam)
        },

        winuser::WM_INPUT => {
            use events::DeviceEvent::{Motion, MouseMotion, MouseWheel, Button, Key};
            use events::MouseScrollDelta::LineDelta;
            use events::ElementState::{Pressed, Released};

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
                                event: Motion { axis: 0, value: x }
                            });
                        }

                        if y != 0.0 {
                            subclass_input.send_event(Event::DeviceEvent {
                                device_id,
                                event: Motion { axis: 1, value: y }
                            });
                        }

                        if x != 0.0 || y != 0.0 {
                            subclass_input.send_event(Event::DeviceEvent {
                                device_id,
                                event: MouseMotion { delta: (x, y) }
                            });
                        }
                    }

                    if util::has_flag(mouse.usButtonFlags, winuser::RI_MOUSE_WHEEL) {
                        let delta = mouse.usButtonData as SHORT / winuser::WHEEL_DELTA;
                        subclass_input.send_event(Event::DeviceEvent {
                            device_id,
                            event: MouseWheel { delta: LineDelta(0.0, delta as f32) }
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
                                event: Button {
                                    button,
                                    state,
                                }
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
                        let state = if pressed {
                            Pressed
                        } else {
                            Released
                        };

                        let scancode = keyboard.MakeCode as _;
                        let extended = util::has_flag(keyboard.Flags, winuser::RI_KEY_E0 as _)
                            | util::has_flag(keyboard.Flags, winuser::RI_KEY_E1 as _);
                        if let Some((vkey, scancode)) = handle_extended_keys(
                            keyboard.VKey as _,
                            scancode,
                            extended,
                        ) {
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
        },

        winuser::WM_TOUCH => {
            let pcount = LOWORD( wparam as DWORD ) as usize;
            let mut inputs = Vec::with_capacity( pcount );
            inputs.set_len( pcount );
            let htouch = lparam as winuser::HTOUCHINPUT;
            if winuser::GetTouchInputInfo(
                htouch,
                pcount as UINT,
                inputs.as_mut_ptr(),
                mem::size_of::<winuser::TOUCHINPUT>() as INT,
            ) > 0 {
                let dpi_factor = get_hwnd_scale_factor(window);
                for input in &inputs {
                    let x = (input.x as f64) / 100f64;
                    let y = (input.y as f64) / 100f64;
                    let location = LogicalPosition::from_physical((x, y), dpi_factor);
                    subclass_input.send_event( Event::WindowEvent {
                        window_id: SuperWindowId(WindowId(window)),
                        event: WindowEvent::Touch(Touch {
                            phase:
                            if input.dwFlags & winuser::TOUCHEVENTF_DOWN != 0 {
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
                        })
                    });
                }
            }
            winuser::CloseTouchInputHandle( htouch );
            0
        }

        winuser::WM_SETFOCUS => {
            use events::WindowEvent::{Focused, CursorMoved};
            subclass_input.send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: Focused(true)
            });

            let x = windowsx::GET_X_LPARAM(lparam) as f64;
            let y = windowsx::GET_Y_LPARAM(lparam) as f64;
            let dpi_factor = get_hwnd_scale_factor(window);
            let position = LogicalPosition::from_physical((x, y), dpi_factor);

            subclass_input.send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: CursorMoved { device_id: DEVICE_ID, position, modifiers: event::get_key_mods() },
            });

            0
        },

        winuser::WM_KILLFOCUS => {
            use events::WindowEvent::Focused;
            subclass_input.send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: Focused(false)
            });
            0
        },

        winuser::WM_SETCURSOR => {
            let call_def_window_proc = {
                let window_state = subclass_input.window_state.lock();
                if window_state.mouse_in_window {
                    let cursor = winuser::LoadCursorW(
                        ptr::null_mut(),
                        window_state.cursor.0,
                    );
                    winuser::SetCursor(cursor);
                    false
                } else {
                    true
                }
            };

            if call_def_window_proc {
                commctrl::DefSubclassProc(window, msg, wparam, lparam)
            } else {
                0
            }
        },

        winuser::WM_DROPFILES => {
            // See `FileDropHandler` for implementation.
            0
        },

        winuser::WM_GETMINMAXINFO => {
            let mmi = lparam as *mut winuser::MINMAXINFO;

            let window_state = subclass_input.window_state.lock();

            if window_state.min_size.is_some() || window_state.max_size.is_some() {
                let style = winuser::GetWindowLongA(window, winuser::GWL_STYLE) as DWORD;
                let ex_style = winuser::GetWindowLongA(window, winuser::GWL_EXSTYLE) as DWORD;
                if let Some(min_size) = window_state.min_size {
                    let (width, height) = adjust_size(min_size, style, ex_style);
                    (*mmi).ptMinTrackSize = POINT { x: width as i32, y: height as i32 };
                }
                if let Some(max_size) = window_state.max_size {
                    let (width, height) = adjust_size(max_size, style, ex_style);
                    (*mmi).ptMaxTrackSize = POINT { x: width as i32, y: height as i32 };
                }
            }

            0
        },

        // Only sent on Windows 8.1 or newer. On Windows 7 and older user has to log out to change
        // DPI, therefore all applications are closed while DPI is changing.
        winuser::WM_DPICHANGED => {
            use events::WindowEvent::HiDpiFactorChanged;

            // This message actually provides two DPI values - x and y. However MSDN says that
            // "you only need to use either the X-axis or the Y-axis value when scaling your
            // application since they are the same".
            // https://msdn.microsoft.com/en-us/library/windows/desktop/dn312083(v=vs.85).aspx
            let new_dpi_x = u32::from(LOWORD(wparam as DWORD));
            let new_dpi_factor = dpi_to_scale_factor(new_dpi_x);

            let suppress_resize = {
                let mut window_state = subclass_input.window_state.lock();
                let suppress_resize = window_state.saved_window_info
                    .as_mut()
                    .map(|saved_window_info| {
                        let dpi_changed = if !saved_window_info.is_fullscreen {
                            saved_window_info.dpi_factor.take() != Some(new_dpi_factor)
                        } else {
                            false
                        };
                        !dpi_changed || saved_window_info.is_fullscreen
                    })
                    .unwrap_or(false);
                // Now we adjust the min/max dimensions for the new DPI.
                if !suppress_resize {
                    let old_dpi_factor = window_state.dpi_factor;
                    window_state.update_min_max(old_dpi_factor, new_dpi_factor);
                }
                window_state.dpi_factor = new_dpi_factor;
                suppress_resize
            };

            // This prevents us from re-applying DPI adjustment to the restored size after exiting
            // fullscreen (the restored size is already DPI adjusted).
            if !suppress_resize {
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
                window_id: SuperWindowId(WindowId(window)),
                event: HiDpiFactorChanged(new_dpi_factor),
            });

            0
        },

        _ => {
            if msg == *DESTROY_MSG_ID {
                winuser::DestroyWindow(window);
                0
            } else if msg == *INITIAL_DPI_MSG_ID {
                use events::WindowEvent::HiDpiFactorChanged;
                let scale_factor = dpi_to_scale_factor(wparam as u32);
                subclass_input.send_event(Event::WindowEvent {
                    window_id: SuperWindowId(WindowId(window)),
                    event: HiDpiFactorChanged(scale_factor),
                });
                // Automatically resize for actual DPI
                let width = LOWORD(lparam as DWORD) as u32;
                let height = HIWORD(lparam as DWORD) as u32;
                let (adjusted_width, adjusted_height): (u32, u32) = PhysicalSize::from_logical(
                    (width, height),
                    scale_factor,
                ).into();
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
    subclass_input_ptr: DWORD_PTR
) -> LRESULT {
    let subclass_input = &mut*(subclass_input_ptr as *mut ThreadMsgTargetSubclassInput<T>);
    match msg {
        winuser::WM_DESTROY => {
            Box::from_raw(subclass_input);
            drop(subclass_input);
            0
        },
        _ if msg == *USER_EVENT_MSG_ID => {
            if let Ok(event) = subclass_input.user_event_receiver.recv() {
                subclass_input.send_event(Event::UserEvent(event));
            }
            0
        }
        _ if msg == *EXEC_MSG_ID => {
            let mut function: Box<Box<FnMut()>> = Box::from_raw(wparam as usize as *mut _);
            function();
            0
        }
        _ => commctrl::DefSubclassProc(window, msg, wparam, lparam)
    }
}
