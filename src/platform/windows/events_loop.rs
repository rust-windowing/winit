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

use std::{ptr, mem};
use std::rc::Rc;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;

use winapi::shared::minwindef::{LOWORD, HIWORD, DWORD, WPARAM, LPARAM, INT, UINT, LRESULT, MAX_PATH};
use winapi::shared::windef::{HWND, POINT};
use winapi::shared::basetsd::{UINT_PTR, DWORD_PTR};
use winapi::shared::windowsx;
use winapi::um::{winuser, shellapi, processthreadsapi, commctrl};

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
use events::{Touch, TouchPhase};


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
    pub mouse_buttons_down: u8
}

pub(crate) struct SubclassInput {
    pub window_state: Rc<RefCell<WindowState>>,
    pub event_queue: Rc<RefCell<VecDeque<Event>>>
}

pub struct EventsLoop {
    // Id of the background thread from the Win32 API.
    thread_id: DWORD,
    pub(crate) event_queue: Rc<RefCell<VecDeque<Event>>>
}

impl EventsLoop {
    pub fn new() -> EventsLoop {
        let thread_id = unsafe { processthreadsapi::GetCurrentThreadId() };

        EventsLoop {
            thread_id,
            event_queue: Rc::new(RefCell::new(VecDeque::new()))
        }
    }

    pub fn poll_events<F>(&mut self, mut callback: F)
        where F: FnMut(Event)
    {
        unsafe {
            // Calling `PostThreadMessageA` on a thread that does not have an events queue yet
            // will fail. In order to avoid this situation, we call `IsGuiThread` to initialize
            // it.
            winuser::IsGUIThread(1);

            let mut msg = mem::uninitialized();

            loop {
                if winuser::PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, 1) == 0 {
                    return;
                }

                match msg.message {
                    x if x == *WAKEUP_MSG_ID => {
                        callback(Event::Awakened);
                    },
                    _ => {
                        // Calls `callback` below.
                        winuser::TranslateMessage(&msg);
                        winuser::DispatchMessageW(&msg);
                    }
                }

                let mut event_queue = self.event_queue.borrow_mut();
                for event in event_queue.drain(..) {
                    callback(event);
                }
            }
        }
    }

    pub fn run_forever<F>(&mut self, mut callback: F)
        where F: FnMut(Event) -> ControlFlow
    {
        unsafe {
            // Calling `PostThreadMessageA` on a thread that does not have an events queue yet
            // will fail. In order to avoid this situation, we call `IsGuiThread` to initialize
            // it.
            winuser::IsGUIThread(1);

            let mut msg = mem::uninitialized();

            loop {
                if winuser::GetMessageW(&mut msg, ptr::null_mut(), 0, 0) == 0 {
                    // Only happens if the message is `WM_QUIT`.
                    debug_assert_eq!(msg.message, winuser::WM_QUIT);
                    return;
                }

                match msg.message {
                    x if x == *WAKEUP_MSG_ID => {
                        if ControlFlow::Break == callback(Event::Awakened) {
                            return;
                        }
                    },
                    _ => {
                        // Calls `callback` below.
                        winuser::TranslateMessage(&msg);
                        winuser::DispatchMessageW(&msg);
                    }
                }

                let mut event_queue = self.event_queue.borrow_mut();
                for event in event_queue.drain(..) {
                    if ControlFlow::Break == callback(event) {
                        return;
                    }
                }
            }
        }
    }

    pub fn create_proxy(&self) -> EventsLoopProxy {
        EventsLoopProxy {
            thread_id: self.thread_id,
        }
    }
}

#[derive(Clone)]
pub struct EventsLoopProxy {
    thread_id: DWORD,
}

