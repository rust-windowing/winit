#![allow(non_snake_case)]

// N.B. (notgull): All window data event handling has been split out and moved into the
// "event_loop/handler/" module.

mod handler;
mod runner;

use std::{
    cell::Cell,
    collections::VecDeque,
    convert::Infallible,
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
    Foundation::{BOOL, HANDLE, HWND, LPARAM, LRESULT, RECT, WAIT_TIMEOUT, WPARAM},
    Graphics::Gdi::{GetUpdateRect, RedrawWindow, ValidateRect, RDW_INTERNALPAINT},
    Media::{timeBeginPeriod, timeEndPeriod, timeGetDevCaps, TIMECAPS, TIMERR_NOERROR},
    System::{Ole::RevokeDragDrop, Threading::GetCurrentThreadId, WindowsProgramming::INFINITE},
    UI::{
        Input::{
            Pointer::{POINTER_INFO, POINTER_PEN_INFO, POINTER_TOUCH_INFO},
            RIM_TYPEKEYBOARD, RIM_TYPEMOUSE,
        },
        WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
            MsgWaitForMultipleObjectsEx, PeekMessageW, PostMessageW, PostThreadMessageW,
            RegisterClassExW, RegisterWindowMessageA, TranslateMessage, CREATESTRUCTW,
            GIDC_ARRIVAL, GIDC_REMOVAL, GWL_STYLE, GWL_USERDATA, MSG, MWMO_INPUTAVAILABLE,
            PM_NOREMOVE, PM_QS_PAINT, PM_REMOVE, QS_ALLEVENTS, RI_KEY_E0, RI_KEY_E1,
            RI_MOUSE_WHEEL, WHEEL_DELTA, WM_CLOSE, WM_CREATE, WM_DESTROY, WM_INPUT,
            WM_INPUT_DEVICE_CHANGE, WM_KEYDOWN, WM_KEYUP, WM_NCCREATE, WM_NCDESTROY, WM_PAINT,
            WM_SYSKEYDOWN, WM_SYSKEYUP, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
            WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT, WS_OVERLAPPED, WS_POPUP, WS_VISIBLE,
        },
    },
};

use crate::{
    event::{DeviceEvent, Event, Force, KeyboardInput},
    event_loop::{
        ControlFlow, DeviceEventFilter, EventLoopClosed, EventLoopWindowTarget as RootELW,
    },
    platform_impl::platform::{
        dpi::become_dpi_aware,
        drop_handler::FileDropHandler,
        event::{self, handle_extended_keys, vkey_to_winit_vkey},
        monitor::{self, MonitorHandle},
        raw_input, util,
        window::InitData,
        window_state::WindowState,
        wrap_device_id, Fullscreen, WindowId,
    },
    window::WindowId as RootWindowId,
};
pub(crate) use handler::WindowMessageMap;
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
    pub message_map: &'static WindowMessageMap,
}

impl<T> WindowData<T> {
    unsafe fn send_event(&self, event: Event<'_, T>) {
        self.event_loop_runner.send_event(event);
    }

    fn window_state_lock(&self) -> MutexGuard<'_, WindowState> {
        self.window_state.lock().unwrap()
    }
}

/// Trait abstraction over `WindowData<T>` that ignores the type parameter.
trait GenericWindowData {
    /// Sends an event to the event loop.
    unsafe fn send_event(&self, event: Event<'_, Infallible>);

    /// Get a mutex guard to the window state.
    fn window_state_lock(&self) -> MutexGuard<'_, WindowState>;
}

impl<T: 'static> GenericWindowData for WindowData<T> {
    unsafe fn send_event(&self, event: Event<'_, Infallible>) {
        self.send_event(event.map_nonuser_event().unwrap_or_else(|_| unreachable!()))
    }

    fn window_state_lock(&self) -> MutexGuard<'_, WindowState> {
        self.window_state_lock()
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

        thread::spawn(move || wait_thread(thread_id, thread_msg_target));
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
        WM_NCDESTROY => {
            super::set_window_long(window, GWL_USERDATA, 0);
            userdata.userdata_removed.set(true);
            0
        }

        WM_CLOSE => {
            use crate::event::WindowEvent::CloseRequested;

            unsafe {
                userdata.send_event(Event::WindowEvent {
                    window_id: RootWindowId(WindowId(window)),
                    event: CloseRequested,
                });
            }

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

        _ => {
            // Call into the message handler.
            userdata
                .message_map
                .handle_message(window, msg, wparam, lparam, userdata)
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
