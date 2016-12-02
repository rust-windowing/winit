#![cfg(target_os = "windows")]

use std::mem;
use std::ptr;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::os::raw::c_int;
use std::sync::{
    Arc,
    Mutex
};
use std::sync::mpsc::Receiver;
use {CreationError, Event, MouseCursor};
use CursorState;

use WindowAttributes;

#[derive(Clone, Default)]
pub struct PlatformSpecificWindowBuilderAttributes {
    pub parent: Option<winapi::HWND>,
}

unsafe impl Send for PlatformSpecificWindowBuilderAttributes {}
unsafe impl Sync for PlatformSpecificWindowBuilderAttributes {}

#[derive(Clone, Default)]
pub struct PlatformSpecificHeadlessBuilderAttributes;

pub use self::monitor::{MonitorId, get_available_monitors, get_primary_monitor};

use winapi;
use user32;
use kernel32;

mod callback;
mod event;
mod init;
mod monitor;

lazy_static! {
    static ref WAKEUP_MSG_ID: u32 = unsafe { user32::RegisterWindowMessageA("Glutin::EventID".as_ptr() as *const i8) };
}

/// Cursor
pub type Cursor = *const winapi::wchar_t;

/// Contains information about states and the window for the callback.
#[derive(Clone)]
pub struct WindowState {
    pub cursor: Cursor,
    pub cursor_state: CursorState,
    pub attributes: WindowAttributes
}

/// The Win32 implementation of the main `Window` object.
pub struct Window {
    /// Main handle for the window.
    window: WindowWrapper,

    /// Receiver for the events dispatched by the window callback.
    events_receiver: Receiver<Event>,

    /// The current window state.
    window_state: Arc<Mutex<WindowState>>,
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

/// A simple wrapper that destroys the window when it is destroyed.
#[doc(hidden)]
pub struct WindowWrapper(winapi::HWND, winapi::HDC);

impl Drop for WindowWrapper {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            user32::DestroyWindow(self.0);
        }
    }
}

#[derive(Clone)]
pub struct WindowProxy {
    hwnd: winapi::HWND,
}

unsafe impl Send for WindowProxy {}
unsafe impl Sync for WindowProxy {}

impl WindowProxy {
    #[inline]
    pub fn wakeup_event_loop(&self) {
        unsafe {
            user32::PostMessageA(self.hwnd, *WAKEUP_MSG_ID, 0, 0);
        }
    }
}

impl Window {
    /// See the docs in the crate root file.
    pub fn new(window: &WindowAttributes, pl_attribs: &PlatformSpecificWindowBuilderAttributes)
               -> Result<Window, CreationError>
    {
        init::new_window(window, pl_attribs)
    }

    /// See the docs in the crate root file.
    ///
    /// Calls SetWindowText on the HWND.
    pub fn set_title(&self, text: &str) {
        let text = OsStr::new(text).encode_wide().chain(Some(0).into_iter())
                                   .collect::<Vec<_>>();

        unsafe {
            user32::SetWindowTextW(self.window.0, text.as_ptr() as winapi::LPCWSTR);
        }
    }

    #[inline]
    pub fn show(&self) {
        unsafe {
            user32::ShowWindow(self.window.0, winapi::SW_SHOW);
        }
    }

    #[inline]
    pub fn hide(&self) {
        unsafe {
            user32::ShowWindow(self.window.0, winapi::SW_HIDE);
        }
    }

    /// See the docs in the crate root file.
    pub fn get_position(&self) -> Option<(i32, i32)> {
        use std::mem;

        let mut placement: winapi::WINDOWPLACEMENT = unsafe { mem::zeroed() };
        placement.length = mem::size_of::<winapi::WINDOWPLACEMENT>() as winapi::UINT;

        if unsafe { user32::GetWindowPlacement(self.window.0, &mut placement) } == 0 {
            return None
        }

        let ref rect = placement.rcNormalPosition;
        Some((rect.left as i32, rect.top as i32))
    }

