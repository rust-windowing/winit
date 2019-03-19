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

use std::{mem, panic, ptr, thread};
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::os::windows::io::AsRawHandle;
use std::sync::{Arc, mpsc, Mutex};

use backtrace::Backtrace;
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
use winapi::shared::winerror::S_OK;
use winapi::um::{libloaderapi, processthreadsapi, ole2, winuser};
use winapi::um::oleidl::LPDROPTARGET;
use winapi::um::winnt::{LONG, LPCSTR, SHORT};

use {
    ControlFlow,
    Event,
    EventsLoopClosed,
    KeyboardInput,
    LogicalPosition,
    LogicalSize,
    PhysicalSize,
    WindowEvent,
    WindowId as SuperWindowId,
};
use events::{DeviceEvent, Touch, TouchPhase};
use platform::platform::{event, WindowId, DEVICE_ID, wrap_device_id, util};
use platform::platform::dpi::{
    become_dpi_aware,
    dpi_to_scale_factor,
    enable_non_client_dpi_scaling,
    get_hwnd_scale_factor,
};
use platform::platform::drop_handler::FileDropHandler;
use platform::platform::event::{handle_extended_keys, process_key_params, vkey_to_winit_vkey};
use platform::platform::raw_input::{get_raw_input_data, get_raw_mouse_button_state};
use platform::platform::window::adjust_size;
use platform::platform::window_state::{CursorFlags, WindowFlags, WindowState};

/// Dummy object that allows inserting a window's state.
// We store a pointer in order to !impl Send and Sync.
pub struct Inserter(*mut u8);

impl Inserter {
    /// Inserts a window's state for the callback to use. The state is removed automatically if the
    /// callback receives a `WM_CLOSE` message for the window.
    pub fn insert(&self, window: HWND, state: Arc<Mutex<WindowState>>) {
        CONTEXT_STASH.with(|context_stash| {
            let mut context_stash = context_stash.borrow_mut();
            let was_in = context_stash.as_mut().unwrap().windows.insert(window, state);
            assert!(was_in.is_none());
        });
    }
}

pub struct EventsLoop {
    thread_msg_target: HWND,
    // Id of the background thread from the Win32 API.
    thread_id: DWORD,
    // Receiver for the events. The sender is in the background thread.
    receiver: mpsc::Receiver<EventsLoopEvent>,
    // Sender instance that's paired with the receiver. Used to construct an `EventsLoopProxy`.
    sender: mpsc::Sender<EventsLoopEvent>,
}

enum EventsLoopEvent {
    WinitEvent(Event),
    Panic(PanicError),
}

impl EventsLoop {
    pub fn new() -> EventsLoop {
        Self::with_dpi_awareness(true)
    }

    pub fn with_dpi_awareness(dpi_aware: bool) -> EventsLoop {
        struct InitData {
            thread_msg_target: HWND,
        }
        unsafe impl Send for InitData {}

        become_dpi_aware(dpi_aware);

        // The main events transfer channel.
        let (tx, rx) = mpsc::channel();

        // Channel to send initialization data created on the event loop thread back to the main
        // thread.
        let (init_tx, init_rx) = mpsc::sync_channel(0);

        let thread_sender = tx.clone();
        let panic_sender = tx.clone();
        let thread = thread::spawn(move || {
            let tx = thread_sender;
            let thread_msg_target = thread_event_target_window();

            CONTEXT_STASH.with(|context_stash| {
                *context_stash.borrow_mut() = Some(ThreadLocalData {
                    sender: tx,
                    windows: HashMap::with_capacity(4),
                    file_drop_handlers: HashMap::with_capacity(4),
                    mouse_buttons_down: 0,
                    panic_error: None,
                });
            });

            unsafe {
                // Calling `PostThreadMessageA` on a thread that does not have an events queue yet
                // will fail. In order to avoid this situation, we call `IsGuiThread` to initialize
                // it.
                winuser::IsGUIThread(1);
                // Then only we unblock the `new()` function. We are sure that we don't call
                // `PostThreadMessageA()` before `new()` returns.
                init_tx.send(InitData{ thread_msg_target }).ok();
                drop(init_tx);

                let mut msg = mem::uninitialized();

                loop {
                    if winuser::GetMessageW(&mut msg, ptr::null_mut(), 0, 0) == 0 {
                        // If a panic occurred in the child callback, forward the panic information
                        // to the parent thread.
                        let panic_payload_opt = CONTEXT_STASH.with(|stash|
                            stash.borrow_mut().as_mut()
                                 .and_then(|s| s.panic_error.take())
                        );
                        if let Some(panic_payload) = panic_payload_opt {
                            panic_sender.send(EventsLoopEvent::Panic(panic_payload)).unwrap();
                        };

                        // Only happens if the message is `WM_QUIT`.
                        debug_assert_eq!(msg.message, winuser::WM_QUIT);
                        break;
                    }

                    // Calls `callback` below.
                    winuser::TranslateMessage(&msg);
                    winuser::DispatchMessageW(&msg);
                }
            }
        });

        // Blocks this function until the background thread has an events loop. See other comments.
        let InitData { thread_msg_target } = init_rx.recv().unwrap();

        let thread_id = unsafe {
            let handle = mem::transmute(thread.as_raw_handle());
            processthreadsapi::GetThreadId(handle)
        };

        EventsLoop {
            thread_msg_target,
            thread_id,
            receiver: rx,
            sender: tx,
        }
    }

