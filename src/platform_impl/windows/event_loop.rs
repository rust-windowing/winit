#![allow(non_snake_case)]

mod runner;

use std::{
    cell::Cell,
    collections::VecDeque,
    ffi::c_void,
    marker::PhantomData,
    mem, panic, ptr,
    rc::Rc,
    sync::{
        atomic::{AtomicU32, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc, Mutex, MutexGuard,
    },
    thread,
    time::{Duration, Instant},
};

use once_cell::sync::Lazy;
use raw_window_handle::{RawDisplayHandle, WindowsDisplayHandle};

use windows_sys::Win32::{
    Devices::HumanInterfaceDevice::MOUSE_MOVE_RELATIVE,
    Foundation::{BOOL, HANDLE, HWND, LPARAM, LRESULT, POINT, RECT, WAIT_TIMEOUT, WPARAM},
    Graphics::Gdi::{
        GetMonitorInfoW, GetUpdateRect, MonitorFromRect, MonitorFromWindow, RedrawWindow,
        ScreenToClient, ValidateRect, MONITORINFO, MONITOR_DEFAULTTONULL, RDW_INTERNALPAINT,
        SC_SCREENSAVE,
    },
    Media::{timeBeginPeriod, timeEndPeriod, timeGetDevCaps, TIMECAPS, TIMERR_NOERROR},
    System::{Ole::RevokeDragDrop, Threading::GetCurrentThreadId, WindowsProgramming::INFINITE},
    UI::{
        Controls::{HOVER_DEFAULT, WM_MOUSELEAVE},
        Input::{
            Ime::{GCS_COMPSTR, GCS_RESULTSTR, ISC_SHOWUICOMPOSITIONWINDOW},
            KeyboardAndMouse::{
                MapVirtualKeyA, ReleaseCapture, SetCapture, TrackMouseEvent, MAPVK_VK_TO_VSC,
                TME_LEAVE, TRACKMOUSEEVENT,
            },
            Pointer::{
                POINTER_FLAG_DOWN, POINTER_FLAG_UP, POINTER_FLAG_UPDATE, POINTER_INFO,
                POINTER_PEN_INFO, POINTER_TOUCH_INFO,
            },
            Touch::{
                CloseTouchInputHandle, GetTouchInputInfo, TOUCHEVENTF_DOWN, TOUCHEVENTF_MOVE,
                TOUCHEVENTF_UP, TOUCHINPUT,
            },
            RIM_TYPEKEYBOARD, RIM_TYPEMOUSE,
        },
        WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetCursorPos,
            GetMenu, GetMessageW, LoadCursorW, MsgWaitForMultipleObjectsEx, PeekMessageW,
            PostMessageW, PostThreadMessageW, RegisterClassExW, RegisterWindowMessageA, SetCursor,
            SetWindowPos, TranslateMessage, CREATESTRUCTW, GIDC_ARRIVAL, GIDC_REMOVAL, GWL_STYLE,
            GWL_USERDATA, HTCAPTION, HTCLIENT, MINMAXINFO, MNC_CLOSE, MSG, MWMO_INPUTAVAILABLE,
            NCCALCSIZE_PARAMS, PM_NOREMOVE, PM_QS_PAINT, PM_REMOVE, PT_PEN, PT_TOUCH, QS_ALLEVENTS,
            RI_KEY_E0, RI_KEY_E1, RI_MOUSE_WHEEL, SC_MINIMIZE, SC_RESTORE, SIZE_MAXIMIZED,
            SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WHEEL_DELTA, WINDOWPOS,
            WM_CAPTURECHANGED, WM_CHAR, WM_CLOSE, WM_CREATE, WM_DESTROY, WM_DPICHANGED,
            WM_DROPFILES, WM_ENTERSIZEMOVE, WM_EXITSIZEMOVE, WM_GETMINMAXINFO, WM_IME_COMPOSITION,
            WM_IME_ENDCOMPOSITION, WM_IME_SETCONTEXT, WM_IME_STARTCOMPOSITION, WM_INPUT,
            WM_INPUT_DEVICE_CHANGE, WM_KEYDOWN, WM_KEYUP, WM_KILLFOCUS, WM_LBUTTONDOWN,
            WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MENUCHAR, WM_MOUSEHWHEEL, WM_MOUSEMOVE,
            WM_MOUSEWHEEL, WM_NCACTIVATE, WM_NCCALCSIZE, WM_NCCREATE, WM_NCDESTROY,
            WM_NCLBUTTONDOWN, WM_PAINT, WM_POINTERDOWN, WM_POINTERUP, WM_POINTERUPDATE,
            WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SETCURSOR, WM_SETFOCUS, WM_SETTINGCHANGE, WM_SIZE,
            WM_SYSCHAR, WM_SYSCOMMAND, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_TOUCH, WM_WINDOWPOSCHANGED,
            WM_WINDOWPOSCHANGING, WM_XBUTTONDOWN, WM_XBUTTONUP, WNDCLASSEXW, WS_EX_LAYERED,
            WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT, WS_OVERLAPPED, WS_POPUP,
            WS_VISIBLE,
        },
    },
};

use crate::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{DeviceEvent, Event, Force, Ime, KeyboardInput, Touch, TouchPhase, WindowEvent},
    event_loop::{
        ControlFlow, DeviceEventFilter, EventLoopClosed, EventLoopWindowTarget as RootELW,
    },
    platform_impl::platform::{
        dark_mode::try_theme,
        dpi::{become_dpi_aware, dpi_to_scale_factor},
        drop_handler::FileDropHandler,
        event::{self, handle_extended_keys, process_key_params, vkey_to_winit_vkey},
        ime::ImeContext,
        monitor::{self, MonitorHandle},
        raw_input, util,
        window::InitData,
        window_state::{CursorFlags, ImeState, WindowFlags, WindowState},
        wrap_device_id, Fullscreen, WindowId, DEVICE_ID,
    },
    window::WindowId as RootWindowId,
};
use runner::{EventLoopRunner, EventLoopRunnerShared};

use super::window::set_skip_taskbar;

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

static GET_POINTER_FRAME_INFO_HISTORY: Lazy<Option<GetPointerFrameInfoHistory>> =
    Lazy::new(|| get_function!("user32.dll", GetPointerFrameInfoHistory));
static SKIP_POINTER_FRAME_MESSAGES: Lazy<Option<SkipPointerFrameMessages>> =
    Lazy::new(|| get_function!("user32.dll", SkipPointerFrameMessages));
static GET_POINTER_DEVICE_RECTS: Lazy<Option<GetPointerDeviceRects>> =
    Lazy::new(|| get_function!("user32.dll", GetPointerDeviceRects));
static GET_POINTER_TOUCH_INFO: Lazy<Option<GetPointerTouchInfo>> =
    Lazy::new(|| get_function!("user32.dll", GetPointerTouchInfo));
static GET_POINTER_PEN_INFO: Lazy<Option<GetPointerPenInfo>> =
    Lazy::new(|| get_function!("user32.dll", GetPointerPenInfo));

pub(crate) struct WindowData<T: 'static> {
    pub window_state: Arc<Mutex<WindowState>>,
    pub event_loop_runner: EventLoopRunnerShared<T>,
    pub _file_drop_handler: Option<FileDropHandler>,
    pub userdata_removed: Cell<bool>,
    pub recurse_depth: Cell<u32>,
}

impl<T> WindowData<T> {
    unsafe fn send_event(&self, event: Event<'_, T>) {
        self.event_loop_runner.send_event(event);
    }

    fn window_state_lock(&self) -> MutexGuard<'_, WindowState> {
        self.window_state.lock().unwrap()
    }
}

struct ThreadMsgTargetData<T: 'static> {
    event_loop_runner: EventLoopRunnerShared<T>,
    user_event_receiver: Receiver<T>,
}

impl<T> ThreadMsgTargetData<T> {
    unsafe fn send_event(&self, event: Event<'_, T>) {
        self.event_loop_runner.send_event(event);
    }
}

pub struct EventLoop<T: 'static> {
    thread_msg_sender: Sender<T>,
    window_target: RootELW<T>,
    msg_hook: Option<Box<dyn FnMut(*const c_void) -> bool + 'static>>,
}

pub(crate) struct PlatformSpecificEventLoopAttributes {
    pub(crate) any_thread: bool,
    pub(crate) dpi_aware: bool,
    pub(crate) msg_hook: Option<Box<dyn FnMut(*const c_void) -> bool + 'static>>,
}

impl Default for PlatformSpecificEventLoopAttributes {
    fn default() -> Self {
        Self {
            any_thread: false,
            dpi_aware: true,
            msg_hook: None,
        }
    }
}

pub struct EventLoopWindowTarget<T: 'static> {
    thread_id: u32,
    thread_msg_target: HWND,
    pub(crate) runner_shared: EventLoopRunnerShared<T>,
}

