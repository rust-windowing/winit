use std::{
    io, mem,
    ops::BitAnd,
    os::raw::c_void,
    ptr, slice,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::{dpi::PhysicalSize, window::CursorIcon};
use winapi::{
    ctypes::wchar_t,
    shared::{
        minwindef::{BOOL, DWORD, UINT},
        windef::{DPI_AWARENESS_CONTEXT, HMONITOR, HWND, LPRECT, RECT},
    },
    um::{
        libloaderapi::{GetProcAddress, LoadLibraryA},
        shellscalingapi::{MONITOR_DPI_TYPE, PROCESS_DPI_AWARENESS},
        winbase::lstrlenW,
        winnt::{HRESULT, LONG, LPCSTR},
        winuser,
    },
};

pub fn has_flag<T>(bitset: T, flag: T) -> bool
where
    T: Copy + PartialEq + BitAnd<T, Output = T>,
{
    bitset & flag == flag
}

pub fn wchar_to_string(wchar: &[wchar_t]) -> String {
    String::from_utf16_lossy(wchar).to_string()
}

pub fn wchar_ptr_to_string(wchar: *const wchar_t) -> String {
    let len = unsafe { lstrlenW(wchar) } as usize;
    let wchar_slice = unsafe { slice::from_raw_parts(wchar, len) };
    wchar_to_string(wchar_slice)
}

pub unsafe fn status_map<T, F: FnMut(&mut T) -> BOOL>(mut fun: F) -> Option<T> {
    let mut data: T = mem::zeroed();
    if fun(&mut data) != 0 {
        Some(data)
    } else {
        None
    }
}

fn win_to_err<F: FnOnce() -> BOOL>(f: F) -> Result<(), io::Error> {
    if f() != 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn get_window_rect(hwnd: HWND) -> Option<RECT> {
    unsafe { status_map(|rect| winuser::GetWindowRect(hwnd, rect)) }
}

pub fn get_client_rect(hwnd: HWND) -> Result<RECT, io::Error> {
    unsafe {
        let mut rect = mem::zeroed();
        let mut top_left = mem::zeroed();

        win_to_err(|| winuser::ClientToScreen(hwnd, &mut top_left))?;
        win_to_err(|| winuser::GetClientRect(hwnd, &mut rect))?;
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
        right: width as LONG,
        top: 0,
        bottom: height as LONG,
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
                bottom: y as LONG,
                right: x as LONG,
            },
        )
        .expect("adjust_window_rect failed");

        let outer_x = (rect.right - rect.left).abs() as _;
        let outer_y = (rect.top - rect.bottom).abs() as _;
        winuser::SetWindowPos(
            window,
            ptr::null_mut(),
            0,
            0,
            outer_x,
            outer_y,
            winuser::SWP_ASYNCWINDOWPOS
                | winuser::SWP_NOZORDER
                | winuser::SWP_NOREPOSITION
                | winuser::SWP_NOMOVE
                | winuser::SWP_NOACTIVATE,
        );
        winuser::InvalidateRgn(window, ptr::null_mut(), 0);
    }
}

pub fn adjust_window_rect(hwnd: HWND, rect: RECT) -> Option<RECT> {
    unsafe {
        let style = winuser::GetWindowLongW(hwnd, winuser::GWL_STYLE);
        let style_ex = winuser::GetWindowLongW(hwnd, winuser::GWL_EXSTYLE);
        adjust_window_rect_with_styles(hwnd, style as _, style_ex as _, rect)
    }
}

pub fn adjust_window_rect_with_styles(
    hwnd: HWND,
    style: DWORD,
    style_ex: DWORD,
    rect: RECT,
) -> Option<RECT> {
    unsafe {
        status_map(|r| {
            *r = rect;

            let b_menu = !winuser::GetMenu(hwnd).is_null() as BOOL;
            if let (Some(get_dpi_for_window), Some(adjust_window_rect_ex_for_dpi)) =
                (*GET_DPI_FOR_WINDOW, *ADJUST_WINDOW_RECT_EX_FOR_DPI)
            {
                let dpi = get_dpi_for_window(hwnd);
                adjust_window_rect_ex_for_dpi(r, style as _, b_menu, style_ex as _, dpi)
            } else {
                winuser::AdjustWindowRectEx(r, style as _, b_menu, style_ex as _)
            }
        })
    }
}

