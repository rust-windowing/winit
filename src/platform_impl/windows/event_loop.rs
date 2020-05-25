#![allow(non_snake_case)]

mod runner;

use parking_lot::Mutex;
use std::{
    marker::PhantomData,
    mem, panic, ptr,
    rc::Rc,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
use winapi::shared::basetsd::{DWORD_PTR, UINT_PTR};

use winapi::{
    shared::{
        minwindef::{BOOL, DWORD, HIWORD, INT, LOWORD, LPARAM, LRESULT, UINT, WPARAM},
        windef::{HWND, POINT, RECT},
        windowsx, winerror,
    },
    um::{
        commctrl, libloaderapi, ole2, processthreadsapi, winbase,
        winnt::{HANDLE, LONG, LPCSTR},
        winuser,
    },
};

use crate::{
    dpi::{PhysicalDelta, PhysicalPosition, PhysicalSize, UnitlessDelta},
    event::{
        Event, Force, KeyEvent, LogicalKey, ModifiersState, PointerButton, PointerButtonEvent,
        PointerId, RawKeyEvent, RawKeyboardEvent, RawPointerButtonEvent, RawPointerEvent,
        WindowEvent, PointerTiltEvent,
    },
    event_loop::{ControlFlow, EventLoopClosed, EventLoopWindowTarget as RootELW},
    platform_impl::platform::{
        dark_mode::try_dark_mode,
        dpi::{become_dpi_aware, dpi_to_scale_factor, enable_non_client_dpi_scaling},
        drop_handler::FileDropHandler,
        event::{self, handle_extended_keys, process_key_params, vkey_to_winit_vkey, PointerState, ButtonsDown},
        monitor, raw_input, util,
        window_state::{WindowFlags, WindowState},
        KeyboardDeviceId, PointerDeviceId, TouchId, WindowId, PenId,
    },
    window::{Fullscreen, Theme},
};
use runner::{EventLoopRunner, EventLoopRunnerShared};

type GetPointerFrameInfoHistory = unsafe extern "system" fn(
    pointerId: UINT,
    entriesCount: *mut UINT,
    pointerCount: *mut UINT,
    pointerInfo: *mut winuser::POINTER_INFO,
) -> BOOL;

type SkipPointerFrameMessages = unsafe extern "system" fn(pointerId: UINT) -> BOOL;
type GetPointerDeviceRects = unsafe extern "system" fn(
    device: HANDLE,
    pointerDeviceRect: *mut RECT,
    displayRect: *mut RECT,
) -> BOOL;

type GetPointerTouchInfo =
    unsafe extern "system" fn(pointerId: UINT, touchInfo: *mut winuser::POINTER_TOUCH_INFO) -> BOOL;

type GetPointerPenInfo =
    unsafe extern "system" fn(pointId: UINT, penInfo: *mut winuser::POINTER_PEN_INFO) -> BOOL;

type GetPointerInfo =
    unsafe extern "system" fn(pointId: UINT, penInfo: *mut winuser::POINTER_INFO) -> BOOL;

lazy_static! {
    static ref GET_POINTER_FRAME_INFO_HISTORY: Option<GetPointerFrameInfoHistory> =
        get_function!("user32.dll", GetPointerFrameInfoHistory);
    static ref SKIP_POINTER_FRAME_MESSAGES: Option<SkipPointerFrameMessages> =
        get_function!("user32.dll", SkipPointerFrameMessages);
    static ref GET_POINTER_DEVICE_RECTS: Option<GetPointerDeviceRects> =
        get_function!("user32.dll", GetPointerDeviceRects);
    static ref GET_POINTER_TOUCH_INFO: Option<GetPointerTouchInfo> =
        get_function!("user32.dll", GetPointerTouchInfo);
    static ref GET_POINTER_PEN_INFO: Option<GetPointerPenInfo> =
        get_function!("user32.dll", GetPointerPenInfo);
    static ref GET_POINTER_INFO: Option<GetPointerInfo> =
        get_function!("user32.dll", GetPointerInfo);
}

pub(crate) struct SubclassInput<T: 'static> {
    pub window_state: Arc<Mutex<WindowState>>,
    pub event_loop_runner: EventLoopRunnerShared<T>,
    pub file_drop_handler: FileDropHandler,
}

impl<T> SubclassInput<T> {
    unsafe fn send_event(&self, event: Event<'_, T>) {
        self.event_loop_runner.send_event(event);
    }

    unsafe fn lock_and_update_pointer_state(
        &self,
        pointer_id: PointerId,
        message_time: i32,
        window: HWND,
        new_state: impl FnOnce(PointerState) -> PointerState,
    ) {
        let mut event_buffer = Vec::new();

        {
            let mut w = self.window_state.lock();
            w.mouse.pointer_tracker.update_pointer_state(
                pointer_id,
                message_time,
                window,
                new_state,
                |event| event_buffer.push(event),
            );
            w.mouse.cursor_flags().refresh_os_cursor(window).ok();
        }

        for event in event_buffer {
            self.send_event(Event::WindowEvent(
                WindowId(window).into(),
                event,
            ))
        }
    }

    unsafe fn lock_and_destroy_pointer(
        &self,
        pointer_id: PointerId,
        window: HWND,
    ) {
        let mut event_opt = None;

        {
            let mut w = self.window_state.lock();
            w.mouse.pointer_tracker.destroy_pointer(
                pointer_id,
                |event| assert!(event_opt.replace(event).is_none()),
            );
            w.mouse.cursor_flags().refresh_os_cursor(window).ok();
        }

        for event in event_opt {
            self.send_event(Event::WindowEvent(
                WindowId(window).into(),
                event,
            ))
        }
    }
}

struct ThreadMsgTargetSubclassInput<T: 'static> {
    event_loop_runner: EventLoopRunnerShared<T>,
    user_event_receiver: Receiver<T>,
}

impl<T> ThreadMsgTargetSubclassInput<T> {
    unsafe fn send_event(&self, event: Event<'_, T>) {
        self.event_loop_runner.send_event(event);
    }
}

pub struct EventLoop<T: 'static> {
    thread_msg_sender: Sender<T>,
    window_target: RootELW<T>,
}

pub struct EventLoopWindowTarget<T: 'static> {
    thread_id: DWORD,
    thread_msg_target: HWND,
    pub(crate) runner_shared: EventLoopRunnerShared<T>,
}

macro_rules! main_thread_check {
    ($fn_name:literal) => {{
        let thread_id = unsafe { processthreadsapi::GetCurrentThreadId() };
        if thread_id != main_thread_id() {
            panic!(concat!(
                "Initializing the event loop outside of the main thread is a significant \
                 cross-platform compatibility hazard. If you really, absolutely need to create an \
                 EventLoop on a different thread, please use the `EventLoopExtWindows::",
                $fn_name,
                "` function."
            ));
        }
    }};
}

