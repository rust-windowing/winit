use std::rc::Rc;
use std::cell::RefCell;
use std::sync::mpsc::{Sender, Receiver, channel};

use Event;
use super::event;

use user32;
use winapi;

/// Stores the current window and its events dispatcher.
/// 
/// We only have one window per thread. We still store the HWND in case where we
///  receive an event for another window.
thread_local!(pub static WINDOW: Rc<RefCell<Option<(winapi::HWND, Sender<Event>)>>> = Rc::new(RefCell::new(None)));

/// Checks that the window is the good one, and if so send the event to it.
fn send_event(input_window: winapi::HWND, event: Event) {
    WINDOW.with(|window| {
        let window = window.borrow();
        let stored = match *window {
            None => return,
            Some(ref v) => v
        };

        let &(ref win, ref sender) = stored;

        if win != &input_window {
            return;
        }

        sender.send(event).ok();  // ignoring if closed
    });
}

/// This is the callback that is called by `DispatchMessage` in the events loop.
/// 
/// Returning 0 tells the Win32 API that the message has been processed.
pub extern "system" fn callback(window: winapi::HWND, msg: winapi::UINT,
    wparam: winapi::WPARAM, lparam: winapi::LPARAM) -> winapi::LRESULT
{
    match msg {
        winapi::WM_DESTROY => {
            use events::Event::Closed;

            WINDOW.with(|w| {
                let w = w.borrow();
                let &(ref win, _) = match *w {
                    None => return,
                    Some(ref v) => v
                };

                if win == &window {
                    unsafe { user32::PostQuitMessage(0); }
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
            let chr: char = unsafe { mem::transmute(wparam as u32) };
            send_event(window, ReceivedCharacter(chr));
            0
        },

        winapi::WM_MOUSEMOVE => {
            use events::Event::MouseMoved;

            let x = winapi::GET_X_LPARAM(lparam) as i32;
            let y = winapi::GET_Y_LPARAM(lparam) as i32;

            send_event(window, MouseMoved((x, y)));

            0
        },

        winapi::WM_MOUSEWHEEL => {
            use events::Event::MouseWheel;

            let value = (wparam >> 16) as i16;
            let value = value as i32;

            send_event(window, MouseWheel(value));

            0
        },

        winapi::WM_KEYDOWN => {
            use events::Event::KeyboardInput;
            use events::ElementState::Pressed;
            let scancode = ((lparam >> 16) & 0xff) as u8;
            let vkey = event::vkeycode_to_element(wparam);
            send_event(window, KeyboardInput(Pressed, scancode, vkey));
            0
        },

        winapi::WM_KEYUP => {
            use events::Event::KeyboardInput;
            use events::ElementState::Released;
            let scancode = ((lparam >> 16) & 0xff) as u8;
            let vkey = event::vkeycode_to_element(wparam);
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

        _ => unsafe {
            user32::DefWindowProcW(window, msg, wparam, lparam)
        }
    }
}
