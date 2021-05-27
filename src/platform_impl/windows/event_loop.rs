#![allow(non_snake_case)]

mod runner;

use parking_lot::Mutex;
use std::{
    cell::Cell,
    collections::VecDeque,
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

use winapi::Windows::Win32::{
    Devices::HumanInterfaceDevice::MOUSE_MOVE_RELATIVE,
    Graphics::Gdi::{
        ClientToScreen, GetMonitorInfoW, GetUpdateRect, MonitorFromRect, MonitorFromWindow,
        RedrawWindow, ScreenToClient, ValidateRect, HBRUSH, MONITORINFO, MONITOR_DEFAULTTONULL,
        MONITOR_FROM_FLAGS, RDW_INTERNALPAINT,
    },
    System::{
        Com::RevokeDragDrop,
        SystemServices::{
            GetModuleHandleW, MsgWaitForMultipleObjectsEx, BOOL, HANDLE, LRESULT,
            MWMO_INPUTAVAILABLE, PSTR, PWSTR,
        },
        Threading::{GetCurrentThreadId, WAIT_TIMEOUT},
        WindowsProgramming::INFINITE,
    },
    UI::{
        Controls::HOVER_DEFAULT,
        DisplayDevices::{POINT, RECT},
        KeyboardAndMouseInput::{
            MapVirtualKeyA, ReleaseCapture, SetCapture, TrackMouseEvent, HRAWINPUT,
            RIM_TYPEKEYBOARD, RIM_TYPEMOUSE, TME_LEAVE, TRACKMOUSEEVENT,
        },
        MenusAndResources::{HCURSOR, HICON},
        PointerInput::{
            POINTER_FLAG_DOWN, POINTER_FLAG_UP, POINTER_FLAG_UPDATE, POINTER_INFO,
            POINTER_PEN_INFO, POINTER_TOUCH_INFO,
        },
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        TouchInput::{
            CloseTouchInputHandle, GetTouchInputInfo, HTOUCHINPUT, TOUCHEVENTF_DOWN,
            TOUCHEVENTF_MOVE, TOUCHEVENTF_UP, TOUCHINPUT,
        },
        WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect,
            GetCursorPos, GetMessageW, GetWindowLongW, LoadCursorW, PeekMessageW, PostMessageW,
            PostThreadMessageW, RegisterClassExW, RegisterWindowMessageA, SetCursor,
            SetWindowLongPtrW, SetWindowPos, TranslateMessage, GIDC_ARRIVAL, GIDC_REMOVAL,
            GWL_EXSTYLE, GWL_STYLE, HTCAPTION, HTCLIENT, HWND, LPARAM, MAPVK_VK_TO_VSC, MINMAXINFO,
            PEEK_MESSAGE_REMOVE_TYPE, PM_QS_PAINT, PM_REMOVE, QS_ALLEVENTS, RI_KEY_E0, RI_KEY_E1,
            RI_MOUSE_WHEEL, SC_MINIMIZE, SC_RESTORE, SC_SCREENSAVE, SIZE_MAXIMIZED, SWP_NOACTIVATE,
            SWP_NOMOVE, SWP_NOZORDER, VK_F4, WHEEL_DELTA, WINDOWPOS, WINDOW_STYLE, WM_CLOSE,
            WM_DESTROY, WM_ENTERSIZEMOVE, WM_EXITSIZEMOVE, WM_INPUT_DEVICE_CHANGE, WM_NCCREATE,
            WM_NCDESTROY, WM_NCLBUTTONDOWN, WM_PAINT, WM_WINDOWPOSCHANGING, WNDCLASSEXW,
            WNDCLASS_STYLES, WPARAM, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TRANSPARENT, WS_POPUP,
            WS_VISIBLE,
        },
    },
};

use crate::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{DeviceEvent, Event, Force, KeyboardInput, Touch, TouchPhase, WindowEvent},
    event_loop::{ControlFlow, EventLoopClosed, EventLoopWindowTarget as RootELW},
    monitor::MonitorHandle as RootMonitorHandle,
    platform_impl::platform::{
        dark_mode::try_theme,
        dpi::{become_dpi_aware, dpi_to_scale_factor, enable_non_client_dpi_scaling},
        drop_handler::FileDropHandler,
        event::{self, handle_extended_keys, process_key_params, vkey_to_winit_vkey},
        monitor::{self, MonitorHandle},
        raw_input, util,
        window_state::{CursorFlags, WindowFlags, WindowState},
        wrap_device_id, WindowId, DEVICE_ID,
    },
    window::{Fullscreen, WindowId as RootWindowId},
};
use runner::{EventLoopRunner, EventLoopRunnerShared};

type GetPointerFrameInfoHistory = unsafe extern "system" fn(
    pointerId: u32,
    entriesCount: *mut u32,
    pointerCount: *mut u32,
    pointerInfo: *mut POINTER_INFO,
) -> BOOL;

type SkipPointerFrameMessages = unsafe extern "system" fn(pointerId: u32) -> BOOL;
type GetPointerDeviceRects = unsafe extern "system" fn(
    device: HANDLE,
    pointerDeviceRect: *mut RECT,
    displayRect: *mut RECT,
) -> BOOL;

type GetPointerTouchInfo =
    unsafe extern "system" fn(pointerId: u32, touchInfo: *mut POINTER_TOUCH_INFO) -> BOOL;

type GetPointerPenInfo =
    unsafe extern "system" fn(pointId: u32, penInfo: *mut POINTER_PEN_INFO) -> BOOL;

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
    pub file_drop_handler: Option<FileDropHandler>,
    pub subclass_removed: Cell<bool>,
    pub recurse_depth: Cell<u32>,
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
    thread_id: u32,
    thread_msg_target: HWND,
    pub(crate) runner_shared: EventLoopRunnerShared<T>,
}

macro_rules! main_thread_check {
    ($fn_name:literal) => {{
        let thread_id = unsafe { GetCurrentThreadId() };
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
        let thread_id = unsafe { GetCurrentThreadId() };

        let thread_msg_target = create_event_target_window();

        thread::spawn(move || wait_thread(thread_id, thread_msg_target));
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
                if !GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    break 'main;
                }
                TranslateMessage(&mut msg);
                DispatchMessageW(&mut msg);

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
            runner.loop_destroyed();
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

    // TODO: Investigate opportunities for caching
    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        monitor::available_monitors()
    }

    pub fn primary_monitor(&self) -> Option<RootMonitorHandle> {
        let monitor = monitor::primary_monitor();
        Some(RootMonitorHandle { inner: monitor })
    }
}

fn main_thread_id() -> u32 {
    static mut MAIN_THREAD_ID: u32 = 0;
    #[used]
    #[allow(non_upper_case_globals)]
    #[link_section = ".CRT$XCU"]
    static INIT_MAIN_THREAD_ID: unsafe fn() = {
        unsafe fn initer() {
            MAIN_THREAD_ID = GetCurrentThreadId();
        }
        initer
    };

    unsafe { MAIN_THREAD_ID }
}

fn get_wait_thread_id() -> u32 {
    let mut msg = unsafe { mem::zeroed() };
    let result = unsafe {
        GetMessageW(
            &mut msg,
            HWND(-1),
            *SEND_WAIT_THREAD_ID_MSG_ID,
            *SEND_WAIT_THREAD_ID_MSG_ID,
        )
    };
    assert_eq!(
        msg.message,
        *SEND_WAIT_THREAD_ID_MSG_ID,
        "this shouldn't be possible. please open an issue with Winit. error code: {}",
        result.as_bool()
    );
    msg.lParam.0 as u32
}

