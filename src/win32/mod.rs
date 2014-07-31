use std::kinds::marker::NoSend;
use std::sync::atomics::AtomicBool;
use std::ptr;
use {Event, Hints};

pub use self::monitor::{MonitorID, get_available_monitors, get_primary_monitor};
pub use self::init::WINDOWS_LIST;

mod event;
mod ffi;
mod init;
mod monitor;

pub struct Window {
    window: ffi::HWND,
    hdc: ffi::HDC,
    context: ffi::HGLRC,
    gl_library: ffi::HMODULE,
    events_receiver: Receiver<Event>,
    is_closed: AtomicBool,
    nosend: NoSend,
}

impl Window {
    pub fn new(dimensions: Option<(uint, uint)>, title: &str,
        hints: &Hints, monitor: Option<MonitorID>)
        -> Result<Window, String>
    {
        init::new_window(dimensions, title, hints, monitor)
    }

    pub fn is_closed(&self) -> bool {
        use std::sync::atomics::Relaxed;
        self.is_closed.load(Relaxed)
    }

    /// Calls SetWindowText on the HWND.
    pub fn set_title(&self, text: &str) {
        unsafe {
            ffi::SetWindowTextW(self.window,
                text.utf16_units().collect::<Vec<u16>>().append_one(0).as_ptr() as ffi::LPCWSTR);
        }
    }

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

    pub fn set_position(&self, x: uint, y: uint) {
        use libc;

        unsafe {
            ffi::SetWindowPos(self.window, ptr::mut_null(), x as libc::c_int, y as libc::c_int,
                0, 0, ffi::SWP_NOZORDER | ffi::SWP_NOSIZE);
            ffi::UpdateWindow(self.window);
        }
    }

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

    pub fn set_inner_size(&self, x: uint, y: uint) {
        use libc;

        unsafe {
            ffi::SetWindowPos(self.window, ptr::mut_null(), 0, 0, x as libc::c_int,
                y as libc::c_int, ffi::SWP_NOZORDER | ffi::SWP_NOREPOSITION);
            ffi::UpdateWindow(self.window);
        }
    }

    // TODO: return iterator
    pub fn poll_events(&self) -> Vec<Event> {
        use std::mem;

        loop {
            let mut msg = unsafe { mem::uninitialized() };

            if unsafe { ffi::PeekMessageW(&mut msg, ptr::mut_null(), 0, 0, 0x1) } == 0 {
                break
            }

            unsafe { ffi::TranslateMessage(&msg) };
            unsafe { ffi::DispatchMessageW(&msg) };
        }

        let mut events = Vec::new();
        loop {
            match self.events_receiver.try_recv() {
                Ok(ev) => events.push(ev),
                Err(_) => break
            }
        }

        if events.iter().find(|e| match e { &&::Closed => true, _ => false }).is_some() {
            use std::sync::atomics::Relaxed;
            self.is_closed.store(true, Relaxed);
        }
        
        events
    }

    // TODO: return iterator
    pub fn wait_events(&self) -> Vec<Event> {
        loop {
            unsafe { ffi::WaitMessage() };

            let events = self.poll_events();
            if events.len() >= 1 {
                return events
            }
        }
    }

    pub unsafe fn make_current(&self) {
        ffi::wglMakeCurrent(self.hdc, self.context)
    }

    pub fn get_proc_address(&self, addr: &str) -> *const () {
        use std::c_str::ToCStr;

        unsafe {
            addr.with_c_str(|s| {
                let p = ffi::wglGetProcAddress(s) as *const ();
                if !p.is_null() { return p; }
                ffi::GetProcAddress(self.gl_library, s) as *const ()
            })
        }
    }

    pub fn swap_buffers(&self) {
        unsafe {
            ffi::SwapBuffers(self.hdc);
        }
    }
}

#[unsafe_destructor]
impl Drop for Window {
    fn drop(&mut self) {
        unsafe { ffi::DestroyWindow(self.window); }
    }
}
