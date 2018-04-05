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
use MonitorId as RootMonitorId;

use winapi::shared::minwindef::{UINT, WORD, DWORD, BOOL};
use winapi::shared::windef::{HWND, HDC, RECT, POINT};
use winapi::shared::hidusage;
use winapi::um::{winuser, dwmapi, wingdi, libloaderapi, processthreadsapi};
use winapi::um::winnt::{LPCWSTR, LONG};

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

            winuser::SetWindowTextW(self.window.0, text.as_ptr() as LPCWSTR);
        }
    }

    #[inline]
    pub fn show(&self) {
        unsafe {
            winuser::ShowWindow(self.window.0, winuser::SW_SHOW);
        }
    }

    #[inline]
    pub fn hide(&self) {
        unsafe {
            winuser::ShowWindow(self.window.0, winuser::SW_HIDE);
        }
    }

    /// See the docs in the crate root file.
    pub fn get_position(&self) -> Option<(i32, i32)> {
        use std::mem;

        let mut placement: winuser::WINDOWPLACEMENT = unsafe { mem::zeroed() };
        placement.length = mem::size_of::<winuser::WINDOWPLACEMENT>() as UINT;

        if unsafe { winuser::GetWindowPlacement(self.window.0, &mut placement) } == 0 {
            return None
        }

        let ref rect = placement.rcNormalPosition;
        Some((rect.left as i32, rect.top as i32))
    }

    /// See the docs in the crate root file.
    pub fn set_position(&self, x: i32, y: i32) {
        unsafe {
            winuser::SetWindowPos(self.window.0, ptr::null_mut(), x as raw::c_int, y as raw::c_int,
                                 0, 0, winuser::SWP_ASYNCWINDOWPOS | winuser::SWP_NOZORDER | winuser::SWP_NOSIZE);
            winuser::UpdateWindow(self.window.0);
        }
    }

    /// See the docs in the crate root file.
    #[inline]
    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        let mut rect: RECT = unsafe { mem::uninitialized() };

        if unsafe { winuser::GetClientRect(self.window.0, &mut rect) } == 0 {
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
        let mut rect: RECT = unsafe { mem::uninitialized() };

        if unsafe { winuser::GetWindowRect(self.window.0, &mut rect) } == 0 {
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
            let mut rect = RECT { top: 0, left: 0, bottom: y as LONG, right: x as LONG };
            let dw_style = winuser::GetWindowLongA(self.window.0, winuser::GWL_STYLE) as DWORD;
            let b_menu = !winuser::GetMenu(self.window.0).is_null() as BOOL;
            let dw_style_ex = winuser::GetWindowLongA(self.window.0, winuser::GWL_EXSTYLE) as DWORD;
            winuser::AdjustWindowRectEx(&mut rect, dw_style, b_menu, dw_style_ex);
            let outer_x = (rect.right - rect.left).abs() as raw::c_int;
            let outer_y = (rect.top - rect.bottom).abs() as raw::c_int;

            winuser::SetWindowPos(self.window.0, ptr::null_mut(), 0, 0, outer_x, outer_y,
                winuser::SWP_ASYNCWINDOWPOS | winuser::SWP_NOZORDER | winuser::SWP_NOREPOSITION | winuser::SWP_NOMOVE);
            winuser::UpdateWindow(self.window.0);
        }
    }

    /// See the docs in the crate root file.
    #[inline]
    pub fn set_min_dimensions(&self, dimensions: Option<(u32, u32)>) {
        let mut window_state = self.window_state.lock().unwrap();
        window_state.attributes.min_dimensions = dimensions;

        // Make windows re-check the window size bounds.
        if let Some(inner_size) = self.get_inner_size() {
            unsafe {
                let mut rect = RECT { top: 0, left: 0, bottom: inner_size.1 as LONG, right: inner_size.0 as LONG };
                let dw_style = winuser::GetWindowLongA(self.window.0, winuser::GWL_STYLE) as DWORD;
                let b_menu = !winuser::GetMenu(self.window.0).is_null() as BOOL;
                let dw_style_ex = winuser::GetWindowLongA(self.window.0, winuser::GWL_EXSTYLE) as DWORD;
                winuser::AdjustWindowRectEx(&mut rect, dw_style, b_menu, dw_style_ex);
                let outer_x = (rect.right - rect.left).abs() as raw::c_int;
                let outer_y = (rect.top - rect.bottom).abs() as raw::c_int;

                winuser::SetWindowPos(self.window.0, ptr::null_mut(), 0, 0, outer_x, outer_y,
                    winuser::SWP_ASYNCWINDOWPOS | winuser::SWP_NOZORDER | winuser::SWP_NOREPOSITION | winuser::SWP_NOMOVE);
            }
        }
    }

    /// See the docs in the crate root file.
    #[inline]
    pub fn set_max_dimensions(&self, dimensions: Option<(u32, u32)>) {
        let mut window_state = self.window_state.lock().unwrap();
        window_state.attributes.max_dimensions = dimensions;

        // Make windows re-check the window size bounds.
        if let Some(inner_size) = self.get_inner_size() {
            unsafe {
                let mut rect = RECT { top: 0, left: 0, bottom: inner_size.1 as LONG, right: inner_size.0 as LONG };
                let dw_style = winuser::GetWindowLongA(self.window.0, winuser::GWL_STYLE) as DWORD;
                let b_menu = !winuser::GetMenu(self.window.0).is_null() as BOOL;
                let dw_style_ex = winuser::GetWindowLongA(self.window.0, winuser::GWL_EXSTYLE) as DWORD;
                winuser::AdjustWindowRectEx(&mut rect, dw_style, b_menu, dw_style_ex);
                let outer_x = (rect.right - rect.left).abs() as raw::c_int;
                let outer_y = (rect.top - rect.bottom).abs() as raw::c_int;

                winuser::SetWindowPos(self.window.0, ptr::null_mut(), 0, 0, outer_x, outer_y,
                    winuser::SWP_ASYNCWINDOWPOS | winuser::SWP_NOZORDER | winuser::SWP_NOREPOSITION | winuser::SWP_NOMOVE);
            }
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
    pub fn hwnd(&self) -> HWND {
        self.window.0
    }

    #[inline]
    pub fn set_cursor(&self, cursor: MouseCursor) {
        let cursor_id = match cursor {
            MouseCursor::Arrow | MouseCursor::Default => winuser::IDC_ARROW,
            MouseCursor::Hand => winuser::IDC_HAND,
            MouseCursor::Crosshair => winuser::IDC_CROSS,
            MouseCursor::Text | MouseCursor::VerticalText => winuser::IDC_IBEAM,
            MouseCursor::NotAllowed | MouseCursor::NoDrop => winuser::IDC_NO,
            MouseCursor::EResize => winuser::IDC_SIZEWE,
            MouseCursor::NResize => winuser::IDC_SIZENS,
            MouseCursor::WResize => winuser::IDC_SIZEWE,
            MouseCursor::SResize => winuser::IDC_SIZENS,
            MouseCursor::EwResize | MouseCursor::ColResize => winuser::IDC_SIZEWE,
            MouseCursor::NsResize | MouseCursor::RowResize => winuser::IDC_SIZENS,
            MouseCursor::Wait | MouseCursor::Progress => winuser::IDC_WAIT,
            MouseCursor::Help => winuser::IDC_HELP,
            _ => winuser::IDC_ARROW, // use arrow for the missing cases.
        };

        let mut cur = self.window_state.lock().unwrap();
        cur.cursor = cursor_id;
    }

    // TODO: it should be possible to rework this function by using the `execute_in_thread` method
    // of the events loop.
    pub fn set_cursor_state(&self, state: CursorState) -> Result<(), String> {
        let mut current_state = self.window_state.lock().unwrap();

        let foreground_thread_id = unsafe { winuser::GetWindowThreadProcessId(self.window.0, ptr::null_mut()) };
        let current_thread_id = unsafe { processthreadsapi::GetCurrentThreadId() };

        unsafe { winuser::AttachThreadInput(foreground_thread_id, current_thread_id, 1) };

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
                    if winuser::GetClientRect(self.window.0, &mut rect) == 0 {
                        return Err(format!("GetWindowRect failed"));
                    }
                    winuser::ClientToScreen(self.window.0, mem::transmute(&mut rect.left));
                    winuser::ClientToScreen(self.window.0, mem::transmute(&mut rect.right));
                    if winuser::ClipCursor(&rect) == 0 {
                        return Err(format!("ClipCursor failed"));
                    }
                    current_state.cursor_state = CursorState::Grab;
                    Ok(())
                }
            },

            (CursorState::Normal, CursorState::Grab) => {
                unsafe {
                    if winuser::ClipCursor(ptr::null()) == 0 {
                        return Err(format!("ClipCursor failed"));
                    }
                    current_state.cursor_state = CursorState::Normal;
                    Ok(())
                }
            },

            _ => unimplemented!(),
        };

        unsafe { winuser::AttachThreadInput(foreground_thread_id, current_thread_id, 0) };

        res
    }

    #[inline]
    pub fn hidpi_factor(&self) -> f32 {
        1.0
    }

    pub fn set_cursor_position(&self, x: i32, y: i32) -> Result<(), ()> {
        let mut point = POINT {
            x: x,
            y: y,
        };

        unsafe {
            if winuser::ClientToScreen(self.window.0, &mut point) == 0 {
                return Err(());
            }

            if winuser::SetCursorPos(point.x, point.y) == 0 {
                return Err(());
            }
        }

        Ok(())
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        WindowId(self.window.0)
    }

    #[inline]
    pub fn set_maximized(&self, _maximized: bool) {
        unimplemented!()
    }

    #[inline]
    pub fn set_fullscreen(&self, _monitor: Option<RootMonitorId>) {
        unimplemented!()
    }

    #[inline]
    pub fn set_decorations(&self, _decorations: bool) {
        unimplemented!()
    }

    #[inline]
    pub fn get_current_monitor(&self) -> RootMonitorId {
        unimplemented!()
    }
}

