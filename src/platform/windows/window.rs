#![cfg(target_os = "windows")]

use std::ffi::OsStr;
use std::io;
use std::mem;
use std::os::raw;
use std::os::windows::ffi::OsStrExt;
use std::ptr;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc::channel;

use platform::platform::events_loop;
use platform::platform::EventsLoop;
use platform::platform::PlatformSpecificWindowBuilderAttributes;
use platform::platform::MonitorId;
use platform::platform::WindowId;

use CreationError;
use CursorState;
use MouseCursor;
use WindowAttributes;

use dwmapi;
use kernel32;
use user32;
use winapi;

/// The Win32 implementation of the main `Window` object.
pub struct Window {
    /// Main handle for the window.
    window: WindowWrapper,

    /// The current window state.
    window_state: Arc<Mutex<events_loop::WindowState>>,
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

impl Window {
    pub fn new(events_loop: &EventsLoop, w_attr: &WindowAttributes,
               pl_attr: &PlatformSpecificWindowBuilderAttributes) -> Result<Window, CreationError>
    {
        let mut w_attr = Some(w_attr.clone());
        let mut pl_attr = Some(pl_attr.clone());
        
        let (tx, rx) = channel();

        events_loop.execute_in_thread(move |inserter| {
            // We dispatch an `init` function because of code style.
            let win = unsafe { init(w_attr.take().unwrap(), pl_attr.take().unwrap(), inserter) };
            let _ = tx.send(win);
        });

        rx.recv().unwrap()
    }

