use std::{self, mem, ptr, slice, io};
use std::ops::BitAnd;
use std::sync::atomic::{AtomicBool, Ordering};

use MouseCursor;
use winapi::ctypes::wchar_t;
use winapi::shared::minwindef::{BOOL, DWORD};
use winapi::shared::windef::{HWND, POINT, RECT};
use winapi::um::errhandlingapi::GetLastError;
use winapi::um::winbase::{
    FormatMessageW,
    FORMAT_MESSAGE_ALLOCATE_BUFFER,
    FORMAT_MESSAGE_FROM_SYSTEM,
    FORMAT_MESSAGE_IGNORE_INSERTS,
    lstrlenW,
    LocalFree,
};
use winapi::um::winnt::{
    LPCWSTR,
    MAKELANGID,
    LANG_NEUTRAL,
    SUBLANG_DEFAULT,
};
use winapi::um::winuser;

pub fn has_flag<T>(bitset: T, flag: T) -> bool
where T:
    Copy + PartialEq + BitAnd<T, Output = T>
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
    let mut data: T = mem::uninitialized();
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

pub fn get_cursor_pos() -> Option<POINT> {
    unsafe { status_map(|cursor_pos| winuser::GetCursorPos(cursor_pos)) }
}

pub fn get_window_rect(hwnd: HWND) -> Option<RECT> {
    unsafe { status_map(|rect| winuser::GetWindowRect(hwnd, rect)) }
}

pub fn get_client_rect(hwnd: HWND) -> Result<RECT, io::Error> {
    unsafe {
        let mut rect = mem::uninitialized();
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

pub fn adjust_window_rect(hwnd: HWND, rect: RECT) -> Option<RECT> {
    unsafe {
        let style = winuser::GetWindowLongW(hwnd, winuser::GWL_STYLE);
        let style_ex = winuser::GetWindowLongW(hwnd, winuser::GWL_EXSTYLE);
        adjust_window_rect_with_styles(hwnd, style as _, style_ex as _, rect)
    }
}

pub fn adjust_window_rect_with_styles(hwnd: HWND, style: DWORD, style_ex: DWORD, rect: RECT) -> Option<RECT> {
    unsafe { status_map(|r| {
        *r = rect;

        let b_menu = !winuser::GetMenu(hwnd).is_null() as BOOL;
        winuser::AdjustWindowRectEx(r, style as _ , b_menu, style_ex as _)
    }) }
}

pub fn set_cursor_hidden(hidden: bool) {
    static HIDDEN: AtomicBool = AtomicBool::new(false);
    let changed = HIDDEN.swap(hidden, Ordering::SeqCst) ^ hidden;
    if changed {
        unsafe{ winuser::ShowCursor(!hidden as BOOL) };
    }
}

pub fn set_cursor_clip(rect: Option<RECT>) -> Result<(), io::Error> {
    unsafe {
        let rect_ptr = rect.as_ref().map(|r| r as *const RECT).unwrap_or(ptr::null());
        win_to_err(|| winuser::ClipCursor(rect_ptr))
    }
}

pub fn is_focused(window: HWND) -> bool {
    window == unsafe{ winuser::GetActiveWindow() }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct WinError(Option<String>);

impl WinError {
    pub fn from_last_error() -> Self {
        WinError(unsafe { get_last_error() })
    }
}

pub unsafe fn get_last_error() -> Option<String> {
    let err = GetLastError();
    if err != 0 {
        let buf_addr: LPCWSTR = {
            let mut buf_addr: LPCWSTR = mem::uninitialized();
            FormatMessageW(
               FORMAT_MESSAGE_ALLOCATE_BUFFER
               | FORMAT_MESSAGE_FROM_SYSTEM
               | FORMAT_MESSAGE_IGNORE_INSERTS,
               ptr::null(),
               err,
               MAKELANGID(LANG_NEUTRAL, SUBLANG_DEFAULT) as DWORD,
               // This is a pointer to a pointer
               &mut buf_addr as *mut LPCWSTR as *mut _,
               0,
               ptr::null_mut(),
            );
            buf_addr
        };
        if !buf_addr.is_null() {
            let buf_len = lstrlenW(buf_addr) as usize;
            let buf_slice = std::slice::from_raw_parts(buf_addr, buf_len);
            let string = wchar_to_string(buf_slice);
            LocalFree(buf_addr as *mut _);
            return Some(string);
        }
    }
    None
}

impl MouseCursor {
    pub(crate) fn to_windows_cursor(self) -> *const wchar_t {
        match self {
            MouseCursor::Arrow | MouseCursor::Default => winuser::IDC_ARROW,
            MouseCursor::Hand => winuser::IDC_HAND,
            MouseCursor::Crosshair => winuser::IDC_CROSS,
            MouseCursor::Text | MouseCursor::VerticalText => winuser::IDC_IBEAM,
            MouseCursor::NotAllowed | MouseCursor::NoDrop => winuser::IDC_NO,
            MouseCursor::Grab | MouseCursor::Grabbing |
            MouseCursor::Move | MouseCursor::AllScroll => winuser::IDC_SIZEALL,
            MouseCursor::EResize | MouseCursor::WResize |
            MouseCursor::EwResize | MouseCursor::ColResize => winuser::IDC_SIZEWE,
            MouseCursor::NResize | MouseCursor::SResize |
            MouseCursor::NsResize | MouseCursor::RowResize => winuser::IDC_SIZENS,
            MouseCursor::NeResize | MouseCursor::SwResize |
            MouseCursor::NeswResize => winuser::IDC_SIZENESW,
            MouseCursor::NwResize | MouseCursor::SeResize |
            MouseCursor::NwseResize => winuser::IDC_SIZENWSE,
            MouseCursor::Wait => winuser::IDC_WAIT,
            MouseCursor::Progress => winuser::IDC_APPSTARTING,
            MouseCursor::Help => winuser::IDC_HELP,
            _ => winuser::IDC_ARROW, // use arrow for the missing cases.
        }
    }
}
