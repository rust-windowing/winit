use std::{self, mem, ptr, slice};
use std::ops::BitAnd;

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

pub fn get_cursor_pos() -> Option<POINT> {
    unsafe { status_map(|cursor_pos| winuser::GetCursorPos(cursor_pos)) }
}

pub fn get_window_rect(hwnd: HWND) -> Option<RECT> {
    unsafe { status_map(|rect| winuser::GetWindowRect(hwnd, rect)) }
}

// This won't be needed anymore if we just add a derive to winapi.
pub fn rect_eq(a: &RECT, b: &RECT) -> bool {
    let left_eq = a.left == b.left;
    let right_eq = a.right == b.right;
    let top_eq = a.top == b.top;
    let bottom_eq = a.bottom == b.bottom;
    left_eq && right_eq && top_eq && bottom_eq
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