impl<T: 'static> EventLoop<T> {
    pub fn new() -> EventLoop<T> {
        main_thread_check!("new_any_thread");

        Self::new_any_thread()
    }

    pub fn new_any_thread() -> EventLoop<T> {
        become_dpi_aware();
        Self::new_dpi_unaware_any_thread()
    }

    pub fn new_dpi_unaware() -> EventLoop<T> {
        main_thread_check!("new_dpi_unaware_any_thread");

        Self::new_dpi_unaware_any_thread()
    }

    pub fn new_dpi_unaware_any_thread() -> EventLoop<T> {
        let thread_id = unsafe { processthreadsapi::GetCurrentThreadId() };

        let thread_msg_target = create_event_target_window();

        let send_thread_msg_target = thread_msg_target as usize;
        thread::spawn(move || wait_thread(thread_id, send_thread_msg_target as HWND));
        let wait_thread_id = get_wait_thread_id();

        let runner_shared = Rc::new(EventLoopRunner::new(thread_msg_target, wait_thread_id));

        let thread_msg_sender =
            subclass_event_target_window(thread_msg_target, runner_shared.clone());
        raw_input::register_all_mice_and_keyboards_for_raw_input(thread_msg_target);

        EventLoop {
            thread_msg_sender,
            window_target: RootELW {
                p: EventLoopWindowTarget {
                    thread_id,
                    thread_msg_target,
                    runner_shared,
                },
                _marker: PhantomData,
            },
        }
    }

    pub fn window_target(&self) -> &RootELW<T> {
        &self.window_target
    }

    pub fn run<F>(mut self, event_handler: F) -> !
    where
        F: 'static + FnMut(Event<'_, T>, &RootELW<T>, &mut ControlFlow),
    {
        self.run_return(event_handler);
        ::std::process::exit(0);
    }

    pub fn run_return<F>(&mut self, mut event_handler: F)
    where
        F: FnMut(Event<'_, T>, &RootELW<T>, &mut ControlFlow),
    {
        let event_loop_windows_ref = &self.window_target;

        unsafe {
            self.window_target
                .p
                .runner_shared
                .set_event_handler(move |event, control_flow| {
                    event_handler(event, event_loop_windows_ref, control_flow)
                });
        }

        let runner = &self.window_target.p.runner_shared;

        unsafe {
            let mut msg = mem::zeroed();

            runner.poll();
            'main: loop {
                if 0 == winuser::GetMessageW(&mut msg, ptr::null_mut(), 0, 0) {
                    break 'main;
                }
                winuser::TranslateMessage(&mut msg);
                winuser::DispatchMessageW(&mut msg);

                if let Err(payload) = runner.take_panic_error() {
                    runner.reset_runner();
                    panic::resume_unwind(payload);
                }

                if runner.control_flow() == ControlFlow::Exit && !runner.handling_events() {
                    break 'main;
                }
            }
        }

        unsafe {
            runner.call_event_handler(Event::LoopDestroyed);
        }
        runner.reset_runner();
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
            target_window: self.thread_msg_target,
        }
    }
}

fn main_thread_id() -> DWORD {
    static mut MAIN_THREAD_ID: DWORD = 0;
    #[used]
    #[allow(non_upper_case_globals)]
    #[link_section = ".CRT$XCU"]
    static INIT_MAIN_THREAD_ID: unsafe fn() = {
        unsafe fn initer() {
            MAIN_THREAD_ID = processthreadsapi::GetCurrentThreadId();
        }
        initer
    };

    unsafe { MAIN_THREAD_ID }
}

fn get_wait_thread_id() -> DWORD {
    unsafe {
        let mut msg = mem::zeroed();
        let result = winuser::GetMessageW(
            &mut msg,
            -1 as _,
            *SEND_WAIT_THREAD_ID_MSG_ID,
            *SEND_WAIT_THREAD_ID_MSG_ID,
        );
        assert_eq!(
            msg.message, *SEND_WAIT_THREAD_ID_MSG_ID,
            "this shouldn't be possible. please open an issue with Winit. error code: {}",
            result
        );
        msg.lParam as DWORD
    }
}

fn wait_thread(parent_thread_id: DWORD, msg_window_id: HWND) {
    unsafe {
        let mut msg: winuser::MSG;

        let cur_thread_id = processthreadsapi::GetCurrentThreadId();
        winuser::PostThreadMessageW(
            parent_thread_id,
            *SEND_WAIT_THREAD_ID_MSG_ID,
            0,
            cur_thread_id as LPARAM,
        );

        let mut wait_until_opt = None;
        'main: loop {
            // Zeroing out the message ensures that the `WaitUntilInstantBox` doesn't get
            // double-freed if `MsgWaitForMultipleObjectsEx` returns early and there aren't
            // additional messages to process.
            msg = mem::zeroed();

            if wait_until_opt.is_some() {
                if 0 != winuser::PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, winuser::PM_REMOVE) {
                    winuser::TranslateMessage(&mut msg);
                    winuser::DispatchMessageW(&mut msg);
                }
            } else {
                if 0 == winuser::GetMessageW(&mut msg, ptr::null_mut(), 0, 0) {
                    break 'main;
                } else {
                    winuser::TranslateMessage(&mut msg);
                    winuser::DispatchMessageW(&mut msg);
                }
            }

            if msg.message == *WAIT_UNTIL_MSG_ID {
                wait_until_opt = Some(*WaitUntilInstantBox::from_raw(msg.lParam as *mut _));
            } else if msg.message == *CANCEL_WAIT_UNTIL_MSG_ID {
                wait_until_opt = None;
            }

            if let Some(wait_until) = wait_until_opt {
                let now = Instant::now();
                if now < wait_until {
                    // MsgWaitForMultipleObjects tends to overshoot just a little bit. We subtract
                    // 1 millisecond from the requested time and spinlock for the remainder to
                    // compensate for that.
                    let resume_reason = winuser::MsgWaitForMultipleObjectsEx(
                        0,
                        ptr::null(),
                        dur2timeout(wait_until - now).saturating_sub(1),
                        winuser::QS_ALLEVENTS,
                        winuser::MWMO_INPUTAVAILABLE,
                    );
                    if resume_reason == winerror::WAIT_TIMEOUT {
                        winuser::PostMessageW(msg_window_id, *PROCESS_NEW_EVENTS_MSG_ID, 0, 0);
                        wait_until_opt = None;
                    }
                } else {
                    winuser::PostMessageW(msg_window_id, *PROCESS_NEW_EVENTS_MSG_ID, 0, 0);
                    wait_until_opt = None;
                }
            }
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

pub struct EventLoopProxy<T: 'static> {
    target_window: HWND,
    event_send: Sender<T>,
}
unsafe impl<T: Send + 'static> Send for EventLoopProxy<T> {}

impl<T: 'static> Clone for EventLoopProxy<T> {
    fn clone(&self) -> Self {
        Self {
            target_window: self.target_window,
            event_send: self.event_send.clone(),
        }
    }
}

impl<T: 'static> EventLoopProxy<T> {
    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed<T>> {
        unsafe {
            if winuser::PostMessageW(self.target_window, *USER_EVENT_MSG_ID, 0, 0) != 0 {
                self.event_send.send(event).ok();
                Ok(())
            } else {
                Err(EventLoopClosed(event))
            }
        }
    }
}

type WaitUntilInstantBox = Box<Instant>;

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
    static ref PROCESS_NEW_EVENTS_MSG_ID: u32 = {
        unsafe {
            winuser::RegisterWindowMessageA("Winit::ProcessNewEvents\0".as_ptr() as *const i8)
        }
    };
    /// lparam is the wait thread's message id.
    static ref SEND_WAIT_THREAD_ID_MSG_ID: u32 = {
        unsafe {
            winuser::RegisterWindowMessageA("Winit::SendWaitThreadId\0".as_ptr() as *const i8)
        }
    };
    /// lparam points to a `Box<Instant>` signifying the time `PROCESS_NEW_EVENTS_MSG_ID` should
    /// be sent.
    static ref WAIT_UNTIL_MSG_ID: u32 = {
        unsafe {
            winuser::RegisterWindowMessageA("Winit::WaitUntil\0".as_ptr() as *const i8)
        }
    };
    static ref CANCEL_WAIT_UNTIL_MSG_ID: u32 = {
        unsafe {
            winuser::RegisterWindowMessageA("Winit::CancelWaitUntil\0".as_ptr() as *const i8)
        }
    };
    // Message sent by a `Window` when it wants to be destroyed by the main thread.
    // WPARAM and LPARAM are unused.
    pub static ref DESTROY_MSG_ID: u32 = {
        unsafe {
            winuser::RegisterWindowMessageA("Winit::DestroyMsg\0".as_ptr() as LPCSTR)
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

fn create_event_target_window() -> HWND {
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
        window
    }
}

fn subclass_event_target_window<T>(
    window: HWND,
    event_loop_runner: EventLoopRunnerShared<T>,
) -> Sender<T> {
    unsafe {
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

        tx
    }
}

const WINDOW_SUBCLASS_ID: UINT_PTR = 0;
const THREAD_EVENT_TARGET_SUBCLASS_ID: UINT_PTR = 1;
pub(crate) fn subclass_window<T>(window: HWND, subclass_input: SubclassInput<T>) {
    subclass_input.event_loop_runner.register_window(window);
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

fn normalize_pointer_pressure(pressure: u32) -> Option<Force> {
    match pressure {
        0..=1024 => Some(Force::Normalized(pressure as f64 / 1024.0)),
        _ => None,
    }
}

/// Flush redraw events for Winit's windows.
///
/// Winit's API guarantees that all redraw events will be clustered together and dispatched all at
/// once, but the standard Windows message loop doesn't always exhibit that behavior. If multiple
/// windows have had redraws scheduled, but an input event is pushed to the message queue between
/// the `WM_PAINT` call for the first window and the `WM_PAINT` call for the second window, Windows
/// will dispatch the input event immediately instead of flushing all the redraw events. This
/// function explicitly pulls all of Winit's redraw events out of the event queue so that they
/// always all get processed in one fell swoop.
///
/// Returns `true` if this invocation flushed all the redraw events. If this function is re-entrant,
/// it won't flush the redraw events and will return `false`.
#[must_use]
unsafe fn flush_paint_messages<T: 'static>(
    except: Option<HWND>,
    runner: &EventLoopRunner<T>,
) -> bool {
    if !runner.redrawing() {
        runner.main_events_cleared();
        let mut msg = mem::zeroed();
        runner.owned_windows(|redraw_window| {
            if Some(redraw_window) == except {
                return;
            }

            if 0 == winuser::PeekMessageW(
                &mut msg,
                redraw_window,
                winuser::WM_PAINT,
                winuser::WM_PAINT,
                winuser::PM_REMOVE | winuser::PM_QS_PAINT,
            ) {
                return;
            }

            winuser::TranslateMessage(&mut msg);
            winuser::DispatchMessageW(&mut msg);
        });
        true
    } else {
        false
    }
}

unsafe fn process_control_flow<T: 'static>(runner: &EventLoopRunner<T>) {
    match runner.control_flow() {
        ControlFlow::Poll => {
            winuser::PostMessageW(runner.thread_msg_target(), *PROCESS_NEW_EVENTS_MSG_ID, 0, 0);
        }
        ControlFlow::Wait => (),
        ControlFlow::WaitUntil(until) => {
            winuser::PostThreadMessageW(
                runner.wait_thread_id(),
                *WAIT_UNTIL_MSG_ID,
                0,
                Box::into_raw(WaitUntilInstantBox::new(until)) as LPARAM,
            );
        }
        ControlFlow::Exit => (),
    }
}