impl<T: 'static> EventLoop<T> {
    pub(crate) fn new(attributes: &mut PlatformSpecificEventLoopAttributes) -> Self {
        let thread_id = unsafe { GetCurrentThreadId() };

        if !attributes.any_thread && thread_id != main_thread_id() {
            panic!(
                "Initializing the event loop outside of the main thread is a significant \
                 cross-platform compatibility hazard. If you absolutely need to create an \
                 EventLoop on a different thread, you can use the \
                 `EventLoopBuilderExtWindows::any_thread` function."
            );
        }

        if attributes.dpi_aware {
            become_dpi_aware();
        }

        let thread_msg_target = create_event_target_window::<T>();

        thread::Builder::new()
            .name("winit wait thread".to_string())
            .spawn(move || wait_thread(thread_id, thread_msg_target))
            .expect("Failed to spawn winit wait thread");
        let wait_thread_id = get_wait_thread_id();

        let runner_shared = Rc::new(EventLoopRunner::new(thread_msg_target, wait_thread_id));

        let thread_msg_sender =
            insert_event_target_window_data::<T>(thread_msg_target, runner_shared.clone());
        raw_input::register_all_mice_and_keyboards_for_raw_input(
            thread_msg_target,
            Default::default(),
        );

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
            msg_hook: attributes.msg_hook.take(),
        }
    }

    pub fn window_target(&self) -> &RootELW<T> {
        &self.window_target
    }

    pub fn run<F>(mut self, event_handler: F) -> !
    where
        F: 'static + FnMut(Event<'_, T>, &RootELW<T>, &mut ControlFlow),
    {
        let exit_code = self.run_return(event_handler);
        ::std::process::exit(exit_code);
    }

    pub fn run_return<F>(&mut self, mut event_handler: F) -> i32
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

        let exit_code = unsafe {
            let mut msg = mem::zeroed();

            runner.poll();
            'main: loop {
                if GetMessageW(&mut msg, 0, 0, 0) == false.into() {
                    break 'main 0;
                }

                let handled = if let Some(callback) = self.msg_hook.as_deref_mut() {
                    callback(&mut msg as *mut _ as *mut _)
                } else {
                    false
                };
                if !handled {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }

                if let Err(payload) = runner.take_panic_error() {
                    runner.reset_runner();
                    panic::resume_unwind(payload);
                }

                if let ControlFlow::ExitWithCode(code) = runner.control_flow() {
                    if !runner.handling_events() {
                        break 'main code;
                    }
                }
            }
        };

        unsafe {
            runner.loop_destroyed();
        }

        runner.reset_runner();
        exit_code
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

    pub fn primary_monitor(&self) -> Option<MonitorHandle> {
        let monitor = monitor::primary_monitor();
        Some(monitor)
    }

    pub fn raw_display_handle(&self) -> RawDisplayHandle {
        RawDisplayHandle::Windows(WindowsDisplayHandle::empty())
    }

    pub fn set_device_event_filter(&self, filter: DeviceEventFilter) {
        raw_input::register_all_mice_and_keyboards_for_raw_input(self.thread_msg_target, filter);
    }
}

/// Returns the id of the main thread.
///
/// Windows has no real API to check if the current executing thread is the "main thread", unlike
/// macOS.
///
/// Windows will let us look up the current thread's id, but there's no API that lets us check what
/// the id of the main thread is. We would somehow need to get the main thread's id before a
/// developer could spin off any other threads inside of the main entrypoint in order to emulate the
/// capabilities of other platforms.
///
/// We can get the id of the main thread by using CRT initialization. CRT initialization can be used
/// to setup global state within a program. The OS will call a list of function pointers which
/// assign values to a static variable. To have get a hold of the main thread id, we need to place
/// our function pointer inside of the `.CRT$XCU` section so it is called before the main
/// entrypoint.
///
/// Full details of CRT initialization can be found here:
/// <https://docs.microsoft.com/en-us/cpp/c-runtime-library/crt-initialization?view=msvc-160>
fn main_thread_id() -> u32 {
    static mut MAIN_THREAD_ID: u32 = 0;

    /// Function pointer used in CRT initialization section to set the above static field's value.

    // Mark as used so this is not removable.
    #[used]
    #[allow(non_upper_case_globals)]
    // Place the function pointer inside of CRT initialization section so it is loaded before
    // main entrypoint.
    //
    // See: https://doc.rust-lang.org/stable/reference/abi.html#the-link_section-attribute
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
    unsafe {
        let mut msg = mem::zeroed();
        let result = GetMessageW(
            &mut msg,
            -1,
            SEND_WAIT_THREAD_ID_MSG_ID.get(),
            SEND_WAIT_THREAD_ID_MSG_ID.get(),
        );
        assert_eq!(
            msg.message,
            SEND_WAIT_THREAD_ID_MSG_ID.get(),
            "this shouldn't be possible. please open an issue with Winit. error code: {result}"
        );
        msg.lParam as u32
    }
}

static WAIT_PERIOD_MIN: Lazy<Option<u32>> = Lazy::new(|| unsafe {
    let mut caps = TIMECAPS {
        wPeriodMin: 0,
        wPeriodMax: 0,
    };
    if timeGetDevCaps(&mut caps, mem::size_of::<TIMECAPS>() as u32) == TIMERR_NOERROR {
        Some(caps.wPeriodMin)
    } else {
        None
    }
});

