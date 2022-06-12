use std::{
    ffi::{c_void, OsStr, OsString},
    io,
    iter::once,
    mem,
    ops::BitAnd,
    os::windows::prelude::{OsStrExt, OsStringExt},
    ptr,
    sync::atomic::{AtomicBool, Ordering},
};

use once_cell::sync::Lazy;
use windows_sys::{
    core::{HRESULT, PCWSTR},
    Win32::{
        Foundation::{BOOL, HINSTANCE, HWND, RECT},
        Graphics::Gdi::{ClientToScreen, InvalidateRgn, HMONITOR},
        System::{
            LibraryLoader::{GetProcAddress, LoadLibraryA},
            SystemServices::IMAGE_DOS_HEADER,
        },
        UI::{
            HiDpi::{DPI_AWARENESS_CONTEXT, MONITOR_DPI_TYPE, PROCESS_DPI_AWARENESS},
            Input::KeyboardAndMouse::GetActiveWindow,
            WindowsAndMessaging::{
                AdjustWindowRectEx, ClipCursor, GetClientRect, GetClipCursor, GetMenu,
                GetSystemMetrics, GetWindowPlacement, GetWindowRect, SetWindowPos, ShowCursor,
                GWL_EXSTYLE, GWL_STYLE, IDC_APPSTARTING, IDC_ARROW, IDC_CROSS, IDC_HAND, IDC_HELP,
                IDC_IBEAM, IDC_NO, IDC_SIZEALL, IDC_SIZENESW, IDC_SIZENS, IDC_SIZENWSE, IDC_SIZEWE,
                IDC_WAIT, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
                SM_YVIRTUALSCREEN, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOMOVE,
                SWP_NOREPOSITION, SWP_NOZORDER, SW_MAXIMIZE, WINDOWPLACEMENT, WINDOW_EX_STYLE,
                WINDOW_STYLE, WS_CAPTION, WS_SIZEBOX,
            },
        },
    },
};

use crate::{dpi::PhysicalSize, platform_impl::platform::get_window_long, window::CursorIcon};

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

pub unsafe fn status_map<T, F: FnMut(&mut T) -> BOOL>(mut fun: F) -> Option<T> {
    let mut data: T = mem::zeroed();
    if fun(&mut data) != false.into() {
        Some(data)
    } else {
        None
    }
}

