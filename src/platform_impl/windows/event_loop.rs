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
        winnt::{HANDLE, LONG, LPCSTR, SHORT},
        winuser,
    },
};

use self::runner::{ELRShared, EventLoopRunnerShared};
use crate::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{DeviceEvent, Event, Force, KeyboardInput, Touch, TouchPhase, WindowEvent},
    event_loop::{ControlFlow, EventLoopClosed, EventLoopWindowTarget as RootELW},
    platform_impl::platform::{
        dark_mode::try_dark_mode,
        dpi::{become_dpi_aware, dpi_to_scale_factor, enable_non_client_dpi_scaling},
        drop_handler::FileDropHandler,
        event::{self, handle_extended_keys, process_key_params, vkey_to_winit_vkey},
        monitor, raw_input, util,
        window_state::{CursorFlags, WindowFlags, WindowState},
        wrap_device_id, WindowId, DEVICE_ID,
    },
    window::{Fullscreen, WindowId as RootWindowId},
};

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
        let runner_shared = Rc::new(ELRShared::new());
        let (thread_msg_target, thread_msg_sender) =
            thread_event_target_window(runner_shared.clone());
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
                .set_runner(self, move |event, control_flow| {
                    event_handler(event, event_loop_windows_ref, control_flow)
                })
        }

        let runner = &self.window_target.p.runner_shared;

        unsafe {
            let mut msg = mem::zeroed();
            let mut unread_message_exists = false;

            'main: loop {
                if let Err(payload) = runner.take_panic_error() {
                    runner.destroy_runner();
                    panic::resume_unwind(payload);
                }

                runner.new_events();
                loop {
                    if !unread_message_exists {
                        if 0 == winuser::PeekMessageW(
                            &mut msg,
                            ptr::null_mut(),
                            0,
                            0,
                            winuser::PM_REMOVE,
                        ) {
                            break;
                        }
                    }
                    winuser::TranslateMessage(&mut msg);
                    winuser::DispatchMessageW(&mut msg);

                    unread_message_exists = false;

                    if msg.message == winuser::WM_PAINT {
                        // An "external" redraw was requested.
                        // Note that the WM_PAINT has been dispatched and
                        // has caused the event loop to emit the MainEventsCleared event.
                        // See EventLoopRunner::process_event().
                        // The call to main_events_cleared() below will do nothing.
                        break;
                    }
                }
                // Make sure we emit the MainEventsCleared event if no WM_PAINT message was received.
                runner.main_events_cleared();
                // Drain eventual WM_PAINT messages sent if user called request_redraw()
                // during handling of MainEventsCleared.
                loop {
                    if 0 == winuser::PeekMessageW(
                        &mut msg,
                        ptr::null_mut(),
                        winuser::WM_PAINT,
                        winuser::WM_PAINT,
                        winuser::PM_QS_PAINT | winuser::PM_REMOVE,
                    ) {
                        break;
                    }

                    winuser::TranslateMessage(&mut msg);
                    winuser::DispatchMessageW(&mut msg);
                }
                runner.redraw_events_cleared();
                match runner.control_flow() {
                    ControlFlow::Exit => break 'main,
                    ControlFlow::Wait => {
                        if 0 == winuser::GetMessageW(&mut msg, ptr::null_mut(), 0, 0) {
                            break 'main;
                        }
                        unread_message_exists = true;
                    }
                    ControlFlow::WaitUntil(resume_time) => {
                        wait_until_time_or_msg(resume_time);
                    }
                    ControlFlow::Poll => (),
                }
            }
        }

        runner.destroy_loop();
        runner.destroy_runner();
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

