use std::sync::atomic::AtomicBool;
use std::mem;
use std::ptr;
use std::ffi::CString;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::sync::{
    Arc,
    Mutex
};
use std::sync::mpsc::Receiver;
use libc;
use {CreationError, Event, MouseCursor};
use CursorState;

use PixelFormat;
use BuilderAttribs;

pub use self::headless::HeadlessContext;
pub use self::monitor::{MonitorID, get_available_monitors, get_primary_monitor};

use winapi;
use user32;
use kernel32;
use gdi32;

mod callback;
mod event;
mod gl;
mod headless;
mod init;
mod make_current_guard;
mod monitor;

/// The Win32 implementation of the main `Window` object.
pub struct Window {
    /// Main handle for the window.
    window: WindowWrapper,

    /// OpenGL context.
    context: ContextWrapper,

    /// Binded to `opengl32.dll`.
    ///
    /// `wglGetProcAddress` returns null for GL 1.1 functions because they are
    ///  already defined by the system. This module contains them.
    gl_library: winapi::HMODULE,

    /// Receiver for the events dispatched by the window callback.
    events_receiver: Receiver<Event>,

    /// True if a `Closed` event has been received.
    is_closed: AtomicBool,

    /// The current cursor state.
    cursor_state: Arc<Mutex<CursorState>>,

    /// The pixel format that has been used to create this window.
    pixel_format: PixelFormat,
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

/// A simple wrapper that destroys the context when it is destroyed.
// FIXME: remove `pub` (https://github.com/rust-lang/rust/issues/23585)
#[doc(hidden)]
pub struct ContextWrapper(pub winapi::HGLRC);

impl Drop for ContextWrapper {
    fn drop(&mut self) {
        unsafe {
            gl::wgl::DeleteContext(self.0 as *const libc::c_void);
        }
    }
}

/// A simple wrapper that destroys the window when it is destroyed.
// FIXME: remove `pub` (https://github.com/rust-lang/rust/issues/23585)
#[doc(hidden)]
pub struct WindowWrapper(pub winapi::HWND, pub winapi::HDC);

impl Drop for WindowWrapper {
    fn drop(&mut self) {
        unsafe {
            user32::DestroyWindow(self.0);
        }
    }
}

#[derive(Clone)]
pub struct WindowProxy;

impl WindowProxy {
    pub fn wakeup_event_loop(&self) {
        unimplemented!()
    }
}

impl Window {
    /// See the docs in the crate root file.
    pub fn new(builder: BuilderAttribs) -> Result<Window, CreationError> {
        let (builder, sharing) = builder.extract_non_static();
        let sharing = sharing.map(|w| init::ContextHack(w.context.0));
        init::new_window(builder, sharing)
    }

    /// See the docs in the crate root file.
    pub fn is_closed(&self) -> bool {
        use std::sync::atomic::Ordering::Relaxed;
        self.is_closed.load(Relaxed)
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

    pub fn show(&self) {
        unsafe {
            user32::ShowWindow(self.window.0, winapi::SW_SHOW);
        }
    }

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
        use libc;

        unsafe {
            user32::SetWindowPos(self.window.0, ptr::null_mut(), x as libc::c_int, y as libc::c_int,
                0, 0, winapi::SWP_NOZORDER | winapi::SWP_NOSIZE);
            user32::UpdateWindow(self.window.0);
        }
    }

    /// See the docs in the crate root file.
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
        use libc;

        unsafe {
            user32::SetWindowPos(self.window.0, ptr::null_mut(), 0, 0, x as libc::c_int,
                y as libc::c_int, winapi::SWP_NOZORDER | winapi::SWP_NOREPOSITION);
            user32::UpdateWindow(self.window.0);
        }
    }

    pub fn create_window_proxy(&self) -> WindowProxy {
        WindowProxy
    }

    /// See the docs in the crate root file.
    pub fn poll_events(&self) -> PollEventsIterator {
        PollEventsIterator {
            window: self,
        }
    }

    /// See the docs in the crate root file.
    pub fn wait_events(&self) -> WaitEventsIterator {
        WaitEventsIterator {
            window: self,
        }
    }

    /// See the docs in the crate root file.
    pub unsafe fn make_current(&self) {
        // TODO: check return value
        gl::wgl::MakeCurrent(self.window.1 as *const libc::c_void,
                             self.context.0 as *const libc::c_void);
    }

    /// See the docs in the crate root file.
    pub fn is_current(&self) -> bool {
        unsafe { gl::wgl::GetCurrentContext() == self.context.0 as *const libc::c_void }
    }

