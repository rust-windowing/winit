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

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::OsString;
use std::mem;
use std::os::windows::ffi::OsStringExt;
use std::os::windows::io::AsRawHandle;
use std::ptr;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Barrier;
use std::sync::Mutex;
use std::sync::Condvar;
use std::thread;

use kernel32;
use shell32;
use user32;
use winapi;

use platform::platform::event;
use platform::platform::Cursor;
use platform::platform::WindowId;
use platform::platform::DEVICE_ID;

use ControlFlow;
use CursorState;
use Event;
use EventsLoopClosed;
use KeyboardInput;
use WindowAttributes;
use WindowEvent;
use WindowId as SuperWindowId;

/// Contains information about states and the window that the callback is going to use.
#[derive(Clone)]
pub struct WindowState {
    /// Cursor to set at the next `WM_SETCURSOR` event received.
    pub cursor: Cursor,
    /// Cursor state to set at the next `WM_SETCURSOR` event received.
    pub cursor_state: CursorState,
    /// Used by `WM_GETMINMAXINFO`.
    pub attributes: WindowAttributes,
    /// Will contain `true` if the mouse is hovering the window.
    pub mouse_in_window: bool,
}

/// Dummy object that allows inserting a window's state.
// We store a pointer in order to !impl Send and Sync.
pub struct Inserter(*mut u8);

impl Inserter {
    /// Inserts a window's state for the callback to use. The state is removed automatically if the
    /// callback receives a `WM_CLOSE` message for the window.
    pub fn insert(&self, window: winapi::HWND, state: Arc<Mutex<WindowState>>) {
        CONTEXT_STASH.with(|context_stash| {
            let mut context_stash = context_stash.borrow_mut();
            let was_in = context_stash.as_mut().unwrap().windows.insert(window, state);
            assert!(was_in.is_none());
        });
    }
}

pub struct EventsLoop {
    // Id of the background thread from the Win32 API.
    thread_id: winapi::DWORD,
    // Receiver for the events. The sender is in the background thread.
    receiver: mpsc::Receiver<Event>,
    // Variable that contains the block state of the win32 event loop thread during a WM_SIZE event.
    // The mutex's value is `true` when it's blocked, and should be set to false when it's done
    // blocking. That's done by the parent thread when it receives a Resized event.
    win32_block_loop: Arc<(Mutex<bool>, Condvar)>
}

impl EventsLoop {
    pub fn new() -> EventsLoop {
        // The main events transfer channel.
        let (tx, rx) = mpsc::channel();
        let win32_block_loop = Arc::new((Mutex::new(false), Condvar::new()));
        let win32_block_loop_child = win32_block_loop.clone();

        // Local barrier in order to block the `new()` function until the background thread has
        // an events queue.
        let barrier = Arc::new(Barrier::new(2));
        let barrier_clone = barrier.clone();

        let thread = thread::spawn(move || {
            CONTEXT_STASH.with(|context_stash| {
                *context_stash.borrow_mut() = Some(ThreadLocalData {
                    sender: tx,
                    windows: HashMap::with_capacity(4),
                    win32_block_loop: win32_block_loop_child
                });
            });

            unsafe {
                // Calling `PostThreadMessageA` on a thread that does not have an events queue yet
                // will fail. In order to avoid this situation, we call `IsGuiThread` to initialize
                // it.
                user32::IsGUIThread(1);
                // Then only we unblock the `new()` function. We are sure that we don't call
                // `PostThreadMessageA()` before `new()` returns.
                barrier_clone.wait();
                drop(barrier_clone);

                let mut msg = mem::uninitialized();

                loop {
                    if user32::GetMessageW(&mut msg, ptr::null_mut(), 0, 0) == 0 {
                        // Only happens if the message is `WM_QUIT`.
                        debug_assert_eq!(msg.message, winapi::WM_QUIT);
                        break;
                    }

                    match msg.message {
                        x if x == *EXEC_MSG_ID => {
                            let mut function: Box<Box<FnMut(Inserter)>> = Box::from_raw(msg.wParam as usize as *mut _);
                            function(Inserter(ptr::null_mut()));
                        },
                        x if x == *WAKEUP_MSG_ID => {
                            send_event(Event::Awakened);
                        },
                        _ => {
                            // Calls `callback` below.
                            user32::TranslateMessage(&msg);
                            user32::DispatchMessageW(&msg);
                        }
                    }
                }
            }
        });

        // Blocks this function until the background thread has an events loop. See other comments.
        barrier.wait();

        EventsLoop {
            thread_id: unsafe { kernel32::GetThreadId(thread.as_raw_handle()) },
            receiver: rx,
            win32_block_loop
        }
    }

