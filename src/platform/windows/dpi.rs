#![allow(non_camel_case_types, non_snake_case)]
use std::mem;
use std::os::raw::c_void;
use std::sync::{Once, ONCE_INIT};
use winapi;
use user32;
use gdi32;
use kernel32;

type DPI_AWARENESS_CONTEXT = isize;

const DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE: DPI_AWARENESS_CONTEXT = -3;

type SetProcessDPIAware = unsafe extern "system" fn () -> winapi::BOOL;
type SetProcessDpiAwareness = unsafe extern "system" fn (value: winapi::PROCESS_DPI_AWARENESS) -> winapi::HRESULT;
type SetProcessDpiAwarenessContext = unsafe extern "system" fn (value: DPI_AWARENESS_CONTEXT) -> winapi::BOOL;
type GetDpiForWindow = unsafe extern "system" fn (hwnd: winapi::HWND) -> winapi::UINT;
type GetDpiForMonitor = unsafe extern "system" fn (hmonitor: winapi::HMONITOR, dpi_type: winapi::MONITOR_DPI_TYPE, dpi_x: *mut winapi::UINT, dpi_y: *mut winapi::UINT) -> winapi::HRESULT;

// Helper function to dynamically load function pointer.
// `library` and `function` must be zero-terminated.
fn get_function_impl(library: &str, function: &str) -> Option<*const c_void> {
    unsafe {
        // Library names we will use are ASCII so we can use the A version to avoid string conversion.
        let module = kernel32::LoadLibraryA(library.as_ptr() as winapi::LPCSTR);
        if module.is_null() {
            return None;
        }

        let function_ptr = kernel32::GetProcAddress(module, function.as_ptr() as winapi::LPCSTR);
        if function_ptr.is_null() {
            return None;
        }

        Some(function_ptr)
    }
}

macro_rules! get_function {
    ($lib:expr, $func:ident) => {
        get_function_impl(concat!($lib, '\0'), concat!(stringify!($func), '\0')).map(|f| unsafe { mem::transmute::<*const _, $func>(f) })
    }
}

lazy_static! {
    static ref GET_DPI_FOR_WINDOW: Option<GetDpiForWindow> = get_function!("user32.dll", GetDpiForWindow);
    static ref GET_DPI_FOR_MONITOR: Option<GetDpiForMonitor> = get_function!("shcore.dll", GetDpiForMonitor);
}

pub fn become_dpi_aware(enable: bool) {
    if !enable {
        return;
    }

    static ENABLE_DPI_AWARENESS: Once = ONCE_INIT;
    ENABLE_DPI_AWARENESS.call_once(|| {
        unsafe {
            if let Some(SetProcessDpiAwarenessContext) = get_function!("user32.dll", SetProcessDpiAwarenessContext) {
                // We are on Windows 10 Anniversary Update (1607) or later.

                // Note that there is also newer DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2 which will also enable scaling
                // of the window title, but if we use it then glViewort will not work correctly. Until this issue is
                // investigated we are using older DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE.
                SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE);
            } else if let Some(SetProcessDpiAwareness) = get_function!("shcore.dll", SetProcessDpiAwareness) {
                // We are on Windows 8.1 or later.
                SetProcessDpiAwareness(winapi::Process_Per_Monitor_DPI_Aware);
            } else if let Some(SetProcessDPIAware) = get_function!("user32.dll", SetProcessDPIAware) {
                // We are on Vista or later.
                SetProcessDPIAware();
            }
        }
    });
}

pub unsafe fn get_monitor_dpi(hmonitor: winapi::HMONITOR) -> Option<u32> {
    if let Some(GetDpiForMonitor) = *GET_DPI_FOR_MONITOR {
        // We are on Windows 8.1 or later.
        let mut dpi_x = 0;
        let mut dpi_y = 0;
        if GetDpiForMonitor(hmonitor, winapi::MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) == winapi::S_OK {
            // MSDN says that "the values of *dpiX and *dpiY are identical. You only need to record one of the values
            // to determine the DPI and respond appropriately".
            //
            // https://msdn.microsoft.com/en-us/library/windows/desktop/dn280510(v=vs.85).aspx
            return Some(dpi_x as u32)
        }
    }

    None
}

pub unsafe fn get_window_dpi(hwnd: winapi::HWND, hdc: winapi::HDC) -> u32 {
    if let Some(GetDpiForWindow) = *GET_DPI_FOR_WINDOW {
        // We are on Windows 10 Anniversary Update (1607) or later.
        match GetDpiForWindow(hwnd) {
            0 => 96, // 0 is returned if hwnd is invalid
            dpi => dpi as u32,
        }
    } else if let Some(GetDpiForMonitor) = *GET_DPI_FOR_MONITOR {
        // We are on Windows 8.1 or later.
        let monitor = user32::MonitorFromWindow(hwnd, winapi::MONITOR_DEFAULTTONEAREST);
        if monitor.is_null() {
            return 96;
        }

        let mut dpi_x = 0;
        let mut dpi_y = 0;
        if GetDpiForMonitor(monitor, winapi::MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) == winapi::S_OK {
            dpi_x as u32
        } else {
            96
        }
    } else {
        // We are on Vista or later.
        if user32::IsProcessDPIAware() != winapi::FALSE {
            // If the process is DPI aware then scaling is not performed by OS and must be performed by the application.
            // Therefore we return real DPI value.
            gdi32::GetDeviceCaps(hdc, winapi::wingdi::LOGPIXELSX) as u32
        } else {
            // If the process is DPI unaware then scaling is performed by OS and we must return 96 to prevent the application
            // from scaling itself which would lead to double scaling.
            96
        }
    }
}