impl Drop for Window {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            // We are sending WM_CLOSE, and our callback will process this by calling DefWindowProcW,
            // which in turn will send a WM_DESTROY.
            winuser::PostMessageW(self.window.0, winuser::WM_CLOSE, 0, 0);
        }
    }
}

/// A simple non-owning wrapper around a window.
#[doc(hidden)]
pub struct WindowWrapper(HWND, HDC);

unsafe fn init(window: WindowAttributes, pl_attribs: PlatformSpecificWindowBuilderAttributes,
               inserter: events_loop::Inserter) -> Result<Window, CreationError> {
    let title = OsStr::new(&window.title).encode_wide().chain(Some(0).into_iter())
        .collect::<Vec<_>>();

    // registering the window class
    let class_name = register_window_class();

    // building a RECT object with coordinates
    let mut rect = RECT {
        left: 0, right: window.dimensions.unwrap_or((1024, 768)).0 as LONG,
        top: 0, bottom: window.dimensions.unwrap_or((1024, 768)).1 as LONG,
    };

    // switching to fullscreen if necessary
    // this means adjusting the window's position so that it overlaps the right monitor,
    //  and change the monitor's resolution if necessary
    let fullscreen = if let Some(RootMonitorId { ref inner }) = window.fullscreen {
        try!(switch_to_fullscreen(&mut rect, inner));
        true
    } else {
        false
    };

    // computing the style and extended style of the window
    let (ex_style, style) = if fullscreen || !window.decorations {
        (winuser::WS_EX_APPWINDOW,
            //winapi::WS_POPUP is incompatible with winapi::WS_CHILD
            if pl_attribs.parent.is_some() {
                winuser::WS_CLIPSIBLINGS | winuser::WS_CLIPCHILDREN
            }
            else {
                winuser::WS_POPUP | winuser::WS_CLIPSIBLINGS | winuser::WS_CLIPCHILDREN
            }
        )
    } else {
        (winuser::WS_EX_APPWINDOW | winuser::WS_EX_WINDOWEDGE,
            winuser::WS_OVERLAPPEDWINDOW | winuser::WS_CLIPSIBLINGS | winuser::WS_CLIPCHILDREN)
    };

    // adjusting the window coordinates using the style
    winuser::AdjustWindowRectEx(&mut rect, style, 0, ex_style);

    // creating the real window this time, by using the functions in `extra_functions`
    let real_window = {
        let (width, height) = if fullscreen || window.dimensions.is_some() {
            let min_dimensions = window.min_dimensions
                .map(|d| (d.0 as raw::c_int, d.1 as raw::c_int))
                .unwrap_or((0, 0));
            let max_dimensions = window.max_dimensions
                .map(|d| (d.0 as raw::c_int, d.1 as raw::c_int))
                .unwrap_or((raw::c_int::max_value(), raw::c_int::max_value()));

            (
                Some((rect.right - rect.left).min(max_dimensions.0).max(min_dimensions.0)),
                Some((rect.bottom - rect.top).min(max_dimensions.1).max(min_dimensions.1))
            )
        } else {
            (None, None)
        };

        let (x, y) = if fullscreen {
            (Some(rect.left), Some(rect.top))
        } else {
            (None, None)
        };

        let mut style = if !window.visible {
            style
        } else {
            style | winuser::WS_VISIBLE
        };

        if pl_attribs.parent.is_some() {
            style |= winuser::WS_CHILD;
        }

        let handle = winuser::CreateWindowExW(ex_style | winuser::WS_EX_ACCEPTFILES,
            class_name.as_ptr(),
            title.as_ptr() as LPCWSTR,
            style | winuser::WS_CLIPSIBLINGS | winuser::WS_CLIPCHILDREN,
            x.unwrap_or(winuser::CW_USEDEFAULT), y.unwrap_or(winuser::CW_USEDEFAULT),
            width.unwrap_or(winuser::CW_USEDEFAULT), height.unwrap_or(winuser::CW_USEDEFAULT),
            pl_attribs.parent.unwrap_or(ptr::null_mut()),
            ptr::null_mut(), libloaderapi::GetModuleHandleW(ptr::null()),
            ptr::null_mut());

        if handle.is_null() {
            return Err(CreationError::OsError(format!("CreateWindowEx function failed: {}",
                                              format!("{}", io::Error::last_os_error()))));
        }

        let hdc = winuser::GetDC(handle);
        if hdc.is_null() {
            return Err(CreationError::OsError(format!("GetDC function failed: {}",
                                              format!("{}", io::Error::last_os_error()))));
        }

        WindowWrapper(handle, hdc)
    };

    // Set up raw mouse input
    {
        let mut rid: winuser::RAWINPUTDEVICE = mem::uninitialized();
        rid.usUsagePage = hidusage::HID_USAGE_PAGE_GENERIC;
        rid.usUsage = hidusage::HID_USAGE_GENERIC_MOUSE;
        rid.dwFlags = 0;
        rid.hwndTarget = real_window.0;

        winuser::RegisterRawInputDevices(&rid, 1, mem::size_of::<winuser::RAWINPUTDEVICE>() as u32);
    }

    // Register for touch events if applicable
    {
        let digitizer = winuser::GetSystemMetrics( winuser::SM_DIGITIZER ) as u32;
        if digitizer & winuser::NID_READY != 0 {
            winuser::RegisterTouchWindow( real_window.0, winuser::TWF_WANTPALM );
        }
    }
    

    // Creating a mutex to track the current window state
    let window_state = Arc::new(Mutex::new(events_loop::WindowState {
        cursor: winuser::IDC_ARROW, // use arrow by default
        cursor_state: CursorState::Normal,
        attributes: window.clone(),
        mouse_in_window: false,
    }));

    inserter.insert(real_window.0, window_state.clone());

    // making the window transparent
    if window.transparent {
        let bb = dwmapi::DWM_BLURBEHIND {
            dwFlags: 0x1, // FIXME: DWM_BB_ENABLE;
            fEnable: 1,
            hRgnBlur: ptr::null_mut(),
            fTransitionOnMaximized: 0,
        };

        dwmapi::DwmEnableBlurBehindWindow(real_window.0, &bb);
    }

    // calling SetForegroundWindow if fullscreen
    if fullscreen {
        winuser::SetForegroundWindow(real_window.0);
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

    let class = winuser::WNDCLASSEXW {
        cbSize: mem::size_of::<winuser::WNDCLASSEXW>() as UINT,
        style: winuser::CS_HREDRAW | winuser::CS_VREDRAW | winuser::CS_OWNDC,
        lpfnWndProc: Some(events_loop::callback),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: libloaderapi::GetModuleHandleW(ptr::null()),
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
    winuser::RegisterClassExW(&class);

    class_name
}

unsafe fn switch_to_fullscreen(rect: &mut RECT, monitor: &MonitorId)
                               -> Result<(), CreationError>
{
    // adjusting the rect
    {
        let pos = monitor.get_position();
        rect.left += pos.0 as LONG;
        rect.right += pos.0 as LONG;
        rect.top += pos.1 as LONG;
        rect.bottom += pos.1 as LONG;
    }

    // changing device settings
    let mut screen_settings: wingdi::DEVMODEW = mem::zeroed();
    screen_settings.dmSize = mem::size_of::<wingdi::DEVMODEW>() as WORD;
    screen_settings.dmPelsWidth = (rect.right - rect.left) as DWORD;
    screen_settings.dmPelsHeight = (rect.bottom - rect.top) as DWORD;
    screen_settings.dmBitsPerPel = 32;      // TODO: ?
    screen_settings.dmFields = wingdi::DM_BITSPERPEL | wingdi::DM_PELSWIDTH | wingdi::DM_PELSHEIGHT;

    let result = winuser::ChangeDisplaySettingsExW(monitor.get_adapter_name().as_ptr(),
                                                  &mut screen_settings, ptr::null_mut(),
                                                  winuser::CDS_FULLSCREEN, ptr::null_mut());

    if result != winuser::DISP_CHANGE_SUCCESSFUL {
        return Err(CreationError::OsError(format!("ChangeDisplaySettings failed: {}", result)));
    }

    Ok(())
}
