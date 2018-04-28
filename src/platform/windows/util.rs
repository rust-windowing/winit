use std::ops::BitAnd;

use winapi::ctypes::wchar_t;

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