fn wait_thread(parent_thread_id: u32, msg_window_id: HWND) {
    unsafe {
        let mut msg;

        let cur_thread_id = GetCurrentThreadId();
        PostThreadMessageW(
            parent_thread_id,
            *SEND_WAIT_THREAD_ID_MSG_ID,
            None,
            LPARAM(cur_thread_id as isize),
        );

        let mut wait_until_opt = None;
        'main: loop {
            // Zeroing out the message ensures that the `WaitUntilInstantBox` doesn't get
            // double-freed if `MsgWaitForMultipleObjectsEx` returns early and there aren't
            // additional messages to process.
            msg = mem::zeroed();

            if wait_until_opt.is_some() {
                if PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    TranslateMessage(&mut msg);
                    DispatchMessageW(&mut msg);
                }
            } else if !GetMessageW(&mut msg, None, 0, 0).as_bool() {
                break 'main;
            } else {
                TranslateMessage(&mut msg);
                DispatchMessageW(&mut msg);
            }

            if msg.message == *WAIT_UNTIL_MSG_ID {
                wait_until_opt = Some(*WaitUntilInstantBox::from_raw(msg.lParam.0 as *mut _));
            } else if msg.message == *CANCEL_WAIT_UNTIL_MSG_ID {
                wait_until_opt = None;
            }

            if let Some(wait_until) = wait_until_opt {
                let now = Instant::now();
                if now < wait_until {
                    // MsgWaitForMultipleObjects tends to overshoot just a little bit. We subtract
                    // 1 millisecond from the requested time and spinlock for the remainder to
                    // compensate for that.
                    let resume_reason = MsgWaitForMultipleObjectsEx(
                        0,
                        ptr::null(),
                        dur2timeout(wait_until - now).saturating_sub(1),
                        QS_ALLEVENTS,
                        MWMO_INPUTAVAILABLE,
                    );
                    if resume_reason == WAIT_TIMEOUT.0 {
                        PostMessageW(msg_window_id, *PROCESS_NEW_EVENTS_MSG_ID, None, None);
                        wait_until_opt = None;
                    }
                } else {
                    PostMessageW(msg_window_id, *PROCESS_NEW_EVENTS_MSG_ID, None, None);
                    wait_until_opt = None;
                }
            }
        }
    }
}

// Implementation taken from https://github.com/rust-lang/rust/blob/db5476571d9b27c862b95c1e64764b0ac8980e23/src/libstd/sys/windows/mod.rs
fn dur2timeout(dur: Duration) -> u32 {
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
            if ms > u32::max_value() as u64 {
                INFINITE
            } else {
                ms as u32
            }
        })
        .unwrap_or(INFINITE)
}

impl<T> Drop for EventLoop<T> {
    fn drop(&mut self) {
        unsafe {
            DestroyWindow(self.window_target.p.thread_msg_target);
        }
    }
}

pub(crate) struct EventLoopThreadExecutor {
    thread_id: u32,
    target_window: HWND,
}

unsafe impl Send for EventLoopThreadExecutor {}
unsafe impl Sync for EventLoopThreadExecutor {}

impl EventLoopThreadExecutor {
    /// Check to see if we're in the parent event loop's thread.
    pub(super) fn in_event_loop_thread(&self) -> bool {
        let cur_thread_id = unsafe { GetCurrentThreadId() };
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

                let res =
                    PostMessageW(self.target_window, *EXEC_MSG_ID, WPARAM(raw as usize), None);
                assert!(
                    res.as_bool(),
                    "PostMessage failed ; is the messages queue full?"
                );
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
            if PostMessageW(self.target_window, *USER_EVENT_MSG_ID, None, None).as_bool() {
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
            RegisterWindowMessageA(PSTR("Winit::WakeupMsg".as_mut_ptr()))
        }
    };
    // Message sent when we want to execute a closure in the thread.
    // WPARAM contains a Box<Box<dyn FnMut()>> that must be retrieved with `Box::from_raw`,
    // and LPARAM is unused.
    static ref EXEC_MSG_ID: u32 = {
        unsafe {
            RegisterWindowMessageA(PSTR("Winit::ExecMsg".as_mut_ptr()))
        }
    };
    static ref PROCESS_NEW_EVENTS_MSG_ID: u32 = {
        unsafe {
            RegisterWindowMessageA(PSTR("Winit::ProcessNewEvents".as_mut_ptr()))
        }
    };
    /// lparam is the wait thread's message id.
    static ref SEND_WAIT_THREAD_ID_MSG_ID: u32 = {
        unsafe {
            RegisterWindowMessageA(PSTR("Winit::SendWaitThreadId".as_mut_ptr()))
        }
    };
    /// lparam points to a `Box<Instant>` signifying the time `PROCESS_NEW_EVENTS_MSG_ID` should
    /// be sent.
    static ref WAIT_UNTIL_MSG_ID: u32 = {
        unsafe {
            RegisterWindowMessageA(PSTR("Winit::WaitUntil".as_mut_ptr()))
        }
    };
    static ref CANCEL_WAIT_UNTIL_MSG_ID: u32 = {
        unsafe {
            RegisterWindowMessageA(PSTR("Winit::CancelWaitUntil".as_mut_ptr()))
        }
    };
    // Message sent by a `Window` when it wants to be destroyed by the main thread.
    // WPARAM and LPARAM are unused.
    pub static ref DESTROY_MSG_ID: u32 = {
        unsafe {
            RegisterWindowMessageA(PSTR("Winit::DestroyMsg".as_mut_ptr()))
        }
    };
    // WPARAM is a bool specifying the `WindowFlags::MARKER_RETAIN_STATE_ON_SIZE` flag. See the
    // documentation in the `window_state` module for more information.
    pub static ref SET_RETAIN_STATE_ON_SIZE_MSG_ID: u32 = unsafe {
        RegisterWindowMessageA(PSTR("Winit::SetRetainMaximized".as_mut_ptr()))
    };
    static ref THREAD_EVENT_TARGET_WINDOW_CLASS: Vec<u16> = unsafe {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        let class_name: Vec<_> = OsStr::new("Winit Thread Event Target")
            .encode_wide()
            .chain(Some(0).into_iter())
            .collect();

        let class = WNDCLASSEXW {
            cbSize: mem::size_of::<WNDCLASSEXW>() as u32,
            style: WNDCLASS_STYLES(0),
            lpfnWndProc: Some(util::DefWindowProcW),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: GetModuleHandleW(None),
            hIcon: HICON::NULL,
            hCursor: HCURSOR::NULL, // must be null in order for cursor state to work properly
            hbrBackground: HBRUSH::NULL,
            lpszMenuName: PWSTR::NULL,
            lpszClassName: PWSTR(class_name.as_mut_ptr()),
            hIconSm: HICON::NULL,
        };

        RegisterClassExW(&class);

        class_name
    };
}

