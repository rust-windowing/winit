use std::kinds::marker::NoSend;
use std::sync::Mutex;
use std::sync::atomics::AtomicBool;
use std::ptr;
use {Event, Hints, MonitorID};

mod event;
mod ffi;

pub struct Window {
    window: ffi::HWND,
    hdc: ffi::HDC,
    context: ffi::HGLRC,
    gl_library: ffi::HMODULE,
    events_receiver: Receiver<Event>,
    should_close: AtomicBool,
    nosend: NoSend,
}

/// Stores the list of all the windows.
/// Only available on callback thread.
local_data_key!(pub WINDOWS_LIST: Mutex<Vec<(ffi::HWND, Sender<Event>)>>)

impl Window {
    pub fn new(dimensions: Option<(uint, uint)>, title: &str,
        _hints: &Hints, monitor: Option<MonitorID>)
        -> Result<Window, String>
    {
        use std::mem;
        use std::os;

        // initializing WINDOWS_LIST if needed
        // this is safe because WINDOWS_LIST is task-local
        if WINDOWS_LIST.get().is_none() {
            WINDOWS_LIST.replace(Some(Mutex::new(Vec::new())));
        }

        // registering the window class
        let class_name: Vec<u16> = "Window Class".utf16_units().collect::<Vec<u16>>()
            .append_one(0);
        
        let class = ffi::WNDCLASSEX {
            cbSize: mem::size_of::<ffi::WNDCLASSEX>() as ffi::UINT,
            style: ffi::CS_HREDRAW | ffi::CS_VREDRAW,
            lpfnWndProc: callback,
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: unsafe { ffi::GetModuleHandleW(ptr::null()) },
            hIcon: ptr::mut_null(),
            hCursor: ptr::mut_null(),
            hbrBackground: ptr::mut_null(),
            lpszMenuName: ptr::null(),
            lpszClassName: class_name.as_ptr(),
            hIconSm: ptr::mut_null(),
        };

        if unsafe { ffi::RegisterClassExW(&class) } == 0 {
            use std::os;
            return Err(format!("RegisterClassEx function failed: {}",
                os::error_string(os::errno() as uint)))
        }

        // building a RECT object with coordinates
        let mut rect = ffi::RECT {
            left: 0, right: dimensions.map(|(w, _)| w as ffi::LONG).unwrap_or(1024),
            top: 0, bottom: dimensions.map(|(_, h)| h as ffi::LONG).unwrap_or(768),
        };

        // switching to fullscreen
        if monitor.is_some() {
            let mut screen_settings: ffi::DEVMODE = unsafe { mem::zeroed() };
            screen_settings.dmSize = mem::size_of::<ffi::DEVMODE>() as ffi::WORD;
            screen_settings.dmPelsWidth = 1024;
            screen_settings.dmPelsHeight = 768;
            screen_settings.dmBitsPerPel = 32;
            screen_settings.dmFields = ffi::DM_BITSPERPEL | ffi::DM_PELSWIDTH | ffi::DM_PELSHEIGHT;

            let result = unsafe { ffi::ChangeDisplaySettingsW(&mut screen_settings, ffi::CDS_FULLSCREEN) };
            if result != ffi::DISP_CHANGE_SUCCESSFUL {
                return Err(format!("ChangeDisplaySettings failed: {}", result))
            }
        }

        // computing the style and extended style
        let (ex_style, style) = if monitor.is_some() {
            (ffi::WS_EX_APPWINDOW, ffi::WS_POPUP | ffi::WS_CLIPSIBLINGS | ffi::WS_CLIPCHILDREN)
        } else {
            (ffi::WS_EX_APPWINDOW | ffi::WS_EX_WINDOWEDGE,
                ffi::WS_OVERLAPPEDWINDOW | ffi::WS_CLIPSIBLINGS | ffi::WS_CLIPCHILDREN)
        };

        // adjusting
        unsafe { ffi::AdjustWindowRectEx(&mut rect, style, 0, ex_style) };

        // creating the window
        let handle = unsafe {
            let handle = ffi::CreateWindowExW(ex_style, class_name.as_ptr(),
                title.utf16_units().collect::<Vec<u16>>().append_one(0).as_ptr() as ffi::LPCWSTR,
                style | ffi::WS_VISIBLE | ffi::WS_CLIPSIBLINGS | ffi::WS_CLIPCHILDREN,
                if monitor.is_some() { 0 } else { ffi::CW_USEDEFAULT},
                if monitor.is_some() { 0 } else { ffi::CW_USEDEFAULT},
                rect.right - rect.left, rect.bottom - rect.top,
                ptr::mut_null(), ptr::mut_null(), ffi::GetModuleHandleW(ptr::null()),
                ptr::mut_null());

            if handle.is_null() {
                use std::os;
                return Err(format!("CreateWindowEx function failed: {}",
                    os::error_string(os::errno() as uint)))
            }

            handle
        };

        // calling SetForegroundWindow if fullscreen
        if monitor.is_some() {
            unsafe { ffi::SetForegroundWindow(handle) };
        }

        // adding it to WINDOWS_LIST
        let events_receiver = {
            let (tx, rx) = channel();
            let list = WINDOWS_LIST.get().unwrap();
            let mut list = list.lock();
            list.push((handle, tx));
            rx
        };

        // Getting the HDC of the window
        let hdc = {
            let hdc = unsafe { ffi::GetDC(handle) };
            if hdc.is_null() {
                return Err(format!("GetDC function failed: {}",
                    os::error_string(os::errno() as uint)))
            }
            hdc
        };

        // getting the pixel format that we will use
        // TODO: use something cleaner which uses hints
        let pixel_format = {
            let mut output: ffi::PIXELFORMATDESCRIPTOR = unsafe { mem::uninitialized() };

            if unsafe { ffi::DescribePixelFormat(hdc, 1,
                mem::size_of::<ffi::PIXELFORMATDESCRIPTOR>() as ffi::UINT, &mut output) } == 0
            {
                return Err(format!("DescribePixelFormat function failed: {}",
                    os::error_string(os::errno() as uint)))
            }

            output
        };

        // calling SetPixelFormat
        unsafe {
            if ffi::SetPixelFormat(hdc, 1, &pixel_format) == 0 {
                return Err(format!("SetPixelFormat function failed: {}",
                    os::error_string(os::errno() as uint)))
            }
        }

        // creating the context
        let context = {
            let ctxt = unsafe { ffi::wglCreateContext(hdc) };
            if ctxt.is_null() {
                return Err(format!("wglCreateContext function failed: {}",
                    os::error_string(os::errno() as uint)))
            }
            ctxt
        };

        // loading opengl32
        let gl_library = {
            let name = "opengl32.dll".utf16_units().collect::<Vec<u16>>().append_one(0).as_ptr();
            let lib = unsafe { ffi::LoadLibraryW(name) };
            if lib.is_null() {
                return Err(format!("LoadLibrary function failed: {}",
                    os::error_string(os::errno() as uint)))
            }
            lib
        };

        // building the struct
        Ok(Window{
            window: handle,
            hdc: hdc,
            context: context,
            gl_library: gl_library,
            events_receiver: events_receiver,
            should_close: AtomicBool::new(false),
            nosend: NoSend,
        })
    }