unsafe fn wait_until_time_or_msg(wait_until: Instant) {
    let now = Instant::now();
    if now < wait_until {
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
            let mut msg = mem::zeroed();
            while Instant::now() < wait_until {
                if 0 != winuser::PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, 0) {
                    break;
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

fn normalize_pointer_pressure(pressure: u32) -> Option<Force> {
    match pressure {
        1..=1024 => Some(Force::Normalized(pressure as f64 / 1024.0)),
        _ => None,
    }
}

/// Emit a `ModifiersChanged` event whenever modifiers have changed.
fn update_modifiers<T>(window: HWND, subclass_input: &SubclassInput<T>) {
    use crate::event::WindowEvent::ModifiersChanged;

    let modifiers = event::get_key_mods();
    let mut window_state = subclass_input.window_state.lock();
    if window_state.modifiers_state != modifiers {
        window_state.modifiers_state = modifiers;

        // Drop lock
        drop(window_state);

        unsafe {
            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: ModifiersChanged(modifiers),
            });
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

    match msg {
        winuser::WM_ENTERSIZEMOVE => {
            subclass_input.event_loop_runner.set_modal_loop(true);
            0
        }
        winuser::WM_EXITSIZEMOVE => {
            subclass_input.event_loop_runner.set_modal_loop(false);
            0
        }
        winuser::WM_NCCREATE => {
            enable_non_client_dpi_scaling(window);
            commctrl::DefSubclassProc(window, msg, wparam, lparam)
        }

        winuser::WM_NCLBUTTONDOWN => {
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

            drop(subclass_input);
            Box::from_raw(subclass_input_ptr as *mut SubclassInput<T>);
            0
        }

        winuser::WM_PAINT => {
            subclass_input.send_event(Event::RedrawRequested(RootWindowId(WindowId(window))));
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
            use crate::event::WindowEvent::Moved;

            let windowpos = lparam as *const winuser::WINDOWPOS;
            if (*windowpos).flags & winuser::SWP_NOMOVE != winuser::SWP_NOMOVE {
                let physical_position =
                    PhysicalPosition::new((*windowpos).x as i32, (*windowpos).y as i32);
                subclass_input.send_event(Event::WindowEvent {
                    window_id: RootWindowId(WindowId(window)),
                    event: Moved(physical_position),
                });
            }

            // This is necessary for us to still get sent WM_SIZE.
            commctrl::DefSubclassProc(window, msg, wparam, lparam)
        }

        winuser::WM_SIZE => {
            use crate::event::WindowEvent::Resized;
            let w = LOWORD(lparam as DWORD) as u32;
            let h = HIWORD(lparam as DWORD) as u32;

            let physical_size = PhysicalSize::new(w, h);
            let event = Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: Resized(physical_size),
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

        winuser::WM_CHAR | winuser::WM_SYSCHAR => {
            use crate::event::WindowEvent::ReceivedCharacter;
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
                        subclass_input.send_event(Event::WindowEvent {
                            window_id: RootWindowId(WindowId(window)),
                            event: ReceivedCharacter(chr),
                        });
                    }
                }
            } else {
                subclass_input.window_state.lock().high_surrogate = None;

                if let Some(chr) = char::from_u32(wparam as u32) {
                    subclass_input.send_event(Event::WindowEvent {
                        window_id: RootWindowId(WindowId(window)),
                        event: ReceivedCharacter(chr),
                    });
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
            let position = PhysicalPosition::new(x, y);
            let cursor_moved;
            {
                // handle spurious WM_MOUSEMOVE messages
                // see https://devblogs.microsoft.com/oldnewthing/20031001-00/?p=42343
                // and http://debugandconquer.blogspot.com/2015/08/the-cause-of-spurious-mouse-move.html
                let mut w = subclass_input.window_state.lock();
                cursor_moved = w.mouse.last_position != Some(position);
                w.mouse.last_position = Some(position);
            }
            if cursor_moved {
                update_modifiers(window, subclass_input);

                subclass_input.send_event(Event::WindowEvent {
                    window_id: RootWindowId(WindowId(window)),
                    event: CursorMoved {
                        device_id: DEVICE_ID,
                        position,
                        modifiers: event::get_key_mods(),
                    },
                });
            }

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

            update_modifiers(window, subclass_input);

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

            update_modifiers(window, subclass_input);

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
                    update_modifiers(window, subclass_input);

                    #[allow(deprecated)]
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
                            is_synthetic: false,
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
                update_modifiers(window, subclass_input);

                #[allow(deprecated)]
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
                        is_synthetic: false,
                    },
                });
            }
            0
        }

        winuser::WM_LBUTTONDOWN => {
            use crate::event::{ElementState::Pressed, MouseButton::Left, WindowEvent::MouseInput};

            capture_mouse(window, &mut *subclass_input.window_state.lock());

            update_modifiers(window, subclass_input);

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

            update_modifiers(window, subclass_input);

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

            update_modifiers(window, subclass_input);

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

            update_modifiers(window, subclass_input);

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

            update_modifiers(window, subclass_input);

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

            update_modifiers(window, subclass_input);

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

            update_modifiers(window, subclass_input);

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

            update_modifiers(window, subclass_input);

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
                for input in &inputs {
                    let mut location = POINT {
                        x: input.x / 100,
                        y: input.y / 100,
                    };

                    if winuser::ScreenToClient(window, &mut location as *mut _) == 0 {
                        continue;
                    }

                    let x = location.x as f64 + (input.x % 100) as f64 / 100f64;
                    let y = location.y as f64 + (input.y % 100) as f64 / 100f64;
                    let location = PhysicalPosition::new(x, y);
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
                            force: None, // WM_TOUCH doesn't support pressure information
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

                    let mut location = POINT {
                        x: x.floor() as i32,
                        y: y.floor() as i32,
                    };

                    if winuser::ScreenToClient(window, &mut location as *mut _) == 0 {
                        continue;
                    }

                    let force = match pointer_info.pointerType {
                        winuser::PT_TOUCH => {
                            let mut touch_info = mem::MaybeUninit::uninit();
                            GET_POINTER_TOUCH_INFO.and_then(|GetPointerTouchInfo| {
                                match GetPointerTouchInfo(
                                    pointer_info.pointerId,
                                    touch_info.as_mut_ptr(),
                                ) {
                                    0 => None,
                                    _ => normalize_pointer_pressure(
                                        touch_info.assume_init().pressure,
                                    ),
                                }
                            })
                        }
                        winuser::PT_PEN => {
                            let mut pen_info = mem::MaybeUninit::uninit();
                            GET_POINTER_PEN_INFO.and_then(|GetPointerPenInfo| {
                                match GetPointerPenInfo(
                                    pointer_info.pointerId,
                                    pen_info.as_mut_ptr(),
                                ) {
                                    0 => None,
                                    _ => {
                                        normalize_pointer_pressure(pen_info.assume_init().pressure)
                                    }
                                }
                            })
                        }
                        _ => None,
                    };

                    let x = location.x as f64 + x.fract();
                    let y = location.y as f64 + y.fract();
                    let location = PhysicalPosition::new(x, y);
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
                            force,
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
            use crate::event::{ElementState::Released, WindowEvent::Focused};
            for windows_keycode in event::get_pressed_keys() {
                let scancode =
                    winuser::MapVirtualKeyA(windows_keycode as _, winuser::MAPVK_VK_TO_VSC);
                let virtual_keycode = event::vkey_to_winit_vkey(windows_keycode);

                update_modifiers(window, subclass_input);

                #[allow(deprecated)]
                subclass_input.send_event(Event::WindowEvent {
                    window_id: RootWindowId(WindowId(window)),
                    event: WindowEvent::KeyboardInput {
                        device_id: DEVICE_ID,
                        input: KeyboardInput {
                            scancode,
                            virtual_keycode,
                            state: Released,
                            modifiers: event::get_key_mods(),
                        },
                        is_synthetic: true,
                    },
                })
            }

            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: Focused(true),
            });

            0
        }

        winuser::WM_KILLFOCUS => {
            use crate::event::{
                ElementState::Released,
                ModifiersState,
                WindowEvent::{Focused, ModifiersChanged},
            };
            for windows_keycode in event::get_pressed_keys() {
                let scancode =
                    winuser::MapVirtualKeyA(windows_keycode as _, winuser::MAPVK_VK_TO_VSC);
                let virtual_keycode = event::vkey_to_winit_vkey(windows_keycode);

                #[allow(deprecated)]
                subclass_input.send_event(Event::WindowEvent {
                    window_id: RootWindowId(WindowId(window)),
                    event: WindowEvent::KeyboardInput {
                        device_id: DEVICE_ID,
                        input: KeyboardInput {
                            scancode,
                            virtual_keycode,
                            state: Released,
                            modifiers: event::get_key_mods(),
                        },
                        is_synthetic: true,
                    },
                })
            }

            subclass_input.window_state.lock().modifiers_state = ModifiersState::empty();
            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: ModifiersChanged(ModifiersState::empty()),
            });

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
            use crate::event::WindowEvent::ScaleFactorChanged;

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

            let _ = subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: ScaleFactorChanged {
                    scale_factor: new_scale_factor,
                    new_inner_size: &mut new_physical_inner_size,
                },
            });

            // Unset maximized if we're changing the window's size.
            if new_physical_inner_size != old_physical_inner_size {
                WindowState::set_window_flags(subclass_input.window_state.lock(), window, |f| {
                    f.set(WindowFlags::MAXIMIZED, false)
                });
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

                // If we're not dragging the window, offset the window so that the cursor's
                // relative horizontal position in the title bar is preserved.
                let dragging_window = subclass_input.event_loop_runner.in_modal_loop();
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
            use crate::event::WindowEvent::ThemeChanged;

            let is_dark_mode = try_dark_mode(window);
            let mut window_state = subclass_input.window_state.lock();
            let changed = window_state.is_dark_mode != is_dark_mode;

            if changed {
                use crate::window::Theme::*;
                let theme = if is_dark_mode { Dark } else { Light };

                window_state.is_dark_mode = is_dark_mode;
                mem::drop(window_state);
                subclass_input.send_event(Event::WindowEvent {
                    window_id: RootWindowId(WindowId(window)),
                    event: ThemeChanged(theme),
                });
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
    }
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
            let in_modal_loop = subclass_input.event_loop_runner.in_modal_loop();
            if in_modal_loop {
                let runner = &subclass_input.event_loop_runner;
                runner.main_events_cleared();
                // Drain eventual WM_PAINT messages sent if user called request_redraw()
                // during handling of MainEventsCleared.
                let mut msg = mem::zeroed();
                loop {
                    if 0 == winuser::PeekMessageW(
                        &mut msg,
                        ptr::null_mut(),
                        winuser::WM_PAINT,
                        winuser::WM_PAINT,
                        winuser::PM_QS_PAINT | winuser::PM_REMOVE,
                    ) {
                        break;
                    }

                    if msg.hwnd != window {
                        winuser::TranslateMessage(&mut msg);
                        winuser::DispatchMessageW(&mut msg);
                    }
                }
                runner.redraw_events_cleared();
                match runner.control_flow() {
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

            if let Some(data) = raw_input::get_raw_input_data(lparam as _) {
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

                    let button_state = raw_input::get_raw_mouse_button_state(mouse.usButtonFlags);
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

                            #[allow(deprecated)]
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
