use std::sync::atomic::AtomicBool;
use std::ptr;
use std::ffi::CString;
use std::collections::RingBuf;
use std::sync::mpsc::Receiver;
use libc;
use {CreationError, Event, MouseCursor};

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
mod monitor;

/// The Win32 implementation of the main `Window` object.
pub struct Window {
    /// Main handle for the window.
    window: winapi::HWND,

    /// This represents a "draw context" for the surface of the window.
    hdc: winapi::HDC,

    /// OpenGL context.
    context: winapi::HGLRC,

    /// Binded to `opengl32.dll`.
    ///
    /// `wglGetProcAddress` returns null for GL 1.1 functions because they are
    ///  already defined by the system. This module contains them.
    gl_library: winapi::HMODULE,

    /// Receiver for the events dispatched by the window callback.
    events_receiver: Receiver<Event>,

    /// True if a `Closed` event has been received.
    is_closed: AtomicBool,
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

impl Window {
    /// See the docs in the crate root file.
    pub fn new(builder: BuilderAttribs) -> Result<Window, CreationError> {
        let (builder, sharing) = builder.extract_non_static();
        let sharing = sharing.map(|w| init::ContextHack(w.context));
        init::new_window(builder, sharing)
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
    pub fn is_closed(&self) -> bool {
        use std::sync::atomic::Ordering::Relaxed;
        self.is_closed.load(Relaxed)
    }

    /// See the docs in the crate root file.
    ///
    /// Calls SetWindowText on the HWND.
    pub fn set_title(&self, text: &str) {
        unsafe {
            user32::SetWindowTextW(self.window,
                text.utf16_units().chain(Some(0).into_iter())
                .collect::<Vec<u16>>().as_ptr() as winapi::LPCWSTR);
        }
    }

    pub fn show(&self) {
        unsafe {
            user32::ShowWindow(self.window, winapi::SW_SHOW);
        }
    }

    pub fn hide(&self) {
        unsafe {
            user32::ShowWindow(self.window, winapi::SW_HIDE);
        }
    }

    /// See the docs in the crate root file.
    pub fn get_position(&self) -> Option<(i32, i32)> {
        use std::mem;

        let mut placement: winapi::WINDOWPLACEMENT = unsafe { mem::zeroed() };
        placement.length = mem::size_of::<winapi::WINDOWPLACEMENT>() as winapi::UINT;

        if unsafe { user32::GetWindowPlacement(self.window, &mut placement) } == 0 {
            return None
        }

        let ref rect = placement.rcNormalPosition;
        Some((rect.left as i32, rect.top as i32))
    }

    /// See the docs in the crate root file.
    pub fn set_position(&self, x: i32, y: i32) {
        use libc;

        unsafe {
            user32::SetWindowPos(self.window, ptr::null_mut(), x as libc::c_int, y as libc::c_int,
                0, 0, winapi::SWP_NOZORDER | winapi::SWP_NOSIZE);
            user32::UpdateWindow(self.window);
        }
    }

    /// See the docs in the crate root file.
    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        use std::mem;
        let mut rect: winapi::RECT = unsafe { mem::uninitialized() };

        if unsafe { user32::GetClientRect(self.window, &mut rect) } == 0 {
            return None
        }

        Some((
            (rect.right - rect.left) as u32,
            (rect.bottom - rect.top) as u32
        ))
    }

    /// See the docs in the crate root file.
    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        use std::mem;
        let mut rect: winapi::RECT = unsafe { mem::uninitialized() };

        if unsafe { user32::GetWindowRect(self.window, &mut rect) } == 0 {
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
            user32::SetWindowPos(self.window, ptr::null_mut(), 0, 0, x as libc::c_int,
                y as libc::c_int, winapi::SWP_NOZORDER | winapi::SWP_NOREPOSITION);
            user32::UpdateWindow(self.window);
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
        gl::wgl::MakeCurrent(self.hdc as *const libc::c_void, self.context as *const libc::c_void);
    }

    /// See the docs in the crate root file.
    pub fn get_proc_address(&self, addr: &str) -> *const () {
        let addr = CString::from_slice(addr.as_bytes());
        let addr = addr.as_slice_with_nul().as_ptr();

        unsafe {
            let p = gl::wgl::GetProcAddress(addr) as *const ();
            if !p.is_null() { return p; }
            kernel32::GetProcAddress(self.gl_library, addr) as *const ()
        }
    }

    /// See the docs in the crate root file.
    pub fn swap_buffers(&self) {
        unsafe {
            gdi32::SwapBuffers(self.hdc);
        }
    }

    pub fn platform_display(&self) -> *mut libc::c_void {
        unimplemented!()
    }

    pub fn platform_window(&self) -> *mut libc::c_void {
        self.window as *mut libc::c_void
    }

    /// See the docs in the crate root file.
    pub fn get_api(&self) -> ::Api {
        ::Api::OpenGl
    }

    pub fn set_window_resize_callback(&mut self, _: Option<fn(u32, u32)>) {
    }

    pub fn set_cursor(&self, cursor: MouseCursor) {
        unimplemented!()
    }

    pub fn hidpi_factor(&self) -> f32 {
        1.0
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

#[unsafe_destructor]
impl Drop for Window {
    fn drop(&mut self) {
        use std::ptr;
        // we don't call MakeCurrent(0, 0) because we are not sure that the context
        // is still the current one
        unsafe { user32::PostMessageW(self.window, winapi::WM_DESTROY, 0, 0); }
        unsafe { gl::wgl::DeleteContext(self.context as *const libc::c_void); }
        unsafe { user32::DestroyWindow(self.window); }
    }
}