fn create_event_target_window() -> HWND {
    unsafe {
        let window = CreateWindowExW(
            WS_EX_NOACTIVATE | WS_EX_TRANSPARENT | WS_EX_LAYERED,
            PWSTR(THREAD_EVENT_TARGET_WINDOW_CLASS.as_mut_ptr()),
            None,
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            None,
            None,
            GetModuleHandleW(None),
            ptr::null_mut(),
        );
        SetWindowLongPtrW(
            window,
            GWL_STYLE,
            // The window technically has to be visible to receive WM_PAINT messages (which are used
            // for delivering events during resizes), but it isn't displayed to the user because of
            // the LAYERED style.
            (WS_VISIBLE | WS_POPUP).0 as isize,
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
        let subclass_result = SetWindowSubclass(
            window,
            Some(thread_event_target_callback::<T>),
            THREAD_EVENT_TARGET_SUBCLASS_ID,
            input_ptr as usize,
        );
        assert!(subclass_result.as_bool());

        tx
    }
}

fn remove_event_target_window_subclass<T: 'static>(window: HWND) {
    let removal_result = unsafe {
        RemoveWindowSubclass(
            window,
            Some(thread_event_target_callback::<T>),
            THREAD_EVENT_TARGET_SUBCLASS_ID,
        )
    };
    assert!(removal_result.as_bool());
}

/// Capture mouse input, allowing `window` to receive mouse events when the cursor is outside of
/// the window.
unsafe fn capture_mouse(window: HWND, window_state: &mut WindowState) {
    window_state.mouse.capture_count += 1;
    SetCapture(window);
}

/// Release mouse input, stopping windows on this thread from receiving mouse input when the cursor
/// is outside the window.
unsafe fn release_mouse(mut window_state: parking_lot::MutexGuard<'_, WindowState>) {
    window_state.mouse.capture_count = window_state.mouse.capture_count.saturating_sub(1);
    if window_state.mouse.capture_count == 0 {
        // ReleaseCapture() causes a WM_CAPTURECHANGED where we lock the window_state.
        drop(window_state);
        ReleaseCapture();
    }
}

const WINDOW_SUBCLASS_ID: usize = 0;
const THREAD_EVENT_TARGET_SUBCLASS_ID: usize = 1;
pub(crate) fn subclass_window<T>(window: HWND, subclass_input: SubclassInput<T>) {
    subclass_input.event_loop_runner.register_window(window);
    let input_ptr = Box::into_raw(Box::new(subclass_input));
    let subclass_result = unsafe {
        SetWindowSubclass(
            window,
            Some(public_window_callback::<T>),
            WINDOW_SUBCLASS_ID,
            input_ptr as usize,
        )
    };
    assert!(subclass_result.as_bool());
}

fn remove_window_subclass<T: 'static>(window: HWND) {
    let removal_result = unsafe {
        RemoveWindowSubclass(
            window,
            Some(public_window_callback::<T>),
            WINDOW_SUBCLASS_ID,
        )
    };
    assert!(removal_result.as_bool());
}

fn normalize_pointer_pressure(pressure: u32) -> Option<Force> {
    match pressure {
        1..=1024 => Some(Force::Normalized(pressure as f64 / 1024.0)),
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

            if !PeekMessageW(
                &mut msg,
                redraw_window,
                WM_PAINT,
                WM_PAINT,
                PM_REMOVE | PM_QS_PAINT,
            )
            .as_bool()
            {
                return;
            }

            TranslateMessage(&mut msg);
            DispatchMessageW(&mut msg);
        });
        true
    } else {
        false
    }
}

unsafe fn process_control_flow<T: 'static>(runner: &EventLoopRunner<T>) {
    match runner.control_flow() {
        ControlFlow::Poll => {
            PostMessageW(
                runner.thread_msg_target(),
                *PROCESS_NEW_EVENTS_MSG_ID,
                None,
                None,
            );
        }
        ControlFlow::Wait => (),
        ControlFlow::WaitUntil(until) => {
            PostThreadMessageW(
                runner.wait_thread_id(),
                *WAIT_UNTIL_MSG_ID,
                None,
                LPARAM(Box::into_raw(WaitUntilInstantBox::new(until)) as isize),
            );
        }
        ControlFlow::Exit => (),
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
                window_id: RootWindowId(WindowId(window.0)),
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
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    uidsubclass: usize,
    subclass_input_ptr: usize,
) -> LRESULT {
    let subclass_input_ptr = subclass_input_ptr as *mut SubclassInput<T>;
    let (result, subclass_removed, recurse_depth) = {
        let subclass_input = &*subclass_input_ptr;
        subclass_input
            .recurse_depth
            .set(subclass_input.recurse_depth.get() + 1);

        let result =
            public_window_callback_inner(window, msg, wparam, lparam, uidsubclass, subclass_input);

        let subclass_removed = subclass_input.subclass_removed.get();
        let recurse_depth = subclass_input.recurse_depth.get() - 1;
        subclass_input.recurse_depth.set(recurse_depth);

        (result, subclass_removed, recurse_depth)
    };

    if subclass_removed && recurse_depth == 0 {
        Box::from_raw(subclass_input_ptr);
    }

    result
}