fn wait_thread(parent_thread_id: u32, msg_window_id: HWND) {
    unsafe {
        let mut msg: MSG;

        let cur_thread_id = GetCurrentThreadId();
        PostThreadMessageW(
            parent_thread_id,
            SEND_WAIT_THREAD_ID_MSG_ID.get(),
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
                if PeekMessageW(&mut msg, 0, 0, 0, PM_REMOVE) != false.into() {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            } else if GetMessageW(&mut msg, 0, 0, 0) == false.into() {
                break 'main;
            } else {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            if msg.message == WAIT_UNTIL_MSG_ID.get() {
                wait_until_opt = Some(*WaitUntilInstantBox::from_raw(msg.lParam as *mut _));
            } else if msg.message == CANCEL_WAIT_UNTIL_MSG_ID.get() {
                wait_until_opt = None;
            }

            if let Some(wait_until) = wait_until_opt {
                let now = Instant::now();
                if now < wait_until {
                    // Windows' scheduler has a default accuracy of several ms. This isn't good enough for
                    // `WaitUntil`, so we request the Windows scheduler to use a higher accuracy if possible.
                    // If we couldn't query the timer capabilities, then we use the default resolution.
                    if let Some(period) = *WAIT_PERIOD_MIN {
                        timeBeginPeriod(period);
                    }
                    // `MsgWaitForMultipleObjects` is bound by the granularity of the scheduler period.
                    // Because of this, we try to reduce the requested time just enough to undershoot `wait_until`
                    // by the smallest amount possible, and then we busy loop for the remaining time inside the
                    // NewEvents message handler.
                    let resume_reason = MsgWaitForMultipleObjectsEx(
                        0,
                        ptr::null(),
                        dur2timeout(wait_until - now).saturating_sub(WAIT_PERIOD_MIN.unwrap_or(1)),
                        QS_ALLEVENTS,
                        MWMO_INPUTAVAILABLE,
                    );
                    if let Some(period) = *WAIT_PERIOD_MIN {
                        timeEndPeriod(period);
                    }
                    if resume_reason == WAIT_TIMEOUT {
                        PostMessageW(msg_window_id, PROCESS_NEW_EVENTS_MSG_ID.get(), 0, 0);
                        wait_until_opt = None;
                    }
                } else {
                    PostMessageW(msg_window_id, PROCESS_NEW_EVENTS_MSG_ID.get(), 0, 0);
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
            if dur.subsec_nanos() % 1_000_000 > 0 {
                ms.checked_add(1)
            } else {
                Some(ms)
            }
        })
        .map(|ms| {
            if ms > u32::MAX as u64 {
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
                let boxed2: ThreadExecFn = Box::new(Box::new(function));

                let raw = Box::into_raw(boxed2);

                let res = PostMessageW(self.target_window, EXEC_MSG_ID.get(), raw as usize, 0);
                assert!(
                    res != false.into(),
                    "PostMessage failed; is the messages queue full?"
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
            if PostMessageW(self.target_window, USER_EVENT_MSG_ID.get(), 0, 0) != false.into() {
                self.event_send.send(event).ok();
                Ok(())
            } else {
                Err(EventLoopClosed(event))
            }
        }
    }
}

type WaitUntilInstantBox = Box<Instant>;

/// A lazily-initialized window message ID.
pub struct LazyMessageId {
    /// The ID.
    id: AtomicU32,

    /// The name of the message.
    name: &'static str,
}

/// An invalid custom window ID.
const INVALID_ID: u32 = 0x0;

impl LazyMessageId {
    /// Create a new `LazyId`.
    const fn new(name: &'static str) -> Self {
        Self {
            id: AtomicU32::new(INVALID_ID),
            name,
        }
    }

    /// Get the message ID.
    pub fn get(&self) -> u32 {
        // Load the ID.
        let id = self.id.load(Ordering::Relaxed);

        if id != INVALID_ID {
            return id;
        }

        // Register the message.
        // SAFETY: We are sure that the pointer is a valid C string ending with '\0'.
        assert!(self.name.ends_with('\0'));
        let new_id = unsafe { RegisterWindowMessageA(self.name.as_ptr()) };

        assert_ne!(
            new_id,
            0,
            "RegisterWindowMessageA returned zero for '{}': {}",
            self.name,
            std::io::Error::last_os_error()
        );

        // Store the new ID. Since `RegisterWindowMessageA` returns the same value for any given string,
        // the target value will always either be a). `INVALID_ID` or b). the correct ID. Therefore a
        // compare-and-swap operation here (or really any consideration) is never necessary.
        self.id.store(new_id, Ordering::Relaxed);

        new_id
    }
}

// Message sent by the `EventLoopProxy` when we want to wake up the thread.
// WPARAM and LPARAM are unused.
static USER_EVENT_MSG_ID: LazyMessageId = LazyMessageId::new("Winit::WakeupMsg\0");
// Message sent when we want to execute a closure in the thread.
// WPARAM contains a Box<Box<dyn FnMut()>> that must be retrieved with `Box::from_raw`,
// and LPARAM is unused.
static EXEC_MSG_ID: LazyMessageId = LazyMessageId::new("Winit::ExecMsg\0");
static PROCESS_NEW_EVENTS_MSG_ID: LazyMessageId = LazyMessageId::new("Winit::ProcessNewEvents\0");
/// lparam is the wait thread's message id.
static SEND_WAIT_THREAD_ID_MSG_ID: LazyMessageId = LazyMessageId::new("Winit::SendWaitThreadId\0");
/// lparam points to a `Box<Instant>` signifying the time `PROCESS_NEW_EVENTS_MSG_ID` should
/// be sent.
static WAIT_UNTIL_MSG_ID: LazyMessageId = LazyMessageId::new("Winit::WaitUntil\0");
static CANCEL_WAIT_UNTIL_MSG_ID: LazyMessageId = LazyMessageId::new("Winit::CancelWaitUntil\0");
// Message sent by a `Window` when it wants to be destroyed by the main thread.
// WPARAM and LPARAM are unused.
pub static DESTROY_MSG_ID: LazyMessageId = LazyMessageId::new("Winit::DestroyMsg\0");
// WPARAM is a bool specifying the `WindowFlags::MARKER_RETAIN_STATE_ON_SIZE` flag. See the
// documentation in the `window_state` module for more information.
pub static SET_RETAIN_STATE_ON_SIZE_MSG_ID: LazyMessageId =
    LazyMessageId::new("Winit::SetRetainMaximized\0");
static THREAD_EVENT_TARGET_WINDOW_CLASS: Lazy<Vec<u16>> =
    Lazy::new(|| util::encode_wide("Winit Thread Event Target"));
/// When the taskbar is created, it registers a message with the "TaskbarCreated" string and then broadcasts this message to all top-level windows
/// <https://docs.microsoft.com/en-us/windows/win32/shell/taskbar#taskbar-creation-notification>
pub static TASKBAR_CREATED: LazyMessageId = LazyMessageId::new("TaskbarCreated\0");

fn create_event_target_window<T: 'static>() -> HWND {
    use windows_sys::Win32::UI::WindowsAndMessaging::CS_HREDRAW;
    use windows_sys::Win32::UI::WindowsAndMessaging::CS_VREDRAW;
    unsafe {
        let class = WNDCLASSEXW {
            cbSize: mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(thread_event_target_callback::<T>),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: util::get_instance_handle(),
            hIcon: 0,
            hCursor: 0, // must be null in order for cursor state to work properly
            hbrBackground: 0,
            lpszMenuName: ptr::null(),
            lpszClassName: THREAD_EVENT_TARGET_WINDOW_CLASS.as_ptr(),
            hIconSm: 0,
        };

        RegisterClassExW(&class);
    }

    unsafe {
        let window = CreateWindowExW(
            WS_EX_NOACTIVATE
                | WS_EX_TRANSPARENT
                | WS_EX_LAYERED
                // WS_EX_TOOLWINDOW prevents this window from ever showing up in the taskbar, which
                // we want to avoid. If you remove this style, this window won't show up in the
                // taskbar *initially*, but it can show up at some later point. This can sometimes
                // happen on its own after several hours have passed, although this has proven
                // difficult to reproduce. Alternatively, it can be manually triggered by killing
                // `explorer.exe` and then starting the process back up.
                // It is unclear why the bug is triggered by waiting for several hours.
                | WS_EX_TOOLWINDOW,
            THREAD_EVENT_TARGET_WINDOW_CLASS.as_ptr(),
            ptr::null(),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            0,
            0,
            util::get_instance_handle(),
            ptr::null(),
        );

        super::set_window_long(
            window,
            GWL_STYLE,
            // The window technically has to be visible to receive WM_PAINT messages (which are used
            // for delivering events during resizes), but it isn't displayed to the user because of
            // the LAYERED style.
            (WS_VISIBLE | WS_POPUP) as isize,
        );
        window
    }
}

fn insert_event_target_window_data<T>(
    thread_msg_target: HWND,
    event_loop_runner: EventLoopRunnerShared<T>,
) -> Sender<T> {
    let (tx, rx) = mpsc::channel();

    let userdata = ThreadMsgTargetData {
        event_loop_runner,
        user_event_receiver: rx,
    };
    let input_ptr = Box::into_raw(Box::new(userdata));

    unsafe { super::set_window_long(thread_msg_target, GWL_USERDATA, input_ptr as isize) };

    tx
}

/// Capture mouse input, allowing `window` to receive mouse events when the cursor is outside of
/// the window.
unsafe fn capture_mouse(window: HWND, window_state: &mut WindowState) {
    window_state.mouse.capture_count += 1;
    SetCapture(window);
}

/// Release mouse input, stopping windows on this thread from receiving mouse input when the cursor
/// is outside the window.
unsafe fn release_mouse(mut window_state: MutexGuard<'_, WindowState>) {
    window_state.mouse.capture_count = window_state.mouse.capture_count.saturating_sub(1);
    if window_state.mouse.capture_count == 0 {
        // ReleaseCapture() causes a WM_CAPTURECHANGED where we lock the window_state.
        drop(window_state);
        ReleaseCapture();
    }
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

            if PeekMessageW(
                &mut msg,
                redraw_window,
                WM_PAINT,
                WM_PAINT,
                PM_REMOVE | PM_QS_PAINT,
            ) == false.into()
            {
                return;
            }

            TranslateMessage(&msg);
            DispatchMessageW(&msg);
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
                PROCESS_NEW_EVENTS_MSG_ID.get(),
                0,
                0,
            );
        }
        ControlFlow::Wait => (),
        ControlFlow::WaitUntil(until) => {
            PostThreadMessageW(
                runner.wait_thread_id(),
                WAIT_UNTIL_MSG_ID.get(),
                0,
                Box::into_raw(WaitUntilInstantBox::new(until)) as isize,
            );
        }
        ControlFlow::ExitWithCode(_) => (),
    }
}

/// Emit a `ModifiersChanged` event whenever modifiers have changed.
fn update_modifiers<T>(window: HWND, userdata: &WindowData<T>) {
    use crate::event::WindowEvent::ModifiersChanged;

    let modifiers = event::get_key_mods();
    let mut window_state = userdata.window_state_lock();
    if window_state.modifiers_state != modifiers {
        window_state.modifiers_state = modifiers;

        // Drop lock
        drop(window_state);

        unsafe {
            userdata.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: ModifiersChanged(modifiers),
            });
        }
    }
}