fn win_to_err<F: FnOnce() -> BOOL>(f: F) -> Result<(), io::Error> {
    if f() != false.into() {
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

pub fn adjust_size(hwnd: HWND, size: PhysicalSize<u32>, is_decorated: bool) -> PhysicalSize<u32> {
    let (width, height): (u32, u32) = size.into();
    let rect = RECT {
        left: 0,
        right: width as i32,
        top: 0,
        bottom: height as i32,
    };
    let rect = adjust_window_rect(hwnd, rect, is_decorated).unwrap_or(rect);
    PhysicalSize::new((rect.right - rect.left) as _, (rect.bottom - rect.top) as _)
}

pub(crate) fn set_inner_size_physical(window: HWND, x: u32, y: u32, is_decorated: bool) {
    unsafe {
        let rect = adjust_window_rect(
            window,
            RECT {
                top: 0,
                left: 0,
                bottom: y as i32,
                right: x as i32,
            },
            is_decorated,
        )
        .expect("adjust_window_rect failed");

        let outer_x = (rect.right - rect.left).abs() as _;
        let outer_y = (rect.top - rect.bottom).abs() as _;
        SetWindowPos(
            window,
            0,
            0,
            0,
            outer_x,
            outer_y,
            SWP_ASYNCWINDOWPOS | SWP_NOZORDER | SWP_NOREPOSITION | SWP_NOMOVE | SWP_NOACTIVATE,
        );
        InvalidateRgn(window, 0, false.into());
    }
}

pub fn adjust_window_rect(hwnd: HWND, rect: RECT, is_decorated: bool) -> Option<RECT> {
    unsafe {
        let mut style = get_window_long(hwnd, GWL_STYLE) as u32;
        // if the window isn't decorated, remove `WS_SIZEBOX` and `WS_CAPTION` so
        // `AdjustWindowRect*` functions doesn't account for the hidden caption and borders and
        // calculates a correct size for the client area.
        if !is_decorated {
            style &= !(WS_CAPTION | WS_SIZEBOX);
        }
        let style_ex = get_window_long(hwnd, GWL_EXSTYLE) as u32;
        adjust_window_rect_with_styles(hwnd, style, style_ex, rect)
    }
}

pub fn adjust_window_rect_with_styles(
    hwnd: HWND,
    style: WINDOW_STYLE,
    style_ex: WINDOW_EX_STYLE,
    rect: RECT,
) -> Option<RECT> {
    unsafe {
        status_map(|r| {
            *r = rect;

            let b_menu = GetMenu(hwnd) != 0;
            if let (Some(get_dpi_for_window), Some(adjust_window_rect_ex_for_dpi)) =
                (*GET_DPI_FOR_WINDOW, *ADJUST_WINDOW_RECT_EX_FOR_DPI)
            {
                let dpi = get_dpi_for_window(hwnd);
                adjust_window_rect_ex_for_dpi(r, style, b_menu.into(), style_ex, dpi)
            } else {
                AdjustWindowRectEx(r, style, b_menu.into(), style_ex)
            }
        })
    }
}

pub fn set_cursor_hidden(hidden: bool) {
    static HIDDEN: AtomicBool = AtomicBool::new(false);
    let changed = HIDDEN.swap(hidden, Ordering::SeqCst) ^ hidden;
    if changed {
        unsafe { ShowCursor(BOOL::from(!hidden)) };
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

pub fn is_maximized(window: HWND) -> bool {
    unsafe {
        let mut placement: WINDOWPLACEMENT = mem::zeroed();
        placement.length = mem::size_of::<WINDOWPLACEMENT>() as u32;
        GetWindowPlacement(window, &mut placement);
        placement.showCmd == SW_MAXIMIZE
    }
}

// pub fn hwnd_decoration_thickness(hwnd: HWND, border_only: bool) -> RECT {
//     unsafe {
//         let style = get_window_long(hwnd, GWL_STYLE) as u32;
//         let style_ex = get_window_long(hwnd, GWL_EXSTYLE) as u32;

//         let adjust_style = if !border_only {
//             style
//         } else {
//             style & !WS_CAPTION
//         };
//         let mut decoration_thickness = RECT {
//             left: 0,
//             top: 0,
//             right: 0,
//             bottom: 0,
//         };
//         if has_flag(style, WS_SIZEBOX) {
//             #[allow(non_snake_case)]
//             if let Some(AdjustWindowRectExForDpi) = *ADJUST_WINDOW_RECT_EX_FOR_DPI {
//                 AdjustWindowRectExForDpi(
//                     &mut decoration_thickness,
//                     adjust_style,
//                     false as _,
//                     style_ex,
//                     hwnd_dpi(hwnd),
//                 );
//             } else {
//                 AdjustWindowRectEx(
//                     &mut decoration_thickness,
//                     adjust_style,
//                     false as _,
//                     style_ex,
//                 );
//             }
//             decoration_thickness.left *= -1;
//             decoration_thickness.top *= -1;
//         } else if has_flag(style, WS_BORDER) {
//             decoration_thickness = RECT {
//                 left: 1,
//                 top: 1,
//                 right: 1,
//                 bottom: 1,
//             };
//         }
//         decoration_thickness
//     }
// }

pub fn get_instance_handle() -> HINSTANCE {
    // Gets the instance handle by taking the address of the
    // pseudo-variable created by the microsoft linker:
    // https://devblogs.microsoft.com/oldnewthing/20041025-00/?p=37483

    // This is preferred over GetModuleHandle(NULL) because it also works in DLLs:
    // https://stackoverflow.com/questions/21718027/getmodulehandlenull-vs-hinstance

    extern "C" {
        static __ImageBase: IMAGE_DOS_HEADER;
    }

    unsafe { &__ImageBase as *const _ as _ }
}

impl CursorIcon {
    pub(crate) fn to_windows_cursor(self) -> PCWSTR {
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
pub(super) fn get_function_impl(library: &str, function: &str) -> Option<*const c_void> {
    assert_eq!(library.chars().last(), Some('\0'));
    assert_eq!(function.chars().last(), Some('\0'));

    // Library names we will use are ASCII so we can use the A version to avoid string conversion.
    let module = unsafe { LoadLibraryA(library.as_ptr()) };
    if module == 0 {
        return None;
    }

    unsafe { GetProcAddress(module, function.as_ptr()) }.map(|function_ptr| function_ptr as _)
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

pub static GET_DPI_FOR_WINDOW: Lazy<Option<GetDpiForWindow>> =
    Lazy::new(|| get_function!("user32.dll", GetDpiForWindow));
pub static ADJUST_WINDOW_RECT_EX_FOR_DPI: Lazy<Option<AdjustWindowRectExForDpi>> =
    Lazy::new(|| get_function!("user32.dll", AdjustWindowRectExForDpi));
pub static GET_DPI_FOR_MONITOR: Lazy<Option<GetDpiForMonitor>> =
    Lazy::new(|| get_function!("shcore.dll", GetDpiForMonitor));
pub static ENABLE_NON_CLIENT_DPI_SCALING: Lazy<Option<EnableNonClientDpiScaling>> =
    Lazy::new(|| get_function!("user32.dll", EnableNonClientDpiScaling));
pub static SET_PROCESS_DPI_AWARENESS_CONTEXT: Lazy<Option<SetProcessDpiAwarenessContext>> =
    Lazy::new(|| get_function!("user32.dll", SetProcessDpiAwarenessContext));
pub static SET_PROCESS_DPI_AWARENESS: Lazy<Option<SetProcessDpiAwareness>> =
    Lazy::new(|| get_function!("shcore.dll", SetProcessDpiAwareness));
pub static SET_PROCESS_DPI_AWARE: Lazy<Option<SetProcessDPIAware>> =
    Lazy::new(|| get_function!("user32.dll", SetProcessDPIAware));