    pub fn poll_events<F>(&mut self, mut callback: F)
        where F: FnMut(Event)
    {
        loop {
            let event = match self.receiver.try_recv() {
                Ok(e) => e,
                Err(_) => return
            };
            let is_resize = match event {
                Event::WindowEvent{ event: WindowEvent::Resized(..), .. } => true,
                _ => false
            };

            callback(event);
            if is_resize {
                let (ref mutex, ref cvar) = *self.win32_block_loop;
                let mut block_thread = mutex.lock().unwrap();
                *block_thread = false;
                cvar.notify_all();
            }
        }
    }

    pub fn run_forever<F>(&mut self, mut callback: F)
        where F: FnMut(Event) -> ControlFlow
    {
        loop {
            let event = match self.receiver.recv() {
                Ok(e) => e,
                Err(_) => return
            };
            let is_resize = match event {
                Event::WindowEvent{ event: WindowEvent::Resized(..), .. } => true,
                _ => false
            };

            let flow = callback(event);
            if is_resize {
                let (ref mutex, ref cvar) = *self.win32_block_loop;
                let mut block_thread = mutex.lock().unwrap();
                *block_thread = false;
                cvar.notify_all();
            }
            match flow {
                ControlFlow::Continue => continue,
                ControlFlow::Break => break,
            }
        }
    }