    pub fn set_title(&self, text: &str) {
        unsafe {
            let text = OsStr::new(text).encode_wide().chain(Some(0).into_iter())
                                       .collect::<Vec<_>>();

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
            user32::SetWindowPos(self.window.0, ptr::null_mut(), x as raw::c_int, y as raw::c_int,
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
            let outer_x = (rect.right - rect.left).abs() as raw::c_int;
            let outer_y = (rect.top - rect.bottom).abs() as raw::c_int;

            user32::SetWindowPos(self.window.0, ptr::null_mut(), 0, 0, outer_x, outer_y,
                winapi::SWP_NOZORDER | winapi::SWP_NOREPOSITION | winapi::SWP_NOMOVE);
            user32::UpdateWindow(self.window.0);
        }
    }

    // TODO: remove
    pub fn platform_display(&self) -> *mut ::libc::c_void {
        panic!()        // Deprecated function ; we don't care anymore
    }
    // TODO: remove
    pub fn platform_window(&self) -> *mut ::libc::c_void {
        self.window.0 as *mut ::libc::c_void
    }

    /// Returns the `hwnd` of this window.
    #[inline]
    pub fn hwnd(&self) -> winapi::HWND {
        self.window.0
    }

    #[inline]
    pub fn set_cursor(&self, cursor: MouseCursor) {
        let cursor_id = match cursor {
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

    // TODO: it should be possible to rework this function by using the `execute_in_thread` method
    // of the events loop.
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

    #[inline]
    pub fn id(&self) -> WindowId {
        WindowId(self.window.0)
    }
}

impl Drop for Window {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            // We are sending WM_CLOSE, and our callback will process this by calling DefWindowProcW, 
            // which in turn will send a WM_DESTROY.
            user32::PostMessageW(self.window.0, winapi::WM_CLOSE, 0, 0);
        }
    }
}

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

unsafe fn init(window: WindowAttributes, pl_attribs: PlatformSpecificWindowBuilderAttributes,
               inserter: events_loop::Inserter) -> Result<Window, CreationError> {
    let title = OsStr::new(&window.title).encode_wide().chain(Some(0).into_iter())
        .collect::<Vec<_>>();

    // registering the window class
    let class_name = register_window_class();

    // building a RECT object with coordinates
    let mut rect = winapi::RECT {
        left: 0, right: window.dimensions.unwrap_or((1024, 768)).0 as winapi::LONG,
        top: 0, bottom: window.dimensions.unwrap_or((1024, 768)).1 as winapi::LONG,
    };

    // switching to fullscreen if necessary
    // this means adjusting the window's position so that it overlaps the right monitor,
    //  and change the monitor's resolution if necessary
    if window.monitor.is_some() {
        let monitor = window.monitor.as_ref().unwrap();
        try!(switch_to_fullscreen(&mut rect, monitor));
    }

    // computing the style and extended style of the window
    let (ex_style, style) = if window.monitor.is_some() || !window.decorations {
        (winapi::WS_EX_APPWINDOW,
            //winapi::WS_POPUP is incompatible with winapi::WS_CHILD
            if pl_attribs.parent.is_some() {
                winapi::WS_CLIPSIBLINGS | winapi::WS_CLIPCHILDREN
            }
            else {
                winapi::WS_POPUP | winapi::WS_CLIPSIBLINGS | winapi::WS_CLIPCHILDREN
            }
        )
    } else {
        (winapi::WS_EX_APPWINDOW | winapi::WS_EX_WINDOWEDGE,
            winapi::WS_OVERLAPPEDWINDOW | winapi::WS_CLIPSIBLINGS | winapi::WS_CLIPCHILDREN)
    };

    // adjusting the window coordinates using the style
    user32::AdjustWindowRectEx(&mut rect, style, 0, ex_style);

    // creating the real window this time, by using the functions in `extra_functions`
    let real_window = {
        let (width, height) = if window.monitor.is_some() || window.dimensions.is_some() {
            (Some(rect.right - rect.left), Some(rect.bottom - rect.top))
        } else {
            (None, None)
        };

        let (x, y) = if window.monitor.is_some() {
            (Some(rect.left), Some(rect.top))
        } else {
            (None, None)
        };

        let mut style = if !window.visible {
            style
        } else {
            style | winapi::WS_VISIBLE
        };

        if pl_attribs.parent.is_some() {
            style |= winapi::WS_CHILD;
        }

        let handle = user32::CreateWindowExW(ex_style | winapi::WS_EX_ACCEPTFILES,
            class_name.as_ptr(),
            title.as_ptr() as winapi::LPCWSTR,
            style | winapi::WS_CLIPSIBLINGS | winapi::WS_CLIPCHILDREN,
            x.unwrap_or(winapi::CW_USEDEFAULT), y.unwrap_or(winapi::CW_USEDEFAULT),
            width.unwrap_or(winapi::CW_USEDEFAULT), height.unwrap_or(winapi::CW_USEDEFAULT),
            pl_attribs.parent.unwrap_or(ptr::null_mut()),
            ptr::null_mut(), kernel32::GetModuleHandleW(ptr::null()),
            ptr::null_mut());

        if handle.is_null() {
            return Err(CreationError::OsError(format!("CreateWindowEx function failed: {}",
                                              format!("{}", io::Error::last_os_error()))));
        }

        let hdc = user32::GetDC(handle);
        if hdc.is_null() {
            return Err(CreationError::OsError(format!("GetDC function failed: {}",
                                              format!("{}", io::Error::last_os_error()))));
        }

        WindowWrapper(handle, hdc)
    };

    // Set up raw mouse input
    {
        let mut rid: winapi::RAWINPUTDEVICE = mem::uninitialized();
        rid.usUsagePage = winapi::HID_USAGE_PAGE_GENERIC;
        rid.usUsage = winapi::HID_USAGE_GENERIC_MOUSE;
        rid.dwFlags = 0;
        rid.hwndTarget = real_window.0;

        user32::RegisterRawInputDevices(&rid, 1, mem::size_of::<winapi::RAWINPUTDEVICE>() as u32);
    }

    // Creating a mutex to track the current window state
    let window_state = Arc::new(Mutex::new(events_loop::WindowState {
        cursor: winapi::IDC_ARROW, // use arrow by default
        cursor_state: CursorState::Normal,
        attributes: window.clone(),
        mouse_in_window: false,
    }));

    inserter.insert(real_window.0, window_state.clone());

    // making the window transparent
    if window.transparent {
        let bb = winapi::DWM_BLURBEHIND {
            dwFlags: 0x1, // FIXME: DWM_BB_ENABLE;
            fEnable: 1,
            hRgnBlur: ptr::null_mut(),
            fTransitionOnMaximized: 0,
        };

        dwmapi::DwmEnableBlurBehindWindow(real_window.0, &bb);
    }

    // calling SetForegroundWindow if fullscreen
    if window.monitor.is_some() {
        user32::SetForegroundWindow(real_window.0);
    }

    // Building the struct.
    Ok(Window {
        window: real_window,
        window_state: window_state,
    })
}

unsafe fn register_window_class() -> Vec<u16> {
    let class_name = OsStr::new("Window Class").encode_wide().chain(Some(0).into_iter())
                                               .collect::<Vec<_>>();

    let class = winapi::WNDCLASSEXW {
        cbSize: mem::size_of::<winapi::WNDCLASSEXW>() as winapi::UINT,
        style: winapi::CS_HREDRAW | winapi::CS_VREDRAW | winapi::CS_OWNDC,
        lpfnWndProc: Some(events_loop::callback),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: kernel32::GetModuleHandleW(ptr::null()),
        hIcon: ptr::null_mut(),
        hCursor: ptr::null_mut(),       // must be null in order for cursor state to work properly
        hbrBackground: ptr::null_mut(),
        lpszMenuName: ptr::null(),
        lpszClassName: class_name.as_ptr(),
        hIconSm: ptr::null_mut(),
    };

    // We ignore errors because registering the same window class twice would trigger
    //  an error, and because errors here are detected during CreateWindowEx anyway.
    // Also since there is no weird element in the struct, there is no reason for this
    //  call to fail.
    user32::RegisterClassExW(&class);

    class_name
}

unsafe fn switch_to_fullscreen(rect: &mut winapi::RECT, monitor: &MonitorId)
                               -> Result<(), CreationError>
{
    // adjusting the rect
    {
        let pos = monitor.get_position();
        rect.left += pos.0 as winapi::LONG;
        rect.right += pos.0 as winapi::LONG;
        rect.top += pos.1 as winapi::LONG;
        rect.bottom += pos.1 as winapi::LONG;
    }

    // changing device settings
    let mut screen_settings: winapi::DEVMODEW = mem::zeroed();
    screen_settings.dmSize = mem::size_of::<winapi::DEVMODEW>() as winapi::WORD;
    screen_settings.dmPelsWidth = (rect.right - rect.left) as winapi::DWORD;
    screen_settings.dmPelsHeight = (rect.bottom - rect.top) as winapi::DWORD;
    screen_settings.dmBitsPerPel = 32;      // TODO: ?
    screen_settings.dmFields = winapi::DM_BITSPERPEL | winapi::DM_PELSWIDTH | winapi::DM_PELSHEIGHT;

    let result = user32::ChangeDisplaySettingsExW(monitor.get_adapter_name().as_ptr(),
                                                  &mut screen_settings, ptr::null_mut(),
                                                  winapi::CDS_FULLSCREEN, ptr::null_mut());

    if result != winapi::DISP_CHANGE_SUCCESSFUL {
        return Err(CreationError::OsError(format!("ChangeDisplaySettings failed: {}", result)));
    }

    Ok(())
}