    pub fn poll_events<F>(&mut self, mut callback: F)
        where F: FnMut(Event)
    {
        loop {
            let event = match self.receiver.try_recv() {
                Ok(EventsLoopEvent::WinitEvent(e)) => e,
                Ok(EventsLoopEvent::Panic(panic)) => {
                    eprintln!("resuming child thread unwind at: {:?}", Backtrace::new());
                    panic::resume_unwind(panic)
                },
                Err(_) => break,
            };

            callback(event);
        }
    }

    pub fn run_forever<F>(&mut self, mut callback: F)
        where F: FnMut(Event) -> ControlFlow
    {
        loop {
            let event = match self.receiver.recv() {
                Ok(EventsLoopEvent::WinitEvent(e)) => e,
                Ok(EventsLoopEvent::Panic(panic)) => {
                    eprintln!("resuming child thread unwind at: {:?}", Backtrace::new());
                    panic::resume_unwind(panic)
                },
                Err(_) => break,
            };

            let flow = callback(event);
            match flow {
                ControlFlow::Continue => continue,
                ControlFlow::Break => break,
            }
        }
    }

    pub fn create_proxy(&self) -> EventsLoopProxy {
        EventsLoopProxy {
            thread_id: self.thread_id,
            thread_msg_target: self.thread_msg_target,
            sender: self.sender.clone(),
        }
    }

    /// Executes a function in the background thread.
    ///
    /// Note that we use a FnMut instead of a FnOnce because we're too lazy to create an equivalent
    /// to the unstable FnBox.
    ///
    /// The `Inserted` can be used to inject a `WindowState` for the callback to use. The state is
    /// removed automatically if the callback receives a `WM_CLOSE` message for the window.
    pub(super) fn execute_in_thread<F>(&self, function: F)
        where F: FnMut(Inserter) + Send + 'static
    {
        self.create_proxy().execute_in_thread(function)
    }
}

impl Drop for EventsLoop {
    fn drop(&mut self) {
        unsafe {
            // Posting `WM_QUIT` will cause `GetMessage` to stop.
            winuser::PostThreadMessageA(self.thread_id, winuser::WM_QUIT, 0, 0);
        }
    }
}

#[derive(Clone)]
pub struct EventsLoopProxy {
    thread_id: DWORD,
    thread_msg_target: HWND,
    sender: mpsc::Sender<EventsLoopEvent>,
}

unsafe impl Send for EventsLoopProxy {}
unsafe impl Sync for EventsLoopProxy {}

impl EventsLoopProxy {
    pub fn wakeup(&self) -> Result<(), EventsLoopClosed> {
        self.sender.send(EventsLoopEvent::WinitEvent(Event::Awakened)).map_err(|_| EventsLoopClosed)
    }