    pub fn create_proxy(&self) -> EventsLoopProxy {
        EventsLoopProxy {
            thread_id: self.thread_id,
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
        unsafe {
            let boxed = Box::new(function) as Box<FnMut(_)>;
            let boxed2 = Box::new(boxed);

            let raw = Box::into_raw(boxed2);

            let res = user32::PostThreadMessageA(self.thread_id, *EXEC_MSG_ID,
                                                 raw as *mut () as usize as winapi::WPARAM, 0);
            // PostThreadMessage can only fail if the thread ID is invalid (which shouldn't happen
            // as the events loop is still alive) or if the queue is full.
            assert!(res != 0, "PostThreadMessage failed ; is the messages queue full?");
        }
    }
}

impl Drop for EventsLoop {
    fn drop(&mut self) {
        unsafe {
            // Posting `WM_QUIT` will cause `GetMessage` to stop.
            user32::PostThreadMessageA(self.thread_id, winapi::WM_QUIT, 0, 0);
        }
    }
}

#[derive(Clone)]
pub struct EventsLoopProxy {
    thread_id: winapi::DWORD,
}

impl EventsLoopProxy {
    pub fn wakeup(&self) -> Result<(), EventsLoopClosed> {
        unsafe {
            if user32::PostThreadMessageA(self.thread_id, *WAKEUP_MSG_ID, 0, 0) != 0 {
                Ok(())
            } else {
                // https://msdn.microsoft.com/fr-fr/library/windows/desktop/ms644946(v=vs.85).aspx
                // > If the function fails, the return value is zero. To get extended error
                // > information, call GetLastError. GetLastError returns ERROR_INVALID_THREAD_ID
                // > if idThread is not a valid thread identifier, or if the thread specified by
                // > idThread does not have a message queue. GetLastError returns
                // > ERROR_NOT_ENOUGH_QUOTA when the message limit is hit.
                // TODO: handle ERROR_NOT_ENOUGH_QUOTA
                Err(EventsLoopClosed)
            }
        }
    }
}

lazy_static! {
    // Message sent by the `EventsLoopProxy` when we want to wake up the thread.
    // WPARAM and LPARAM are unused.
    static ref WAKEUP_MSG_ID: u32 = {
        unsafe {
            user32::RegisterWindowMessageA("Winit::WakeupMsg\0".as_ptr() as *const i8)
        }
    };
    // Message sent when we want to execute a closure in the thread.
    // WPARAM contains a Box<Box<FnMut()>> that must be retreived with `Box::from_raw`,
    // and LPARAM is unused.
    static ref EXEC_MSG_ID: u32 = {
        unsafe {
            user32::RegisterWindowMessageA("Winit::ExecMsg\0".as_ptr() as *const i8)
        }
    };
}

// There's no parameters passed to the callback function, so it needs to get its context stashed
// in a thread-local variable.
thread_local!(static CONTEXT_STASH: RefCell<Option<ThreadLocalData>> = RefCell::new(None));
struct ThreadLocalData {
    sender: mpsc::Sender<Event>,
    windows: HashMap<winapi::HWND, Arc<Mutex<WindowState>>>,
    win32_block_loop: Arc<(Mutex<bool>, Condvar)>
}

// Utility function that dispatches an event on the current thread.
fn send_event(event: Event) {
    CONTEXT_STASH.with(|context_stash| {
        let context_stash = context_stash.borrow();

        let _ = context_stash.as_ref().unwrap().sender.send(event);   // Ignoring if closed
    });
}

/// Any window whose callback is configured to this function will have its events propagated
/// through the events loop of the thread the window was created in.
//
// This is the callback that is called by `DispatchMessage` in the events loop.
//
// Returning 0 tells the Win32 API that the message has been processed.
// FIXME: detect WM_DWMCOMPOSITIONCHANGED and call DwmEnableBlurBehindWindow if necessary
pub unsafe extern "system" fn callback(window: winapi::HWND, msg: winapi::UINT,
                                       wparam: winapi::WPARAM, lparam: winapi::LPARAM)
                                       -> winapi::LRESULT
{
    match msg {
        winapi::WM_CLOSE => {
            use events::WindowEvent::Closed;
            send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: Closed
            });
            CONTEXT_STASH.with(|context_stash| {
                let mut context_stash = context_stash.borrow_mut();
                context_stash.as_mut().unwrap().windows.remove(&window);
            });
            user32::DefWindowProcW(window, msg, wparam, lparam)
        },

        winapi::WM_ERASEBKGND => {
            1
        },

        winapi::WM_SIZE => {
            use events::WindowEvent::Resized;
            let w = winapi::LOWORD(lparam as winapi::DWORD) as u32;
            let h = winapi::HIWORD(lparam as winapi::DWORD) as u32;

            // Wait for the parent thread to process the resize event before returning from the
            // callback.
            CONTEXT_STASH.with(|context_stash| {
                let mut context_stash = context_stash.borrow_mut();
                let cstash = context_stash.as_mut().unwrap();

                let event = Event::WindowEvent {
                    window_id: SuperWindowId(WindowId(window)),
                    event: Resized(w, h),
                };

                // If this window has been inserted into the window map, the resize event happened
                // during the event loop. If it hasn't, the event happened on window creation and
                // should be ignored.
                if cstash.windows.get(&window).is_some() {
                    let (ref mutex, ref cvar) = *cstash.win32_block_loop;
                    let mut block_thread = mutex.lock().unwrap();
                    *block_thread = true;

                    // The event needs to be sent after the lock to ensure that `notify_all` is
                    // called after `wait`.
                    cstash.sender.send(event).ok();

                    while *block_thread {
                        block_thread = cvar.wait(block_thread).unwrap();
                    }
                } else {
                    cstash.sender.send(event).ok();
                }
            });
            0
        },