/// Emit a `ModifiersChanged` event whenever modifiers have changed.
fn update_modifiers<T>(window: HWND, subclass_input: &SubclassInput<T>) {
    let modifiers = event::get_key_mods();
    let mut window_state = subclass_input.window_state.lock();
    if window_state.modifiers_state != modifiers {
        window_state.modifiers_state = modifiers;

        // Drop lock
        drop(window_state);

        unsafe {
            subclass_input.send_event(Event::WindowEvent(
                WindowId(window).into(),
                WindowEvent::ModifiersChanged(modifiers),
            ));
        }
    }
}

/// Any window whose callback is configured to this function will have its events propagated
/// through the events loop of the thread the window was created in.
//
// This is the callback that is called by `DispatchMessage` in the events loop.
//
// Returning 0 tells the Win32 API that the message has been processed.
// FIXME: detect WM_DWMCOMPOSITIONCHANGED and call DwmEnableBlurBehindWindow if necessary
unsafe extern "system" fn public_window_callback<T: 'static>(
    window: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
    _: UINT_PTR,
    subclass_input_ptr: DWORD_PTR,
) -> LRESULT {
    let subclass_input = &*(subclass_input_ptr as *const SubclassInput<T>);

    winuser::RedrawWindow(
        subclass_input.event_loop_runner.thread_msg_target(),
        ptr::null(),
        ptr::null_mut(),
        winuser::RDW_INTERNALPAINT,
    );

    // I decided to bind the closure to `callback` and pass it to catch_unwind rather than passing
    // the closure to catch_unwind directly so that the match body indendation wouldn't change and
    // the git blame and history would be preserved.
    let callback = || match msg {
        winuser::WM_ENTERSIZEMOVE => {
            subclass_input
                .window_state
                .lock()
                .set_window_flags_in_place(|f| f.insert(WindowFlags::MARKER_IN_SIZE_MOVE));
            0
        }

        winuser::WM_EXITSIZEMOVE => {
            subclass_input
                .window_state
                .lock()
                .set_window_flags_in_place(|f| f.remove(WindowFlags::MARKER_IN_SIZE_MOVE));
            0
        }

        winuser::WM_NCCREATE => {
            enable_non_client_dpi_scaling(window);
            commctrl::DefSubclassProc(window, msg, wparam, lparam)
        }
        winuser::WM_NCLBUTTONDOWN => {
            if wparam == winuser::HTCAPTION as WPARAM {
                winuser::PostMessageW(window, winuser::WM_MOUSEMOVE, 0, 0);
            }
            commctrl::DefSubclassProc(window, msg, wparam, lparam)
        }

        winuser::WM_CLOSE => {
            subclass_input.send_event(Event::WindowEvent(
                WindowId(window).into(),
                WindowEvent::CloseRequested,
            ));
            0
        }

        winuser::WM_DESTROY => {
            ole2::RevokeDragDrop(window);
            subclass_input.send_event(Event::WindowEvent(
                WindowId(window).into(),
                WindowEvent::Destroyed,
            ));
            subclass_input.event_loop_runner.remove_window(window);

            drop(subclass_input);
            Box::from_raw(subclass_input_ptr as *mut SubclassInput<T>);
            0
        }

        winuser::WM_PAINT => {
            if subclass_input.event_loop_runner.should_buffer() {
                // this branch can happen in response to `UpdateWindow`, if win32 decides to
                // redraw the window outside the normal flow of the event loop.
                winuser::RedrawWindow(
                    window,
                    ptr::null(),
                    ptr::null_mut(),
                    winuser::RDW_INTERNALPAINT,
                );
            } else {
                let managing_redraw =
                    flush_paint_messages(Some(window), &subclass_input.event_loop_runner);
                subclass_input.send_event(Event::RedrawRequested(WindowId(window).into()));
                if managing_redraw {
                    subclass_input.event_loop_runner.redraw_events_cleared();
                    process_control_flow(&subclass_input.event_loop_runner);
                }
            }

            commctrl::DefSubclassProc(window, msg, wparam, lparam)
        }

        winuser::WM_WINDOWPOSCHANGING => {
            let mut window_state = subclass_input.window_state.lock();
            if let Some(ref mut fullscreen) = window_state.fullscreen {
                let window_pos = &mut *(lparam as *mut winuser::WINDOWPOS);
                let new_rect = RECT {
                    left: window_pos.x,
                    top: window_pos.y,
                    right: window_pos.x + window_pos.cx,
                    bottom: window_pos.y + window_pos.cy,
                };
                let new_monitor =
                    winuser::MonitorFromRect(&new_rect, winuser::MONITOR_DEFAULTTONULL);
                match fullscreen {
                    Fullscreen::Borderless(ref mut fullscreen_monitor) => {
                        if new_monitor != fullscreen_monitor.inner.hmonitor()
                            && new_monitor != ptr::null_mut()
                        {
                            if let Ok(new_monitor_info) = monitor::get_monitor_info(new_monitor) {
                                let new_monitor_rect = new_monitor_info.rcMonitor;
                                window_pos.x = new_monitor_rect.left;
                                window_pos.y = new_monitor_rect.top;
                                window_pos.cx = new_monitor_rect.right - new_monitor_rect.left;
                                window_pos.cy = new_monitor_rect.bottom - new_monitor_rect.top;
                            }
                            *fullscreen_monitor = crate::monitor::MonitorHandle {
                                inner: monitor::MonitorHandle::new(new_monitor),
                            };
                        }
                    }
                    Fullscreen::Exclusive(ref video_mode) => {
                        let old_monitor = video_mode.video_mode.monitor.hmonitor();
                        if let Ok(old_monitor_info) = monitor::get_monitor_info(old_monitor) {
                            let old_monitor_rect = old_monitor_info.rcMonitor;
                            window_pos.x = old_monitor_rect.left;
                            window_pos.y = old_monitor_rect.top;
                            window_pos.cx = old_monitor_rect.right - old_monitor_rect.left;
                            window_pos.cy = old_monitor_rect.bottom - old_monitor_rect.top;
                        }
                    }
                }
            }

            0
        }

        // WM_MOVE supplies client area positions, so we send Moved here instead.
        winuser::WM_WINDOWPOSCHANGED => {
            let windowpos = lparam as *const winuser::WINDOWPOS;
            if (*windowpos).flags & winuser::SWP_NOMOVE != winuser::SWP_NOMOVE {
                let physical_position =
                    PhysicalPosition::new((*windowpos).x as i32, (*windowpos).y as i32);
                subclass_input.send_event(Event::WindowEvent(
                    WindowId(window).into(),
                    WindowEvent::Moved(physical_position),
                ));
            }

            // This is necessary for us to still get sent WM_SIZE.
            commctrl::DefSubclassProc(window, msg, wparam, lparam)
        }

        winuser::WM_SIZE => {
            let w = LOWORD(lparam as DWORD) as u32;
            let h = HIWORD(lparam as DWORD) as u32;

            let physical_size = PhysicalSize::new(w, h);
            let event =
                Event::WindowEvent(WindowId(window).into(), WindowEvent::Resized(physical_size));

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

        winuser::WM_CHAR | winuser::WM_SYSCHAR => {
            use std::char;
            let is_high_surrogate = 0xD800 <= wparam && wparam <= 0xDBFF;
            let is_low_surrogate = 0xDC00 <= wparam && wparam <= 0xDFFF;

            if is_high_surrogate {
                subclass_input.window_state.lock().high_surrogate = Some(wparam as u16);
            } else if is_low_surrogate {
                let high_surrogate = subclass_input.window_state.lock().high_surrogate.take();

                if let Some(high_surrogate) = high_surrogate {
                    let pair = [high_surrogate, wparam as u16];
                    if let Some(Ok(chr)) = char::decode_utf16(pair.iter().copied()).next() {
                        subclass_input.send_event(Event::WindowEvent(
                            WindowId(window).into(),
                            WindowEvent::CharReceived(chr),
                        ));
                    }
                }
            } else {
                subclass_input.window_state.lock().high_surrogate = None;

                if let Some(chr) = char::from_u32(wparam as u32) {
                    subclass_input.send_event(Event::WindowEvent(
                        WindowId(window).into(),
                        WindowEvent::CharReceived(chr),
                    ));
                }
            }
            0
        }

        // this is necessary for us to maintain minimize/restore state
        winuser::WM_SYSCOMMAND => {
            if wparam == winuser::SC_RESTORE {
                let mut w = subclass_input.window_state.lock();
                w.set_window_flags_in_place(|f| f.set(WindowFlags::MINIMIZED, false));
            }
            if wparam == winuser::SC_MINIMIZE {
                let mut w = subclass_input.window_state.lock();
                w.set_window_flags_in_place(|f| f.set(WindowFlags::MINIMIZED, true));
            }
            // Send `WindowEvent::Minimized` here if we decide to implement one

            if wparam == winuser::SC_SCREENSAVE {
                let window_state = subclass_input.window_state.lock();
                if window_state.fullscreen.is_some() {
                    return 0;
                }
            }

            winuser::DefWindowProcW(window, msg, wparam, lparam)
        }

        winuser::WM_MOUSEMOVE => {
            let x = windowsx::GET_X_LPARAM(lparam) as f64;
            let y = windowsx::GET_Y_LPARAM(lparam) as f64;
            let position = PhysicalPosition::new(x, y);
            update_modifiers(window, subclass_input);

            subclass_input.lock_and_update_pointer_state(
                PointerId::MOUSE_ID,
                winuser::GetMessageTime(),
                window,
                |pointer_state| PointerState {
                    position,
                    in_window: true,
                    ..pointer_state
                },
            );

            0
        }

        winuser::WM_MOUSELEAVE => {
            subclass_input.lock_and_update_pointer_state(
                PointerId::MOUSE_ID,
                winuser::GetMessageTime(),
                window,
                |pointer_state| PointerState {
                    in_window: false,
                    ..pointer_state
                },
            );

            0
        }

        winuser::WM_MOUSEWHEEL => {
            let value = (wparam >> 16) as i16;
            let value = value as i32;
            let value = value as f64 / winuser::WHEEL_DELTA as f64;

            update_modifiers(window, subclass_input);

            subclass_input.send_event(Event::WindowEvent(
                WindowId(window).into(),
                WindowEvent::PointerScrollStarted(PointerId::MOUSE_ID),
            ));
            subclass_input.send_event(Event::WindowEvent(
                WindowId(window).into(),
                WindowEvent::PointerScrollLines(PointerId::MOUSE_ID, UnitlessDelta::new(0.0, value)),
            ));
            subclass_input.send_event(Event::WindowEvent(
                WindowId(window).into(),
                WindowEvent::PointerScrollEnded(PointerId::MOUSE_ID),
            ));

            0
        }

        winuser::WM_MOUSEHWHEEL => {
            let value = (wparam >> 16) as i16;
            let value = value as i32;
            let value = value as f64 / winuser::WHEEL_DELTA as f64;

            update_modifiers(window, subclass_input);

            subclass_input.send_event(Event::WindowEvent(
                WindowId(window).into(),
                WindowEvent::PointerScrollStarted(PointerId::MOUSE_ID),
            ));
            subclass_input.send_event(Event::WindowEvent(
                WindowId(window).into(),
                WindowEvent::PointerScrollLines(PointerId::MOUSE_ID, UnitlessDelta::new(value, 0.0)),
            ));
            subclass_input.send_event(Event::WindowEvent(
                WindowId(window).into(),
                WindowEvent::PointerScrollEnded(PointerId::MOUSE_ID),
            ));

            0
        }

        winuser::WM_KEYDOWN | winuser::WM_SYSKEYDOWN => {
            if msg == winuser::WM_SYSKEYDOWN && wparam as i32 == winuser::VK_F4 {
                commctrl::DefSubclassProc(window, msg, wparam, lparam)
            } else {
                if let Some((scan_code, logical_key, is_repeat)) =
                    process_key_params(wparam, lparam)
                {
                    update_modifiers(window, subclass_input);

                    subclass_input.send_event(Event::WindowEvent(
                        WindowId(window).into(),
                        WindowEvent::Key(KeyEvent {
                            logical_key,
                            scan_code,
                            is_down: true,
                            is_repeat,
                            is_synthetic: false,
                        }),
                    ));
                    // Windows doesn't emit a delete character by default, but in order to make it
                    // consistent with the other platforms we'll emit a delete character here.
                    if logical_key == Some(LogicalKey::Delete) {
                        subclass_input.send_event(Event::WindowEvent(
                            WindowId(window).into(),
                            WindowEvent::CharReceived('\u{7F}'),
                        ));
                    }
                }
                0
            }
        }

        winuser::WM_KEYUP | winuser::WM_SYSKEYUP => {
            if let Some((scan_code, logical_key, _)) = process_key_params(wparam, lparam) {
                update_modifiers(window, subclass_input);

                subclass_input.send_event(Event::WindowEvent(
                    WindowId(window).into(),
                    WindowEvent::Key(KeyEvent {
                        logical_key,
                        scan_code,
                        is_down: false,
                        is_repeat: false,
                        is_synthetic: false,
                    }),
                ));
            }
            0
        }

        winuser::WM_LBUTTONDOWN => {
            update_modifiers(window, subclass_input);

            subclass_input.lock_and_update_pointer_state(
                PointerId::MOUSE_ID,
                winuser::GetMessageTime(),
                window,
                |pointer_state| PointerState {
                    buttons_down: pointer_state.buttons_down | PointerButton::MOUSE_LEFT.into_flags(),
                    ..pointer_state
                },
            );
            0
        }

        winuser::WM_LBUTTONUP => {
            update_modifiers(window, subclass_input);

            subclass_input.lock_and_update_pointer_state(
                PointerId::MOUSE_ID,
                winuser::GetMessageTime(),
                window,
                |pointer_state| PointerState {
                    buttons_down: pointer_state.buttons_down & !PointerButton::MOUSE_LEFT.into_flags(),
                    ..pointer_state
                },
            );
            0
        }

        winuser::WM_RBUTTONDOWN => {
            update_modifiers(window, subclass_input);

            subclass_input.lock_and_update_pointer_state(
                PointerId::MOUSE_ID,
                winuser::GetMessageTime(),
                window,
                |pointer_state| PointerState {
                    buttons_down: pointer_state.buttons_down | PointerButton::MOUSE_RIGHT.into_flags(),
                    ..pointer_state
                },
            );
            0
        }

        winuser::WM_RBUTTONUP => {
            update_modifiers(window, subclass_input);

            subclass_input.lock_and_update_pointer_state(
                PointerId::MOUSE_ID,
                winuser::GetMessageTime(),
                window,
                |pointer_state| PointerState {
                    buttons_down: pointer_state.buttons_down & !PointerButton::MOUSE_RIGHT.into_flags(),
                    ..pointer_state
                },
            );
            0
        }

        winuser::WM_MBUTTONDOWN => {
            update_modifiers(window, subclass_input);

            subclass_input.lock_and_update_pointer_state(
                PointerId::MOUSE_ID,
                winuser::GetMessageTime(),
                window,
                |pointer_state| PointerState {
                    buttons_down: pointer_state.buttons_down | PointerButton::MOUSE_MIDDLE.into_flags(),
                    ..pointer_state
                },
            );
            0
        }

        winuser::WM_MBUTTONUP => {
            update_modifiers(window, subclass_input);

            subclass_input.lock_and_update_pointer_state(
                PointerId::MOUSE_ID,
                winuser::GetMessageTime(),
                window,
                |pointer_state| PointerState {
                    buttons_down: pointer_state.buttons_down & !PointerButton::MOUSE_MIDDLE.into_flags(),
                    ..pointer_state
                },
            );
            0
        }

        winuser::WM_XBUTTONDOWN => {
            update_modifiers(window, subclass_input);

            let button = match winuser::GET_XBUTTON_WPARAM(wparam) {
                1 => PointerButton::MOUSE_X1,
                2 => PointerButton::MOUSE_X2,
                _ => {
                    warn!("invalid x-button passed");
                    return 0;
                }
            };

            subclass_input.lock_and_update_pointer_state(
                PointerId::MOUSE_ID,
                winuser::GetMessageTime(),
                window,
                |pointer_state| PointerState {
                    buttons_down: pointer_state.buttons_down | button.into_flags(),
                    ..pointer_state
                },
            );
            0
        }

        winuser::WM_XBUTTONUP => {
            update_modifiers(window, subclass_input);

            let button = match winuser::GET_XBUTTON_WPARAM(wparam) {
                1 => PointerButton::MOUSE_X1,
                2 => PointerButton::MOUSE_X2,
                _ => {
                    warn!("invalid x-button passed");
                    return 0;
                }
            };

            subclass_input.lock_and_update_pointer_state(
                PointerId::MOUSE_ID,
                winuser::GetMessageTime(),
                window,
                |pointer_state| PointerState {
                    buttons_down: pointer_state.buttons_down & !button.into_flags(),
                    ..pointer_state
                },
            );
            0
        }

        winuser::WM_TOUCH => {
            let pcount = LOWORD(wparam as DWORD) as usize;
            let mut inputs = Vec::<winuser::TOUCHINPUT>::with_capacity(pcount);
            inputs.set_len(pcount);
            let htouch = lparam as winuser::HTOUCHINPUT;
            if winuser::GetTouchInputInfo(
                htouch,
                pcount as UINT,
                inputs.as_mut_ptr(),
                mem::size_of::<winuser::TOUCHINPUT>() as INT,
            ) > 0
            {
                for input in &inputs {
                    let mut position = POINT {
                        x: input.x / 100,
                        y: input.y / 100,
                    };

                    if winuser::ScreenToClient(window, &mut position as *mut _) == 0 {
                        continue;
                    }

                    let x = position.x as f64 + (input.x % 100) as f64 / 100f64;
                    let y = position.y as f64 + (input.y % 100) as f64 / 100f64;
                    let position = PhysicalPosition::new(x, y);
                    let pointer_id = PointerId::TouchId(TouchId(input.dwID).into());

                    if input.dwFlags & winuser::TOUCHEVENTF_DOWN != 0 {
                        subclass_input.send_event(Event::WindowEvent(
                            WindowId(window).into(),
                            WindowEvent::PointerCreated(pointer_id),
                        ));
                    }

                    let mut button_and_mask = ButtonsDown::empty();
                    button_and_mask.set(
                        !PointerButton::TOUCH_CONTACT.into_flags(),
                        input.dwFlags & winuser::TOUCHEVENTF_UP != 0
                    );
                    let mut button_or_mask = ButtonsDown::empty();
                    button_or_mask.set(
                        PointerButton::TOUCH_CONTACT.into_flags(),
                        input.dwFlags & winuser::TOUCHEVENTF_DOWN != 0
                    );
                    let contact_area = Some(PhysicalSize::new(input.cxContact as f64, input.cyContact as f64))
                        .filter(|_| input.dwMask & winuser::TOUCHINPUTMASKF_CONTACTAREA != 0);

                    subclass_input.lock_and_update_pointer_state(
                        pointer_id,
                        winuser::GetMessageTime(),
                        window,
                        |pointer_state| PointerState {
                            in_window: true,
                            position,
                            buttons_down: pointer_state.buttons_down & button_and_mask | button_or_mask,
                            contact_area: contact_area.unwrap_or(pointer_state.contact_area),
                            ..pointer_state
                        }
                    );
                    subclass_input.send_event(Event::WindowEvent(
                        WindowId(window).into(),
                        WindowEvent::PointerMoved(pointer_id, position),
                    ));

                    if input.dwFlags & winuser::TOUCHEVENTF_INRANGE == 0 {
                        subclass_input.lock_and_destroy_pointer(pointer_id, window);
                    }
                }
            }
            winuser::CloseTouchInputHandle(htouch);
            0
        }

        // WM_POINTERENTER and WM_POINTERLEAVE send events when the pointer enters/leaves the entire
        // window area, **NOT** just the client area. We process `WM_NCPOINTERUPDATE` to handle the
        // case where the pointer is in the window area but not the client area.
        winuser::WM_POINTERENTER | winuser::WM_POINTERLEAVE | winuser::WM_NCPOINTERUPDATE => {
            if let Some(GetPointerInfo) = *GET_POINTER_INFO {
                let pointer_id = LOWORD(wparam as DWORD) as u32;
                let mut pointer_info: winuser::POINTER_INFO = mem::zeroed();
                if GetPointerInfo(pointer_id, &mut pointer_info) == 0 {
                    return 0;
                }
                let pointer_id = match pointer_info.pointerType {
                    winuser::PT_TOUCH => PointerId::TouchId(TouchId(pointer_id).into()),
                    winuser::PT_PEN => PointerId::PenId(PenId(pointer_id).into()),
                    _ => PointerId::MOUSE_ID,
                };

                subclass_input.lock_and_update_pointer_state(
                    pointer_id,
                    winuser::GetMessageTime(),
                    window,
                    |pointer_state| PointerState {
                        in_window: util::point_in_window_client(window, pointer_info.ptPixelLocation).unwrap_or(false),
                        ..pointer_state
                    }
                );
            }

            0
        }

        winuser::WM_POINTERDOWN | winuser::WM_POINTERUPDATE | winuser::WM_POINTERUP => {
            if let (
                Some(GetPointerFrameInfoHistory),
                Some(SkipPointerFrameMessages),
                Some(GetPointerDeviceRects),
            ) = (
                *GET_POINTER_FRAME_INFO_HISTORY,
                *SKIP_POINTER_FRAME_MESSAGES,
                *GET_POINTER_DEVICE_RECTS,
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
                let mut pointer_infos =
                    Vec::<winuser::POINTER_INFO>::with_capacity(pointer_info_count);
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

                // https://docs.microsoft.com/en-us/windows/desktop/api/winuser/nf-winuser-getpointerframeinfohistory
                // The information retrieved appears in reverse chronological order, with the most recent entry in the first
                // row of the returned array
                for pointer_info in pointer_infos.iter().rev() {
                    let mut device_rect = mem::MaybeUninit::uninit();
                    let mut display_rect = mem::MaybeUninit::uninit();

                    if (GetPointerDeviceRects(
                        pointer_info.sourceDevice,
                        device_rect.as_mut_ptr(),
                        display_rect.as_mut_ptr(),
                    )) == 0
                    {
                        continue;
                    }

                    let device_rect = device_rect.assume_init();
                    let display_rect = display_rect.assume_init();

                    // For the most precise himetric to pixel conversion we calculate the ratio between the resolution
                    // of the display device (pixel) and the touch device (himetric).
                    let himetric_to_pixel_ratio_x = (display_rect.right - display_rect.left) as f64
                        / (device_rect.right - device_rect.left) as f64;
                    let himetric_to_pixel_ratio_y = (display_rect.bottom - display_rect.top) as f64
                        / (device_rect.bottom - device_rect.top) as f64;

                    // ptHimetricLocation's origin is 0,0 even on multi-monitor setups.
                    // On multi-monitor setups we need to translate the himetric location to the rect of the
                    // display device it's attached to.
                    let x = display_rect.left as f64
                        + pointer_info.ptHimetricLocation.x as f64 * himetric_to_pixel_ratio_x;
                    let y = display_rect.top as f64
                        + pointer_info.ptHimetricLocation.y as f64 * himetric_to_pixel_ratio_y;

                    let mut position = POINT {
                        x: x.floor() as i32,
                        y: y.floor() as i32,
                    };

                    if winuser::ScreenToClient(window, &mut position as *mut _) == 0 {
                        continue;
                    }



                    let x = position.x as f64 + x.fract();
                    let y = position.y as f64 + y.fract();
                    let position = PhysicalPosition::new(x, y);

                    let pi = std::f64::consts::PI;
                    let deg_to_twist = |deg: u32| (deg as f64 / 360.0) * pi;
                    let mut force = None;
                    let mut tilt = None;
                    let mut twist = None;
                    let mut contact_area = None;
                    let pointer_id: PointerId;

                    let mut buttons_down = ButtonsDown::empty();
                    buttons_down.set(
                        PointerButton::TOUCH_CONTACT.into_flags(),
                        pointer_info.pointerFlags & winuser::POINTER_FLAG_INCONTACT != 0
                    );

                    match pointer_info.pointerType {
                        winuser::PT_TOUCH => {
                            pointer_id = PointerId::TouchId(TouchId(pointer_info.pointerId).into());
                            let mut touch_info = mem::MaybeUninit::uninit();
                            if let Some(GetPointerTouchInfo) = *GET_POINTER_TOUCH_INFO {
                                let get_touch_info_result = GetPointerTouchInfo(
                                    pointer_info.pointerId,
                                    touch_info.as_mut_ptr()
                                );
                                if 0 != get_touch_info_result {
                                    let touch_info = touch_info.assume_init();
                                    if touch_info.touchMask & winuser::TOUCH_MASK_PRESSURE != 0 {
                                        force = normalize_pointer_pressure(touch_info.pressure);
                                    }
                                    if touch_info.touchMask & winuser::TOUCH_MASK_ORIENTATION != 0 {
                                        twist = Some(deg_to_twist(touch_info.orientation));
                                    }
                                    if touch_info.touchMask & winuser::TOUCH_MASK_CONTACTAREA != 0 {
                                        let rect = touch_info.rcContactRaw;
                                        contact_area = Some(PhysicalSize::new(rect.right - rect.left, rect.bottom - rect.top).cast());
                                    }
                                }
                            }
                        }
                        winuser::PT_PEN => {
                            pointer_id = PointerId::PenId(PenId(pointer_info.pointerId).into());
                            let mut pen_info = mem::MaybeUninit::uninit();
                            if let Some(GetPointerPenInfo) = *GET_POINTER_PEN_INFO {
                                let get_pen_info_result = GetPointerPenInfo(
                                    pointer_info.pointerId,
                                    pen_info.as_mut_ptr(),
                                );
                                if 0 != get_pen_info_result {
                                    let pen_info = pen_info.assume_init();
                                    if pen_info.penMask & winuser::PEN_MASK_PRESSURE != 0 {
                                        force = normalize_pointer_pressure(pen_info.pressure);
                                    }
                                    if pen_info.penMask & winuser::PEN_MASK_ROTATION != 0 {
                                        twist = Some(deg_to_twist(pen_info.rotation));
                                    }
                                    if pen_info.penMask & (winuser::PEN_MASK_TILT_X | winuser::PEN_MASK_TILT_Y) != 0 {
                                        tilt = Some(PointerTiltEvent::new_tilt_angle(
                                            pen_info.tiltX as f64 / 90.0 * (pi / 2.0),
                                            pen_info.tiltY as f64 / 90.0 * (pi / 2.0),
                                        ));
                                    }
                                    buttons_down.set(
                                        PointerButton::PEN_BARREL.into_flags(),
                                        pen_info.penFlags & winuser::PEN_FLAG_BARREL != 0
                                    );
                                    buttons_down.set(
                                        PointerButton::PEN_ERASER.into_flags(),
                                        pen_info.penFlags & winuser::PEN_FLAG_ERASER != 0
                                    );
                                }
                            }
                        }
                        _ => return 0,
                    }

                    subclass_input.lock_and_update_pointer_state(
                        pointer_id,
                        winuser::GetMessageTime(),
                        window,
                        |pointer_state| PointerState {
                            in_window: true,
                            force: force.unwrap_or(pointer_state.force),
                            tilt: tilt.unwrap_or(pointer_state.tilt),
                            twist: twist.unwrap_or(pointer_state.twist),
                            contact_area: contact_area.unwrap_or(pointer_state.contact_area),
                            position,
                            buttons_down,
                        }
                    );

                    if pointer_info.pointerFlags & winuser::POINTER_FLAG_INRANGE == 0 {
                        subclass_input.lock_and_destroy_pointer(pointer_id, window);
                    }
                }

                SkipPointerFrameMessages(pointer_id);
            }
            0
        }

        winuser::WM_SETFOCUS => {
            for windows_keycode in event::get_pressed_keys() {
                let scan_code =
                    winuser::MapVirtualKeyA(windows_keycode as _, winuser::MAPVK_VK_TO_VSC);
                let logical_key = event::vkey_to_winit_vkey(windows_keycode);

                update_modifiers(window, subclass_input);

                subclass_input.send_event(Event::WindowEvent(
                    WindowId(window).into(),
                    WindowEvent::Key(KeyEvent {
                        logical_key,
                        scan_code,
                        is_down: true,
                        is_repeat: false,
                        is_synthetic: true,
                    }),
                ));
            }

            subclass_input.send_event(Event::WindowEvent(
                WindowId(window).into(),
                WindowEvent::FocusGained,
            ));

            0
        }

        winuser::WM_KILLFOCUS => {
            for windows_keycode in event::get_pressed_keys() {
                let scan_code =
                    winuser::MapVirtualKeyA(windows_keycode as _, winuser::MAPVK_VK_TO_VSC);
                let logical_key = event::vkey_to_winit_vkey(windows_keycode);

                update_modifiers(window, subclass_input);

                subclass_input.send_event(Event::WindowEvent(
                    WindowId(window).into(),
                    WindowEvent::Key(KeyEvent {
                        logical_key,
                        scan_code,
                        is_down: false,
                        is_repeat: false,
                        is_synthetic: true,
                    }),
                ));
            }

            subclass_input.window_state.lock().modifiers_state = ModifiersState::empty();
            subclass_input.send_event(Event::WindowEvent(
                WindowId(window).into(),
                WindowEvent::ModifiersChanged(ModifiersState::empty()),
            ));

            subclass_input.send_event(Event::WindowEvent(
                WindowId(window).into(),
                WindowEvent::FocusLost,
            ));
            0
        }

        winuser::WM_SETCURSOR => {
            let set_cursor_to = {
                if util::cursor_in_window_client(window).unwrap_or(false) {
                    let window_state = subclass_input.window_state.lock();
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
                if let Some(min_size) = window_state.min_size {
                    let min_size = min_size.to_physical(window_state.scale_factor);
                    let (width, height): (u32, u32) = util::adjust_size(window, min_size).into();
                    (*mmi).ptMinTrackSize = POINT {
                        x: width as i32,
                        y: height as i32,
                    };
                }
                if let Some(max_size) = window_state.max_size {
                    let max_size = max_size.to_physical(window_state.scale_factor);
                    let (width, height): (u32, u32) = util::adjust_size(window, max_size).into();
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
            // This message actually provides two DPI values - x and y. However MSDN says that
            // "you only need to use either the X-axis or the Y-axis value when scaling your
            // application since they are the same".
            // https://msdn.microsoft.com/en-us/library/windows/desktop/dn312083(v=vs.85).aspx
            let new_dpi_x = u32::from(LOWORD(wparam as DWORD));
            let new_scale_factor = dpi_to_scale_factor(new_dpi_x);
            let old_scale_factor: f64;

            let allow_resize = {
                let mut window_state = subclass_input.window_state.lock();
                old_scale_factor = window_state.scale_factor;
                window_state.scale_factor = new_scale_factor;

                if new_scale_factor == old_scale_factor {
                    return 0;
                }

                window_state.fullscreen.is_none()
                    && !window_state.window_flags().contains(WindowFlags::MAXIMIZED)
            };

            let style = winuser::GetWindowLongW(window, winuser::GWL_STYLE) as _;
            let style_ex = winuser::GetWindowLongW(window, winuser::GWL_EXSTYLE) as _;

            // New size as suggested by Windows.
            let suggested_rect = *(lparam as *const RECT);

            // The window rect provided is the window's outer size, not it's inner size. However,
            // win32 doesn't provide an `UnadjustWindowRectEx` function to get the client rect from
            // the outer rect, so we instead adjust the window rect to get the decoration margins
            // and remove them from the outer size.
            let margin_left: i32;
            let margin_top: i32;
            // let margin_right: i32;
            // let margin_bottom: i32;
            {
                let adjusted_rect =
                    util::adjust_window_rect_with_styles(window, style, style_ex, suggested_rect)
                        .unwrap_or(suggested_rect);
                margin_left = suggested_rect.left - adjusted_rect.left;
                margin_top = suggested_rect.top - adjusted_rect.top;
                // margin_right = adjusted_rect.right - suggested_rect.right;
                // margin_bottom = adjusted_rect.bottom - suggested_rect.bottom;
            }

            let old_physical_inner_rect = {
                let mut old_physical_inner_rect = mem::zeroed();
                winuser::GetClientRect(window, &mut old_physical_inner_rect);
                let mut origin = mem::zeroed();
                winuser::ClientToScreen(window, &mut origin);

                old_physical_inner_rect.left += origin.x;
                old_physical_inner_rect.right += origin.x;
                old_physical_inner_rect.top += origin.y;
                old_physical_inner_rect.bottom += origin.y;

                old_physical_inner_rect
            };
            let old_physical_inner_size = PhysicalSize::new(
                (old_physical_inner_rect.right - old_physical_inner_rect.left) as u32,
                (old_physical_inner_rect.bottom - old_physical_inner_rect.top) as u32,
            );

            // `allow_resize` prevents us from re-applying DPI adjustment to the restored size after
            // exiting fullscreen (the restored size is already DPI adjusted).
            let mut new_physical_inner_size = match allow_resize {
                // We calculate our own size because the default suggested rect doesn't do a great job
                // of preserving the window's logical size.
                true => old_physical_inner_size
                    .to_logical::<f64>(old_scale_factor)
                    .to_physical::<u32>(new_scale_factor),
                false => old_physical_inner_size,
            };

            let _ = subclass_input.send_event(Event::WindowEvent(
                WindowId(window).into(),
                WindowEvent::ScaleFactorChanged(new_scale_factor, &mut new_physical_inner_size),
            ));

            let dragging_window: bool;

            {
                let window_state = subclass_input.window_state.lock();
                dragging_window = window_state
                    .window_flags()
                    .contains(WindowFlags::MARKER_IN_SIZE_MOVE);
                // Unset maximized if we're changing the window's size.
                if new_physical_inner_size != old_physical_inner_size {
                    WindowState::set_window_flags(window_state, window, |f| {
                        f.set(WindowFlags::MAXIMIZED, false)
                    });
                }
            }

            let new_outer_rect: RECT;
            {
                let suggested_ul = (
                    suggested_rect.left + margin_left,
                    suggested_rect.top + margin_top,
                );

                let mut conservative_rect = RECT {
                    left: suggested_ul.0,
                    top: suggested_ul.1,
                    right: suggested_ul.0 + new_physical_inner_size.width as LONG,
                    bottom: suggested_ul.1 + new_physical_inner_size.height as LONG,
                };

                conservative_rect = util::adjust_window_rect_with_styles(
                    window,
                    style,
                    style_ex,
                    conservative_rect,
                )
                .unwrap_or(conservative_rect);

                // If we're dragging the window, offset the window so that the cursor's
                // relative horizontal position in the title bar is preserved.
                if dragging_window {
                    let bias = {
                        let cursor_pos = {
                            let mut pos = mem::zeroed();
                            winuser::GetCursorPos(&mut pos);
                            pos
                        };
                        let suggested_cursor_horizontal_ratio = (cursor_pos.x - suggested_rect.left)
                            as f64
                            / (suggested_rect.right - suggested_rect.left) as f64;

                        (cursor_pos.x
                            - (suggested_cursor_horizontal_ratio
                                * (conservative_rect.right - conservative_rect.left) as f64)
                                as LONG)
                            - conservative_rect.left
                    };
                    conservative_rect.left += bias;
                    conservative_rect.right += bias;
                }

                // Check to see if the new window rect is on the monitor with the new DPI factor.
                // If it isn't, offset the window so that it is.
                let new_dpi_monitor = winuser::MonitorFromWindow(window, 0);
                let conservative_rect_monitor = winuser::MonitorFromRect(&conservative_rect, 0);
                new_outer_rect = if conservative_rect_monitor == new_dpi_monitor {
                    conservative_rect
                } else {
                    let get_monitor_rect = |monitor| {
                        let mut monitor_info = winuser::MONITORINFO {
                            cbSize: mem::size_of::<winuser::MONITORINFO>() as _,
                            ..mem::zeroed()
                        };
                        winuser::GetMonitorInfoW(monitor, &mut monitor_info);
                        monitor_info.rcMonitor
                    };
                    let wrong_monitor = conservative_rect_monitor;
                    let wrong_monitor_rect = get_monitor_rect(wrong_monitor);
                    let new_monitor_rect = get_monitor_rect(new_dpi_monitor);

                    // The direction to nudge the window in to get the window onto the monitor with
                    // the new DPI factor. We calculate this by seeing which monitor edges are
                    // shared and nudging away from the wrong monitor based on those.
                    let delta_nudge_to_dpi_monitor = (
                        if wrong_monitor_rect.left == new_monitor_rect.right {
                            -1
                        } else if wrong_monitor_rect.right == new_monitor_rect.left {
                            1
                        } else {
                            0
                        },
                        if wrong_monitor_rect.bottom == new_monitor_rect.top {
                            1
                        } else if wrong_monitor_rect.top == new_monitor_rect.bottom {
                            -1
                        } else {
                            0
                        },
                    );

                    let abort_after_iterations = new_monitor_rect.right - new_monitor_rect.left
                        + new_monitor_rect.bottom
                        - new_monitor_rect.top;
                    for _ in 0..abort_after_iterations {
                        conservative_rect.left += delta_nudge_to_dpi_monitor.0;
                        conservative_rect.right += delta_nudge_to_dpi_monitor.0;
                        conservative_rect.top += delta_nudge_to_dpi_monitor.1;
                        conservative_rect.bottom += delta_nudge_to_dpi_monitor.1;

                        if winuser::MonitorFromRect(&conservative_rect, 0) == new_dpi_monitor {
                            break;
                        }
                    }

                    conservative_rect
                };
            }

            winuser::SetWindowPos(
                window,
                ptr::null_mut(),
                new_outer_rect.left,
                new_outer_rect.top,
                new_outer_rect.right - new_outer_rect.left,
                new_outer_rect.bottom - new_outer_rect.top,
                winuser::SWP_NOZORDER | winuser::SWP_NOACTIVATE,
            );

            0
        }

        winuser::WM_SETTINGCHANGE => {
            let is_dark_mode = try_dark_mode(window);
            let mut window_state = subclass_input.window_state.lock();
            let changed = window_state.is_dark_mode != is_dark_mode;

            if changed {
                let theme = if is_dark_mode {
                    Theme::Dark
                } else {
                    Theme::Light
                };

                window_state.is_dark_mode = is_dark_mode;
                mem::drop(window_state);
                subclass_input.send_event(Event::WindowEvent(
                    WindowId(window).into(),
                    WindowEvent::ThemeChanged(theme),
                ));
            }

            commctrl::DefSubclassProc(window, msg, wparam, lparam)
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
            } else {
                commctrl::DefSubclassProc(window, msg, wparam, lparam)
            }
        }
    };

    subclass_input
        .event_loop_runner
        .catch_unwind(callback)
        .unwrap_or(-1)
}

unsafe extern "system" fn thread_event_target_callback<T: 'static>(
    window: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
    _: UINT_PTR,
    subclass_input_ptr: DWORD_PTR,
) -> LRESULT {
    let subclass_input = &mut *(subclass_input_ptr as *mut ThreadMsgTargetSubclassInput<T>);
    let runner = subclass_input.event_loop_runner.clone();

    if msg != winuser::WM_PAINT {
        winuser::RedrawWindow(
            window,
            ptr::null(),
            ptr::null_mut(),
            winuser::RDW_INTERNALPAINT,
        );
    }

    // I decided to bind the closure to `callback` and pass it to catch_unwind rather than passing
    // the closure to catch_unwind directly so that the match body indendation wouldn't change and
    // the git blame and history would be preserved.
    let callback = || match msg {
        winuser::WM_DESTROY => {
            Box::from_raw(subclass_input);
            drop(subclass_input);
            0
        }
        // Because WM_PAINT comes after all other messages, we use it during modal loops to detect
        // when the event queue has been emptied. See `process_event` for more details.
        winuser::WM_PAINT => {
            winuser::ValidateRect(window, ptr::null());
            // If the WM_PAINT handler in `public_window_callback` has already flushed the redraw
            // events, `handling_events` will return false and we won't emit a second
            // `RedrawEventsCleared` event.
            if subclass_input.event_loop_runner.handling_events() {
                // This WM_PAINT handler will never be re-entrant because `flush_paint_messages`
                // doesn't call WM_PAINT for the thread event target (i.e. this window).
                assert!(flush_paint_messages(
                    None,
                    &subclass_input.event_loop_runner
                ));
                subclass_input.event_loop_runner.redraw_events_cleared();
                process_control_flow(&subclass_input.event_loop_runner);
            }

            0
        }

        winuser::WM_INPUT_DEVICE_CHANGE => {
            let mut device_info: winuser::RID_DEVICE_INFO = mem::zeroed();
            device_info.cbSize = mem::size_of::<winuser::RID_DEVICE_INFO>() as _;

            let result = winuser::GetRawInputDeviceInfoW(
                lparam as _,
                winuser::RIDI_DEVICEINFO,
                &mut device_info as *mut _ as *mut _,
                ptr::null_mut(),
            );
            assert!(result > 0);

            match device_info.dwType {
                winuser::RIM_TYPEMOUSE => subclass_input.send_event(Event::RawPointerEvent(
                    PointerDeviceId(lparam as _).into(),
                    match wparam as _ {
                        winuser::GIDC_ARRIVAL => RawPointerEvent::Added,
                        winuser::GIDC_REMOVAL => RawPointerEvent::Removed,
                        _ => unreachable!(),
                    },
                )),
                winuser::RIM_TYPEKEYBOARD => subclass_input.send_event(Event::RawKeyboardEvent(
                    KeyboardDeviceId(lparam as _).into(),
                    match wparam as _ {
                        winuser::GIDC_ARRIVAL => RawKeyboardEvent::Added,
                        winuser::GIDC_REMOVAL => RawKeyboardEvent::Removed,
                        _ => unreachable!(),
                    },
                )),
                _ => (),
            }

            0
        }

        winuser::WM_INPUT => {
            if let Some(data) = raw_input::get_raw_input_data(lparam as _) {
                if data.header.dwType == winuser::RIM_TYPEMOUSE {
                    let pointer_id = PointerDeviceId(data.header.hDevice).into();
                    let mouse = data.data.mouse();

                    if util::has_flag(mouse.usFlags, winuser::MOUSE_MOVE_RELATIVE) {
                        let x = mouse.lLastX as f64;
                        let y = mouse.lLastY as f64;

                        if x != 0.0 || y != 0.0 {
                            subclass_input.send_event(Event::RawPointerEvent(
                                pointer_id,
                                RawPointerEvent::MovedRelative(PhysicalDelta::new(x, y)),
                            ));
                        }
                    }

                    if util::has_flag(mouse.usButtonFlags, winuser::RI_MOUSE_WHEEL) {
                        let delta = mouse.usButtonData as f64 / winuser::WHEEL_DELTA as f64;
                        subclass_input.send_event(Event::RawPointerEvent(
                            pointer_id,
                            RawPointerEvent::Wheel(UnitlessDelta::new(0.0, delta)),
                        ));
                    }
                    if util::has_flag(
                        mouse.usButtonFlags,
                        0x0800, /*winuser::RI_MOUSE_HWHEEL*/
                    ) {
                        let delta = mouse.usButtonData as f64 / winuser::WHEEL_DELTA as f64;
                        subclass_input.send_event(Event::RawPointerEvent(
                            pointer_id,
                            RawPointerEvent::Wheel(UnitlessDelta::new(delta, 0.0)),
                        ));
                    }

                    let button_state = raw_input::get_raw_mouse_button_state(mouse.usButtonFlags);
                    // Left, middle, and right, respectively.
                    for (button, is_down) in button_state.iter().cloned().flatten() {
                        subclass_input.send_event(Event::RawPointerEvent(
                            pointer_id,
                            RawPointerEvent::Button(RawPointerButtonEvent { button, is_down }),
                        ));
                    }
                } else if data.header.dwType == winuser::RIM_TYPEKEYBOARD {
                    let keyboard_id = KeyboardDeviceId(data.header.hDevice).into();
                    let keyboard = data.data.keyboard();

                    let pressed = keyboard.Message == winuser::WM_KEYDOWN
                        || keyboard.Message == winuser::WM_SYSKEYDOWN;
                    let released = keyboard.Message == winuser::WM_KEYUP
                        || keyboard.Message == winuser::WM_SYSKEYUP;

                    if pressed || released {
                        let is_down = pressed;

                        let scan_code = keyboard.MakeCode as _;
                        let extended = util::has_flag(keyboard.Flags, winuser::RI_KEY_E0 as _)
                            | util::has_flag(keyboard.Flags, winuser::RI_KEY_E1 as _);

                        if let Some((vkey, scan_code)) =
                            handle_extended_keys(keyboard.VKey as _, scan_code, extended)
                        {
                            let logical_key = vkey_to_winit_vkey(vkey);

                            subclass_input.send_event(Event::RawKeyboardEvent(
                                keyboard_id,
                                RawKeyboardEvent::Key(RawKeyEvent {
                                    logical_key,
                                    scan_code,
                                    is_down,
                                }),
                            ));
                        }
                    }
                }
            }

            commctrl::DefSubclassProc(window, msg, wparam, lparam)
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
        _ if msg == *PROCESS_NEW_EVENTS_MSG_ID => {
            winuser::PostThreadMessageW(
                subclass_input.event_loop_runner.wait_thread_id(),
                *CANCEL_WAIT_UNTIL_MSG_ID,
                0,
                0,
            );

            // if the control_flow is WaitUntil, make sure the given moment has actually passed
            // before emitting NewEvents
            if let ControlFlow::WaitUntil(wait_until) =
                subclass_input.event_loop_runner.control_flow()
            {
                let mut msg = mem::zeroed();
                while Instant::now() < wait_until {
                    if 0 != winuser::PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, 0) {
                        // This works around a "feature" in PeekMessageW. If the message PeekMessageW
                        // gets is a WM_PAINT message that had RDW_INTERNALPAINT set (i.e. doesn't
                        // have an update region), PeekMessageW will remove that window from the
                        // redraw queue even though we told it not to remove messages from the
                        // queue. We fix it by re-dispatching an internal paint message to that
                        // window.
                        if msg.message == winuser::WM_PAINT {
                            let mut rect = mem::zeroed();
                            if 0 == winuser::GetUpdateRect(msg.hwnd, &mut rect, 0) {
                                winuser::RedrawWindow(
                                    msg.hwnd,
                                    ptr::null(),
                                    ptr::null_mut(),
                                    winuser::RDW_INTERNALPAINT,
                                );
                            }
                        }

                        break;
                    }
                }
            }
            subclass_input.event_loop_runner.poll();
            0
        }
        _ => commctrl::DefSubclassProc(window, msg, wparam, lparam),
    };

    runner.catch_unwind(callback).unwrap_or(-1)
}