    /// Executes a function in the background thread.
    ///
    /// Note that we use FnMut instead of FnOnce because boxing FnOnce won't work on stable Rust
    /// until 2030 when the design of Box is finally complete.
    /// https://github.com/rust-lang/rust/issues/28796
    ///
    /// The `Inserted` can be used to inject a `WindowState` for the callback to use. The state is
    /// removed automatically if the callback receives a `WM_CLOSE` message for the window.
    ///
    /// Note that if you are using this to change some property of a window and updating
    /// `WindowState` then you should call this within the lock of `WindowState`. Otherwise the
    /// events may be sent to the other thread in different order to the one in which you set
    /// `WindowState`, leaving them out of sync.
    pub fn execute_in_thread<F>(&self, mut function: F)
    where
        F: FnMut(Inserter) + Send + 'static,
    {
        if unsafe{ processthreadsapi::GetCurrentThreadId() } == self.thread_id {
            function(Inserter(ptr::null_mut()));
        } else {
            // We are using double-boxing here because it make casting back much easier
            let double_box: ThreadExecFn = Box::new(Box::new(function) as Box<FnMut(_)>);
            let raw = Box::into_raw(double_box);

            let res = unsafe {
                winuser::PostMessageW(
                    self.thread_msg_target,
                    *EXEC_MSG_ID,
                    raw as *mut () as usize as WPARAM,
                    0,
                )
            };
            assert!(res != 0, "PostMessage failed; is the messages queue full?");
        }
    }
}

type ThreadExecFn = Box<Box<FnMut(Inserter)>>;

