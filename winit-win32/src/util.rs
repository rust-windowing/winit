use std::ffi::{OsStr, OsString, c_void};
use std::iter::once;
use std::ops::BitAnd;
use std::os::windows::prelude::{OsStrExt, OsStringExt};
use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{io, mem, ptr};

use windows_sys::Win32::Foundation::{
    BOOL, HANDLE, HINSTANCE, HMODULE, HWND, NTSTATUS, POINT, RECT,
};
use windows_sys::Win32::Graphics::Gdi::{ClientToScreen, HMONITOR};
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};
use windows_sys::Win32::System::SystemInformation::OSVERSIONINFOW;
use windows_sys::Win32::System::SystemServices::IMAGE_DOS_HEADER;
use windows_sys::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT, MONITOR_DPI_TYPE, PROCESS_DPI_AWARENESS,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetActiveWindow;
use windows_sys::Win32::UI::Input::Pointer::{POINTER_INFO, POINTER_PEN_INFO, POINTER_TOUCH_INFO};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    ClipCursor, GetClientRect, GetClipCursor, GetCursorPos, GetSystemMetrics, GetWindowPlacement,
    GetWindowRect, HMENU, IDC_APPSTARTING, IDC_ARROW, IDC_CROSS, IDC_HAND, IDC_HELP, IDC_IBEAM,
    IDC_NO, IDC_SIZEALL, IDC_SIZENESW, IDC_SIZENS, IDC_SIZENWSE, IDC_SIZEWE, IDC_WAIT, IsIconic,
    SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SW_MAXIMIZE,
    ShowCursor, WINDOW_EX_STYLE, WINDOW_LONG_PTR_INDEX, WINDOW_STYLE, WINDOWPLACEMENT,
};
use windows_sys::core::{HRESULT, PCWSTR};
use winit_core::cursor::CursorIcon;
use winit_core::event::DeviceId;

macro_rules! os_error {
    ($error:expr) => {{ winit_core::error::OsError::new(line!(), file!(), $error) }};
}

pub fn encode_wide(string: impl AsRef<OsStr>) -> Vec<u16> {
    string.as_ref().encode_wide().chain(once(0)).collect()
}

pub fn decode_wide(mut wide_c_string: &[u16]) -> OsString {
    if let Some(null_pos) = wide_c_string.iter().position(|c| *c == 0) {
        wide_c_string = &wide_c_string[..null_pos];
    }

    OsString::from_wide(wide_c_string)
}

pub fn has_flag<T>(bitset: T, flag: T) -> bool
where
    T: Copy + PartialEq + BitAnd<T, Output = T>,
{
    bitset & flag == flag
}

pub(crate) fn win_to_err(result: BOOL) -> Result<(), io::Error> {
    if result != false.into() { Ok(()) } else { Err(io::Error::last_os_error()) }
}

pub enum WindowArea {
    Outer,
    Inner,
}

impl WindowArea {
    pub fn get_rect(self, hwnd: HWND) -> Result<RECT, io::Error> {
        let mut rect = unsafe { mem::zeroed() };

        match self {
            WindowArea::Outer => {
                win_to_err(unsafe { GetWindowRect(hwnd, &mut rect) })?;
            },
            WindowArea::Inner => unsafe {
                let mut top_left = mem::zeroed();

                win_to_err(ClientToScreen(hwnd, &mut top_left))?;
                win_to_err(GetClientRect(hwnd, &mut rect))?;
                rect.left += top_left.x;
                rect.top += top_left.y;
                rect.right += top_left.x;
                rect.bottom += top_left.y;
            },
        }

        Ok(rect)
    }
}

pub fn is_maximized(window: HWND) -> bool {
    unsafe {
        let mut placement: WINDOWPLACEMENT = mem::zeroed();
        placement.length = mem::size_of::<WINDOWPLACEMENT>() as u32;
        GetWindowPlacement(window, &mut placement);
        placement.showCmd == SW_MAXIMIZE as u32
    }
}

pub fn set_cursor_hidden(hidden: bool) {
    static HIDDEN: AtomicBool = AtomicBool::new(false);
    let changed = HIDDEN.swap(hidden, Ordering::SeqCst) ^ hidden;
    if changed {
        unsafe { ShowCursor(BOOL::from(!hidden)) };
    }
}

pub fn get_cursor_position() -> Result<POINT, io::Error> {
    unsafe {
        let mut point: POINT = mem::zeroed();
        win_to_err(GetCursorPos(&mut point)).map(|_| point)
    }
}

pub fn get_cursor_clip() -> Result<RECT, io::Error> {
    unsafe {
        let mut rect: RECT = mem::zeroed();
        win_to_err(GetClipCursor(&mut rect)).map(|_| rect)
    }
}