        winapi::WM_MOVE => {
            use events::WindowEvent::Moved;
            let x = winapi::LOWORD(lparam as winapi::DWORD) as i32;
            let y = winapi::HIWORD(lparam as winapi::DWORD) as i32;
            send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: Moved(x, y),
            });
            0
        },

        winapi::WM_CHAR => {
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
        winapi::WM_SYSCHAR => {
            0
        }

        winapi::WM_MOUSEMOVE => {
            use events::WindowEvent::{CursorEntered, CursorMoved};
            let mouse_outside_window = CONTEXT_STASH.with(|context_stash| {
                let mut context_stash = context_stash.borrow_mut();
                if let Some(context_stash) = context_stash.as_mut() {
                    if let Some(w) = context_stash.windows.get_mut(&window) {
                        let mut w = w.lock().unwrap();
                        if !w.mouse_in_window {
                            w.mouse_in_window = true;
                            return true;
                        }
                    }
                }

                false
            });

            if mouse_outside_window {
                send_event(Event::WindowEvent {
                    window_id: SuperWindowId(WindowId(window)),
                    event: CursorEntered { device_id: DEVICE_ID },
                });

                // Calling TrackMouseEvent in order to receive mouse leave events.
                user32::TrackMouseEvent(&mut winapi::TRACKMOUSEEVENT {
                    cbSize: mem::size_of::<winapi::TRACKMOUSEEVENT>() as winapi::DWORD,
                    dwFlags: winapi::TME_LEAVE,
                    hwndTrack: window,
                    dwHoverTime: winapi::HOVER_DEFAULT,
                });
            }

            let x = winapi::GET_X_LPARAM(lparam) as f64;
            let y = winapi::GET_Y_LPARAM(lparam) as f64;

            send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: CursorMoved { device_id: DEVICE_ID, position: (x, y) },
            });

            0
        },

        winapi::WM_MOUSELEAVE => {
            use events::WindowEvent::CursorLeft;
            let mouse_in_window = CONTEXT_STASH.with(|context_stash| {
                let mut context_stash = context_stash.borrow_mut();
                if let Some(context_stash) = context_stash.as_mut() {
                    if let Some(w) = context_stash.windows.get_mut(&window) {
                        let mut w = w.lock().unwrap();
                        if w.mouse_in_window {
                            w.mouse_in_window = false;
                            return true;
                        }
                    }
                }

                false
            });

            if mouse_in_window {
                send_event(Event::WindowEvent {
                    window_id: SuperWindowId(WindowId(window)),
                    event: CursorLeft { device_id: DEVICE_ID }
                });
            }

            0
        },

        winapi::WM_MOUSEWHEEL => {
            use events::{DeviceEvent, WindowEvent};
            use events::MouseScrollDelta::LineDelta;
            use events::TouchPhase;

            let value = (wparam >> 16) as i16;
            let value = value as i32;
            let value = value as f32 / winapi::WHEEL_DELTA as f32;

            send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: WindowEvent::MouseWheel { device_id: DEVICE_ID, delta: LineDelta(0.0, value), phase: TouchPhase::Moved },
            });

            send_event(Event::DeviceEvent {
                device_id: DEVICE_ID,
                event: DeviceEvent::MouseWheel { delta: LineDelta(0.0, value) },
            });

            0
        },

        winapi::WM_KEYDOWN | winapi::WM_SYSKEYDOWN => {
            use events::ElementState::Pressed;
            use events::VirtualKeyCode;
            if msg == winapi::WM_SYSKEYDOWN && wparam as i32 == winapi::VK_F4 {
                user32::DefWindowProcW(window, msg, wparam, lparam)
            } else {
                let (scancode, vkey) = event::vkeycode_to_element(wparam, lparam);
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
                0
            }
        },

        winapi::WM_KEYUP | winapi::WM_SYSKEYUP => {
            use events::ElementState::Released;
            let (scancode, vkey) = event::vkeycode_to_element(wparam, lparam);
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
            0
        },

        winapi::WM_LBUTTONDOWN => {
            use events::WindowEvent::MouseInput;
            use events::MouseButton::Left;
            use events::ElementState::Pressed;
            send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: MouseInput { device_id: DEVICE_ID, state: Pressed, button: Left }
            });
            0
        },

        winapi::WM_LBUTTONUP => {
            use events::WindowEvent::MouseInput;
            use events::MouseButton::Left;
            use events::ElementState::Released;
            send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: MouseInput { device_id: DEVICE_ID, state: Released, button: Left }
            });
            0
        },

        winapi::WM_RBUTTONDOWN => {
            use events::WindowEvent::MouseInput;
            use events::MouseButton::Right;
            use events::ElementState::Pressed;
            send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: MouseInput { device_id: DEVICE_ID, state: Pressed, button: Right }
            });
            0
        },

        winapi::WM_RBUTTONUP => {
            use events::WindowEvent::MouseInput;
            use events::MouseButton::Right;
            use events::ElementState::Released;
            send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: MouseInput { device_id: DEVICE_ID, state: Released, button: Right }
            });
            0
        },

        winapi::WM_MBUTTONDOWN => {
            use events::WindowEvent::MouseInput;
            use events::MouseButton::Middle;
            use events::ElementState::Pressed;
            send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: MouseInput { device_id: DEVICE_ID, state: Pressed, button: Middle }
            });
            0
        },

        winapi::WM_MBUTTONUP => {
            use events::WindowEvent::MouseInput;
            use events::MouseButton::Middle;
            use events::ElementState::Released;
            send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: MouseInput { device_id: DEVICE_ID, state: Released, button: Middle }
            });
            0
        },

        winapi::WM_XBUTTONDOWN => {
            use events::WindowEvent::MouseInput;
            use events::MouseButton::Other;
            use events::ElementState::Pressed;
            let xbutton = winapi::HIWORD(wparam as winapi::DWORD) as winapi::c_int; // waiting on PR for winapi to add GET_XBUTTON_WPARAM
            send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: MouseInput { device_id: DEVICE_ID, state: Pressed, button: Other(xbutton as u8) }
            });
            0
        },

        winapi::WM_XBUTTONUP => {
            use events::WindowEvent::MouseInput;
            use events::MouseButton::Other;
            use events::ElementState::Released;
            let xbutton = winapi::HIWORD(wparam as winapi::DWORD) as winapi::c_int;
            send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: MouseInput { device_id: DEVICE_ID, state: Released, button: Other(xbutton as u8) }
            });
            0
        },

        winapi::WM_INPUT => {
            use events::DeviceEvent::{Motion, MouseMotion};
            let mut data: winapi::RAWINPUT = mem::uninitialized();
            let mut data_size = mem::size_of::<winapi::RAWINPUT>() as winapi::UINT;
            user32::GetRawInputData(mem::transmute(lparam), winapi::RID_INPUT,
                                    mem::transmute(&mut data), &mut data_size,
                                    mem::size_of::<winapi::RAWINPUTHEADER>() as winapi::UINT);

            if data.header.dwType == winapi::RIM_TYPEMOUSE {
                if data.mouse.usFlags & winapi::MOUSE_MOVE_RELATIVE == winapi::MOUSE_MOVE_RELATIVE {
                    let x = data.mouse.lLastX as f64;
                    let y = data.mouse.lLastY as f64;

                    if x != 0.0 {
                        send_event(Event::DeviceEvent {
                            device_id: DEVICE_ID,
                            event: Motion { axis: 0, value: x }
                        });
                    }

                    if y != 0.0 {
                        send_event(Event::DeviceEvent {
                            device_id: DEVICE_ID,
                            event: Motion { axis: 1, value: y }
                        });
                    }

                    if x != 0.0 || y != 0.0 {
                        send_event(Event::DeviceEvent {
                            device_id: DEVICE_ID,
                            event: MouseMotion { delta: (x, y) }
                        });
                    }
                }

                0
            } else {
                user32::DefWindowProcW(window, msg, wparam, lparam)
            }
        },

        winapi::WM_SETFOCUS => {
            use events::WindowEvent::{Focused, CursorMoved};
            send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: Focused(true)
            });

            let x = winapi::GET_X_LPARAM(lparam) as f64;
            let y = winapi::GET_Y_LPARAM(lparam) as f64;

            send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: CursorMoved { device_id: DEVICE_ID, position: (x, y) },
            });
            0
        },

        winapi::WM_KILLFOCUS => {
            use events::WindowEvent::Focused;
            send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: Focused(false)
            });
            0
        },

        winapi::WM_SETCURSOR => {
            let call_def_window_proc = CONTEXT_STASH.with(|context_stash| {
                let cstash = context_stash.borrow();
                let mut call_def_window_proc = false;
                if let Some(cstash) = cstash.as_ref() {
                    if let Some(w_stash) = cstash.windows.get(&window) {
                        if let Ok(window_state) = w_stash.lock() {
                            if window_state.mouse_in_window {
                                match window_state.cursor_state {
                                    CursorState::Normal => {
                                        user32::SetCursor(user32::LoadCursorW(
                                                ptr::null_mut(),
                                                window_state.cursor));
                                    },
                                    CursorState::Grab | CursorState::Hide => {
                                        user32::SetCursor(ptr::null_mut());
                                    }
                                }
                            } else {
                                call_def_window_proc = true;
                            }
                        }
                    }
                }

                call_def_window_proc
            });

            if call_def_window_proc {
                user32::DefWindowProcW(window, msg, wparam, lparam)
            } else {
                0
            }
        },

        winapi::WM_DROPFILES => {
            use events::WindowEvent::DroppedFile;

            let hdrop = wparam as winapi::HDROP;
            let mut pathbuf: [u16; winapi::MAX_PATH] = mem::uninitialized();
            let num_drops = shell32::DragQueryFileW(hdrop, 0xFFFFFFFF, ptr::null_mut(), 0);

            for i in 0..num_drops {
                let nch = shell32::DragQueryFileW(hdrop, i, pathbuf.as_mut_ptr(),
                                                  winapi::MAX_PATH as u32) as usize;
                if nch > 0 {
                    send_event(Event::WindowEvent {
                        window_id: SuperWindowId(WindowId(window)),
                        event: DroppedFile(OsString::from_wide(&pathbuf[0..nch]).into())
                    });
                }
            }

            shell32::DragFinish(hdrop);
            0
        },

        winapi::WM_GETMINMAXINFO => {
            // Equivalent to the windows api [MINMAXINFO](https://msdn.microsoft.com/en-us/library/windows/desktop/ms632605%28v=vs.85%29.aspx)
            // struct. Used because winapi-rs doesn't have this declared.
            // TODO: replace with struct from winapi-rs
            #[repr(C)]
            #[allow(dead_code)]
            struct MinMaxInfo {
                reserved: winapi::POINT, // Do not use/change
                max_size: winapi::POINT,
                max_position: winapi::POINT,
                min_track: winapi::POINT,
                max_track: winapi::POINT
            }

            let mmi = lparam as *mut MinMaxInfo;
            //(*mmi).max_position = winapi::POINT { x: -8, y: -8 }; // The upper left corner of the window if it were maximized on the primary monitor.
            //(*mmi).max_size = winapi::POINT { x: .., y: .. }; // The dimensions of the primary monitor.

            CONTEXT_STASH.with(|context_stash| {
                if let Some(cstash) = context_stash.borrow().as_ref() {
                    if let Some(wstash) = cstash.windows.get(&window) {
                        let window_state = wstash.lock().unwrap();

                        match window_state.attributes.min_dimensions {
                            Some((width, height)) => {
                                (*mmi).min_track = winapi::POINT { x: width as i32, y: height as i32 };
                            },
                            None => { }
                        }

                        match window_state.attributes.max_dimensions {
                            Some((width, height)) => {
                                (*mmi).max_track = winapi::POINT { x: width as i32, y: height as i32 };
                            },
                            None => { }
                        }
                    }
                }
            });

            0
        },

        _ => {
            user32::DefWindowProcW(window, msg, wparam, lparam)
        }
    }
}