lazy_static! {
    // Message sent when we want to execute a closure in the thread.
    // WPARAM contains a Box<Box<FnMut()>> that must be retrieved with `Box::from_raw`,
    // and LPARAM is unused.
    static ref EXEC_MSG_ID: u32 = {
        unsafe {
            winuser::RegisterWindowMessageA("Winit::ExecMsg\0".as_ptr() as LPCSTR)
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
            lpfnWndProc: Some(thread_event_target_callback),
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

fn thread_event_target_window() -> HWND {
    unsafe {
        let window = winuser::CreateWindowExW(
            winuser::WS_EX_NOACTIVATE | winuser::WS_EX_TRANSPARENT | winuser::WS_EX_LAYERED,
            THREAD_EVENT_TARGET_WINDOW_CLASS.as_ptr(),
            ptr::null_mut(),
            0,
            0, 0,
            0, 0,
            ptr::null_mut(),
            ptr::null_mut(),
            libloaderapi::GetModuleHandleW(ptr::null()),
            ptr::null_mut(),
        );
        winuser::SetWindowLongPtrW(
            window,
            winuser::GWL_STYLE,
            (winuser::WS_VISIBLE | winuser::WS_POPUP) as _
        );

        window
    }
}

// There's no parameters passed to the callback function, so it needs to get its context stashed
// in a thread-local variable.
thread_local!(static CONTEXT_STASH: RefCell<Option<ThreadLocalData>> = RefCell::new(None));
struct ThreadLocalData {
    sender: mpsc::Sender<EventsLoopEvent>,
    windows: HashMap<HWND, Arc<Mutex<WindowState>>>,
    file_drop_handlers: HashMap<HWND, FileDropHandler>, // Each window has its own drop handler.
    mouse_buttons_down: u32,
    panic_error: Option<PanicError>,
}
type PanicError = Box<Any + Send + 'static>;

// Utility function that dispatches an event on the current thread.
pub fn send_event(event: Event) {
    CONTEXT_STASH.with(|context_stash| {
        let context_stash = context_stash.borrow();

        let _ = context_stash.as_ref().unwrap().sender.send(EventsLoopEvent::WinitEvent(event));   // Ignoring if closed
    });
}

/// Capture mouse input, allowing `window` to receive mouse events when the cursor is outside of
/// the window.
unsafe fn capture_mouse(window: HWND) {
    let set_capture = CONTEXT_STASH.with(|context_stash| {
        let mut context_stash = context_stash.borrow_mut();
        if let Some(context_stash) = context_stash.as_mut() {
            context_stash.mouse_buttons_down += 1;
            true
        } else {
            false
        }
    });
    if set_capture {
        winuser::SetCapture(window);
    }
}

/// Release mouse input, stopping windows on this thread from receiving mouse input when the cursor
/// is outside the window.
unsafe fn release_mouse() {
    let release_capture = CONTEXT_STASH.with(|context_stash| {
        let mut context_stash = context_stash.borrow_mut();
        if let Some(context_stash) = context_stash.as_mut() {
            context_stash.mouse_buttons_down = context_stash.mouse_buttons_down.saturating_sub(1);
            if context_stash.mouse_buttons_down == 0 {
                return true;
            }
        }
        false
    });
    if release_capture {
        winuser::ReleaseCapture();
    }
}

pub unsafe fn run_catch_panic<F, R>(error: R, f: F) -> R
    where F: panic::UnwindSafe + FnOnce() -> R
{
    // If a panic has been triggered, cancel all future operations in the function.
    if CONTEXT_STASH.with(|stash| stash.borrow().as_ref().map(|s| s.panic_error.is_some()).unwrap_or(false)) {
        return error;
    }

    let callback_result = panic::catch_unwind(f);
    match callback_result {
        Ok(lresult) => lresult,
        Err(err) => CONTEXT_STASH.with(|context_stash| {
            let mut context_stash = context_stash.borrow_mut();
            if let Some(context_stash) = context_stash.as_mut() {
                context_stash.panic_error = Some(err);
                winuser::PostQuitMessage(-1);
            }
            error
        })
    }
}

/// Any window whose callback is configured to this function will have its events propagated
/// through the events loop of the thread the window was created in.
//
// This is the callback that is called by `DispatchMessage` in the events loop.
//
// Returning 0 tells the Win32 API that the message has been processed.
// FIXME: detect WM_DWMCOMPOSITIONCHANGED and call DwmEnableBlurBehindWindow if necessary
pub unsafe extern "system" fn callback(
    window: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // Unwinding into foreign code is undefined behavior. So we catch any panics that occur in our
    // code, and if a panic happens we cancel any future operations.
    run_catch_panic(-1, || callback_inner(window, msg, wparam, lparam))
}

unsafe fn callback_inner(
    window: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        winuser::WM_CREATE => {
            use winapi::shared::winerror::{OLE_E_WRONGCOMPOBJ, RPC_E_CHANGED_MODE};
            let ole_init_result = ole2::OleInitialize(ptr::null_mut());
            // It is ok if the initialize result is `S_FALSE` because it might happen that
            // multiple windows are created on the same thread.
            if ole_init_result == OLE_E_WRONGCOMPOBJ {
                panic!("OleInitialize failed! Result was: `OLE_E_WRONGCOMPOBJ`");
            } else if ole_init_result == RPC_E_CHANGED_MODE {
                panic!("OleInitialize failed! Result was: `RPC_E_CHANGED_MODE`");
            }

            CONTEXT_STASH.with(|context_stash| {
                let mut context_stash = context_stash.borrow_mut();

                let drop_handlers = &mut context_stash.as_mut().unwrap().file_drop_handlers;
                let new_handler = FileDropHandler::new(window);
                let handler_interface_ptr = &mut (*new_handler.data).interface as LPDROPTARGET;
                drop_handlers.insert(window, new_handler);

                assert_eq!(ole2::RegisterDragDrop(window, handler_interface_ptr), S_OK);
            });
            0
        },

        winuser::WM_NCCREATE => {
            enable_non_client_dpi_scaling(window);
            winuser::DefWindowProcW(window, msg, wparam, lparam)
        },

        winuser::WM_CLOSE => {
            use events::WindowEvent::CloseRequested;
            send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: CloseRequested
            });
            0
        },

        winuser::WM_DESTROY => {
            use events::WindowEvent::Destroyed;
            CONTEXT_STASH.with(|context_stash| {
                let mut context_stash = context_stash.borrow_mut();
                ole2::RevokeDragDrop(window);
                let context_stash_mut = context_stash.as_mut().unwrap();
                context_stash_mut.file_drop_handlers.remove(&window);
                context_stash_mut.windows.remove(&window);
            });
            send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: Destroyed
            });
            0
        },

        winuser::WM_PAINT => {
            use events::WindowEvent::Refresh;
            send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: Refresh,
            });
            winuser::DefWindowProcW(window, msg, wparam, lparam)
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
                send_event(Event::WindowEvent {
                    window_id: SuperWindowId(WindowId(window)),
                    event: Moved(logical_position),
                });
            }

            // This is necessary for us to still get sent WM_SIZE.
            winuser::DefWindowProcW(window, msg, wparam, lparam)
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

            // Wait for the parent thread to process the resize event before returning from the
            // callback.
            CONTEXT_STASH.with(|context_stash| {
                let mut context_stash = context_stash.borrow_mut();
                let cstash = context_stash.as_mut().unwrap();

                if let Some(w) = cstash.windows.get_mut(&window) {
                    let mut w = w.lock().unwrap();

                    // See WindowFlags::MARKER_RETAIN_STATE_ON_SIZE docs for info on why this `if` check exists.
                    if !w.window_flags().contains(WindowFlags::MARKER_RETAIN_STATE_ON_SIZE) {
                        let maximized = wparam == winuser::SIZE_MAXIMIZED;
                        w.set_window_flags_in_place(|f| f.set(WindowFlags::MAXIMIZED, maximized));
                    }
                }

                cstash.sender.send(EventsLoopEvent::WinitEvent(event)).ok();
            });
            0
        },

        winuser::WM_CHAR => {
            use std::mem;
            use events::WindowEvent::ReceivedCharacter;
            let chr: char = mem::transmute(wparam as u32);
            send_event(Event::WindowEvent {
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
            let x = windowsx::GET_X_LPARAM(lparam);
            let y = windowsx::GET_Y_LPARAM(lparam);

            let mouse_was_outside_window = CONTEXT_STASH.with(|context_stash| {
                let mut context_stash = context_stash.borrow_mut();
                if let Some(context_stash) = context_stash.as_mut() {
                    if let Some(w) = context_stash.windows.get_mut(&window) {
                        let mut w = w.lock().unwrap();

                        let was_outside_window = !w.mouse.cursor_flags().contains(CursorFlags::IN_WINDOW);
                        w.mouse.set_cursor_flags(window, |f| f.set(CursorFlags::IN_WINDOW, true)).ok();
                        return was_outside_window;
                    }
                }

                false
            });


            if mouse_was_outside_window {
                send_event(Event::WindowEvent {
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

            let dpi_factor = get_hwnd_scale_factor(window);
            let position = LogicalPosition::from_physical((x as f64, y as f64), dpi_factor);

            send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: CursorMoved { device_id: DEVICE_ID, position, modifiers: event::get_key_mods() },
            });

            0
        },

        winuser::WM_MOUSELEAVE => {
            use events::WindowEvent::CursorLeft;

            CONTEXT_STASH.with(|context_stash| {
                let mut context_stash = context_stash.borrow_mut();
                if let Some(context_stash) = context_stash.as_mut() {
                    if let Some(w) = context_stash.windows.get_mut(&window) {
                        let mut w = w.lock().unwrap();
                        w.mouse.set_cursor_flags(window, |f| f.set(CursorFlags::IN_WINDOW, false)).ok();
                    }
                }
            });

            send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: CursorLeft { device_id: DEVICE_ID }
            });

            0
        },

        winuser::WM_MOUSEWHEEL => {
            use events::MouseScrollDelta::LineDelta;
            use events::TouchPhase;

            let value = (wparam >> 16) as i16;
            let value = value as i32;
            let value = value as f32 / winuser::WHEEL_DELTA as f32;

            send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: WindowEvent::MouseWheel { device_id: DEVICE_ID, delta: LineDelta(0.0, value), phase: TouchPhase::Moved, modifiers: event::get_key_mods() },
            });

            0
        },

        winuser::WM_MOUSEHWHEEL => {
            use events::MouseScrollDelta::LineDelta;
            use events::TouchPhase;

            let value = (wparam >> 16) as i16;
            let value = value as i32;
            let value = value as f32 / winuser::WHEEL_DELTA as f32;

            send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: WindowEvent::MouseWheel { device_id: DEVICE_ID, delta: LineDelta(value, 0.0), phase: TouchPhase::Moved, modifiers: event::get_key_mods() },
            });

            0
        },

        winuser::WM_KEYDOWN | winuser::WM_SYSKEYDOWN => {
            use events::ElementState::Pressed;
            use events::VirtualKeyCode;
            if msg == winuser::WM_SYSKEYDOWN && wparam as i32 == winuser::VK_F4 {
                winuser::DefWindowProcW(window, msg, wparam, lparam)
            } else {
                if let Some((scancode, vkey)) = process_key_params(wparam, lparam) {
                    send_event(Event::WindowEvent {
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
                        send_event(Event::WindowEvent {
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
                send_event(Event::WindowEvent {
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

            capture_mouse(window);

            send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: MouseInput { device_id: DEVICE_ID, state: Pressed, button: Left, modifiers: event::get_key_mods() }
            });
            0
        },

        winuser::WM_LBUTTONUP => {
            use events::WindowEvent::MouseInput;
            use events::MouseButton::Left;
            use events::ElementState::Released;

            release_mouse();

            send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: MouseInput { device_id: DEVICE_ID, state: Released, button: Left, modifiers: event::get_key_mods() }
            });
            0
        },

        winuser::WM_RBUTTONDOWN => {
            use events::WindowEvent::MouseInput;
            use events::MouseButton::Right;
            use events::ElementState::Pressed;

            capture_mouse(window);

            send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: MouseInput { device_id: DEVICE_ID, state: Pressed, button: Right, modifiers: event::get_key_mods() }
            });
            0
        },

        winuser::WM_RBUTTONUP => {
            use events::WindowEvent::MouseInput;
            use events::MouseButton::Right;
            use events::ElementState::Released;

            release_mouse();

            send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: MouseInput { device_id: DEVICE_ID, state: Released, button: Right, modifiers: event::get_key_mods() }
            });
            0
        },

        winuser::WM_MBUTTONDOWN => {
            use events::WindowEvent::MouseInput;
            use events::MouseButton::Middle;
            use events::ElementState::Pressed;

            capture_mouse(window);

            send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: MouseInput { device_id: DEVICE_ID, state: Pressed, button: Middle, modifiers: event::get_key_mods() }
            });
            0
        },

        winuser::WM_MBUTTONUP => {
            use events::WindowEvent::MouseInput;
            use events::MouseButton::Middle;
            use events::ElementState::Released;

            release_mouse();

            send_event(Event::WindowEvent {
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

            capture_mouse(window);

            send_event(Event::WindowEvent {
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

            release_mouse();

            send_event(Event::WindowEvent {
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

            send_event(Event::DeviceEvent {
                device_id: wrap_device_id(lparam as _),
                event,
            });

            winuser::DefWindowProcW(window, msg, wparam, lparam)
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
                            send_event(Event::DeviceEvent {
                                device_id,
                                event: Motion { axis: 0, value: x }
                            });
                        }

                        if y != 0.0 {
                            send_event(Event::DeviceEvent {
                                device_id,
                                event: Motion { axis: 1, value: y }
                            });
                        }

                        if x != 0.0 || y != 0.0 {
                            send_event(Event::DeviceEvent {
                                device_id,
                                event: MouseMotion { delta: (x, y) }
                            });
                        }
                    }

                    if util::has_flag(mouse.usButtonFlags, winuser::RI_MOUSE_WHEEL) {
                        let delta = mouse.usButtonData as SHORT / winuser::WHEEL_DELTA;
                        send_event(Event::DeviceEvent {
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
                            send_event(Event::DeviceEvent {
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

                            send_event(Event::DeviceEvent {
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

            winuser::DefWindowProcW(window, msg, wparam, lparam)
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
                    send_event( Event::WindowEvent {
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
            use events::WindowEvent::Focused;
            send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: Focused(true)
            });

            0
        },

        winuser::WM_KILLFOCUS => {
            use events::WindowEvent::Focused;
            send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: Focused(false)
            });
            0
        },

        winuser::WM_SETCURSOR => {
            let set_cursor_to = CONTEXT_STASH.with(|context_stash| {
                context_stash
                    .borrow()
                    .as_ref()
                    .and_then(|cstash| cstash.windows.get(&window))
                    .and_then(|window_state_mutex| {
                        let window_state = window_state_mutex.lock().unwrap();
                        if window_state.mouse.cursor_flags().contains(CursorFlags::IN_WINDOW) {
                            Some(window_state.mouse.cursor)
                        } else {
                            None
                        }
                    })
            });

            match set_cursor_to {
                Some(cursor) => {
                    let cursor = winuser::LoadCursorW(
                        ptr::null_mut(),
                        cursor.to_windows_cursor(),
                    );
                    winuser::SetCursor(cursor);
                    0
                },
                None => winuser::DefWindowProcW(window, msg, wparam, lparam)
            }
        },

        winuser::WM_DROPFILES => {
            // See `FileDropHandler` for implementation.
            0
        },

        winuser::WM_GETMINMAXINFO => {
            let mmi = lparam as *mut winuser::MINMAXINFO;
            //(*mmi).max_position = winapi::shared::windef::POINT { x: -8, y: -8 }; // The upper left corner of the window if it were maximized on the primary monitor.
            //(*mmi).max_size = winapi::shared::windef::POINT { x: .., y: .. }; // The dimensions of the primary monitor.

            CONTEXT_STASH.with(|context_stash| {
                if let Some(cstash) = context_stash.borrow().as_ref() {
                    if let Some(wstash) = cstash.windows.get(&window) {
                        let window_state = wstash.lock().unwrap();

                        if window_state.min_size.is_some() || window_state.max_size.is_some() {
                            let style = winuser::GetWindowLongA(window, winuser::GWL_STYLE) as DWORD;
                            let ex_style = winuser::GetWindowLongA(window, winuser::GWL_EXSTYLE) as DWORD;
                            if let Some(min_size) = window_state.min_size {
                                let min_size = min_size.to_physical(window_state.dpi_factor);
                                let (width, height) = adjust_size(min_size, style, ex_style);
                                (*mmi).ptMinTrackSize = POINT { x: width as i32, y: height as i32 };
                            }
                            if let Some(max_size) = window_state.max_size {
                                let max_size = max_size.to_physical(window_state.dpi_factor);
                                let (width, height) = adjust_size(max_size, style, ex_style);
                                (*mmi).ptMaxTrackSize = POINT { x: width as i32, y: height as i32 };
                            }
                        }
                    }
                }
            });

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

            let allow_resize = CONTEXT_STASH.with(|context_stash| {
                if let Some(wstash) = context_stash.borrow().as_ref().and_then(|cstash| cstash.windows.get(&window)) {
                    let mut window_state = wstash.lock().unwrap();
                    let old_dpi_factor = window_state.dpi_factor;
                    window_state.dpi_factor = new_dpi_factor;

                    new_dpi_factor != old_dpi_factor && window_state.fullscreen.is_none()
                } else {
                    true
                }
            });

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

            send_event(Event::WindowEvent {
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
                send_event(Event::WindowEvent {
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
            } else if msg == *SET_RETAIN_STATE_ON_SIZE_MSG_ID {
                CONTEXT_STASH.with(|context_stash| {
                    if let Some(cstash) = context_stash.borrow().as_ref() {
                        if let Some(wstash) = cstash.windows.get(&window) {
                            let mut window_state = wstash.lock().unwrap();
                            window_state.set_window_flags_in_place(|f| f.set(WindowFlags::MARKER_RETAIN_STATE_ON_SIZE, wparam != 0));
                        }
                    }
                });
                0
            } else {
                winuser::DefWindowProcW(window, msg, wparam, lparam)
            }
        }
    }
}

pub unsafe extern "system" fn thread_event_target_callback(
    window: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // See `callback` comment.
    run_catch_panic(-1, || {
        match msg {
            _ if msg == *EXEC_MSG_ID => {
                let mut function: ThreadExecFn = Box::from_raw(wparam as usize as *mut _);
                function(Inserter(ptr::null_mut()));
                0
            },
            _ => winuser::DefWindowProcW(window, msg, wparam, lparam)
        }
    })
}
