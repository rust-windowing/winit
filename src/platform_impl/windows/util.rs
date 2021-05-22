use std::{
    io, mem,
    ops::BitAnd,
    ptr,
    sync::atomic::{AtomicBool, Ordering},
};

use winapi::{
    windows::HRESULT,
    Windows::Win32::{
        Graphics::Gdi::{ClientToScreen, InvalidateRgn, HMONITOR},
        System::SystemServices::{
            GetProcAddress, LoadLibraryA, BOOL, DPI_AWARENESS_CONTEXT, LRESULT, PSTR, PWSTR,
        },
        UI::{
            DisplayDevices::RECT,
            HiDpi::{
                AdjustWindowRectExForDpi, GetDpiForWindow, MONITOR_DPI_TYPE, PROCESS_DPI_AWARENESS,
            },
            KeyboardAndMouseInput::GetActiveWindow,
            WindowsAndMessaging::{
                ClipCursor, GetClientRect, GetClipCursor, GetMenu, GetSystemMetrics,
                GetWindowLongPtrW, GetWindowLongW, GetWindowRect, SetWindowPos, ShowCursor,
                GWL_EXSTYLE, GWL_STYLE, HWND, IDC_APPSTARTING, IDC_ARROW, IDC_CROSS, IDC_HAND,
                IDC_HELP, IDC_IBEAM, IDC_NO, IDC_SIZEALL, IDC_SIZENESW, IDC_SIZENS, IDC_SIZENWSE,
                IDC_SIZEWE, IDC_WAIT, LPARAM, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
                SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE,
                SWP_NOMOVE, SWP_NOREPOSITION, SWP_NOZORDER, WPARAM,
            },
        },
    },
};

use crate::{dpi::PhysicalSize, window::CursorIcon};