    /// See the docs in the crate root file.
    pub fn set_position(&self, x: i32, y: i32) {
        unsafe {
            user32::SetWindowPos(self.window.0, ptr::null_mut(), x as c_int, y as c_int,
                                 0, 0, winapi::SWP_NOZORDER | winapi::SWP_NOSIZE);
            user32::UpdateWindow(self.window.0);
        }
    }

    /// See the docs in the crate root file.
    #[inline]
    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        let mut rect: winapi::RECT = unsafe { mem::uninitialized() };

        if unsafe { user32::GetClientRect(self.window.0, &mut rect) } == 0 {
            return None
        }

        Some((
            (rect.right - rect.left) as u32,
            (rect.bottom - rect.top) as u32
        ))
    }

    /// See the docs in the crate root file.
    #[inline]
    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        let mut rect: winapi::RECT = unsafe { mem::uninitialized() };

        if unsafe { user32::GetWindowRect(self.window.0, &mut rect) } == 0 {
            return None
        }

        Some((
            (rect.right - rect.left) as u32,
            (rect.bottom - rect.top) as u32
        ))
    }

    /// See the docs in the crate root file.
    pub fn set_inner_size(&self, x: u32, y: u32) {
        unsafe {
            // Calculate the outer size based upon the specified inner size
            let mut rect = winapi::RECT { top: 0, left: 0, bottom: y as winapi::LONG, right: x as winapi::LONG };
            let dw_style = user32::GetWindowLongA(self.window.0, winapi::GWL_STYLE) as winapi::DWORD;
            let b_menu = !user32::GetMenu(self.window.0).is_null() as winapi::BOOL;
            let dw_style_ex = user32::GetWindowLongA(self.window.0, winapi::GWL_EXSTYLE) as winapi::DWORD;
            user32::AdjustWindowRectEx(&mut rect, dw_style, b_menu, dw_style_ex);
            let outer_x = (rect.right - rect.left).abs() as c_int;
            let outer_y = (rect.top - rect.bottom).abs() as c_int;

            user32::SetWindowPos(self.window.0, ptr::null_mut(), 0, 0, outer_x, outer_y,
                winapi::SWP_NOZORDER | winapi::SWP_NOREPOSITION | winapi::SWP_NOMOVE);
            user32::UpdateWindow(self.window.0);
        }
    }

    #[inline]
    pub fn create_window_proxy(&self) -> WindowProxy {
        WindowProxy { hwnd: self.window.0 }
    }

    /// See the docs in the crate root file.
    #[inline]
    pub fn poll_events(&self) -> PollEventsIterator {
        PollEventsIterator {
            window: self,
        }
    }

    /// See the docs in the crate root file.
    #[inline]
    pub fn wait_events(&self) -> WaitEventsIterator {
        WaitEventsIterator {
            window: self,
        }
    }

    #[inline]
    pub fn platform_display(&self) -> *mut ::libc::c_void {
        // What should this return on win32?
        // It could be GetDC(NULL), but that requires a ReleaseDC()
        // to avoid leaking the DC.
        ptr::null_mut()
    }

    #[inline]
    pub fn platform_window(&self) -> *mut ::libc::c_void {
        self.window.0 as *mut ::libc::c_void
    }

    #[inline]
    pub fn set_window_resize_callback(&mut self, _: Option<fn(u32, u32)>) {
    }

    #[inline]
    pub fn set_cursor(&self, _cursor: MouseCursor) {
        let cursor_id = match _cursor {
            MouseCursor::Arrow | MouseCursor::Default => winapi::IDC_ARROW,
            MouseCursor::Hand => winapi::IDC_HAND,
            MouseCursor::Crosshair => winapi::IDC_CROSS,
            MouseCursor::Text | MouseCursor::VerticalText => winapi::IDC_IBEAM,
            MouseCursor::NotAllowed | MouseCursor::NoDrop => winapi::IDC_NO,
            MouseCursor::EResize => winapi::IDC_SIZEWE,
            MouseCursor::NResize => winapi::IDC_SIZENS,
            MouseCursor::WResize => winapi::IDC_SIZEWE,
            MouseCursor::SResize => winapi::IDC_SIZENS,
            MouseCursor::EwResize | MouseCursor::ColResize => winapi::IDC_SIZEWE,
            MouseCursor::NsResize | MouseCursor::RowResize => winapi::IDC_SIZENS,
            MouseCursor::Wait | MouseCursor::Progress => winapi::IDC_WAIT,
            MouseCursor::Help => winapi::IDC_HELP,
            _ => winapi::IDC_ARROW, // use arrow for the missing cases.
        };

        let mut cur = self.window_state.lock().unwrap();
        cur.cursor = cursor_id;
    }


    pub fn set_cursor_state(&self, state: CursorState) -> Result<(), String> {
        let mut current_state = self.window_state.lock().unwrap();

        let foreground_thread_id = unsafe { user32::GetWindowThreadProcessId(self.window.0, ptr::null_mut()) };
        let current_thread_id = unsafe { kernel32::GetCurrentThreadId() };

        unsafe { user32::AttachThreadInput(foreground_thread_id, current_thread_id, 1) };

        let res = match (state, current_state.cursor_state) {
            (CursorState::Normal, CursorState::Normal) => Ok(()),
            (CursorState::Hide, CursorState::Hide) => Ok(()),
            (CursorState::Grab, CursorState::Grab) => Ok(()),

            (CursorState::Hide, CursorState::Normal) => {
                current_state.cursor_state = CursorState::Hide;
                Ok(())
            },

            (CursorState::Normal, CursorState::Hide) => {
                current_state.cursor_state = CursorState::Normal;
                Ok(())
            },

            (CursorState::Grab, CursorState::Normal) | (CursorState::Grab, CursorState::Hide) => {
                unsafe {
                    let mut rect = mem::uninitialized();
                    if user32::GetClientRect(self.window.0, &mut rect) == 0 {
                        return Err(format!("GetWindowRect failed"));
                    }
                    user32::ClientToScreen(self.window.0, mem::transmute(&mut rect.left));
                    user32::ClientToScreen(self.window.0, mem::transmute(&mut rect.right));
                    if user32::ClipCursor(&rect) == 0 {
                        return Err(format!("ClipCursor failed"));
                    }
                    current_state.cursor_state = CursorState::Grab;
                    Ok(())
                }
            },

            (CursorState::Normal, CursorState::Grab) => {
                unsafe {
                    if user32::ClipCursor(ptr::null()) == 0 {
                        return Err(format!("ClipCursor failed"));
                    }
                    current_state.cursor_state = CursorState::Normal;
                    Ok(())
                }
            },

            _ => unimplemented!(),
        };

        unsafe { user32::AttachThreadInput(foreground_thread_id, current_thread_id, 0) };

        res
    }

    #[inline]
    pub fn hidpi_factor(&self) -> f32 {
        1.0
    }

    pub fn set_cursor_position(&self, x: i32, y: i32) -> Result<(), ()> {
        let mut point = winapi::POINT {
            x: x,
            y: y,
        };

        unsafe {
            if user32::ClientToScreen(self.window.0, &mut point) == 0 {
                return Err(());
            }

            if user32::SetCursorPos(point.x, point.y) == 0 {
                return Err(());
            }
        }

        Ok(())
    }
}

impl Drop for Window {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            user32::PostMessageW(self.window.0, winapi::WM_DESTROY, 0, 0);
        }
    }
}

pub struct PollEventsIterator<'a> {
    window: &'a Window,
}

impl<'a> Iterator for PollEventsIterator<'a> {
    type Item = Event;

    #[inline]
    fn next(&mut self) -> Option<Event> {
        self.window.events_receiver.try_recv().ok()
    }
}

pub struct WaitEventsIterator<'a> {
    window: &'a Window,
}

impl<'a> Iterator for WaitEventsIterator<'a> {
    type Item = Event;

    #[inline]
    fn next(&mut self) -> Option<Event> {
        self.window.events_receiver.recv().ok()
    }
}