unsafe fn gain_active_focus<T>(window: HWND, userdata: &WindowData<T>) {
    use crate::event::{ElementState::Released, WindowEvent::Focused};
    for windows_keycode in event::get_pressed_keys() {
        let scancode = MapVirtualKeyA(windows_keycode as u32, MAPVK_VK_TO_VSC);
        let virtual_keycode = event::vkey_to_winit_vkey(windows_keycode);

        update_modifiers(window, userdata);

        #[allow(deprecated)]
        userdata.send_event(Event::WindowEvent {
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

    userdata.send_event(Event::WindowEvent {
        window_id: RootWindowId(WindowId(window)),
        event: Focused(true),
    });
}

unsafe fn lose_active_focus<T>(window: HWND, userdata: &WindowData<T>) {
    use crate::event::{
        ElementState::Released,
        ModifiersState,
        WindowEvent::{Focused, ModifiersChanged},
    };
    for windows_keycode in event::get_pressed_keys() {
        let scancode = MapVirtualKeyA(windows_keycode as u32, MAPVK_VK_TO_VSC);
        let virtual_keycode = event::vkey_to_winit_vkey(windows_keycode);

        #[allow(deprecated)]
        userdata.send_event(Event::WindowEvent {
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

    userdata.window_state_lock().modifiers_state = ModifiersState::empty();
    userdata.send_event(Event::WindowEvent {
        window_id: RootWindowId(WindowId(window)),
        event: ModifiersChanged(ModifiersState::empty()),
    });

    userdata.send_event(Event::WindowEvent {
        window_id: RootWindowId(WindowId(window)),
        event: Focused(false),
    });
}

/// Any window whose callback is configured to this function will have its events propagated
/// through the events loop of the thread the window was created in.
//
// This is the callback that is called by `DispatchMessage` in the events loop.
//
// Returning 0 tells the Win32 API that the message has been processed.
// FIXME: detect WM_DWMCOMPOSITIONCHANGED and call DwmEnableBlurBehindWindow if necessary
pub(super) unsafe extern "system" fn public_window_callback<T: 'static>(
    window: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let userdata = super::get_window_long(window, GWL_USERDATA);

    let userdata_ptr = match (userdata, msg) {
        (0, WM_NCCREATE) => {
            let createstruct = &mut *(lparam as *mut CREATESTRUCTW);
            let initdata = &mut *(createstruct.lpCreateParams as *mut InitData<'_, T>);

            let result = match initdata.on_nccreate(window) {
                Some(userdata) => {
                    super::set_window_long(window, GWL_USERDATA, userdata as _);
                    DefWindowProcW(window, msg, wparam, lparam)
                }
                None => -1, // failed to create the window
            };

            return result;
        }
        // Getting here should quite frankly be impossible,
        // but we'll make window creation fail here just in case.
        (0, WM_CREATE) => return -1,
        (_, WM_CREATE) => {
            let createstruct = &mut *(lparam as *mut CREATESTRUCTW);
            let initdata = createstruct.lpCreateParams;
            let initdata = &mut *(initdata as *mut InitData<'_, T>);

            initdata.on_create();
            return DefWindowProcW(window, msg, wparam, lparam);
        }
        (0, _) => return DefWindowProcW(window, msg, wparam, lparam),
        _ => userdata as *mut WindowData<T>,
    };

    let (result, userdata_removed, recurse_depth) = {
        let userdata = &*(userdata_ptr);

        userdata.recurse_depth.set(userdata.recurse_depth.get() + 1);

        let result = public_window_callback_inner(window, msg, wparam, lparam, userdata);

        let userdata_removed = userdata.userdata_removed.get();
        let recurse_depth = userdata.recurse_depth.get() - 1;
        userdata.recurse_depth.set(recurse_depth);

        (result, userdata_removed, recurse_depth)
    };

    if userdata_removed && recurse_depth == 0 {
        drop(Box::from_raw(userdata_ptr));
    }

    result
}

unsafe fn public_window_callback_inner<T: 'static>(
    window: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    userdata: &WindowData<T>,
) -> LRESULT {
    RedrawWindow(
        userdata.event_loop_runner.thread_msg_target(),
        ptr::null(),
        0,
        RDW_INTERNALPAINT,
    );

    // I decided to bind the closure to `callback` and pass it to catch_unwind rather than passing
    // the closure to catch_unwind directly so that the match body indendation wouldn't change and
    // the git blame and history would be preserved.
    let callback = || match msg {
        WM_NCCALCSIZE => {
            let window_flags = userdata.window_state_lock().window_flags;
            if wparam == 0 || window_flags.contains(WindowFlags::MARKER_DECORATIONS) {
                return DefWindowProcW(window, msg, wparam, lparam);
            }

            let params = &mut *(lparam as *mut NCCALCSIZE_PARAMS);

            if util::is_maximized(window) {
                // Limit the window size when maximized to the current monitor.
                // Otherwise it would include the non-existent decorations.
                //
                // Use `MonitorFromRect` instead of `MonitorFromWindow` to select
                // the correct monitor here.
                // See https://github.com/MicrosoftEdge/WebView2Feedback/issues/2549
                let monitor = MonitorFromRect(&params.rgrc[0], MONITOR_DEFAULTTONULL);
                if let Ok(monitor_info) = monitor::get_monitor_info(monitor) {
                    params.rgrc[0] = monitor_info.monitorInfo.rcWork;
                }
            } else if window_flags.contains(WindowFlags::MARKER_UNDECORATED_SHADOW) {
                // Extend the client area to cover the whole non-client area.
                // https://docs.microsoft.com/en-us/windows/win32/winmsg/wm-nccalcsize#remarks
                //
                // HACK(msiglreith): To add the drop shadow we slightly tweak the non-client area.
                // This leads to a small black 1px border on the top. Adding a margin manually
                // on all 4 borders would result in the caption getting drawn by the DWM.
                //
                // Another option would be to allow the DWM to paint inside the client area.
                // Unfortunately this results in janky resize behavior, where the compositor is
                // ahead of the window surface. Currently, there seems no option to achieve this
                // with the Windows API.
                params.rgrc[0].top += 1;
                params.rgrc[0].bottom += 1;
            }

            0
        }

        WM_ENTERSIZEMOVE => {
            userdata
                .window_state_lock()
                .set_window_flags_in_place(|f| f.insert(WindowFlags::MARKER_IN_SIZE_MOVE));
            0
        }

        WM_EXITSIZEMOVE => {
            let mut state = userdata.window_state_lock();
            if state.dragging {
                state.dragging = false;
                PostMessageW(window, WM_LBUTTONUP, 0, lparam);
            }

            state.set_window_flags_in_place(|f| f.remove(WindowFlags::MARKER_IN_SIZE_MOVE));
            0
        }

        WM_NCLBUTTONDOWN => {
            if wparam == HTCAPTION as _ {
                PostMessageW(window, WM_MOUSEMOVE, 0, lparam);
            }
            DefWindowProcW(window, msg, wparam, lparam)
        }

        WM_CLOSE => {
            use crate::event::WindowEvent::CloseRequested;
            userdata.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: CloseRequested,
            });
            0
        }

        WM_DESTROY => {
            use crate::event::WindowEvent::Destroyed;
            RevokeDragDrop(window);
            userdata.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: Destroyed,
            });
            userdata.event_loop_runner.remove_window(window);
            0
        }

        WM_NCDESTROY => {
            super::set_window_long(window, GWL_USERDATA, 0);
            userdata.userdata_removed.set(true);
            0
        }

        WM_PAINT => {
            if userdata.event_loop_runner.should_buffer() {
                // this branch can happen in response to `UpdateWindow`, if win32 decides to
                // redraw the window outside the normal flow of the event loop.
                RedrawWindow(window, ptr::null(), 0, RDW_INTERNALPAINT);
            } else {
                let managing_redraw =
                    flush_paint_messages(Some(window), &userdata.event_loop_runner);
                userdata.send_event(Event::RedrawRequested(RootWindowId(WindowId(window))));
                if managing_redraw {
                    userdata.event_loop_runner.redraw_events_cleared();
                    process_control_flow(&userdata.event_loop_runner);
                }
            }

            DefWindowProcW(window, msg, wparam, lparam)
        }

        WM_WINDOWPOSCHANGING => {
            let mut window_state = userdata.window_state_lock();
            if let Some(ref mut fullscreen) = window_state.fullscreen {
                let window_pos = &mut *(lparam as *mut WINDOWPOS);
                let new_rect = RECT {
                    left: window_pos.x,
                    top: window_pos.y,
                    right: window_pos.x + window_pos.cx,
                    bottom: window_pos.y + window_pos.cy,
                };

                const NOMOVE_OR_NOSIZE: u32 = SWP_NOMOVE | SWP_NOSIZE;

                let new_rect = if window_pos.flags & NOMOVE_OR_NOSIZE != 0 {
                    let cur_rect = util::WindowArea::Outer.get_rect(window)
                        .expect("Unexpected GetWindowRect failure; please report this error to https://github.com/rust-windowing/winit");

                    match window_pos.flags & NOMOVE_OR_NOSIZE {
                        NOMOVE_OR_NOSIZE => None,

                        SWP_NOMOVE => Some(RECT {
                            left: cur_rect.left,
                            top: cur_rect.top,
                            right: cur_rect.left + window_pos.cx,
                            bottom: cur_rect.top + window_pos.cy,
                        }),

                        SWP_NOSIZE => Some(RECT {
                            left: window_pos.x,
                            top: window_pos.y,
                            right: window_pos.x - cur_rect.left + cur_rect.right,
                            bottom: window_pos.y - cur_rect.top + cur_rect.bottom,
                        }),

                        _ => unreachable!(),
                    }
                } else {
                    Some(new_rect)
                };

                if let Some(new_rect) = new_rect {
                    let new_monitor = MonitorFromRect(&new_rect, MONITOR_DEFAULTTONULL);
                    match fullscreen {
                        Fullscreen::Borderless(ref mut fullscreen_monitor) => {
                            if new_monitor != 0
                                && fullscreen_monitor
                                    .as_ref()
                                    .map(|monitor| new_monitor != monitor.hmonitor())
                                    .unwrap_or(true)
                            {
                                if let Ok(new_monitor_info) = monitor::get_monitor_info(new_monitor)
                                {
                                    let new_monitor_rect = new_monitor_info.monitorInfo.rcMonitor;
                                    window_pos.x = new_monitor_rect.left;
                                    window_pos.y = new_monitor_rect.top;
                                    window_pos.cx = new_monitor_rect.right - new_monitor_rect.left;
                                    window_pos.cy = new_monitor_rect.bottom - new_monitor_rect.top;
                                }
                                *fullscreen_monitor = Some(MonitorHandle::new(new_monitor));
                            }
                        }
                        Fullscreen::Exclusive(ref video_mode) => {
                            let old_monitor = video_mode.monitor.hmonitor();
                            if let Ok(old_monitor_info) = monitor::get_monitor_info(old_monitor) {
                                let old_monitor_rect = old_monitor_info.monitorInfo.rcMonitor;
                                window_pos.x = old_monitor_rect.left;
                                window_pos.y = old_monitor_rect.top;
                                window_pos.cx = old_monitor_rect.right - old_monitor_rect.left;
                                window_pos.cy = old_monitor_rect.bottom - old_monitor_rect.top;
                            }
                        }
                    }
                }
            }

            0
        }

        // WM_MOVE supplies client area positions, so we send Moved here instead.
        WM_WINDOWPOSCHANGED => {
            use crate::event::WindowEvent::Moved;

            let windowpos = lparam as *const WINDOWPOS;
            if (*windowpos).flags & SWP_NOMOVE != SWP_NOMOVE {
                let physical_position = PhysicalPosition::new((*windowpos).x, (*windowpos).y);
                userdata.send_event(Event::WindowEvent {
                    window_id: RootWindowId(WindowId(window)),
                    event: Moved(physical_position),
                });
            }

            // This is necessary for us to still get sent WM_SIZE.
            DefWindowProcW(window, msg, wparam, lparam)
        }

        WM_SIZE => {
            use crate::event::WindowEvent::Resized;
            let w = super::loword(lparam as u32) as u32;
            let h = super::hiword(lparam as u32) as u32;

            let physical_size = PhysicalSize::new(w, h);
            let event = Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: Resized(physical_size),
            };

            {
                let mut w = userdata.window_state_lock();
                // See WindowFlags::MARKER_RETAIN_STATE_ON_SIZE docs for info on why this `if` check exists.
                if !w
                    .window_flags()
                    .contains(WindowFlags::MARKER_RETAIN_STATE_ON_SIZE)
                {
                    let maximized = wparam == SIZE_MAXIMIZED as usize;
                    w.set_window_flags_in_place(|f| f.set(WindowFlags::MAXIMIZED, maximized));
                }
            }
            userdata.send_event(event);
            0
        }

        WM_CHAR | WM_SYSCHAR => {
            use crate::event::WindowEvent::ReceivedCharacter;
            use std::char;
            let is_high_surrogate = (0xD800..=0xDBFF).contains(&wparam);
            let is_low_surrogate = (0xDC00..=0xDFFF).contains(&wparam);

            if is_high_surrogate {
                userdata.window_state_lock().high_surrogate = Some(wparam as u16);
            } else if is_low_surrogate {
                let high_surrogate = userdata.window_state_lock().high_surrogate.take();

                if let Some(high_surrogate) = high_surrogate {
                    let pair = [high_surrogate, wparam as u16];
                    if let Some(Ok(chr)) = char::decode_utf16(pair.iter().copied()).next() {
                        userdata.send_event(Event::WindowEvent {
                            window_id: RootWindowId(WindowId(window)),
                            event: ReceivedCharacter(chr),
                        });
                    }
                }
            } else {
                userdata.window_state_lock().high_surrogate = None;

                if let Some(chr) = char::from_u32(wparam as u32) {
                    userdata.send_event(Event::WindowEvent {
                        window_id: RootWindowId(WindowId(window)),
                        event: ReceivedCharacter(chr),
                    });
                }
            }

            // todo(msiglreith):
            //   Ideally, `WM_SYSCHAR` shouldn't emit a `ReceivedChar` event
            //   indicating user text input. As we lack dedicated support
            //   accelerators/keybindings these events will be additionally
            //   emitted for downstream users.
            //   This means certain key combinations (ie Alt + Space) will
            //   trigger the default system behavior **and** emit a char event.
            if msg == WM_SYSCHAR {
                DefWindowProcW(window, msg, wparam, lparam)
            } else {
                0
            }
        }

        WM_MENUCHAR => (MNC_CLOSE << 16) as isize,

        WM_IME_STARTCOMPOSITION => {
            let ime_allowed = userdata.window_state_lock().ime_allowed;
            if ime_allowed {
                userdata.window_state_lock().ime_state = ImeState::Enabled;

                userdata.send_event(Event::WindowEvent {
                    window_id: RootWindowId(WindowId(window)),
                    event: WindowEvent::Ime(Ime::Enabled),
                });
            }

            DefWindowProcW(window, msg, wparam, lparam)
        }

        WM_IME_COMPOSITION => {
            let ime_allowed_and_composing = {
                let w = userdata.window_state_lock();
                w.ime_allowed && w.ime_state != ImeState::Disabled
            };
            // Windows Hangul IME sends WM_IME_COMPOSITION after WM_IME_ENDCOMPOSITION, so
            // check whether composing.
            if ime_allowed_and_composing {
                let ime_context = ImeContext::current(window);

                if lparam == 0 {
                    userdata.send_event(Event::WindowEvent {
                        window_id: RootWindowId(WindowId(window)),
                        event: WindowEvent::Ime(Ime::Preedit(String::new(), None)),
                    });
                }

                // Google Japanese Input and ATOK have both flags, so
                // first, receive composing result if exist.
                if (lparam as u32 & GCS_RESULTSTR) != 0 {
                    if let Some(text) = ime_context.get_composed_text() {
                        userdata.window_state_lock().ime_state = ImeState::Enabled;

                        userdata.send_event(Event::WindowEvent {
                            window_id: RootWindowId(WindowId(window)),
                            event: WindowEvent::Ime(Ime::Preedit(String::new(), None)),
                        });
                        userdata.send_event(Event::WindowEvent {
                            window_id: RootWindowId(WindowId(window)),
                            event: WindowEvent::Ime(Ime::Commit(text)),
                        });
                    }
                }

                // Next, receive preedit range for next composing if exist.
                if (lparam as u32 & GCS_COMPSTR) != 0 {
                    if let Some((text, first, last)) = ime_context.get_composing_text_and_cursor() {
                        userdata.window_state_lock().ime_state = ImeState::Preedit;
                        let cursor_range = first.map(|f| (f, last.unwrap_or(f)));

                        userdata.send_event(Event::WindowEvent {
                            window_id: RootWindowId(WindowId(window)),
                            event: WindowEvent::Ime(Ime::Preedit(text, cursor_range)),
                        });
                    }
                }
            }

            // Not calling DefWindowProc to hide composing text drawn by IME.
            0
        }

        WM_IME_ENDCOMPOSITION => {
            let ime_allowed_or_composing = {
                let w = userdata.window_state_lock();
                w.ime_allowed || w.ime_state != ImeState::Disabled
            };
            if ime_allowed_or_composing {
                if userdata.window_state_lock().ime_state == ImeState::Preedit {
                    // Windows Hangul IME sends WM_IME_COMPOSITION after WM_IME_ENDCOMPOSITION, so
                    // trying receiving composing result and commit if exists.
                    let ime_context = ImeContext::current(window);
                    if let Some(text) = ime_context.get_composed_text() {
                        userdata.send_event(Event::WindowEvent {
                            window_id: RootWindowId(WindowId(window)),
                            event: WindowEvent::Ime(Ime::Preedit(String::new(), None)),
                        });
                        userdata.send_event(Event::WindowEvent {
                            window_id: RootWindowId(WindowId(window)),
                            event: WindowEvent::Ime(Ime::Commit(text)),
                        });
                    }
                }

                userdata.window_state_lock().ime_state = ImeState::Disabled;

                userdata.send_event(Event::WindowEvent {
                    window_id: RootWindowId(WindowId(window)),
                    event: WindowEvent::Ime(Ime::Disabled),
                });
            }

            DefWindowProcW(window, msg, wparam, lparam)
        }

        WM_IME_SETCONTEXT => {
            // Hide composing text drawn by IME.
            let wparam = wparam & (!ISC_SHOWUICOMPOSITIONWINDOW as usize);

            DefWindowProcW(window, msg, wparam, lparam)
        }

        // this is necessary for us to maintain minimize/restore state
        WM_SYSCOMMAND => {
            if wparam == SC_RESTORE as usize {
                let mut w = userdata.window_state_lock();
                w.set_window_flags_in_place(|f| f.set(WindowFlags::MINIMIZED, false));
            }
            if wparam == SC_MINIMIZE as usize {
                let mut w = userdata.window_state_lock();
                w.set_window_flags_in_place(|f| f.set(WindowFlags::MINIMIZED, true));
            }
            // Send `WindowEvent::Minimized` here if we decide to implement one

            if wparam == SC_SCREENSAVE as usize {
                let window_state = userdata.window_state_lock();
                if window_state.fullscreen.is_some() {
                    return 0;
                }
            }

            DefWindowProcW(window, msg, wparam, lparam)
        }

        WM_MOUSEMOVE => {
            use crate::event::WindowEvent::{CursorEntered, CursorMoved};
            let mouse_was_outside_window = {
                let mut w = userdata.window_state_lock();

                let was_outside_window = !w.mouse.cursor_flags().contains(CursorFlags::IN_WINDOW);
                w.mouse
                    .set_cursor_flags(window, |f| f.set(CursorFlags::IN_WINDOW, true))
                    .ok();
                was_outside_window
            };

            if mouse_was_outside_window {
                userdata.send_event(Event::WindowEvent {
                    window_id: RootWindowId(WindowId(window)),
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

            let x = super::get_x_lparam(lparam as u32) as f64;
            let y = super::get_y_lparam(lparam as u32) as f64;
            let position = PhysicalPosition::new(x, y);
            let cursor_moved;
            {
                // handle spurious WM_MOUSEMOVE messages
                // see https://devblogs.microsoft.com/oldnewthing/20031001-00/?p=42343
                // and http://debugandconquer.blogspot.com/2015/08/the-cause-of-spurious-mouse-move.html
                let mut w = userdata.window_state_lock();
                cursor_moved = w.mouse.last_position != Some(position);
                w.mouse.last_position = Some(position);
            }
            if cursor_moved {
                update_modifiers(window, userdata);

                userdata.send_event(Event::WindowEvent {
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

        WM_MOUSELEAVE => {
            use crate::event::WindowEvent::CursorLeft;
            {
                let mut w = userdata.window_state_lock();
                w.mouse
                    .set_cursor_flags(window, |f| f.set(CursorFlags::IN_WINDOW, false))
                    .ok();
            }

            userdata.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: CursorLeft {
                    device_id: DEVICE_ID,
                },
            });

            0
        }

        WM_MOUSEWHEEL => {
            use crate::event::MouseScrollDelta::LineDelta;

            let value = (wparam >> 16) as i16;
            let value = value as i32;
            let value = value as f32 / WHEEL_DELTA as f32;

            update_modifiers(window, userdata);

            userdata.send_event(Event::WindowEvent {
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

        WM_MOUSEHWHEEL => {
            use crate::event::MouseScrollDelta::LineDelta;

            let value = (wparam >> 16) as i16;
            let value = value as i32;
            let value = -value as f32 / WHEEL_DELTA as f32; // NOTE: inverted! See https://github.com/rust-windowing/winit/pull/2105/

            update_modifiers(window, userdata);

            userdata.send_event(Event::WindowEvent {
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

        WM_KEYDOWN | WM_SYSKEYDOWN => {
            use crate::event::{ElementState::Pressed, VirtualKeyCode};
            if let Some((scancode, vkey)) = process_key_params(wparam, lparam) {
                update_modifiers(window, userdata);

                #[allow(deprecated)]
                userdata.send_event(Event::WindowEvent {
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
                    userdata.send_event(Event::WindowEvent {
                        window_id: RootWindowId(WindowId(window)),
                        event: WindowEvent::ReceivedCharacter('\u{7F}'),
                    });
                }
            }

            if msg == WM_SYSKEYDOWN {
                DefWindowProcW(window, msg, wparam, lparam)
            } else {
                0
            }
        }

        WM_KEYUP | WM_SYSKEYUP => {
            use crate::event::ElementState::Released;
            if let Some((scancode, vkey)) = process_key_params(wparam, lparam) {
                update_modifiers(window, userdata);

                #[allow(deprecated)]
                userdata.send_event(Event::WindowEvent {
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
            if msg == WM_SYSKEYUP && GetMenu(window) != 0 {
                // let Windows handle event if the window has a native menu, a modal event loop
                // is started here on Alt key up.
                DefWindowProcW(window, msg, wparam, lparam)
            } else {
                0
            }
        }

        WM_LBUTTONDOWN => {
            use crate::event::{ElementState::Pressed, MouseButton::Left, WindowEvent::MouseInput};

            capture_mouse(window, &mut userdata.window_state_lock());

            update_modifiers(window, userdata);

            userdata.send_event(Event::WindowEvent {
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

        WM_LBUTTONUP => {
            use crate::event::{
                ElementState::Released, MouseButton::Left, WindowEvent::MouseInput,
            };

            release_mouse(userdata.window_state_lock());

            update_modifiers(window, userdata);

            userdata.send_event(Event::WindowEvent {
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

        WM_RBUTTONDOWN => {
            use crate::event::{
                ElementState::Pressed, MouseButton::Right, WindowEvent::MouseInput,
            };

            capture_mouse(window, &mut userdata.window_state_lock());

            update_modifiers(window, userdata);

            userdata.send_event(Event::WindowEvent {
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

        WM_RBUTTONUP => {
            use crate::event::{
                ElementState::Released, MouseButton::Right, WindowEvent::MouseInput,
            };

            release_mouse(userdata.window_state_lock());

            update_modifiers(window, userdata);

            userdata.send_event(Event::WindowEvent {
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

        WM_MBUTTONDOWN => {
            use crate::event::{
                ElementState::Pressed, MouseButton::Middle, WindowEvent::MouseInput,
            };

            capture_mouse(window, &mut userdata.window_state_lock());

            update_modifiers(window, userdata);

            userdata.send_event(Event::WindowEvent {
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

        WM_MBUTTONUP => {
            use crate::event::{
                ElementState::Released, MouseButton::Middle, WindowEvent::MouseInput,
            };

            release_mouse(userdata.window_state_lock());

            update_modifiers(window, userdata);

            userdata.send_event(Event::WindowEvent {
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

        WM_XBUTTONDOWN => {
            use crate::event::{
                ElementState::Pressed, MouseButton::Other, WindowEvent::MouseInput,
            };
            let xbutton = super::get_xbutton_wparam(wparam as u32);

            capture_mouse(window, &mut userdata.window_state_lock());

            update_modifiers(window, userdata);

            userdata.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: MouseInput {
                    device_id: DEVICE_ID,
                    state: Pressed,
                    button: Other(xbutton),
                    modifiers: event::get_key_mods(),
                },
            });
            0
        }

        WM_XBUTTONUP => {
            use crate::event::{
                ElementState::Released, MouseButton::Other, WindowEvent::MouseInput,
            };
            let xbutton = super::get_xbutton_wparam(wparam as u32);

            release_mouse(userdata.window_state_lock());

            update_modifiers(window, userdata);

            userdata.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: MouseInput {
                    device_id: DEVICE_ID,
                    state: Released,
                    button: Other(xbutton),
                    modifiers: event::get_key_mods(),
                },
            });
            0
        }

        WM_CAPTURECHANGED => {
            // lparam here is a handle to the window which is gaining mouse capture.
            // If it is the same as our window, then we're essentially retaining the capture. This
            // can happen if `SetCapture` is called on our window when it already has the mouse
            // capture.
            if lparam != window {
                userdata.window_state_lock().mouse.capture_count = 0;
            }
            0
        }

        WM_TOUCH => {
            let pcount = super::loword(wparam as u32) as usize;
            let mut inputs = Vec::with_capacity(pcount);
            let htouch = lparam;
            if GetTouchInputInfo(
                htouch,
                pcount as u32,
                inputs.as_mut_ptr(),
                mem::size_of::<TOUCHINPUT>() as i32,
            ) > 0
            {
                inputs.set_len(pcount);
                for input in &inputs {
                    let mut location = POINT {
                        x: input.x / 100,
                        y: input.y / 100,
                    };

                    if ScreenToClient(window, &mut location) == false.into() {
                        continue;
                    }

                    let x = location.x as f64 + (input.x % 100) as f64 / 100f64;
                    let y = location.y as f64 + (input.y % 100) as f64 / 100f64;
                    let location = PhysicalPosition::new(x, y);
                    userdata.send_event(Event::WindowEvent {
                        window_id: RootWindowId(WindowId(window)),
                        event: WindowEvent::Touch(Touch {
                            phase: if util::has_flag(input.dwFlags, TOUCHEVENTF_DOWN) {
                                TouchPhase::Started
                            } else if util::has_flag(input.dwFlags, TOUCHEVENTF_UP) {
                                TouchPhase::Ended
                            } else if util::has_flag(input.dwFlags, TOUCHEVENTF_MOVE) {
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
            0
        }

        WM_POINTERDOWN | WM_POINTERUPDATE | WM_POINTERUP => {
            if let (
                Some(GetPointerFrameInfoHistory),
                Some(SkipPointerFrameMessages),
                Some(GetPointerDeviceRects),
            ) = (
                *GET_POINTER_FRAME_INFO_HISTORY,
                *SKIP_POINTER_FRAME_MESSAGES,
                *GET_POINTER_DEVICE_RECTS,
            ) {
                let pointer_id = super::loword(wparam as u32) as u32;
                let mut entries_count = 0u32;
                let mut pointers_count = 0u32;
                if GetPointerFrameInfoHistory(
                    pointer_id,
                    &mut entries_count,
                    &mut pointers_count,
                    ptr::null_mut(),
                ) == false.into()
                {
                    return 0;
                }

                let pointer_info_count = (entries_count * pointers_count) as usize;
                let mut pointer_infos = Vec::with_capacity(pointer_info_count);
                if GetPointerFrameInfoHistory(
                    pointer_id,
                    &mut entries_count,
                    &mut pointers_count,
                    pointer_infos.as_mut_ptr(),
                ) == false.into()
                {
                    return 0;
                }
                pointer_infos.set_len(pointer_info_count);

                // https://docs.microsoft.com/en-us/windows/desktop/api/winuser/nf-winuser-getpointerframeinfohistory
                // The information retrieved appears in reverse chronological order, with the most recent entry in the first
                // row of the returned array
                for pointer_info in pointer_infos.iter().rev() {
                    let mut device_rect = mem::MaybeUninit::uninit();
                    let mut display_rect = mem::MaybeUninit::uninit();

                    if GetPointerDeviceRects(
                        pointer_info.sourceDevice,
                        device_rect.as_mut_ptr(),
                        display_rect.as_mut_ptr(),
                    ) == false.into()
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

                    if ScreenToClient(window, &mut location) == false.into() {
                        continue;
                    }

                    let force = match pointer_info.pointerType {
                        PT_TOUCH => {
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
                        PT_PEN => {
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
                    userdata.send_event(Event::WindowEvent {
                        window_id: RootWindowId(WindowId(window)),
                        event: WindowEvent::Touch(Touch {
                            phase: if util::has_flag(pointer_info.pointerFlags, POINTER_FLAG_DOWN) {
                                TouchPhase::Started
                            } else if util::has_flag(pointer_info.pointerFlags, POINTER_FLAG_UP) {
                                TouchPhase::Ended
                            } else if util::has_flag(pointer_info.pointerFlags, POINTER_FLAG_UPDATE)
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

        WM_NCACTIVATE => {
            let is_active = wparam != false.into();
            let active_focus_changed = userdata.window_state_lock().set_active(is_active);
            if active_focus_changed {
                if is_active {
                    gain_active_focus(window, userdata);
                } else {
                    lose_active_focus(window, userdata);
                }
            }
            DefWindowProcW(window, msg, wparam, lparam)
        }

        WM_SETFOCUS => {
            let active_focus_changed = userdata.window_state_lock().set_focused(true);
            if active_focus_changed {
                gain_active_focus(window, userdata);
            }
            0
        }

        WM_KILLFOCUS => {
            let active_focus_changed = userdata.window_state_lock().set_focused(false);
            if active_focus_changed {
                lose_active_focus(window, userdata);
            }
            0
        }

        WM_SETCURSOR => {
            let set_cursor_to = {
                let window_state = userdata.window_state_lock();
                // The return value for the preceding `WM_NCHITTEST` message is conveniently
                // provided through the low-order word of lParam. We use that here since
                // `WM_MOUSEMOVE` seems to come after `WM_SETCURSOR` for a given cursor movement.
                let in_client_area = super::loword(lparam as u32) as u32 == HTCLIENT;
                if in_client_area {
                    Some(window_state.mouse.cursor)
                } else {
                    None
                }
            };

            match set_cursor_to {
                Some(cursor) => {
                    let cursor = LoadCursorW(0, cursor.to_windows_cursor());
                    SetCursor(cursor);
                    0
                }
                None => DefWindowProcW(window, msg, wparam, lparam),
            }
        }

        WM_DROPFILES => {
            // See `FileDropHandler` for implementation.
            0
        }

        WM_GETMINMAXINFO => {
            let mmi = lparam as *mut MINMAXINFO;

            let window_state = userdata.window_state_lock();
            let window_flags = window_state.window_flags;

            if window_state.min_size.is_some() || window_state.max_size.is_some() {
                if let Some(min_size) = window_state.min_size {
                    let min_size = min_size.to_physical(window_state.scale_factor);
                    let (width, height): (u32, u32) =
                        window_flags.adjust_size(window, min_size).into();
                    (*mmi).ptMinTrackSize = POINT {
                        x: width as i32,
                        y: height as i32,
                    };
                }
                if let Some(max_size) = window_state.max_size {
                    let max_size = max_size.to_physical(window_state.scale_factor);
                    let (width, height): (u32, u32) =
                        window_flags.adjust_size(window, max_size).into();
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
        WM_DPICHANGED => {
            use crate::event::WindowEvent::ScaleFactorChanged;

            // This message actually provides two DPI values - x and y. However MSDN says that
            // "you only need to use either the X-axis or the Y-axis value when scaling your
            // application since they are the same".
            // https://msdn.microsoft.com/en-us/library/windows/desktop/dn312083(v=vs.85).aspx
            let new_dpi_x = super::loword(wparam as u32) as u32;
            let new_scale_factor = dpi_to_scale_factor(new_dpi_x);
            let old_scale_factor: f64;

            let (allow_resize, window_flags) = {
                let mut window_state = userdata.window_state_lock();
                old_scale_factor = window_state.scale_factor;
                window_state.scale_factor = new_scale_factor;

                if new_scale_factor == old_scale_factor {
                    return 0;
                }

                let allow_resize = window_state.fullscreen.is_none()
                    && !window_state.window_flags().contains(WindowFlags::MAXIMIZED);

                (allow_resize, window_state.window_flags)
            };

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
                let adjusted_rect = window_flags
                    .adjust_rect(window, suggested_rect)
                    .unwrap_or(suggested_rect);
                margin_left = suggested_rect.left - adjusted_rect.left;
                margin_top = suggested_rect.top - adjusted_rect.top;
                // margin_right = adjusted_rect.right - suggested_rect.right;
                // margin_bottom = adjusted_rect.bottom - suggested_rect.bottom;
            }

            let old_physical_inner_rect = util::WindowArea::Inner
                .get_rect(window)
                .expect("failed to query (old) inner window area");
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

            userdata.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(window)),
                event: ScaleFactorChanged {
                    scale_factor: new_scale_factor,
                    new_inner_size: &mut new_physical_inner_size,
                },
            });

            let dragging_window: bool;

            {
                let window_state = userdata.window_state_lock();
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

                conservative_rect = window_flags
                    .adjust_rect(window, conservative_rect)
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
                let new_dpi_monitor = MonitorFromWindow(window, MONITOR_DEFAULTTONULL);
                let conservative_rect_monitor =
                    MonitorFromRect(&conservative_rect, MONITOR_DEFAULTTONULL);
                new_outer_rect = if conservative_rect_monitor == new_dpi_monitor {
                    conservative_rect
                } else {
                    let get_monitor_rect = |monitor| {
                        let mut monitor_info = MONITORINFO {
                            cbSize: mem::size_of::<MONITORINFO>() as _,
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
                    #[allow(clippy::bool_to_int_with_if)]
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

                        if MonitorFromRect(&conservative_rect, MONITOR_DEFAULTTONULL)
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
                0,
                new_outer_rect.left,
                new_outer_rect.top,
                new_outer_rect.right - new_outer_rect.left,
                new_outer_rect.bottom - new_outer_rect.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );

            0
        }

        WM_SETTINGCHANGE => {
            use crate::event::WindowEvent::ThemeChanged;

            let preferred_theme = userdata.window_state_lock().preferred_theme;

            if preferred_theme.is_none() {
                let new_theme = try_theme(window, preferred_theme);
                let mut window_state = userdata.window_state_lock();

                if window_state.current_theme != new_theme {
                    window_state.current_theme = new_theme;
                    drop(window_state);
                    userdata.send_event(Event::WindowEvent {
                        window_id: RootWindowId(WindowId(window)),
                        event: ThemeChanged(new_theme),
                    });
                }
            }

            DefWindowProcW(window, msg, wparam, lparam)
        }

        _ => {
            if msg == DESTROY_MSG_ID.get() {
                DestroyWindow(window);
                0
            } else if msg == SET_RETAIN_STATE_ON_SIZE_MSG_ID.get() {
                let mut window_state = userdata.window_state_lock();
                window_state.set_window_flags_in_place(|f| {
                    f.set(WindowFlags::MARKER_RETAIN_STATE_ON_SIZE, wparam != 0)
                });
                0
            } else if msg == TASKBAR_CREATED.get() {
                let window_state = userdata.window_state_lock();
                set_skip_taskbar(window, window_state.skip_taskbar);
                DefWindowProcW(window, msg, wparam, lparam)
            } else {
                DefWindowProcW(window, msg, wparam, lparam)
            }
        }
    };

    userdata
        .event_loop_runner
        .catch_unwind(callback)
        .unwrap_or(-1)
}

unsafe extern "system" fn thread_event_target_callback<T: 'static>(
    window: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let userdata_ptr = super::get_window_long(window, GWL_USERDATA) as *mut ThreadMsgTargetData<T>;
    if userdata_ptr.is_null() {
        // `userdata_ptr` will always be null for the first `WM_GETMINMAXINFO`, as well as `WM_NCCREATE` and
        // `WM_CREATE`.
        return DefWindowProcW(window, msg, wparam, lparam);
    }
    let userdata = Box::from_raw(userdata_ptr);

    if msg != WM_PAINT {
        RedrawWindow(window, ptr::null(), 0, RDW_INTERNALPAINT);
    }

    let mut userdata_removed = false;

    // I decided to bind the closure to `callback` and pass it to catch_unwind rather than passing
    // the closure to catch_unwind directly so that the match body indendation wouldn't change and
    // the git blame and history would be preserved.
    let callback = || match msg {
        WM_NCDESTROY => {
            super::set_window_long(window, GWL_USERDATA, 0);
            userdata_removed = true;
            0
        }
        // Because WM_PAINT comes after all other messages, we use it during modal loops to detect
        // when the event queue has been emptied. See `process_event` for more details.
        WM_PAINT => {
            ValidateRect(window, ptr::null());
            // If the WM_PAINT handler in `public_window_callback` has already flushed the redraw
            // events, `handling_events` will return false and we won't emit a second
            // `RedrawEventsCleared` event.
            if userdata.event_loop_runner.handling_events() {
                if userdata.event_loop_runner.should_buffer() {
                    // This branch can be triggered when a nested win32 event loop is triggered
                    // inside of the `event_handler` callback.
                    RedrawWindow(window, ptr::null(), 0, RDW_INTERNALPAINT);
                } else {
                    // This WM_PAINT handler will never be re-entrant because `flush_paint_messages`
                    // doesn't call WM_PAINT for the thread event target (i.e. this window).
                    assert!(flush_paint_messages(None, &userdata.event_loop_runner));
                    userdata.event_loop_runner.redraw_events_cleared();
                    process_control_flow(&userdata.event_loop_runner);
                }
            }

            // Default WM_PAINT behaviour. This makes sure modals and popups are shown immediatly when opening them.
            DefWindowProcW(window, msg, wparam, lparam)
        }

        WM_INPUT_DEVICE_CHANGE => {
            let event = match wparam as u32 {
                GIDC_ARRIVAL => DeviceEvent::Added,
                GIDC_REMOVAL => DeviceEvent::Removed,
                _ => unreachable!(),
            };

            userdata.send_event(Event::DeviceEvent {
                device_id: wrap_device_id(lparam as u32),
                event,
            });

            0
        }

        WM_INPUT => {
            use crate::event::{
                DeviceEvent::{Button, Key, Motion, MouseMotion, MouseWheel},
                ElementState::{Pressed, Released},
                MouseScrollDelta::LineDelta,
            };

            if let Some(data) = raw_input::get_raw_input_data(lparam) {
                let device_id = wrap_device_id(data.header.hDevice as u32);

                if data.header.dwType == RIM_TYPEMOUSE {
                    let mouse = data.data.mouse;

                    if util::has_flag(mouse.usFlags as u32, MOUSE_MOVE_RELATIVE) {
                        let x = mouse.lLastX as f64;
                        let y = mouse.lLastY as f64;

                        if x != 0.0 {
                            userdata.send_event(Event::DeviceEvent {
                                device_id,
                                event: Motion { axis: 0, value: x },
                            });
                        }

                        if y != 0.0 {
                            userdata.send_event(Event::DeviceEvent {
                                device_id,
                                event: Motion { axis: 1, value: y },
                            });
                        }

                        if x != 0.0 || y != 0.0 {
                            userdata.send_event(Event::DeviceEvent {
                                device_id,
                                event: MouseMotion { delta: (x, y) },
                            });
                        }
                    }

                    let mouse_button_flags = mouse.Anonymous.Anonymous.usButtonFlags;

                    if util::has_flag(mouse_button_flags as u32, RI_MOUSE_WHEEL) {
                        let delta = mouse.Anonymous.Anonymous.usButtonData as i16 as f32
                            / WHEEL_DELTA as f32;
                        userdata.send_event(Event::DeviceEvent {
                            device_id,
                            event: MouseWheel {
                                delta: LineDelta(0.0, delta),
                            },
                        });
                    }

                    let button_state =
                        raw_input::get_raw_mouse_button_state(mouse_button_flags as u32);
                    // Left, middle, and right, respectively.
                    for (index, state) in button_state.iter().enumerate() {
                        if let Some(state) = *state {
                            // This gives us consistency with X11, since there doesn't
                            // seem to be anything else reasonable to do for a mouse
                            // button ID.
                            let button = (index + 1) as u32;
                            userdata.send_event(Event::DeviceEvent {
                                device_id,
                                event: Button { button, state },
                            });
                        }
                    }
                } else if data.header.dwType == RIM_TYPEKEYBOARD {
                    let keyboard = data.data.keyboard;

                    let pressed =
                        keyboard.Message == WM_KEYDOWN || keyboard.Message == WM_SYSKEYDOWN;
                    let released = keyboard.Message == WM_KEYUP || keyboard.Message == WM_SYSKEYUP;

                    if pressed || released {
                        let state = if pressed { Pressed } else { Released };

                        let scancode = keyboard.MakeCode;
                        let extended = util::has_flag(keyboard.Flags, RI_KEY_E0 as u16)
                            | util::has_flag(keyboard.Flags, RI_KEY_E1 as u16);

                        if let Some((vkey, scancode)) =
                            handle_extended_keys(keyboard.VKey, scancode as u32, extended)
                        {
                            let virtual_keycode = vkey_to_winit_vkey(vkey);

                            #[allow(deprecated)]
                            userdata.send_event(Event::DeviceEvent {
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

            DefWindowProcW(window, msg, wparam, lparam)
        }

        _ if msg == USER_EVENT_MSG_ID.get() => {
            if let Ok(event) = userdata.user_event_receiver.recv() {
                userdata.send_event(Event::UserEvent(event));
            }
            0
        }
        _ if msg == EXEC_MSG_ID.get() => {
            let mut function: ThreadExecFn = Box::from_raw(wparam as *mut _);
            function();
            0
        }
        _ if msg == PROCESS_NEW_EVENTS_MSG_ID.get() => {
            PostThreadMessageW(
                userdata.event_loop_runner.wait_thread_id(),
                CANCEL_WAIT_UNTIL_MSG_ID.get(),
                0,
                0,
            );

            // if the control_flow is WaitUntil, make sure the given moment has actually passed
            // before emitting NewEvents
            if let ControlFlow::WaitUntil(wait_until) = userdata.event_loop_runner.control_flow() {
                let mut msg = mem::zeroed();
                while Instant::now() < wait_until {
                    if PeekMessageW(&mut msg, 0, 0, 0, PM_NOREMOVE) != false.into() {
                        // This works around a "feature" in PeekMessageW. If the message PeekMessageW
                        // gets is a WM_PAINT message that had RDW_INTERNALPAINT set (i.e. doesn't
                        // have an update region), PeekMessageW will remove that window from the
                        // redraw queue even though we told it not to remove messages from the
                        // queue. We fix it by re-dispatching an internal paint message to that
                        // window.
                        if msg.message == WM_PAINT {
                            let mut rect = mem::zeroed();
                            if GetUpdateRect(msg.hwnd, &mut rect, false.into()) == false.into() {
                                RedrawWindow(msg.hwnd, ptr::null(), 0, RDW_INTERNALPAINT);
                            }
                        }

                        break;
                    }
                }
            }
            userdata.event_loop_runner.poll();
            0
        }
        _ => DefWindowProcW(window, msg, wparam, lparam),
    };

    let result = userdata
        .event_loop_runner
        .catch_unwind(callback)
        .unwrap_or(-1);
    if userdata_removed {
        drop(userdata);
    } else {
        Box::into_raw(userdata);
    }
    result
}