unsafe fn public_window_callback_inner<T: 'static>(
    window: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: usize,
    subclass_input: &SubclassInput<T>,
) -> LRESULT {
    RedrawWindow(
        subclass_input.event_loop_runner.thread_msg_target(),
        ptr::null(),
        None,
        RDW_INTERNALPAINT,
    );

    // I decided to bind the closure to `callback` and pass it to catch_unwind rather than passing
    // the closure to catch_unwind directly so that the match body indendation wouldn't change and
    // the git blame and history would be preserved.
    let callback = || match msg {
        WM_ENTERSIZEMOVE => {
            subclass_input
                .window_state
                .lock()
                .set_window_flags_in_place(|f| f.insert(WindowFlags::MARKER_IN_SIZE_MOVE));
            LRESULT::NULL
        }

        WM_EXITSIZEMOVE => {
            subclass_input
                .window_state
                .lock()
                .set_window_flags_in_place(|f| f.remove(WindowFlags::MARKER_IN_SIZE_MOVE));
            LRESULT::NULL
        }

        WM_NCCREATE => {
            enable_non_client_dpi_scaling(window);
            DefSubclassProc(window, msg, wparam, lparam)
        }
        WM_NCLBUTTONDOWN => {
            if wparam.0 == HTCAPTION as usize {
                PostMessageW(
                    window,
                    winapi::Windows::Win32::UI::WindowsAndMessaging::WM_MOUSEMOVE,
                    None,
                    lparam,
                );
            }
            DefSubclassProc(window, msg, wparam, lparam)
        }

        WM_CLOSE => {
            use crate::event::WindowEvent::CloseRequested;
            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window.0)),
                event: CloseRequested,
            });
            LRESULT::NULL
        }

        WM_DESTROY => {
            use crate::event::WindowEvent::Destroyed;
            RevokeDragDrop(window);
            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window.0)),
                event: Destroyed,
            });
            subclass_input.event_loop_runner.remove_window(window);
            LRESULT::NULL
        }

        WM_NCDESTROY => {
            remove_window_subclass::<T>(window);
            subclass_input.subclass_removed.set(true);
            LRESULT::NULL
        }

        WM_PAINT => {
            if subclass_input.event_loop_runner.should_buffer() {
                // this branch can happen in response to `UpdateWindow`, if win32 decides to
                // redraw the window outside the normal flow of the event loop.
                RedrawWindow(window, ptr::null(), None, RDW_INTERNALPAINT);
            } else {
                let managing_redraw =
                    flush_paint_messages(Some(window), &subclass_input.event_loop_runner);
                subclass_input.send_event(Event::RedrawRequested(RootWindowId(WindowId(window.0))));
                if managing_redraw {
                    subclass_input.event_loop_runner.redraw_events_cleared();
                    process_control_flow(&subclass_input.event_loop_runner);
                }
            }

            DefSubclassProc(window, msg, wparam, lparam)
        }

        WM_WINDOWPOSCHANGING => {
            let mut window_state = subclass_input.window_state.lock();
            if let Some(ref mut fullscreen) = window_state.fullscreen {
                let window_pos = &*(lparam.0 as *const WINDOWPOS);
                let mut new_rect = RECT {
                    left: window_pos.x,
                    top: window_pos.y,
                    right: window_pos.x + window_pos.cx,
                    bottom: window_pos.y + window_pos.cy,
                };
                let new_monitor = MonitorFromRect(&mut new_rect, MONITOR_DEFAULTTONULL);
                match fullscreen {
                    Fullscreen::Borderless(ref mut fullscreen_monitor) => {
                        if !new_monitor.is_null()
                            && fullscreen_monitor
                                .as_ref()
                                .map(|monitor| new_monitor != monitor.inner.hmonitor())
                                .unwrap_or(true)
                        {
                            if let Ok(new_monitor_info) = monitor::get_monitor_info(new_monitor) {
                                let new_monitor_rect = new_monitor_info
                                    .__AnonymousBase_winuser_L13558_C43
                                    .rcMonitor;
                                window_pos.x = new_monitor_rect.left;
                                window_pos.y = new_monitor_rect.top;
                                window_pos.cx = new_monitor_rect.right - new_monitor_rect.left;
                                window_pos.cy = new_monitor_rect.bottom - new_monitor_rect.top;
                            }
                            *fullscreen_monitor = Some(crate::monitor::MonitorHandle {
                                inner: MonitorHandle::new(new_monitor),
                            });
                        }
                    }
                    Fullscreen::Exclusive(ref video_mode) => {
                        let old_monitor = video_mode.video_mode.monitor.hmonitor();
                        if let Ok(old_monitor_info) = monitor::get_monitor_info(old_monitor) {
                            let old_monitor_rect = old_monitor_info
                                .__AnonymousBase_winuser_L13558_C43
                                .rcMonitor;
                            window_pos.x = old_monitor_rect.left;
                            window_pos.y = old_monitor_rect.top;
                            window_pos.cx = old_monitor_rect.right - old_monitor_rect.left;
                            window_pos.cy = old_monitor_rect.bottom - old_monitor_rect.top;
                        }
                    }
                }
            }

            LRESULT::NULL
        }

        // WM_MOVE supplies client area positions, so we send Moved here instead.
        winapi::Windows::Win32::UI::WindowsAndMessaging::WM_WINDOWPOSCHANGED => {
            use crate::event::WindowEvent::Moved;

            let windowpos = &*(lparam.0 as *const WINDOWPOS);
            if windowpos.flags & SWP_NOMOVE != SWP_NOMOVE {
                let physical_position =
                    PhysicalPosition::new(windowpos.x as i32, windowpos.y as i32);
                subclass_input.send_event(Event::WindowEvent {
                    window_id: RootWindowId(WindowId(window.0)),
                    event: Moved(physical_position),
                });
            }

            // This is necessary for us to still get sent WM_SIZE.
            DefSubclassProc(window, msg, wparam, lparam)
        }

        winapi::Windows::Win32::UI::WindowsAndMessaging::WM_SIZE => {
            let w = lparam.0 as u32; // LOWORD missing
            let h = (lparam.0 >> 16) as u32; // HIWORD missing

            let physical_size = PhysicalSize::new(w, h);
            let event = Event::WindowEvent {
                window_id: RootWindowId(WindowId(window.0)),
                event: WindowEvent::Resized(physical_size),
            };

            {
                let mut w = subclass_input.window_state.lock();
                // See WindowFlags::MARKER_RETAIN_STATE_ON_SIZE docs for info on why this `if` check exists.
                if !w
                    .window_flags()
                    .contains(WindowFlags::MARKER_RETAIN_STATE_ON_SIZE)
                {
                    let maximized = wparam.0 == SIZE_MAXIMIZED as usize;
                    w.set_window_flags_in_place(|f| f.set(WindowFlags::MAXIMIZED, maximized));
                }
            }

            subclass_input.send_event(event);
            LRESULT::NULL
        }

        winapi::Windows::Win32::UI::WindowsAndMessaging::WM_CHAR
        | winapi::Windows::Win32::UI::WindowsAndMessaging::WM_SYSCHAR => {
            use crate::event::WindowEvent::ReceivedCharacter;
            use std::char;
            let is_high_surrogate = 0xD800 <= wparam.0 && wparam.0 <= 0xDBFF;
            let is_low_surrogate = 0xDC00 <= wparam.0 && wparam.0 <= 0xDFFF;

            if is_high_surrogate {
                subclass_input.window_state.lock().high_surrogate = Some(wparam.0 as u16);
            } else if is_low_surrogate {
                let high_surrogate = subclass_input.window_state.lock().high_surrogate.take();

                if let Some(high_surrogate) = high_surrogate {
                    let pair = [high_surrogate, wparam.0 as u16];
                    if let Some(Ok(chr)) = char::decode_utf16(pair.iter().copied()).next() {
                        subclass_input.send_event(Event::WindowEvent {
                            window_id: RootWindowId(WindowId(window.0)),
                            event: ReceivedCharacter(chr),
                        });
                    }
                }
            } else {
                subclass_input.window_state.lock().high_surrogate = None;

                if let Some(chr) = char::from_u32(wparam.0 as u32) {
                    subclass_input.send_event(Event::WindowEvent {
                        window_id: RootWindowId(WindowId(window.0)),
                        event: ReceivedCharacter(chr),
                    });
                }
            }
            LRESULT::NULL
        }

        // this is necessary for us to maintain minimize/restore state
        winapi::Windows::Win32::UI::WindowsAndMessaging::WM_SYSCOMMAND => {
            if wparam.0 == SC_RESTORE as usize {
                let mut w = subclass_input.window_state.lock();
                w.set_window_flags_in_place(|f| f.set(WindowFlags::MINIMIZED, false));
            }
            if wparam.0 == SC_MINIMIZE as usize {
                let mut w = subclass_input.window_state.lock();
                w.set_window_flags_in_place(|f| f.set(WindowFlags::MINIMIZED, true));
            }
            // Send `WindowEvent::Minimized` here if we decide to implement one

            if wparam.0 == SC_SCREENSAVE as usize {
                let window_state = subclass_input.window_state.lock();
                if window_state.fullscreen.is_some() {
                    return LRESULT::NULL;
                }
            }

            DefWindowProcW(window, msg, wparam, lparam)
        }

        winapi::Windows::Win32::UI::WindowsAndMessaging::WM_MOUSEMOVE => {
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
                    window_id: RootWindowId(WindowId(window.0)),
                    event: CursorEntered {
                        device_id: DEVICE_ID,
                    },
                });

                // Calling TrackMouseEvent in order to receive mouse leave events.
                TrackMouseEvent(&mut TRACKMOUSEEVENT {
                    cbSize: mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: window,
                    dwHoverTime: HOVER_DEFAULT,
                });
            }

            let x = lparam.0 as u32 as f64; // GET_X_LPARAM missing
            let y = (lparam.0 >> 16) as u32 as f64; // GET_Y_LPARAM missing
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
                    window_id: RootWindowId(WindowId(window.0)),
                    event: CursorMoved {
                        device_id: DEVICE_ID,
                        position,
                        modifiers: event::get_key_mods(),
                    },
                });
            }

            LRESULT::NULL
        }

        winapi::Windows::Win32::UI::Controls::WM_MOUSELEAVE => {
            use crate::event::WindowEvent::CursorLeft;
            {
                let mut w = subclass_input.window_state.lock();
                w.mouse
                    .set_cursor_flags(window, |f| f.set(CursorFlags::IN_WINDOW, false))
                    .ok();
            }

            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window.0)),
                event: CursorLeft {
                    device_id: DEVICE_ID,
                },
            });

            LRESULT::NULL
        }

        winapi::Windows::Win32::UI::WindowsAndMessaging::WM_MOUSEWHEEL => {
            use crate::event::MouseScrollDelta::LineDelta;

            let value = (wparam.0 >> 16) as i16;
            let value = value as i32;
            let value = value as f32 / WHEEL_DELTA as f32;

            update_modifiers(window, subclass_input);

            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window.0)),
                event: WindowEvent::MouseWheel {
                    device_id: DEVICE_ID,
                    delta: LineDelta(0.0, value),
                    phase: TouchPhase::Moved,
                    modifiers: event::get_key_mods(),
                },
            });

            LRESULT::NULL
        }

        winapi::Windows::Win32::UI::WindowsAndMessaging::WM_MOUSEHWHEEL => {
            use crate::event::MouseScrollDelta::LineDelta;

            let value = (wparam.0 >> 16) as i16;
            let value = value as i32;
            let value = value as f32 / WHEEL_DELTA as f32;

            update_modifiers(window, subclass_input);

            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window.0)),
                event: WindowEvent::MouseWheel {
                    device_id: DEVICE_ID,
                    delta: LineDelta(value, 0.0),
                    phase: TouchPhase::Moved,
                    modifiers: event::get_key_mods(),
                },
            });

            LRESULT::NULL
        }

        winapi::Windows::Win32::UI::WindowsAndMessaging::WM_KEYDOWN
        | winapi::Windows::Win32::UI::WindowsAndMessaging::WM_SYSKEYDOWN => {
            use crate::event::{ElementState::Pressed, VirtualKeyCode};
            if msg == winapi::Windows::Win32::UI::WindowsAndMessaging::WM_SYSKEYDOWN
                && wparam.0 as u32 == VK_F4
            {
                DefSubclassProc(window, msg, wparam, lparam)
            } else {
                if let Some((scancode, vkey)) = process_key_params(wparam.0 as u32, lparam.0 as u32)
                {
                    update_modifiers(window, subclass_input);

                    #[allow(deprecated)]
                    subclass_input.send_event(Event::WindowEvent {
                        window_id: RootWindowId(WindowId(window.0)),
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
                            window_id: RootWindowId(WindowId(window.0)),
                            event: WindowEvent::ReceivedCharacter('\u{7F}'),
                        });
                    }
                }
                LRESULT::NULL
            }
        }

        winapi::Windows::Win32::UI::WindowsAndMessaging::WM_KEYUP
        | winapi::Windows::Win32::UI::WindowsAndMessaging::WM_SYSKEYUP => {
            use crate::event::ElementState::Released;
            if let Some((scancode, vkey)) = process_key_params(wparam.0 as u32, lparam.0 as u32) {
                update_modifiers(window, subclass_input);

                #[allow(deprecated)]
                subclass_input.send_event(Event::WindowEvent {
                    window_id: RootWindowId(WindowId(window.0)),
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
            LRESULT::NULL
        }

        winapi::Windows::Win32::UI::WindowsAndMessaging::WM_LBUTTONDOWN => {
            use crate::event::{ElementState::Pressed, MouseButton::Left, WindowEvent::MouseInput};

            capture_mouse(window, &mut *subclass_input.window_state.lock());

            update_modifiers(window, subclass_input);

            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window.0)),
                event: MouseInput {
                    device_id: DEVICE_ID,
                    state: Pressed,
                    button: Left,
                    modifiers: event::get_key_mods(),
                },
            });
            LRESULT::NULL
        }

        winapi::Windows::Win32::UI::WindowsAndMessaging::WM_LBUTTONUP => {
            use crate::event::{
                ElementState::Released, MouseButton::Left, WindowEvent::MouseInput,
            };

            release_mouse(subclass_input.window_state.lock());

            update_modifiers(window, subclass_input);

            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window.0)),
                event: MouseInput {
                    device_id: DEVICE_ID,
                    state: Released,
                    button: Left,
                    modifiers: event::get_key_mods(),
                },
            });
            LRESULT::NULL
        }

        winapi::Windows::Win32::UI::WindowsAndMessaging::WM_RBUTTONDOWN => {
            use crate::event::{
                ElementState::Pressed, MouseButton::Right, WindowEvent::MouseInput,
            };

            capture_mouse(window, &mut *subclass_input.window_state.lock());

            update_modifiers(window, subclass_input);

            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window.0)),
                event: MouseInput {
                    device_id: DEVICE_ID,
                    state: Pressed,
                    button: Right,
                    modifiers: event::get_key_mods(),
                },
            });
            LRESULT::NULL
        }

        winapi::Windows::Win32::UI::WindowsAndMessaging::WM_RBUTTONUP => {
            use crate::event::{
                ElementState::Released, MouseButton::Right, WindowEvent::MouseInput,
            };

            release_mouse(subclass_input.window_state.lock());

            update_modifiers(window, subclass_input);

            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window.0)),
                event: MouseInput {
                    device_id: DEVICE_ID,
                    state: Released,
                    button: Right,
                    modifiers: event::get_key_mods(),
                },
            });
            LRESULT::NULL
        }

        winapi::Windows::Win32::UI::WindowsAndMessaging::WM_MBUTTONDOWN => {
            use crate::event::{
                ElementState::Pressed, MouseButton::Middle, WindowEvent::MouseInput,
            };

            capture_mouse(window, &mut *subclass_input.window_state.lock());

            update_modifiers(window, subclass_input);

            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window.0)),
                event: MouseInput {
                    device_id: DEVICE_ID,
                    state: Pressed,
                    button: Middle,
                    modifiers: event::get_key_mods(),
                },
            });
            LRESULT::NULL
        }

        winapi::Windows::Win32::UI::WindowsAndMessaging::WM_MBUTTONUP => {
            use crate::event::{
                ElementState::Released, MouseButton::Middle, WindowEvent::MouseInput,
            };

            release_mouse(subclass_input.window_state.lock());

            update_modifiers(window, subclass_input);

            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window.0)),
                event: MouseInput {
                    device_id: DEVICE_ID,
                    state: Released,
                    button: Middle,
                    modifiers: event::get_key_mods(),
                },
            });
            LRESULT::NULL
        }

        winapi::Windows::Win32::UI::WindowsAndMessaging::WM_XBUTTONDOWN => {
            use crate::event::{
                ElementState::Pressed, MouseButton::Other, WindowEvent::MouseInput,
            };
            let xbutton = (wparam.0 >> 16) as u16; // GET_XBUTTON_WPARAM missing

            capture_mouse(window, &mut *subclass_input.window_state.lock());

            update_modifiers(window, subclass_input);

            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window.0)),
                event: MouseInput {
                    device_id: DEVICE_ID,
                    state: Pressed,
                    button: Other(xbutton),
                    modifiers: event::get_key_mods(),
                },
            });
            LRESULT::NULL
        }

        winapi::Windows::Win32::UI::WindowsAndMessaging::WM_XBUTTONUP => {
            use crate::event::{
                ElementState::Released, MouseButton::Other, WindowEvent::MouseInput,
            };
            let xbutton = (wparam.0 >> 16) as u16; // GET_XBUTTON_WPARAM missing

            release_mouse(subclass_input.window_state.lock());

            update_modifiers(window, subclass_input);

            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window.0)),
                event: MouseInput {
                    device_id: DEVICE_ID,
                    state: Released,
                    button: Other(xbutton),
                    modifiers: event::get_key_mods(),
                },
            });
            LRESULT::NULL
        }

        winapi::Windows::Win32::UI::WindowsAndMessaging::WM_CAPTURECHANGED => {
            // lparam here is a handle to the window which is gaining mouse capture.
            // If it is the same as our window, then we're essentially retaining the capture. This
            // can happen if `SetCapture` is called on our window when it already has the mouse
            // capture.
            if lparam.0 != window.0 {
                subclass_input.window_state.lock().mouse.capture_count = 0;
            }
            LRESULT::NULL
        }

        winapi::Windows::Win32::UI::WindowsAndMessaging::WM_TOUCH => {
            let pcount = wparam.0 as u32; // LOWORD missing
            let mut inputs = Vec::with_capacity(pcount as usize);
            inputs.set_len(pcount as usize);
            let htouch = HTOUCHINPUT(lparam.0);
            if GetTouchInputInfo(
                htouch,
                pcount,
                inputs.as_mut_ptr(),
                mem::size_of::<TOUCHINPUT>() as i32,
            )
            .as_bool()
            {
                for input in &inputs {
                    let mut location = POINT {
                        x: input.x / 100,
                        y: input.y / 100,
                    };

                    if !ScreenToClient(window, &mut location as *mut _).as_bool() {
                        continue;
                    }

                    let x = location.x as f64 + (input.x % 100) as f64 / 100f64;
                    let y = location.y as f64 + (input.y % 100) as f64 / 100f64;
                    let location = PhysicalPosition::new(x, y);
                    subclass_input.send_event(Event::WindowEvent {
                        window_id: RootWindowId(WindowId(window.0)),
                        event: WindowEvent::Touch(Touch {
                            phase: if (input.dwFlags & TOUCHEVENTF_DOWN).0 != 0 {
                                TouchPhase::Started
                            } else if (input.dwFlags & TOUCHEVENTF_UP).0 != 0 {
                                TouchPhase::Ended
                            } else if (input.dwFlags & TOUCHEVENTF_MOVE).0 != 0 {
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
            CloseTouchInputHandle(htouch);
            LRESULT::NULL
        }

        winapi::Windows::Win32::UI::WindowsAndMessaging::WM_POINTERDOWN
        | winapi::Windows::Win32::UI::WindowsAndMessaging::WM_POINTERUPDATE
        | winapi::Windows::Win32::UI::WindowsAndMessaging::WM_POINTERUP => {
            if let (
                Some(GetPointerFrameInfoHistory),
                Some(SkipPointerFrameMessages),
                Some(GetPointerDeviceRects),
            ) = (
                *GET_POINTER_FRAME_INFO_HISTORY,
                *SKIP_POINTER_FRAME_MESSAGES,
                *GET_POINTER_DEVICE_RECTS,
            ) {
                let pointer_id = wparam.0 as u32; // LOWORD missing
                let mut entries_count = 0;
                let mut pointers_count = 0;
                if !GetPointerFrameInfoHistory(
                    pointer_id,
                    &mut entries_count,
                    &mut pointers_count,
                    std::ptr::null_mut(),
                )
                .as_bool()
                {
                    return LRESULT::NULL;
                }

                let pointer_info_count = (entries_count * pointers_count) as usize;
                let mut pointer_infos = Vec::with_capacity(pointer_info_count);
                pointer_infos.set_len(pointer_info_count);
                if !GetPointerFrameInfoHistory(
                    pointer_id,
                    &mut entries_count,
                    &mut pointers_count,
                    pointer_infos.as_mut_ptr(),
                )
                .as_bool()
                {
                    return LRESULT::NULL;
                }

                // https://docs.microsoft.com/en-us/windows/desktop/api/winuser/nf-winuser-getpointerframeinfohistory
                // The information retrieved appears in reverse chronological order, with the most recent entry in the first
                // row of the returned array
                for pointer_info in pointer_infos.iter().rev() {
                    let mut device_rect = mem::MaybeUninit::uninit();
                    let mut display_rect = mem::MaybeUninit::uninit();

                    if !GetPointerDeviceRects(
                        pointer_info.sourceDevice,
                        device_rect.as_mut_ptr(),
                        display_rect.as_mut_ptr(),
                    )
                    .as_bool()
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

                    if !ScreenToClient(window, &mut location as *mut _).as_bool() {
                        continue;
                    }

                    let force = match pointer_info.pointerType {
                        winapi::Windows::Win32::UI::WindowsAndMessaging::PT_TOUCH => {
                            let mut touch_info = mem::MaybeUninit::uninit();
                            GET_POINTER_TOUCH_INFO.and_then(|GetPointerTouchInfo| {
                                if GetPointerTouchInfo(
                                    pointer_info.pointerId,
                                    touch_info.as_mut_ptr(),
                                )
                                .as_bool()
                                {
                                    normalize_pointer_pressure(touch_info.assume_init().pressure)
                                } else {
                                    None
                                }
                            })
                        }
                        winapi::Windows::Win32::UI::WindowsAndMessaging::PT_PEN => {
                            let mut pen_info = mem::MaybeUninit::uninit();
                            GET_POINTER_PEN_INFO.and_then(|GetPointerPenInfo| {
                                if GetPointerPenInfo(pointer_info.pointerId, pen_info.as_mut_ptr())
                                    .as_bool()
                                {
                                    normalize_pointer_pressure(pen_info.assume_init().pressure)
                                } else {
                                    None
                                }
                            })
                        }
                        _ => None,
                    };

                    let x = location.x as f64 + x.fract();
                    let y = location.y as f64 + y.fract();
                    let location = PhysicalPosition::new(x, y);
                    subclass_input.send_event(Event::WindowEvent {
                        window_id: RootWindowId(WindowId(window.0)),
                        event: WindowEvent::Touch(Touch {
                            phase: if (pointer_info.pointerFlags & POINTER_FLAG_DOWN).0 != 0 {
                                TouchPhase::Started
                            } else if (pointer_info.pointerFlags & POINTER_FLAG_UP).0 != 0 {
                                TouchPhase::Ended
                            } else if (pointer_info.pointerFlags & POINTER_FLAG_UPDATE).0 != 0 {
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
            LRESULT::NULL
        }

        winapi::Windows::Win32::UI::WindowsAndMessaging::WM_SETFOCUS => {
            use crate::event::{ElementState::Released, WindowEvent::Focused};
            for windows_keycode in event::get_pressed_keys() {
                let scancode = MapVirtualKeyA(windows_keycode as _, MAPVK_VK_TO_VSC);
                let virtual_keycode = event::vkey_to_winit_vkey(windows_keycode as u32);

                update_modifiers(window, subclass_input);

                #[allow(deprecated)]
                subclass_input.send_event(Event::WindowEvent {
                    window_id: RootWindowId(WindowId(window.0)),
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
                window_id: RootWindowId(WindowId(window.0)),
                event: Focused(true),
            });

            LRESULT::NULL
        }

        winapi::Windows::Win32::UI::WindowsAndMessaging::WM_KILLFOCUS => {
            use crate::event::{
                ElementState::Released,
                ModifiersState,
                WindowEvent::{Focused, ModifiersChanged},
            };
            for windows_keycode in event::get_pressed_keys() {
                let scancode = MapVirtualKeyA(windows_keycode as _, MAPVK_VK_TO_VSC);
                let virtual_keycode = event::vkey_to_winit_vkey(windows_keycode as u32);

                #[allow(deprecated)]
                subclass_input.send_event(Event::WindowEvent {
                    window_id: RootWindowId(WindowId(window.0)),
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
                window_id: RootWindowId(WindowId(window.0)),
                event: ModifiersChanged(ModifiersState::empty()),
            });

            subclass_input.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window.0)),
                event: Focused(false),
            });
            LRESULT::NULL
        }

        winapi::Windows::Win32::UI::WindowsAndMessaging::WM_SETCURSOR => {
            let set_cursor_to = {
                let window_state = subclass_input.window_state.lock();
                // The return value for the preceding `WM_NCHITTEST` message is conveniently
                // provided through the low-order word of lParam. We use that here since
                // `WM_MOUSEMOVE` seems to come after `WM_SETCURSOR` for a given cursor movement.
                let in_client_area = lparam.0 as u32 == HTCLIENT; // LOWORD missing
                in_client_area.then(|| window_state.mouse.cursor)
            };

            match set_cursor_to {
                Some(cursor) => {
                    let cursor = LoadCursorW(None, cursor.to_windows_cursor());
                    SetCursor(cursor);
                    LRESULT::NULL
                }
                None => DefWindowProcW(window, msg, wparam, lparam),
            }
        }

        winapi::Windows::Win32::UI::WindowsAndMessaging::WM_DROPFILES => {
            // See `FileDropHandler` for implementation.
            LRESULT::NULL
        }

        winapi::Windows::Win32::UI::WindowsAndMessaging::WM_GETMINMAXINFO => {
            let mmi = lparam.0 as *mut MINMAXINFO;

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

            LRESULT::NULL
        }

        // Only sent on Windows 8.1 or newer. On Windows 7 and older user has to log out to change
        // DPI, therefore all applications are closed while DPI is changing.
        winapi::Windows::Win32::UI::WindowsAndMessaging::WM_DPICHANGED => {
            use crate::event::WindowEvent::ScaleFactorChanged;

            // This message actually provides two DPI values - x and y. However MSDN says that
            // "you only need to use either the X-axis or the Y-axis value when scaling your
            // application since they are the same".
            // https://msdn.microsoft.com/en-us/library/windows/desktop/dn312083(v=vs.85).aspx
            let new_dpi_x = u32::from(wparam.0 as u32); // LOWORD missing
            let new_scale_factor = dpi_to_scale_factor(new_dpi_x);
            let old_scale_factor: f64;

            let allow_resize = {
                let mut window_state = subclass_input.window_state.lock();
                old_scale_factor = window_state.scale_factor;
                window_state.scale_factor = new_scale_factor;

                if new_scale_factor == old_scale_factor {
                    return LRESULT::NULL;
                }

                window_state.fullscreen.is_none()
                    && !window_state.window_flags().contains(WindowFlags::MAXIMIZED)
            };

            let style = GetWindowLongW(window, GWL_STYLE) as _;
            let style_ex = GetWindowLongW(window, GWL_EXSTYLE) as _;

            // New size as suggested by Windows.
            let suggested_rect = *(lparam.0 as *const RECT);

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
                GetClientRect(window, &mut old_physical_inner_rect);
                let mut origin = mem::zeroed();
                ClientToScreen(window, &mut origin);

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
                window_id: RootWindowId(WindowId(window.0)),
                event: ScaleFactorChanged {
                    scale_factor: new_scale_factor,
                    new_inner_size: &mut new_physical_inner_size,
                },
            });

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
                    right: suggested_ul.0 + new_physical_inner_size.width as i32,
                    bottom: suggested_ul.1 + new_physical_inner_size.height as i32,
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
                            GetCursorPos(&mut pos);
                            pos
                        };
                        let suggested_cursor_horizontal_ratio = (cursor_pos.x - suggested_rect.left)
                            as f64
                            / (suggested_rect.right - suggested_rect.left) as f64;

                        (cursor_pos.x
                            - (suggested_cursor_horizontal_ratio
                                * (conservative_rect.right - conservative_rect.left) as f64)
                                as i32)
                            - conservative_rect.left
                    };
                    conservative_rect.left += bias;
                    conservative_rect.right += bias;
                }

                // Check to see if the new window rect is on the monitor with the new DPI factor.
                // If it isn't, offset the window so that it is.
                let new_dpi_monitor = MonitorFromWindow(window, MONITOR_FROM_FLAGS(0));
                let conservative_rect_monitor =
                    MonitorFromRect(&mut conservative_rect, MONITOR_FROM_FLAGS(0));
                new_outer_rect = if conservative_rect_monitor == new_dpi_monitor {
                    conservative_rect
                } else {
                    let get_monitor_rect = |monitor| {
                        let mut monitor_info = MONITORINFO {
                            cbSize: mem::size_of::<MONITORINFO>() as u32,
                            ..mem::zeroed()
                        };
                        GetMonitorInfoW(monitor, &mut monitor_info);
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

                        if MonitorFromRect(&mut conservative_rect, MONITOR_FROM_FLAGS(0))
                            == new_dpi_monitor
                        {
                            break;
                        }
                    }

                    conservative_rect
                };
            }

            SetWindowPos(
                window,
                None,
                new_outer_rect.left,
                new_outer_rect.top,
                new_outer_rect.right - new_outer_rect.left,
                new_outer_rect.bottom - new_outer_rect.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );

            LRESULT::NULL
        }

        winapi::Windows::Win32::UI::WindowsAndMessaging::WM_WININICHANGE => {
            use crate::event::WindowEvent::ThemeChanged;

            let preferred_theme = subclass_input.window_state.lock().preferred_theme;

            if preferred_theme == None {
                let new_theme = try_theme(window, preferred_theme);
                let mut window_state = subclass_input.window_state.lock();

                if window_state.current_theme != new_theme {
                    window_state.current_theme = new_theme;
                    mem::drop(window_state);
                    subclass_input.send_event(Event::WindowEvent {
                        window_id: RootWindowId(WindowId(window.0)),
                        event: ThemeChanged(new_theme),
                    });
                }
            }

            DefSubclassProc(window, msg, wparam, lparam)
        }

        _ => {
            if msg == *DESTROY_MSG_ID {
                DestroyWindow(window);
                LRESULT::NULL
            } else if msg == *SET_RETAIN_STATE_ON_SIZE_MSG_ID {
                let mut window_state = subclass_input.window_state.lock();
                window_state.set_window_flags_in_place(|f| {
                    f.set(WindowFlags::MARKER_RETAIN_STATE_ON_SIZE, !wparam.is_null())
                });
                LRESULT::NULL
            } else {
                DefSubclassProc(window, msg, wparam, lparam)
            }
        }
    };

    subclass_input
        .event_loop_runner
        .catch_unwind(callback)
        .unwrap_or(LRESULT(-1))
}

unsafe extern "system" fn thread_event_target_callback<T: 'static>(
    window: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _: usize,
    subclass_input_ptr: usize,
) -> LRESULT {
    let subclass_input = Box::from_raw(subclass_input_ptr as *mut ThreadMsgTargetSubclassInput<T>);

    if msg != WM_PAINT {
        RedrawWindow(window, ptr::null(), None, RDW_INTERNALPAINT);
    }

    let mut subclass_removed = false;

    // I decided to bind the closure to `callback` and pass it to catch_unwind rather than passing
    // the closure to catch_unwind directly so that the match body indendation wouldn't change and
    // the git blame and history would be preserved.
    let callback = || match msg {
        WM_NCDESTROY => {
            remove_event_target_window_subclass::<T>(window);
            subclass_removed = true;
            LRESULT::NULL
        }
        // Because WM_PAINT comes after all other messages, we use it during modal loops to detect
        // when the event queue has been emptied. See `process_event` for more details.
        WM_PAINT => {
            ValidateRect(window, ptr::null());
            // If the WM_PAINT handler in `public_window_callback` has already flushed the redraw
            // events, `handling_events` will return false and we won't emit a second
            // `RedrawEventsCleared` event.
            if subclass_input.event_loop_runner.handling_events() {
                if subclass_input.event_loop_runner.should_buffer() {
                    // This branch can be triggered when a nested win32 event loop is triggered
                    // inside of the `event_handler` callback.
                    RedrawWindow(window, ptr::null(), None, RDW_INTERNALPAINT);
                } else {
                    // This WM_PAINT handler will never be re-entrant because `flush_paint_messages`
                    // doesn't call WM_PAINT for the thread event target (i.e. this window).
                    assert!(flush_paint_messages(
                        None,
                        &subclass_input.event_loop_runner
                    ));
                    subclass_input.event_loop_runner.redraw_events_cleared();
                    process_control_flow(&subclass_input.event_loop_runner);
                }
            }

            // Default WM_PAINT behaviour. This makes sure modals and popups are shown immediatly when opening them.
            DefSubclassProc(window, msg, wparam, lparam)
        }

        WM_INPUT_DEVICE_CHANGE => {
            let event = match wparam.0 as u32 {
                GIDC_ARRIVAL => DeviceEvent::Added,
                GIDC_REMOVAL => DeviceEvent::Removed,
                _ => unreachable!(),
            };

            subclass_input.send_event(Event::DeviceEvent {
                device_id: wrap_device_id(lparam.0),
                event,
            });

            LRESULT::NULL
        }

        winapi::Windows::Win32::UI::WindowsAndMessaging::WM_INPUT => {
            use crate::event::{
                DeviceEvent::{Button, Key, Motion, MouseMotion, MouseWheel},
                ElementState::{Pressed, Released},
                MouseScrollDelta::LineDelta,
            };

            if let Some(data) = raw_input::get_raw_input_data(HRAWINPUT(lparam.0)) {
                let device_id = wrap_device_id(data.header.hDevice.0);

                if data.header.dwType == RIM_TYPEMOUSE.0 {
                    let mouse = data.data.mouse;

                    if util::has_flag(mouse.usFlags, MOUSE_MOVE_RELATIVE as u16) {
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

                    if util::has_flag(
                        mouse.Anonymous.Anonymous.usButtonFlags,
                        RI_MOUSE_WHEEL as u16,
                    ) {
                        let delta =
                            mouse.Anonymous.Anonymous.usButtonData as f32 / WHEEL_DELTA as f32;
                        subclass_input.send_event(Event::DeviceEvent {
                            device_id,
                            event: MouseWheel {
                                delta: LineDelta(0.0, delta),
                            },
                        });
                    }

                    let button_state = raw_input::get_raw_mouse_button_state(
                        mouse.Anonymous.Anonymous.usButtonFlags as u32,
                    );
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
                } else if data.header.dwType == RIM_TYPEKEYBOARD.0 {
                    let keyboard = data.data.keyboard;

                    let pressed = keyboard.Message
                        == winapi::Windows::Win32::UI::WindowsAndMessaging::WM_KEYDOWN
                        || keyboard.Message
                            == winapi::Windows::Win32::UI::WindowsAndMessaging::WM_SYSKEYDOWN;
                    let released = keyboard.Message
                        == winapi::Windows::Win32::UI::WindowsAndMessaging::WM_KEYUP
                        || keyboard.Message
                            == winapi::Windows::Win32::UI::WindowsAndMessaging::WM_SYSKEYUP;

                    if pressed || released {
                        let state = if pressed { Pressed } else { Released };

                        let scancode = keyboard.MakeCode as _;
                        let extended = util::has_flag(keyboard.Flags, RI_KEY_E0 as u16)
                            | util::has_flag(keyboard.Flags, RI_KEY_E1 as u16);

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

            DefSubclassProc(window, msg, wparam, lparam)
        }

        _ if msg == *USER_EVENT_MSG_ID => {
            if let Ok(event) = subclass_input.user_event_receiver.recv() {
                subclass_input.send_event(Event::UserEvent(event));
            }
            LRESULT::NULL
        }
        _ if msg == *EXEC_MSG_ID => {
            let mut function: ThreadExecFn = Box::from_raw(wparam.0 as *mut _);
            function();
            LRESULT::NULL
        }
        _ if msg == *PROCESS_NEW_EVENTS_MSG_ID => {
            PostThreadMessageW(
                subclass_input.event_loop_runner.wait_thread_id(),
                *CANCEL_WAIT_UNTIL_MSG_ID,
                None,
                None,
            );

            // if the control_flow is WaitUntil, make sure the given moment has actually passed
            // before emitting NewEvents
            if let ControlFlow::WaitUntil(wait_until) =
                subclass_input.event_loop_runner.control_flow()
            {
                let mut msg = mem::zeroed();
                while Instant::now() < wait_until {
                    if PeekMessageW(&mut msg, None, 0, 0, PEEK_MESSAGE_REMOVE_TYPE(0)).as_bool() {
                        // This works around a "feature" in PeekMessageW. If the message PeekMessageW
                        // gets is a WM_PAINT message that had RDW_INTERNALPAINT set (i.e. doesn't
                        // have an update region), PeekMessageW will remove that window from the
                        // redraw queue even though we told it not to remove messages from the
                        // queue. We fix it by re-dispatching an internal paint message to that
                        // window.
                        if msg.message == WM_PAINT {
                            let mut rect = mem::zeroed();
                            if !GetUpdateRect(msg.hwnd, &mut rect, false).as_bool() {
                                RedrawWindow(msg.hwnd, ptr::null(), None, RDW_INTERNALPAINT);
                            }
                        }

                        break;
                    }
                }
            }
            subclass_input.event_loop_runner.poll();
            LRESULT::NULL
        }
        _ => DefSubclassProc(window, msg, wparam, lparam),
    };

    let result = subclass_input
        .event_loop_runner
        .catch_unwind(callback)
        .unwrap_or(LRESULT(-1));
    if subclass_removed {
        mem::drop(subclass_input);
    } else {
        Box::into_raw(subclass_input);
    }
    result
}
