use std::{mem, ptr, slice, io};
use std::ops::BitAnd;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::window::CursorIcon;
use winapi::ctypes::wchar_t;
use winapi::shared::minwindef::{BOOL, DWORD};
use winapi::shared::windef::{HWND, POINT, RECT};
use winapi::um::winbase::lstrlenW;
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

impl CursorIcon {
    pub(crate) fn to_windows_cursor(self) -> *const wchar_t {
        match self {
            CursorIcon::Arrow | CursorIcon::Default => winuser::IDC_ARROW,
            CursorIcon::Hand => winuser::IDC_HAND,
            CursorIcon::Crosshair => winuser::IDC_CROSS,
            CursorIcon::Text | CursorIcon::VerticalText => winuser::IDC_IBEAM,
            CursorIcon::NotAllowed | CursorIcon::NoDrop => winuser::IDC_NO,
            CursorIcon::Grab | CursorIcon::Grabbing |
            CursorIcon::Move | CursorIcon::AllScroll => winuser::IDC_SIZEALL,
            CursorIcon::EResize | CursorIcon::WResize |
            CursorIcon::EwResize | CursorIcon::ColResize => winuser::IDC_SIZEWE,
            CursorIcon::NResize | CursorIcon::SResize |
            CursorIcon::NsResize | CursorIcon::RowResize => winuser::IDC_SIZENS,
            CursorIcon::NeResize | CursorIcon::SwResize |
            CursorIcon::NeswResize => winuser::IDC_SIZENESW,
            CursorIcon::NwResize | CursorIcon::SeResize |
            CursorIcon::NwseResize => winuser::IDC_SIZENWSE,
            CursorIcon::Wait => winuser::IDC_WAIT,
            CursorIcon::Progress => winuser::IDC_APPSTARTING,
            CursorIcon::Help => winuser::IDC_HELP,
            _ => winuser::IDC_ARROW, // use arrow for the missing cases.
        }
    }
}
