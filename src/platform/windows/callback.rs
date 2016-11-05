use std::mem;
use std::ptr;
use std::cell::RefCell;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;

use CursorState;
use Event;
use super::event;
use super::WindowState;

use user32;
use shell32;
use winapi;

/// There's no parameters passed to the callback function, so it needs to get
/// its context (the HWND, the Sender for events, etc.) stashed in
/// a thread-local variable.
thread_local!(pub static CONTEXT_STASH: RefCell<Option<ThreadLocalData>> = RefCell::new(None));

pub struct ThreadLocalData {
    pub win: winapi::HWND,
    pub sender: Sender<Event>,
    pub window_state: Arc<Mutex<WindowState>>,
    pub mouse_in_window: bool
}

/// Equivalent to the windows api [MINMAXINFO](https://msdn.microsoft.com/en-us/library/windows/desktop/ms632605%28v=vs.85%29.aspx)
/// struct. Used because winapi-rs doesn't have this declared.
#[repr(C)]
#[allow(dead_code)]
struct MinMaxInfo {
    reserved: winapi::POINT, // Do not use/change
    max_size: winapi::POINT,
    max_position: winapi::POINT,
    min_track: winapi::POINT,
    max_track: winapi::POINT
}

/// Checks that the window is the good one, and if so send the event to it.
fn send_event(input_window: winapi::HWND, event: Event) {
    CONTEXT_STASH.with(|context_stash| {
        let context_stash = context_stash.borrow();
        let stored = match *context_stash {
            None => return,
            Some(ref v) => v
        };

        let &ThreadLocalData { ref win, ref sender, .. } = stored;

        if win != &input_window {
            return;
        }

        sender.send(event).ok();  // ignoring if closed
    });
}

