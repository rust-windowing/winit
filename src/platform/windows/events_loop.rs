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
use std::rc::Rc;
use std::{mem, ptr};
use std::cell::RefCell;
use std::sync::Arc;
use std::collections::VecDeque;
use parking_lot::Mutex;

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
use winapi::um::{winuser, processthreadsapi, ole2, commctrl};
use winapi::um::winnt::{LONG, LPCSTR, SHORT};

use {
    ControlFlow,
    Event,
    EventHandler,
    EventLoopClosed,
    KeyboardInput,
    LogicalPosition,
    LogicalSize,
    PhysicalSize,
    WindowEvent,
    WindowId as SuperWindowId,
};
use events::{DeviceEvent, Touch, TouchPhase};
use platform::platform::{event, Cursor, WindowId, DEVICE_ID, wrap_device_id, util};
use platform::platform::dpi::{
    become_dpi_aware,
    dpi_to_scale_factor,
    enable_non_client_dpi_scaling,
    get_hwnd_scale_factor,
};
use platform::platform::drop_handler::FileDropHandler;
use platform::platform::event::{handle_extended_keys, process_key_params, vkey_to_winit_vkey};
use platform::platform::icon::WinIcon;
use platform::platform::raw_input::{get_raw_input_data, get_raw_mouse_button_state};
use platform::platform::window::adjust_size;

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
    pub mouse_buttons_down: u32
}

pub(crate) struct SubclassInput {
    pub window_state: Arc<Mutex<WindowState>>,
    pub event_queue: Rc<RefCell<VecDeque<Event>>>,
    pub file_drop_handler: FileDropHandler
}

impl SubclassInput {
    fn send_event(&self, event: Event) {
        self.event_queue.borrow_mut().push_back(event);
    }
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

pub struct EventLoop {
    // Id of the background thread from the Win32 API.
    thread_id: DWORD,
    pub(crate) event_queue: Rc<RefCell<VecDeque<Event>>>
}

impl EventLoop {
    pub fn new() -> EventLoop {
        Self::with_dpi_awareness(true)
    }

    pub fn with_dpi_awareness(dpi_aware: bool) -> EventLoop {
        become_dpi_aware(dpi_aware);

        let thread_id = unsafe { processthreadsapi::GetCurrentThreadId() };

        EventLoop {
            thread_id,
            event_queue: Rc::new(RefCell::new(VecDeque::new()))
        }
    }

    pub fn run_forever(self, mut event_handler: impl 'static + EventHandler) -> ! {
        let event_loop = ::EventLoop {
            events_loop: self,
            _marker: ::std::marker::PhantomData
        };
        unsafe {
            // Calling `PostThreadMessageA` on a thread that does not have an events queue yet
            // will fail. In order to avoid this situation, we call `IsGuiThread` to initialize
            // it.
            winuser::IsGUIThread(1);

            let mut msg = mem::uninitialized();

            'main: loop {
                if winuser::GetMessageW(&mut msg, ptr::null_mut(), 0, 0) == 0 {
                    // Only happens if the message is `WM_QUIT`.
                    debug_assert_eq!(msg.message, winuser::WM_QUIT);
                    break 'main;
                }

                match msg.message {
                    x if x == *WAKEUP_MSG_ID => {
                        if ControlFlow::Break == event_handler.handle_event(Event::Awakened, &event_loop) {
                            break 'main;
                        }
                    },
                    x if x == *EXEC_MSG_ID => {
                        let mut function: Box<Box<FnMut()>> = Box::from_raw(msg.wParam as usize as *mut _);
                        function()
                    }
                    _ => {
                        // Calls `event_handler` below.
                        winuser::TranslateMessage(&msg);
                        winuser::DispatchMessageW(&msg);
                    }
                }

                loop {
                    // For whatever reason doing this in a `whlie let` loop doesn't drop the `RefMut`,
                    // so we have to do it like this.
                    let event = match event_loop.events_loop.event_queue.borrow_mut().pop_front() {
                        Some(event) => event,
                        None => break
                    };

                    if ControlFlow::Break == event_handler.handle_event(event, &event_loop) {
                        break 'main;
                    }
                }
            }
        }

        drop(event_handler);
        ::std::process::exit(0);
    }

    pub fn create_proxy(&self) -> EventLoopProxy {
        EventLoopProxy {
            thread_id: self.thread_id,
        }
    }
}

impl Drop for EventLoop {
    fn drop(&mut self) {
        unsafe {
            // Posting `WM_QUIT` will cause `GetMessage` to stop.
            winuser::PostThreadMessageA(self.thread_id, winuser::WM_QUIT, 0, 0);
        }
    }
}

#[derive(Clone)]
pub struct EventLoopProxy {
    thread_id: DWORD,
}

impl EventLoopProxy {
    pub fn wakeup(&self) -> Result<(), EventLoopClosed> {
        unsafe {
            if winuser::PostThreadMessageA(self.thread_id, *WAKEUP_MSG_ID, 0, 0) != 0 {
                Ok(())
            } else {
                // https://msdn.microsoft.com/fr-fr/library/windows/desktop/ms644946(v=vs.85).aspx
                // > If the function fails, the return value is zero. To get extended error
                // > information, call GetLastError. GetLastError returns ERROR_INVALID_THREAD_ID
                // > if idThread is not a valid thread identifier, or if the thread specified by
                // > idThread does not have a message queue. GetLastError returns
                // > ERROR_NOT_ENOUGH_QUOTA when the message limit is hit.
                // TODO: handle ERROR_NOT_ENOUGH_QUOTA
                Err(EventLoopClosed)
            }
        }
    }

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

lazy_static! {
    // Message sent by the `EventLoopProxy` when we want to wake up the thread.
    // WPARAM and LPARAM are unused.
    static ref WAKEUP_MSG_ID: u32 = {
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
pub(crate) fn subclass_window(window: HWND, subclass_input: SubclassInput) {
    let input_ptr = Box::into_raw(Box::new(subclass_input));
    let subclass_result = unsafe{ commctrl::SetWindowSubclass(
        window,
        Some(callback),
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
pub unsafe extern "system" fn callback(
    window: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
    _: UINT_PTR,
    subclass_input_ptr: DWORD_PTR
) -> LRESULT {
    let subclass_input = &mut*(subclass_input_ptr as *mut SubclassInput);

    match msg {
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
            subclass_input.send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: Destroyed
            });

            Box::from_raw(subclass_input);
            drop(subclass_input);
            0
        },

        winuser::WM_PAINT => {
            use events::WindowEvent::Redraw;
            subclass_input.send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: Redraw,
            });
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