pub fn set_cursor_hidden(hidden: bool) {
    static HIDDEN: AtomicBool = AtomicBool::new(false);
    let changed = HIDDEN.swap(hidden, Ordering::SeqCst) ^ hidden;
    if changed {
        unsafe { winuser::ShowCursor(!hidden as BOOL) };
    }
}

pub fn get_cursor_clip() -> Result<RECT, io::Error> {
    unsafe {
        let mut rect: RECT = mem::zeroed();
        win_to_err(|| winuser::GetClipCursor(&mut rect)).map(|_| rect)
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
        win_to_err(|| winuser::ClipCursor(rect_ptr))
    }
}

pub fn get_desktop_rect() -> RECT {
    unsafe {
        let left = winuser::GetSystemMetrics(winuser::SM_XVIRTUALSCREEN);
        let top = winuser::GetSystemMetrics(winuser::SM_YVIRTUALSCREEN);
        RECT {
            left,
            top,
            right: left + winuser::GetSystemMetrics(winuser::SM_CXVIRTUALSCREEN),
            bottom: top + winuser::GetSystemMetrics(winuser::SM_CYVIRTUALSCREEN),
        }
    }
}

pub fn is_focused(window: HWND) -> bool {
    window == unsafe { winuser::GetActiveWindow() }
}

impl CursorIcon {
    pub(crate) fn to_windows_cursor(self) -> *const wchar_t {
        match self {
            CursorIcon::Arrow | CursorIcon::Default => winuser::IDC_ARROW,
            CursorIcon::Hand => winuser::IDC_HAND,
            CursorIcon::Crosshair => winuser::IDC_CROSS,
            CursorIcon::Text | CursorIcon::VerticalText => winuser::IDC_IBEAM,
            CursorIcon::NotAllowed | CursorIcon::NoDrop => winuser::IDC_NO,
            CursorIcon::Grab | CursorIcon::Grabbing | CursorIcon::Move | CursorIcon::AllScroll => {
                winuser::IDC_SIZEALL
            }
            CursorIcon::EResize
            | CursorIcon::WResize
            | CursorIcon::EwResize
            | CursorIcon::ColResize => winuser::IDC_SIZEWE,
            CursorIcon::NResize
            | CursorIcon::SResize
            | CursorIcon::NsResize
            | CursorIcon::RowResize => winuser::IDC_SIZENS,
            CursorIcon::NeResize | CursorIcon::SwResize | CursorIcon::NeswResize => {
                winuser::IDC_SIZENESW
            }
            CursorIcon::NwResize | CursorIcon::SeResize | CursorIcon::NwseResize => {
                winuser::IDC_SIZENWSE
            }
            CursorIcon::Wait => winuser::IDC_WAIT,
            CursorIcon::Progress => winuser::IDC_APPSTARTING,
            CursorIcon::Help => winuser::IDC_HELP,
            _ => winuser::IDC_ARROW, // use arrow for the missing cases.
        }
    }
}

// Helper function to dynamically load function pointer.
// `library` and `function` must be zero-terminated.
pub(super) fn get_function_impl(library: &str, function: &str) -> Option<*const c_void> {
    assert_eq!(library.chars().last(), Some('\0'));
    assert_eq!(function.chars().last(), Some('\0'));

    // Library names we will use are ASCII so we can use the A version to avoid string conversion.
    let module = unsafe { LoadLibraryA(library.as_ptr() as LPCSTR) };
    if module.is_null() {
        return None;
    }

    let function_ptr = unsafe { GetProcAddress(module, function.as_ptr() as LPCSTR) };
    if function_ptr.is_null() {
        return None;
    }

    Some(function_ptr as _)
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
pub type GetDpiForWindow = unsafe extern "system" fn(hwnd: HWND) -> UINT;
pub type GetDpiForMonitor = unsafe extern "system" fn(
    hmonitor: HMONITOR,
    dpi_type: MONITOR_DPI_TYPE,
    dpi_x: *mut UINT,
    dpi_y: *mut UINT,
) -> HRESULT;
pub type EnableNonClientDpiScaling = unsafe extern "system" fn(hwnd: HWND) -> BOOL;
pub type AdjustWindowRectExForDpi = unsafe extern "system" fn(
    rect: LPRECT,
    dwStyle: DWORD,
    bMenu: BOOL,
    dwExStyle: DWORD,
    dpi: UINT,
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