/// This is the callback that is called by `DispatchMessage` in the events loop.
///
/// Returning 0 tells the Win32 API that the message has been processed.
// FIXME: detect WM_DWMCOMPOSITIONCHANGED and call DwmEnableBlurBehindWindow if necessary
pub unsafe extern "system" fn callback(window: winapi::HWND, msg: winapi::UINT,
                                       wparam: winapi::WPARAM, lparam: winapi::LPARAM)
                                       -> winapi::LRESULT
{
    match msg {
        winapi::WM_DESTROY => {
            use events::Event::Closed;

            CONTEXT_STASH.with(|context_stash| {
                let context_stash = context_stash.borrow();
                let stored = match *context_stash {
                    None => return,
                    Some(ref v) => v
                };

                let &ThreadLocalData { ref win, .. } = stored;

                if win == &window {
                    user32::PostQuitMessage(0);
                }
            });

            send_event(window, Closed);
            0
        },

        winapi::WM_ERASEBKGND => {
            1
        },

        winapi::WM_SIZE => {
            use events::Event::Resized;
            let w = winapi::LOWORD(lparam as winapi::DWORD) as u32;
            let h = winapi::HIWORD(lparam as winapi::DWORD) as u32;
            send_event(window, Resized(w, h));
            0
        },

        winapi::WM_MOVE => {
            use events::Event::Moved;
            let x = winapi::LOWORD(lparam as winapi::DWORD) as i32;
            let y = winapi::HIWORD(lparam as winapi::DWORD) as i32;
            send_event(window, Moved(x, y));
            0
        },

        winapi::WM_CHAR => {
            use std::mem;
            use events::Event::ReceivedCharacter;
            let chr: char = mem::transmute(wparam as u32);
            send_event(window, ReceivedCharacter(chr));
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
            use events::Event::{MouseEntered, MouseMoved};
            let mouse_outside_window = CONTEXT_STASH.with(|context_stash| {
                let mut context_stash = context_stash.borrow_mut();
                if let Some(context_stash) = context_stash.as_mut() {
                    if !context_stash.mouse_in_window {
                        context_stash.mouse_in_window = true;
                        return true;
                    }
                }

                false
            });

            if mouse_outside_window {
                send_event(window, MouseEntered);
                
                // Calling TrackMouseEvent in order to receive mouse leave events.
                user32::TrackMouseEvent(&mut winapi::TRACKMOUSEEVENT {
                    cbSize: mem::size_of::<winapi::TRACKMOUSEEVENT>() as winapi::DWORD,
                    dwFlags: winapi::TME_LEAVE,
                    hwndTrack: window,
                    dwHoverTime: winapi::HOVER_DEFAULT,
                });
            }

            let x = winapi::GET_X_LPARAM(lparam) as i32;
            let y = winapi::GET_Y_LPARAM(lparam) as i32;

            send_event(window, MouseMoved(x, y));

            0
        },

        winapi::WM_MOUSELEAVE => {
            use events::Event::MouseLeft;
            let mouse_in_window = CONTEXT_STASH.with(|context_stash| {
                let mut context_stash = context_stash.borrow_mut();
                if let Some(context_stash) = context_stash.as_mut() {
                    if context_stash.mouse_in_window {
                        context_stash.mouse_in_window = false;
                        return true;
                    }
                }

                false
            });

            if mouse_in_window {
                send_event(window, MouseLeft);
            }

            0
        },

        winapi::WM_MOUSEWHEEL => {
            use events::Event::MouseWheel;
            use events::MouseScrollDelta::LineDelta;
            use events::TouchPhase;

            let value = (wparam >> 16) as i16;
            let value = value as i32;
            let value = value as f32 / winapi::WHEEL_DELTA as f32;

            send_event(window, MouseWheel(LineDelta(0.0, value), TouchPhase::Moved));

            0
        },

        winapi::WM_KEYDOWN | winapi::WM_SYSKEYDOWN => {
            use events::Event::KeyboardInput;
            use events::ElementState::Pressed;
            if msg == winapi::WM_SYSKEYDOWN && wparam as i32 == winapi::VK_F4 {
                user32::DefWindowProcW(window, msg, wparam, lparam)
            } else {
                let (scancode, vkey) = event::vkeycode_to_element(wparam, lparam);
                send_event(window, KeyboardInput(Pressed, scancode, vkey));
                0
            }
        },

        winapi::WM_KEYUP | winapi::WM_SYSKEYUP => {
            use events::Event::KeyboardInput;
            use events::ElementState::Released;
            let (scancode, vkey) = event::vkeycode_to_element(wparam, lparam);
            send_event(window, KeyboardInput(Released, scancode, vkey));
            0
        },

        winapi::WM_LBUTTONDOWN => {
            use events::Event::MouseInput;
            use events::MouseButton::Left;
            use events::ElementState::Pressed;
            send_event(window, MouseInput(Pressed, Left));
            0
        },

        winapi::WM_LBUTTONUP => {
            use events::Event::MouseInput;
            use events::MouseButton::Left;
            use events::ElementState::Released;
            send_event(window, MouseInput(Released, Left));
            0
        },

        winapi::WM_RBUTTONDOWN => {
            use events::Event::MouseInput;
            use events::MouseButton::Right;
            use events::ElementState::Pressed;
            send_event(window, MouseInput(Pressed, Right));
            0
        },

        winapi::WM_RBUTTONUP => {
            use events::Event::MouseInput;
            use events::MouseButton::Right;
            use events::ElementState::Released;
            send_event(window, MouseInput(Released, Right));
            0
        },

        winapi::WM_MBUTTONDOWN => {
            use events::Event::MouseInput;
            use events::MouseButton::Middle;
            use events::ElementState::Pressed;
            send_event(window, MouseInput(Pressed, Middle));
            0
        },

        winapi::WM_MBUTTONUP => {
            use events::Event::MouseInput;
            use events::MouseButton::Middle;
            use events::ElementState::Released;
            send_event(window, MouseInput(Released, Middle));
            0
        },

        winapi::WM_XBUTTONDOWN => {
            use events::Event::MouseInput;
            use events::MouseButton::Other;
            use events::ElementState::Pressed;
            let xbutton = winapi::HIWORD(wparam as winapi::DWORD) as winapi::c_int; // waiting on PR for winapi to add GET_XBUTTON_WPARAM
            send_event(window, MouseInput(Pressed, Other(xbutton as u8)));
            0
        },

        winapi::WM_XBUTTONUP => {
            use events::Event::MouseInput;
            use events::MouseButton::Other;
            use events::ElementState::Released;
            let xbutton = winapi::HIWORD(wparam as winapi::DWORD) as winapi::c_int; 
            send_event(window, MouseInput(Released, Other(xbutton as u8)));
            0
        },

        winapi::WM_INPUT => {
            let mut data: winapi::RAWINPUT = mem::uninitialized();
            let mut data_size = mem::size_of::<winapi::RAWINPUT>() as winapi::UINT;
            user32::GetRawInputData(mem::transmute(lparam), winapi::RID_INPUT,
                                    mem::transmute(&mut data), &mut data_size,
                                    mem::size_of::<winapi::RAWINPUTHEADER>() as winapi::UINT);

            if data.header.dwType == winapi::RIM_TYPEMOUSE {
                let _x = data.mouse.lLastX;  // FIXME: this is not always the relative movement
                let _y = data.mouse.lLastY;
                // TODO:
                //send_event(window, Event::MouseRawMovement { x: x, y: y });

                0

            } else {
                user32::DefWindowProcW(window, msg, wparam, lparam)
            }
        },

        winapi::WM_SETFOCUS => {
            use events::Event::Focused;
            send_event(window, Focused(true));
            0
        },

        winapi::WM_KILLFOCUS => {
            use events::Event::Focused;
            send_event(window, Focused(false));
            0
        },

        winapi::WM_SETCURSOR => {
            let call_def_window_proc = CONTEXT_STASH.with(|context_stash| {
                let cstash = context_stash.borrow();
                let mut call_def_window_proc = false;
                if let Some(cstash) = cstash.as_ref() {
                    if let Ok(window_state) = cstash.window_state.lock() {
                        if cstash.mouse_in_window {
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

                call_def_window_proc
            });

            if call_def_window_proc {
                user32::DefWindowProcW(window, msg, wparam, lparam)
            } else {    
                0
            }
        },

        winapi::WM_DROPFILES => {
            use events::Event::DroppedFile;

            let hdrop = wparam as winapi::HDROP;
            let mut pathbuf: [u16; winapi::MAX_PATH] = mem::uninitialized();
            let num_drops = shell32::DragQueryFileW(hdrop, 0xFFFFFFFF, ptr::null_mut(), 0);

            for i in 0..num_drops {
                let nch = shell32::DragQueryFileW(hdrop, i, pathbuf.as_mut_ptr(),
                                                  winapi::MAX_PATH as u32) as usize;
                if nch > 0 {
                    send_event(window, DroppedFile(OsString::from_wide(&pathbuf[0..nch]).into()));
                }
            }

            shell32::DragFinish(hdrop);
            0
        },

        winapi::WM_GETMINMAXINFO => {
            let mmi = lparam as *mut MinMaxInfo;
            //(*mmi).max_position = winapi::POINT { x: -8, y: -8 }; // The upper left corner of the window if it were maximized on the primary monitor.
            //(*mmi).max_size = winapi::POINT { x: .., y: .. }; // The dimensions of the primary monitor.

            CONTEXT_STASH.with(|context_stash| {
                match context_stash.borrow().as_ref() {
                    Some(cstash) => {
                        let window_state = cstash.window_state.lock().unwrap();

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
                    },
                    None => { }
                }
            });
            0
        },

        x if x == *super::WAKEUP_MSG_ID => {
            use events::Event::Awakened;
            send_event(window, Awakened);
            0
        },

        _ => {
            user32::DefWindowProcW(window, msg, wparam, lparam)
        }
    }
}