#[link(name = "USER32")]
extern "system" {
    pub fn DefWindowProcW(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT;
}

pub fn has_flag<T>(bitset: T, flag: T) -> bool
where
    T: Copy + PartialEq + BitAnd<T, Output = T>,
{
    bitset & flag == flag
}

pub unsafe fn status_map<T, F: FnMut(&mut T) -> BOOL>(mut fun: F) -> Option<T> {
    let mut data: T = mem::zeroed();
    if !fun(&mut data).as_bool() {
        Some(data)
    } else {
        None
    }
}

fn win_to_err<F: FnOnce() -> BOOL>(f: F) -> Result<(), io::Error> {
    if !f().as_bool() {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn get_window_rect(hwnd: HWND) -> Option<RECT> {
    unsafe { status_map(|rect| GetWindowRect(hwnd, rect)) }
}

pub fn get_client_rect(hwnd: HWND) -> Result<RECT, io::Error> {
    unsafe {
        let mut rect = mem::zeroed();
        let mut top_left = mem::zeroed();

        win_to_err(|| ClientToScreen(hwnd, &mut top_left))?;
        win_to_err(|| GetClientRect(hwnd, &mut rect))?;
        rect.left += top_left.x;
        rect.top += top_left.y;
        rect.right += top_left.x;
        rect.bottom += top_left.y;

        Ok(rect)
    }
}

pub fn adjust_size(hwnd: HWND, size: PhysicalSize<u32>) -> PhysicalSize<u32> {
    let (width, height): (u32, u32) = size.into();
    let rect = RECT {
        left: 0,
        right: width as i32,
        top: 0,
        bottom: height as i32,
    };
    let rect = adjust_window_rect(hwnd, rect).unwrap_or(rect);
    PhysicalSize::new((rect.right - rect.left) as _, (rect.bottom - rect.top) as _)
}

pub(crate) fn set_inner_size_physical(window: HWND, x: u32, y: u32) {
    unsafe {
        let rect = adjust_window_rect(
            window,
            RECT {
                top: 0,
                left: 0,
                bottom: y as i32,
                right: x as i32,
            },
        )
        .expect("adjust_window_rect failed");

        let outer_x = (rect.right - rect.left).abs() as _;
        let outer_y = (rect.top - rect.bottom).abs() as _;
        SetWindowPos(
            window,
            None,
            0,
            0,
            outer_x,
            outer_y,
            SWP_ASYNCWINDOWPOS | SWP_NOZORDER | SWP_NOREPOSITION | SWP_NOMOVE | SWP_NOACTIVATE,
        );
        InvalidateRgn(window, None, false);
    }
}

pub fn adjust_window_rect(hwnd: HWND, rect: RECT) -> Option<RECT> {
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        let style_ex = GetWindowLongW(hwnd, GWL_EXSTYLE);
        adjust_window_rect_with_styles(hwnd, style as u32, style_ex as _, rect)
    }
}

pub fn adjust_window_rect_with_styles(
    hwnd: HWND,
    style: u32,
    style_ex: u32,
    rect: RECT,
) -> Option<RECT> {
    unsafe {
        status_map(|r| {
            *r = rect;

            let b_menu = !GetMenu(hwnd).is_null();

            // if let (Some(get_dpi_for_window), Some(adjust_window_rect_ex_for_dpi)) =
            //     (*GET_DPI_FOR_WINDOW, *ADJUST_WINDOW_RECT_EX_FOR_DPI)
            // {

            let dpi = GetDpiForWindow(hwnd);
            AdjustWindowRectExForDpi(r, style, b_menu, style_ex, dpi)

            // } else {
            //     winuser::AdjustWindowRectEx(r, style as _, b_menu, style_ex as _)
            // }
        })
    }
}

pub fn set_cursor_hidden(hidden: bool) {
    static HIDDEN: AtomicBool = AtomicBool::new(false);
    let changed = HIDDEN.swap(hidden, Ordering::SeqCst) ^ hidden;
    if changed {
        unsafe { ShowCursor(!hidden) };
    }
}

pub fn get_cursor_clip() -> Result<RECT, io::Error> {
    unsafe {
        let mut rect: RECT = mem::zeroed();
        win_to_err(|| GetClipCursor(&mut rect)).map(|_| rect)
    }
}

/// Sets the cursor's clip rect.
///
/// Note that calling this will automatically dispatch a `WM_MOUSEMOVE` event.
pub fn set_cursor_clip(rect: Option<RECT>) -> Result<(), io::Error> {
    unsafe {
        let rect_ptr = rect
            .as_ref()
            .map(|r| r as *const RECT)
            .unwrap_or(ptr::null());
        win_to_err(|| ClipCursor(rect_ptr))
    }
}

pub fn get_desktop_rect() -> RECT {
    unsafe {
        let left = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let top = GetSystemMetrics(SM_YVIRTUALSCREEN);
        RECT {
            left,
            top,
            right: left + GetSystemMetrics(SM_CXVIRTUALSCREEN),
            bottom: top + GetSystemMetrics(SM_CYVIRTUALSCREEN),
        }
    }
}

pub fn is_focused(window: HWND) -> bool {
    window == unsafe { GetActiveWindow() }
}

impl CursorIcon {
    pub(crate) fn to_windows_cursor(self) -> PWSTR {
        match self {
            CursorIcon::Arrow | CursorIcon::Default => IDC_ARROW,
            CursorIcon::Hand => IDC_HAND,
            CursorIcon::Crosshair => IDC_CROSS,
            CursorIcon::Text | CursorIcon::VerticalText => IDC_IBEAM,
            CursorIcon::NotAllowed | CursorIcon::NoDrop => IDC_NO,
            CursorIcon::Grab | CursorIcon::Grabbing | CursorIcon::Move | CursorIcon::AllScroll => {
                IDC_SIZEALL
            }
            CursorIcon::EResize
            | CursorIcon::WResize
            | CursorIcon::EwResize
            | CursorIcon::ColResize => IDC_SIZEWE,
            CursorIcon::NResize
            | CursorIcon::SResize
            | CursorIcon::NsResize
            | CursorIcon::RowResize => IDC_SIZENS,
            CursorIcon::NeResize | CursorIcon::SwResize | CursorIcon::NeswResize => IDC_SIZENESW,
            CursorIcon::NwResize | CursorIcon::SeResize | CursorIcon::NwseResize => IDC_SIZENWSE,
            CursorIcon::Wait => IDC_WAIT,
            CursorIcon::Progress => IDC_APPSTARTING,
            CursorIcon::Help => IDC_HELP,
            _ => IDC_ARROW, // use arrow for the missing cases.
        }
    }
}

// Helper function to dynamically load function pointer.
// `library` and `function` must be zero-terminated.
pub(super) fn get_function_impl(
    library: &str,
    function: &str,
) -> Option<*const std::os::raw::c_void> {
    // Library names we will use are ASCII so we can use the A version to avoid string conversion.
    let module = unsafe { LoadLibraryA(PSTR(library.as_mut_ptr())) };
    if module.is_null() {
        return None;
    }

    unsafe { GetProcAddress(module, PSTR(function.as_mut_ptr())) }.map(|func| func as _)
}

macro_rules! get_function {
    ($lib:expr, $func:ident) => {
        crate::platform_impl::platform::util::get_function_impl(
            concat!($lib, '\0'),
            concat!(stringify!($func), '\0'),
        )
        .map(|f| unsafe { std::mem::transmute::<*const _, $func>(f) })
    };
}

pub type SetProcessDPIAware = unsafe extern "system" fn() -> BOOL;
pub type SetProcessDpiAwareness =
    unsafe extern "system" fn(value: PROCESS_DPI_AWARENESS) -> HRESULT;
pub type SetProcessDpiAwarenessContext =
    unsafe extern "system" fn(value: DPI_AWARENESS_CONTEXT) -> BOOL;
pub type GetDpiForWindow = unsafe extern "system" fn(hwnd: HWND) -> u32;
pub type GetDpiForMonitor = unsafe extern "system" fn(
    hmonitor: HMONITOR,
    dpi_type: MONITOR_DPI_TYPE,
    dpi_x: *mut u32,
    dpi_y: *mut u32,
) -> HRESULT;
pub type EnableNonClientDpiScaling = unsafe extern "system" fn(hwnd: HWND) -> BOOL;
pub type AdjustWindowRectExForDpi = unsafe extern "system" fn(
    rect: *mut RECT,
    dwStyle: u32,
    bMenu: BOOL,
    dwExStyle: u32,
    dpi: u32,
) -> BOOL;

lazy_static! {
    pub static ref GET_DPI_FOR_WINDOW: Option<GetDpiForWindow> =
        get_function!("user32.dll", GetDpiForWindow);
    pub static ref ADJUST_WINDOW_RECT_EX_FOR_DPI: Option<AdjustWindowRectExForDpi> =
        get_function!("user32.dll", AdjustWindowRectExForDpi);
    pub static ref GET_DPI_FOR_MONITOR: Option<GetDpiForMonitor> =
        get_function!("shcore.dll", GetDpiForMonitor);
    pub static ref ENABLE_NON_CLIENT_DPI_SCALING: Option<EnableNonClientDpiScaling> =
        get_function!("user32.dll", EnableNonClientDpiScaling);
    pub static ref SET_PROCESS_DPI_AWARENESS_CONTEXT: Option<SetProcessDpiAwarenessContext> =
        get_function!("user32.dll", SetProcessDpiAwarenessContext);
    pub static ref SET_PROCESS_DPI_AWARENESS: Option<SetProcessDpiAwareness> =
        get_function!("shcore.dll", SetProcessDpiAwareness);
    pub static ref SET_PROCESS_DPI_AWARE: Option<SetProcessDPIAware> =
        get_function!("user32.dll", SetProcessDPIAware);
}