    pub fn should_close(&self) -> bool {
        use std::sync::atomics::Relaxed;
        self.should_close.load(Relaxed)
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
            self.should_close.store(true, Relaxed);
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

    pub fn make_current(&self) {
        unsafe { ffi::wglMakeCurrent(self.hdc, self.context) }
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

fn send_event(window: ffi::HWND, event: Event) {
    let locked = WINDOWS_LIST.get().unwrap();
    let mut locked = locked.lock();
    locked.retain(|&(ref val, ref sender)| {
        if val != &window { return true }

        sender.send_opt(event).is_ok()
    });
}

extern "stdcall" fn callback(window: ffi::HWND, msg: ffi::UINT,
    wparam: ffi::WPARAM, lparam: ffi::LPARAM) -> ffi::LRESULT
{
    match msg {
        ffi::WM_DESTROY => {
            use Closed;
            unsafe { ffi::PostQuitMessage(0); }
            send_event(window, Closed);
            0
        },

        ffi::WM_SIZE => {
            use SizeChanged;
            let w = ffi::LOWORD(lparam as ffi::DWORD) as uint;
            let h = ffi::HIWORD(lparam as ffi::DWORD) as uint;
            send_event(window, SizeChanged(w, h));
            0
        },

        ffi::WM_MOVE => {
            use events::Moved;
            let x = ffi::LOWORD(lparam as ffi::DWORD) as i16 as int;
            let y = ffi::HIWORD(lparam as ffi::DWORD) as i16 as int;
            send_event(window, Moved(x, y));
            0
        },

        ffi::WM_CHAR => {
            use std::mem;
            use events::ReceivedCharacter;
            let chr: char = unsafe { mem::transmute(wparam) };
            send_event(window, ReceivedCharacter(chr));
            0
        },

        ffi::WM_MOUSEMOVE => {
            use CursorPositionChanged;

            let x = ffi::GET_X_LPARAM(lparam) as uint;
            let y = ffi::GET_Y_LPARAM(lparam) as uint;

            send_event(window, CursorPositionChanged(x, y));

            0
        },

        ffi::WM_KEYDOWN => {
            use events::Pressed;
            let element = event::vkeycode_to_element(wparam);
            if element.is_some() {
                send_event(window, Pressed(element.unwrap()));
            }
            0
        },

        ffi::WM_KEYUP => {
            use events::Released;
            let element = event::vkeycode_to_element(wparam);
            if element.is_some() {
                send_event(window, Released(element.unwrap()));
            }
            0
        },

        ffi::WM_LBUTTONDOWN => {
            use events::{Pressed, Button0};
            send_event(window, Pressed(Button0));
            0
        },

        ffi::WM_LBUTTONUP => {
            use events::{Released, Button0};
            send_event(window, Released(Button0));
            0
        },

        ffi::WM_RBUTTONDOWN => {
            use events::{Pressed, Button1};
            send_event(window, Pressed(Button1));
            0
        },

        ffi::WM_RBUTTONUP => {
            use events::{Released, Button1};
            send_event(window, Released(Button1));
            0
        },

        ffi::WM_MBUTTONDOWN => {
            use events::{Pressed, Button2};
            send_event(window, Pressed(Button2));
            0
        },

        ffi::WM_MBUTTONUP => {
            use events::{Released, Button2};
            send_event(window, Released(Button2));
            0
        },

        ffi::WM_SETFOCUS => {
            use events::Focused;
            send_event(window, Focused(true));
            0
        },

        ffi::WM_KILLFOCUS => {
            use events::Focused;
            send_event(window, Focused(false));
            0
        },

        _ => unsafe {
            ffi::DefWindowProcW(window, msg, wparam, lparam)
        }
    }
}

/*fn hints_to_pixelformat(hints: &Hints) -> ffi::PIXELFORMATDESCRIPTOR {
    use std::mem;

    ffi::PIXELFORMATDESCRIPTOR {
        nSize: size_of::<ffi::PIXELFORMATDESCRIPTOR>(),
        nVersion: 1,
        dwFlags:
            if hints.stereo { PFD_STEREO } else { 0 },
        iPixelType: PFD_TYPE_RGBA,
        cColorBits: hints.red_bits + hints.green_bits + hints.blue_bits,
        cRedBits: 

    pub nSize: WORD,
    pub nVersion: WORD,
    pub dwFlags: DWORD,
    pub iPixelType: BYTE,
    pub cColorBits: BYTE,
    pub cRedBits: BYTE,
    pub cRedShift: BYTE,
    pub cGreenBits: BYTE,
    pub cGreenShift: BYTE,
    pub cBlueBits: BYTE,
    pub cBlueShift: BYTE,
    pub cAlphaBits: BYTE,
    pub cAlphaShift: BYTE,
    pub cAccumBits: BYTE,
    pub cAccumRedBits: BYTE,
    pub cAccumGreenBits: BYTE,
    pub cAccumBlueBits: BYTE,
    pub cAccumAlphaBits: BYTE,
    pub cDepthBits: BYTE,
    pub cStencilBits: BYTE,
    pub cAuxBuffers: BYTE,
    pub iLayerType: BYTE,
    pub bReserved: BYTE,
    pub dwLayerMask: DWORD,
    pub dwVisibleMask: DWORD,
    pub dwDamageMask: DWORD,
    }
}*/
