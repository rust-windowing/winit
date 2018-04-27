use std::{self, mem, ptr};
use std::ops::BitAnd;

use winapi::ctypes::wchar_t;
use winapi::shared::minwindef::DWORD;
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

pub fn has_flag<T>(bitset: T, flag: T) -> bool
where T:
    Copy + PartialEq + BitAnd<T, Output = T>
{
    bitset & flag == flag
}

pub fn wchar_to_string(wchar: &[wchar_t]) -> String {
    String::from_utf16_lossy(wchar)
        .trim_right_matches(0 as char)
        .to_string()
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