    /// See the docs in the crate root file.
    pub fn get_proc_address(&self, addr: &str) -> *const () {
        let addr = CString::new(addr.as_bytes()).unwrap();
        let addr = addr.as_ptr();

        unsafe {
            let p = gl::wgl::GetProcAddress(addr) as *const ();
            if !p.is_null() { return p; }
            kernel32::GetProcAddress(self.gl_library, addr) as *const ()
        }
    }

    /// See the docs in the crate root file.
    pub fn swap_buffers(&self) {
        unsafe {
            gdi32::SwapBuffers(self.window.1);
        }
    }

    pub fn platform_display(&self) -> *mut libc::c_void {
        unimplemented!()
    }

    pub fn platform_window(&self) -> *mut libc::c_void {
        self.window.0 as *mut libc::c_void
    }

    /// See the docs in the crate root file.
    pub fn get_api(&self) -> ::Api {
        ::Api::OpenGl
    }

    pub fn get_pixel_format(&self) -> PixelFormat {
        self.pixel_format.clone()
    }

    pub fn set_window_resize_callback(&mut self, _: Option<fn(u32, u32)>) {
    }

    pub fn set_cursor(&self, _cursor: MouseCursor) {
        unimplemented!()
    }

    pub fn set_cursor_state(&self, state: CursorState) -> Result<(), String> {
        let mut current_state = self.cursor_state.lock().unwrap();

        let foreground_thread_id = unsafe { user32::GetWindowThreadProcessId(self.window.0, ptr::null_mut()) };
        let current_thread_id = unsafe { kernel32::GetCurrentThreadId() };

        unsafe { user32::AttachThreadInput(foreground_thread_id, current_thread_id, 1) };

        let res = match (state, *current_state) {
            (CursorState::Normal, CursorState::Normal) => Ok(()),
            (CursorState::Hide, CursorState::Hide) => Ok(()),
            (CursorState::Grab, CursorState::Grab) => Ok(()),

            (CursorState::Hide, CursorState::Normal) => {
                unsafe {
                    user32::SetCursor(ptr::null_mut());
                    *current_state = CursorState::Hide;
                    Ok(())
                }
            },

            (CursorState::Normal, CursorState::Hide) => {
                unsafe {
                    user32::SetCursor(user32::LoadCursorW(ptr::null_mut(), winapi::IDC_ARROW));
                    *current_state = CursorState::Normal;
                    Ok(())
                }
            },

            (CursorState::Grab, CursorState::Normal) => {
                unsafe {
                    user32::SetCursor(ptr::null_mut());
                    let mut rect = mem::uninitialized();
                    if user32::GetClientRect(self.window.0, &mut rect) == 0 {
                        return Err(format!("GetWindowRect failed"));
                    }
                    user32::ClientToScreen(self.window.0, mem::transmute(&mut rect.left));
                    user32::ClientToScreen(self.window.0, mem::transmute(&mut rect.right));
                    if user32::ClipCursor(&rect) == 0 {
                        return Err(format!("ClipCursor failed"));
                    }
                    *current_state = CursorState::Grab;
                    Ok(())
                }
            },

            (CursorState::Normal, CursorState::Grab) => {
                unsafe {
                    user32::SetCursor(user32::LoadCursorW(ptr::null_mut(), winapi::IDC_ARROW));
                    if user32::ClipCursor(ptr::null()) == 0 {
                        return Err(format!("ClipCursor failed"));
                    }
                    *current_state = CursorState::Normal;
                    Ok(())
                }
            },

            _ => unimplemented!(),
        };

        unsafe { user32::AttachThreadInput(foreground_thread_id, current_thread_id, 0) };

        res
    }

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

pub struct PollEventsIterator<'a> {
    window: &'a Window,
}

impl<'a> Iterator for PollEventsIterator<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        use events::Event::Closed;

        match self.window.events_receiver.try_recv() {
            Ok(Closed) => {
                use std::sync::atomic::Ordering::Relaxed;
                self.window.is_closed.store(true, Relaxed);
                Some(Closed)
            },
            Ok(ev) => Some(ev),
            Err(_) => None
        }
    }
}

pub struct WaitEventsIterator<'a> {
    window: &'a Window,
}

impl<'a> Iterator for WaitEventsIterator<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        use events::Event::Closed;

        match self.window.events_receiver.recv() {
            Ok(Closed) => {
                use std::sync::atomic::Ordering::Relaxed;
                self.window.is_closed.store(true, Relaxed);
                Some(Closed)
            },
            Ok(ev) => Some(ev),
            Err(_) => None
        }
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            // we don't call MakeCurrent(0, 0) because we are not sure that the context
            // is still the current one
            user32::PostMessageW(self.window.0, winapi::WM_DESTROY, 0, 0);
        }
    }
}
