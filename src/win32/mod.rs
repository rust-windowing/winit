use std::sync::atomics::AtomicBool;
use std::ptr;
use {Event, WindowBuilder};

pub use self::monitor::{MonitorID, get_available_monitors, get_primary_monitor};

mod event;
mod ffi;
mod init;
mod monitor;

/// The Win32 implementation of the main `Window` object.
pub struct Window {
    /// Main handle for the window.
    window: ffi::HWND,

    /// This represents a "draw context" for the surface of the window.
    hdc: ffi::HDC,

    /// OpenGL context.
    context: ffi::HGLRC,

    /// Binded to `opengl32.dll`.
    ///
    /// `wglGetProcAddress` returns null for GL 1.1 functions because they are
    ///  already defined by the system. This module contains them.
    gl_library: ffi::HMODULE,

    /// Receiver for the events dispatched by the window callback.
    events_receiver: Receiver<Event>,

    /// True if a `Closed` event has been received.
    is_closed: AtomicBool,
}

impl Window {
    /// See the docs in the crate root file.
    pub fn new(builder: WindowBuilder) -> Result<Window, String> {
        init::new_window(builder)
    }

    /// See the docs in the crate root file.
    pub fn is_closed(&self) -> bool {
        use std::sync::atomics::Relaxed;
        self.is_closed.load(Relaxed)
    }

    /// See the docs in the crate root file.
    /// 
    /// Calls SetWindowText on the HWND.
    pub fn set_title(&self, text: &str) {
        unsafe {
            ffi::SetWindowTextW(self.window,
                text.utf16_units().collect::<Vec<u16>>().append_one(0).as_ptr() as ffi::LPCWSTR);
        }
    }

    /// See the docs in the crate root file.
    pub fn get_position(&self) -> Option<(int, int)> {
        use std::mem;

        let mut placement: ffi::WINDOWPLACEMENT = unsafe { mem::zeroed() };
        placement.length = mem::size_of::<ffi::WINDOWPLACEMENT>() as ffi::UINT;

        if unsafe { ffi::GetWindowPlacement(self.window, &mut placement) } == 0 {
            return None
        }

        let ref rect = placement.rcNormalPosition;
        Some((rect.left as int, rect.top as int))
    }

    /// See the docs in the crate root file.
    pub fn set_position(&self, x: int, y: int) {
        use libc;

        unsafe {
            ffi::SetWindowPos(self.window, ptr::null(), x as libc::c_int, y as libc::c_int,
                0, 0, ffi::SWP_NOZORDER | ffi::SWP_NOSIZE);
            ffi::UpdateWindow(self.window);
        }
    }

    /// See the docs in the crate root file.
    pub fn get_inner_size(&self) -> Option<(uint, uint)> {
        use std::mem;
        let mut rect: ffi::RECT = unsafe { mem::uninitialized() };

        if unsafe { ffi::GetClientRect(self.window, &mut rect) } == 0 {
            return None
        }

        Some((
            (rect.right - rect.left) as uint,
            (rect.bottom - rect.top) as uint
        ))
    }

    /// See the docs in the crate root file.
    pub fn get_outer_size(&self) -> Option<(uint, uint)> {
        use std::mem;
        let mut rect: ffi::RECT = unsafe { mem::uninitialized() };

        if unsafe { ffi::GetWindowRect(self.window, &mut rect) } == 0 {
            return None
        }

        Some((
            (rect.right - rect.left) as uint,
            (rect.bottom - rect.top) as uint
        ))
    }

    /// See the docs in the crate root file.
    pub fn set_inner_size(&self, x: uint, y: uint) {
        use libc;

        unsafe {
            ffi::SetWindowPos(self.window, ptr::null(), 0, 0, x as libc::c_int,
                y as libc::c_int, ffi::SWP_NOZORDER | ffi::SWP_NOREPOSITION);
            ffi::UpdateWindow(self.window);
        }
    }

    /// See the docs in the crate root file.
    // TODO: return iterator
    pub fn poll_events(&self) -> Vec<Event> {
        let mut events = Vec::new();
        loop {
            match self.events_receiver.try_recv() {
                Ok(ev) => events.push(ev),
                Err(_) => break
            }
        }

        // if one of the received events is `Closed`, setting `is_closed` to true
        if events.iter().find(|e| match e { &&::Closed => true, _ => false }).is_some() {
            use std::sync::atomics::Relaxed;
            self.is_closed.store(true, Relaxed);
        }
        
        events
    }

    /// See the docs in the crate root file.
    // TODO: return iterator
    pub fn wait_events(&self) -> Vec<Event> {
        match self.events_receiver.recv_opt() {
            Ok(ev) => {
                // if the received event is `Closed`, setting `is_closed` to true
                match ev {
                    ::Closed => {
                        use std::sync::atomics::Relaxed;
                        self.is_closed.store(true, Relaxed);
                    },
                    _ => ()
                };

                // looing for other possible events in the queue
                let mut result = self.poll_events();
                result.insert(0, ev);
                result
            },

            Err(_) => {
                use std::sync::atomics::Relaxed;
                self.is_closed.store(true, Relaxed);
                vec![]
            }
        }
    }

    /// See the docs in the crate root file.
    pub unsafe fn make_current(&self) {
        // TODO: check return value
        ffi::wgl::MakeCurrent(self.hdc, self.context);
    }

    /// See the docs in the crate root file.
    pub fn get_proc_address(&self, addr: &str) -> *const () {
        use std::c_str::ToCStr;

        unsafe {
            addr.with_c_str(|s| {
                let p = ffi::wgl::GetProcAddress(s) as *const ();
                if !p.is_null() { return p; }
                ffi::GetProcAddress(self.gl_library, s) as *const ()
            })
        }
    }

    /// See the docs in the crate root file.
    pub fn swap_buffers(&self) {
        unsafe {
            // calling glFlush is necessary on Windows 8
            ffi::glFlush();

            ffi::SwapBuffers(self.hdc);
        }
    }
}

#[unsafe_destructor]
impl Drop for Window {
    fn drop(&mut self) {
        use std::ptr;
        unsafe { ffi::wgl::MakeCurrent(ptr::null(), ptr::null()); }
        unsafe { ffi::wgl::DeleteContext(self.context); }
        unsafe { ffi::DestroyWindow(self.window); }
    }
}