/// Sets the cursor's clip rect.
///
/// Note that calling this will automatically dispatch a `WM_MOUSEMOVE` event.
pub fn set_cursor_clip(rect: Option<RECT>) -> Result<(), io::Error> {
    unsafe {
        let rect_ptr = rect.as_ref().map(|r| r as *const RECT).unwrap_or(ptr::null());
        win_to_err(ClipCursor(rect_ptr))
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

pub fn is_minimized(window: HWND) -> bool {
    unsafe { IsIconic(window) != false.into() }
}

pub fn get_instance_handle() -> HMODULE {
    // Gets the instance handle by taking the address of the
    // pseudo-variable created by the microsoft linker:
    // https://devblogs.microsoft.com/oldnewthing/20041025-00/?p=37483

    // This is preferred over GetModuleHandle(NULL) because it also works in DLLs:
    // https://stackoverflow.com/questions/21718027/getmodulehandlenull-vs-hinstance

    unsafe extern "C" {
        static __ImageBase: IMAGE_DOS_HEADER;
    }

    unsafe { &__ImageBase as *const _ as _ }
}

pub(crate) fn to_windows_cursor(cursor: CursorIcon) -> PCWSTR {
    match cursor {
        CursorIcon::Default => IDC_ARROW,
        CursorIcon::Pointer => IDC_HAND,
        CursorIcon::Crosshair => IDC_CROSS,
        CursorIcon::Text | CursorIcon::VerticalText => IDC_IBEAM,
        CursorIcon::NotAllowed | CursorIcon::NoDrop => IDC_NO,
        CursorIcon::Grab | CursorIcon::Grabbing | CursorIcon::Move | CursorIcon::AllScroll => {
            IDC_SIZEALL
        },
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

// Helper function to dynamically load function pointer as some functions
// may not be available on all Windows platforms supported by winit.
//
// `library` and `function` must be zero-terminated.
pub(super) fn get_function_impl(library: &str, function: &str) -> Option<*const c_void> {
    assert_eq!(library.chars().last(), Some('\0'));
    assert_eq!(function.chars().last(), Some('\0'));

    // Library names we will use are ASCII so we can use the A version to avoid string conversion.
    let module = unsafe { LoadLibraryA(library.as_ptr()) };
    if module.is_null() {
        return None;
    }

    unsafe { GetProcAddress(module, function.as_ptr()) }.map(|function_ptr| function_ptr as _)
}

macro_rules! get_function {
    ($lib:expr, $func:ident) => {
        crate::util::get_function_impl(concat!($lib, '\0'), concat!(stringify!($func), '\0'))
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
    dw_style: u32,
    b_menu: BOOL,
    dw_ex_style: u32,
    dpi: u32,
) -> BOOL;

pub type GetPointerFrameInfoHistory = unsafe extern "system" fn(
    pointer_id: u32,
    entries_count: *mut u32,
    pointer_count: *mut u32,
    pointer_info: *mut POINTER_INFO,
) -> BOOL;

pub type SkipPointerFrameMessages = unsafe extern "system" fn(pointer_id: u32) -> BOOL;
pub type GetPointerDeviceRects = unsafe extern "system" fn(
    device: HANDLE,
    pointer_device_rect: *mut RECT,
    display_rect: *mut RECT,
) -> BOOL;

pub type GetPointerTouchInfo =
    unsafe extern "system" fn(pointer_id: u32, touch_info: *mut POINTER_TOUCH_INFO) -> BOOL;
pub type GetPointerPenInfo =
    unsafe extern "system" fn(pointer_id: u32, pen_info: *mut POINTER_PEN_INFO) -> BOOL;

pub type CreateWindowInBand = unsafe extern "system" fn(
    dwexstyle: WINDOW_EX_STYLE,
    lpclassname: PCWSTR,
    lpwindowname: PCWSTR,
    dwstyle: WINDOW_STYLE,
    x: i32,
    y: i32,
    nwidth: i32,
    nheight: i32,
    hwndparent: HWND,
    hmenu: HMENU,
    hinstance: HINSTANCE,
    lpparam: *const c_void,
    dwband: u32,
) -> HWND;

pub(crate) static WIN10_BUILD_VERSION: LazyLock<Option<u32>> = LazyLock::new(|| {
    type RtlGetVersion = unsafe extern "system" fn(*mut OSVERSIONINFOW) -> NTSTATUS;
    let handle = get_function!("ntdll.dll", RtlGetVersion);

    if let Some(rtl_get_version) = handle {
        unsafe {
            let mut vi = OSVERSIONINFOW {
                dwOSVersionInfoSize: 0,
                dwMajorVersion: 0,
                dwMinorVersion: 0,
                dwBuildNumber: 0,
                dwPlatformId: 0,
                szCSDVersion: [0; 128],
            };

            let status = (rtl_get_version)(&mut vi);

            if status >= 0 && vi.dwMajorVersion == 10 && vi.dwMinorVersion == 0 {
                Some(vi.dwBuildNumber)
            } else {
                None
            }
        }
    } else {
        None
    }
});

pub(crate) static GET_DPI_FOR_WINDOW: LazyLock<Option<GetDpiForWindow>> =
    LazyLock::new(|| get_function!("user32.dll", GetDpiForWindow));
pub(crate) static ADJUST_WINDOW_RECT_EX_FOR_DPI: LazyLock<Option<AdjustWindowRectExForDpi>> =
    LazyLock::new(|| get_function!("user32.dll", AdjustWindowRectExForDpi));
pub(crate) static GET_DPI_FOR_MONITOR: LazyLock<Option<GetDpiForMonitor>> =
    LazyLock::new(|| get_function!("shcore.dll", GetDpiForMonitor));
pub(crate) static ENABLE_NON_CLIENT_DPI_SCALING: LazyLock<Option<EnableNonClientDpiScaling>> =
    LazyLock::new(|| get_function!("user32.dll", EnableNonClientDpiScaling));
pub(crate) static SET_PROCESS_DPI_AWARENESS_CONTEXT: LazyLock<
    Option<SetProcessDpiAwarenessContext>,
> = LazyLock::new(|| get_function!("user32.dll", SetProcessDpiAwarenessContext));
pub(crate) static SET_PROCESS_DPI_AWARENESS: LazyLock<Option<SetProcessDpiAwareness>> =
    LazyLock::new(|| get_function!("shcore.dll", SetProcessDpiAwareness));
pub(crate) static SET_PROCESS_DPI_AWARE: LazyLock<Option<SetProcessDPIAware>> =
    LazyLock::new(|| get_function!("user32.dll", SetProcessDPIAware));
pub(crate) static GET_POINTER_FRAME_INFO_HISTORY: LazyLock<Option<GetPointerFrameInfoHistory>> =
    LazyLock::new(|| get_function!("user32.dll", GetPointerFrameInfoHistory));
pub(crate) static SKIP_POINTER_FRAME_MESSAGES: LazyLock<Option<SkipPointerFrameMessages>> =
    LazyLock::new(|| get_function!("user32.dll", SkipPointerFrameMessages));
pub(crate) static GET_POINTER_DEVICE_RECTS: LazyLock<Option<GetPointerDeviceRects>> =
    LazyLock::new(|| get_function!("user32.dll", GetPointerDeviceRects));
pub(crate) static GET_POINTER_TOUCH_INFO: LazyLock<Option<GetPointerTouchInfo>> =
    LazyLock::new(|| get_function!("user32.dll", GetPointerTouchInfo));
pub(crate) static GET_POINTER_PEN_INFO: LazyLock<Option<GetPointerPenInfo>> =
    LazyLock::new(|| get_function!("user32.dll", GetPointerPenInfo));
pub(crate) static CREATE_WINDOW_IN_BAND: LazyLock<Option<CreateWindowInBand>> =
    LazyLock::new(|| get_function!("user32.dll", CreateWindowInBand));

pub(crate) fn wrap_device_id(id: u32) -> DeviceId {
    DeviceId::from_raw(id as i64)
}

#[inline(always)]
pub(crate) const fn get_xbutton_wparam(x: u32) -> u16 {
    hiword(x)
}

#[inline(always)]
pub(crate) const fn get_x_lparam(x: u32) -> i16 {
    loword(x) as _
}

#[inline(always)]
pub(crate) const fn get_y_lparam(x: u32) -> i16 {
    hiword(x) as _
}

#[inline(always)]
pub(crate) const fn primarylangid(lgid: u16) -> u16 {
    lgid & 0x3ff
}

#[inline(always)]
pub(crate) const fn loword(x: u32) -> u16 {
    (x & 0xffff) as u16
}

#[inline(always)]
pub(crate) const fn hiword(x: u32) -> u16 {
    ((x >> 16) & 0xffff) as u16
}

#[inline(always)]
pub(crate) unsafe fn get_window_long(hwnd: HWND, nindex: WINDOW_LONG_PTR_INDEX) -> isize {
    #[cfg(target_pointer_width = "64")]
    return unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(hwnd, nindex) };
    #[cfg(target_pointer_width = "32")]
    return unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::GetWindowLongW(hwnd, nindex) as isize
    };
}

#[inline(always)]
pub(crate) unsafe fn set_window_long(
    hwnd: HWND,
    nindex: WINDOW_LONG_PTR_INDEX,
    dwnewlong: isize,
) -> isize {
    #[cfg(target_pointer_width = "64")]
    return unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW(hwnd, nindex, dwnewlong)
    };
    #[cfg(target_pointer_width = "32")]
    return unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::SetWindowLongW(hwnd, nindex, dwnewlong as i32)
            as isize
    };
}