impl EventsLoopProxy {
    pub fn wakeup(&self) -> Result<(), EventsLoopClosed> {
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
            winuser::RegisterWindowMessageA("Winit::WakeupMsg\0".as_ptr() as *const i8)
        }
    };
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
    window: HWND, msg: UINT,
    wparam: WPARAM, lparam: LPARAM,
    _: UINT_PTR, subclass_input_ptr: DWORD_PTR
) -> LRESULT {
    let subclass_input = &mut*(subclass_input_ptr as *mut SubclassInput);

    match msg {
        winuser::WM_CLOSE => {
            use events::WindowEvent::Closed;
            subclass_input.event_queue.borrow_mut().push_back(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: Closed
            });
            commctrl::DefSubclassProc(window, msg, wparam, lparam)
        },
        winuser::WM_DESTROY => {
            Box::from_raw(subclass_input);
            drop(subclass_input);
            0
        }

        winuser::WM_ERASEBKGND => {
            1
        },

        winuser::WM_PAINT => {
            use events::WindowEvent::Refresh;
            send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: Refresh,
            });
            winuser::DefWindowProcW(window, msg, wparam, lparam)
        },

        winuser::WM_SIZE => {
            use events::WindowEvent::Resized;
            let w = LOWORD(lparam as DWORD) as u32;
            let h = HIWORD(lparam as DWORD) as u32;

            subclass_input.event_queue.borrow_mut().push_back(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: Resized(w, h),
            });
            0
        },

        winuser::WM_MOVE => {
            use events::WindowEvent::Moved;
            let x = LOWORD(lparam as DWORD) as i32;
            let y = HIWORD(lparam as DWORD) as i32;
            subclass_input.event_queue.borrow_mut().push_back(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: Moved(x, y),
            });
            0
        },

        winuser::WM_CHAR => {
            use std::mem;
            use events::WindowEvent::ReceivedCharacter;
            let chr: char = mem::transmute(wparam as u32);
            subclass_input.event_queue.borrow_mut().push_back(Event::WindowEvent {
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
                let mut window_state = subclass_input.window_state.borrow_mut();
                if !window_state.mouse_in_window {
                    window_state.mouse_in_window = true;
                    true
                } else {
                    false
                }
            };

            if mouse_outside_window {
                subclass_input.event_queue.borrow_mut().push_back(Event::WindowEvent {
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

            subclass_input.event_queue.borrow_mut().push_back(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: CursorMoved { device_id: DEVICE_ID, position: (x, y), modifiers: event::get_key_mods() },
            });

            0
        },

        winuser::WM_MOUSELEAVE => {
            use events::WindowEvent::CursorLeft;
            let mouse_in_window = {
                let mut window_state = subclass_input.window_state.borrow_mut();
                if window_state.mouse_in_window {
                    window_state.mouse_in_window = false;
                    true
                } else {
                    false
                }
            };

            if mouse_in_window {
                subclass_input.event_queue.borrow_mut().push_back(Event::WindowEvent {
                    window_id: SuperWindowId(WindowId(window)),
                    event: CursorLeft { device_id: DEVICE_ID }
                });
            }

            0
        },

        winuser::WM_MOUSEWHEEL => {
            use events::{DeviceEvent, WindowEvent};
            use events::MouseScrollDelta::LineDelta;
            use events::TouchPhase;

            let value = (wparam >> 16) as i16;
            let value = value as i32;
            let value = value as f32 / winuser::WHEEL_DELTA as f32;

            subclass_input.event_queue.borrow_mut().push_back(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: WindowEvent::MouseWheel { device_id: DEVICE_ID, delta: LineDelta(0.0, value), phase: TouchPhase::Moved, modifiers: event::get_key_mods() },
            });

            subclass_input.event_queue.borrow_mut().push_back(Event::DeviceEvent {
                device_id: DEVICE_ID,
                event: DeviceEvent::MouseWheel { delta: LineDelta(0.0, value) },
            });

            0
        },

        winuser::WM_KEYDOWN | winuser::WM_SYSKEYDOWN => {
            use events::ElementState::Pressed;
            use events::VirtualKeyCode;
            if msg == winuser::WM_SYSKEYDOWN && wparam as i32 == winuser::VK_F4 {
                commctrl::DefSubclassProc(window, msg, wparam, lparam)
            } else {
                let (scancode, vkey) = event::vkeycode_to_element(wparam, lparam);
                subclass_input.event_queue.borrow_mut().push_back(Event::WindowEvent {
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
                    subclass_input.event_queue.borrow_mut().push_back(Event::WindowEvent {
                        window_id: SuperWindowId(WindowId(window)),
                        event: WindowEvent::ReceivedCharacter('\u{7F}'),
                    });
                }
                0
            }
        },

        winuser::WM_KEYUP | winuser::WM_SYSKEYUP => {
            use events::ElementState::Released;
            let (scancode, vkey) = event::vkeycode_to_element(wparam, lparam);
            subclass_input.event_queue.borrow_mut().push_back(Event::WindowEvent {
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

        winuser::WM_LBUTTONDOWN => {
            use events::WindowEvent::MouseInput;
            use events::MouseButton::Left;
            use events::ElementState::Pressed;

            subclass_input.window_state.borrow_mut().mouse_buttons_down += 1;

            subclass_input.event_queue.borrow_mut().push_back(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: MouseInput { device_id: DEVICE_ID, state: Pressed, button: Left, modifiers: event::get_key_mods() }
            });
            0
        },

        winuser::WM_LBUTTONUP => {
            use events::WindowEvent::MouseInput;
            use events::MouseButton::Left;
            use events::ElementState::Released;

            subclass_input.window_state.borrow_mut().mouse_buttons_down -= 1;

            subclass_input.event_queue.borrow_mut().push_back(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: MouseInput { device_id: DEVICE_ID, state: Released, button: Left, modifiers: event::get_key_mods() }
            });
            0
        },

        winuser::WM_RBUTTONDOWN => {
            use events::WindowEvent::MouseInput;
            use events::MouseButton::Right;
            use events::ElementState::Pressed;

            subclass_input.window_state.borrow_mut().mouse_buttons_down += 1;

            subclass_input.event_queue.borrow_mut().push_back(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: MouseInput { device_id: DEVICE_ID, state: Pressed, button: Right, modifiers: event::get_key_mods() }
            });
            0
        },

        winuser::WM_RBUTTONUP => {
            use events::WindowEvent::MouseInput;
            use events::MouseButton::Right;
            use events::ElementState::Released;

            subclass_input.window_state.borrow_mut().mouse_buttons_down -= 1;

            subclass_input.event_queue.borrow_mut().push_back(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: MouseInput { device_id: DEVICE_ID, state: Released, button: Right, modifiers: event::get_key_mods() }
            });
            0
        },

        winuser::WM_MBUTTONDOWN => {
            use events::WindowEvent::MouseInput;
            use events::MouseButton::Middle;
            use events::ElementState::Pressed;

            subclass_input.window_state.borrow_mut().mouse_buttons_down += 1;

            subclass_input.event_queue.borrow_mut().push_back(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: MouseInput { device_id: DEVICE_ID, state: Pressed, button: Middle, modifiers: event::get_key_mods() }
            });
            0
        },

        winuser::WM_MBUTTONUP => {
            use events::WindowEvent::MouseInput;
            use events::MouseButton::Middle;
            use events::ElementState::Released;

            subclass_input.window_state.borrow_mut().mouse_buttons_down -= 1;

            subclass_input.event_queue.borrow_mut().push_back(Event::WindowEvent {
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

            subclass_input.window_state.borrow_mut().mouse_buttons_down += 1;

            subclass_input.event_queue.borrow_mut().push_back(Event::WindowEvent {
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

            subclass_input.window_state.borrow_mut().mouse_buttons_down -= 1;

            subclass_input.event_queue.borrow_mut().push_back(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: MouseInput { device_id: DEVICE_ID, state: Released, button: Other(xbutton as u8), modifiers: event::get_key_mods() }
            });
            0
        },

        winuser::WM_INPUT => {
            use events::DeviceEvent::{Motion, MouseMotion};
            let mut data: winuser::RAWINPUT = mem::uninitialized();
            let mut data_size = mem::size_of::<winuser::RAWINPUT>() as UINT;
            winuser::GetRawInputData(mem::transmute(lparam), winuser::RID_INPUT,
                                    mem::transmute(&mut data), &mut data_size,
                                    mem::size_of::<winuser::RAWINPUTHEADER>() as UINT);

            if data.header.dwType == winuser::RIM_TYPEMOUSE {
                let mouse = data.data.mouse();
                if mouse.usFlags & winuser::MOUSE_MOVE_RELATIVE == winuser::MOUSE_MOVE_RELATIVE {
                    let x = mouse.lLastX as f64;
                    let y = mouse.lLastY as f64;

                    if x != 0.0 {
                        subclass_input.event_queue.borrow_mut().push_back(Event::DeviceEvent {
                            device_id: DEVICE_ID,
                            event: Motion { axis: 0, value: x }
                        });
                    }

                    if y != 0.0 {
                        subclass_input.event_queue.borrow_mut().push_back(Event::DeviceEvent {
                            device_id: DEVICE_ID,
                            event: Motion { axis: 1, value: y }
                        });
                    }

                    if x != 0.0 || y != 0.0 {
                        subclass_input.event_queue.borrow_mut().push_back(Event::DeviceEvent {
                            device_id: DEVICE_ID,
                            event: MouseMotion { delta: (x, y) }
                        });
                    }
                }

                0
            } else {
                commctrl::DefSubclassProc(window, msg, wparam, lparam)
            }
        },

        winuser::WM_TOUCH => {
            let pcount = LOWORD( wparam as DWORD ) as usize;
            let mut inputs = Vec::with_capacity( pcount );
            inputs.set_len( pcount );
            let htouch = lparam as winuser::HTOUCHINPUT;
            if winuser::GetTouchInputInfo( htouch, pcount as UINT,
                                           inputs.as_mut_ptr(),
                                           mem::size_of::<winuser::TOUCHINPUT>() as INT ) > 0 {
                for input in &inputs {
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
                            location: ((input.x as f64) / 100f64,
                                       (input.y as f64) / 100f64),
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
            subclass_input.event_queue.borrow_mut().push_back(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: Focused(true)
            });

            let x = windowsx::GET_X_LPARAM(lparam) as f64;
            let y = windowsx::GET_Y_LPARAM(lparam) as f64;

            subclass_input.event_queue.borrow_mut().push_back(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: CursorMoved { device_id: DEVICE_ID, position: (x, y), modifiers: event::get_key_mods() },
            });
            0
        },

        winuser::WM_KILLFOCUS => {
            use events::WindowEvent::Focused;
            subclass_input.event_queue.borrow_mut().push_back(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(window)),
                event: Focused(false)
            });
            0
        },

        winuser::WM_SETCURSOR => {
            let call_def_window_proc = {
                let window_state = subclass_input.window_state.borrow_mut();
                if window_state.mouse_in_window {
                    match window_state.cursor_state {
                        CursorState::Normal => {
                            winuser::SetCursor(winuser::LoadCursorW(
                                    ptr::null_mut(),
                                    window_state.cursor));
                        },
                        CursorState::Grab | CursorState::Hide => {
                            winuser::SetCursor(ptr::null_mut());
                        }
                    }
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
            use events::WindowEvent::DroppedFile;

            let hdrop = wparam as shellapi::HDROP;
            let mut pathbuf: [u16; MAX_PATH] = mem::uninitialized();
            let num_drops = shellapi::DragQueryFileW(hdrop, 0xFFFFFFFF, ptr::null_mut(), 0);

            for i in 0..num_drops {
                let nch = shellapi::DragQueryFileW(hdrop, i, pathbuf.as_mut_ptr(),
                                                  MAX_PATH as u32) as usize;
                if nch > 0 {
                    subclass_input.event_queue.borrow_mut().push_back(Event::WindowEvent {
                        window_id: SuperWindowId(WindowId(window)),
                        event: DroppedFile(OsString::from_wide(&pathbuf[0..nch]).into())
                    });
                }
            }

            shellapi::DragFinish(hdrop);
            0
        },

        winuser::WM_GETMINMAXINFO => {
            let mmi = lparam as *mut winuser::MINMAXINFO;
            //(*mmi).max_position = winapi::shared::windef::POINT { x: -8, y: -8 }; // The upper left corner of the window if it were maximized on the primary monitor.
            //(*mmi).max_size = winapi::shared::windef::POINT { x: .., y: .. }; // The dimensions of the primary monitor.

            let window_state = subclass_input.window_state.borrow();

            match window_state.attributes.min_dimensions {
                Some((width, height)) => {
                    (*mmi).ptMinTrackSize = POINT { x: width as i32, y: height as i32 };
                },
                None => { }
            }

            match window_state.attributes.max_dimensions {
                Some((width, height)) => {
                    (*mmi).ptMaxTrackSize = POINT { x: width as i32, y: height as i32 };
                },
                None => { }
            }

            0
        },

        _ => {
            commctrl::DefSubclassProc(window, msg, wparam, lparam)
        }
    }
}
