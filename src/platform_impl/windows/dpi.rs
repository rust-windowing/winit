#![allow(non_snake_case, unused_unsafe)]

use std::{mem, os::raw::c_void, sync::Once};

use winapi::{
    shared::{
        minwindef::{BOOL, FALSE, UINT},
        windef::{DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE, HMONITOR, HWND},
        winerror::S_OK,
    },
    um::{
        shellscalingapi::{
            MDT_EFFECTIVE_DPI, MONITOR_DPI_TYPE, PROCESS_DPI_AWARENESS,
            PROCESS_PER_MONITOR_DPI_AWARE,
        },
        wingdi::{GetDeviceCaps, LOGPIXELSX},
        winnt::HRESULT,
        winuser::{self, MONITOR_DEFAULTTONEAREST},
    },
};

const DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2: DPI_AWARENESS_CONTEXT = -4isize as _;

type SetProcessDPIAware = unsafe extern "system" fn() -> BOOL;
type SetProcessDpiAwareness = unsafe extern "system" fn(value: PROCESS_DPI_AWARENESS) -> HRESULT;
type SetProcessDpiAwarenessContext =
    unsafe extern "system" fn(value: DPI_AWARENESS_CONTEXT) -> BOOL;
type GetDpiForWindow = unsafe extern "system" fn(hwnd: HWND) -> UINT;
type GetDpiForMonitor = unsafe extern "system" fn(
    hmonitor: HMONITOR,
    dpi_type: MONITOR_DPI_TYPE,
    dpi_x: *mut UINT,
    dpi_y: *mut UINT,
) -> HRESULT;
type EnableNonClientDpiScaling = unsafe extern "system" fn(hwnd: HWND) -> BOOL;

lazy_static! {
    static ref GET_DPI_FOR_WINDOW: Option<GetDpiForWindow> =
        get_function!("user32.dll", GetDpiForWindow);
    static ref GET_DPI_FOR_MONITOR: Option<GetDpiForMonitor> =
        get_function!("shcore.dll", GetDpiForMonitor);
    static ref ENABLE_NON_CLIENT_DPI_SCALING: Option<EnableNonClientDpiScaling> =
        get_function!("user32.dll", EnableNonClientDpiScaling);
}

pub fn become_dpi_aware(enable: bool) {
    if !enable {
        return;
    }
    static ENABLE_DPI_AWARENESS: Once = Once::new();
    ENABLE_DPI_AWARENESS.call_once(|| {
        unsafe {
            if let Some(SetProcessDpiAwarenessContext) =
                get_function!("user32.dll", SetProcessDpiAwarenessContext)
            {
                // We are on Windows 10 Anniversary Update (1607) or later.
                if SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
                    == FALSE
                {
                    // V2 only works with Windows 10 Creators Update (1703). Try using the older
                    // V1 if we can't set V2.
                    SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE);
                }
            } else if let Some(SetProcessDpiAwareness) =
                get_function!("shcore.dll", SetProcessDpiAwareness)
            {
                // We are on Windows 8.1 or later.
                SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE);
            } else if let Some(SetProcessDPIAware) = get_function!("user32.dll", SetProcessDPIAware)
            {
                // We are on Vista or later.
                SetProcessDPIAware();
            }
        }
    });
}

pub fn enable_non_client_dpi_scaling(hwnd: HWND) {
    unsafe {
        if let Some(EnableNonClientDpiScaling) = *ENABLE_NON_CLIENT_DPI_SCALING {
            EnableNonClientDpiScaling(hwnd);
        }
    }
}

pub fn get_monitor_dpi(hmonitor: HMONITOR) -> Option<u32> {
    unsafe {
        if let Some(GetDpiForMonitor) = *GET_DPI_FOR_MONITOR {
            // We are on Windows 8.1 or later.
            let mut dpi_x = 0;
            let mut dpi_y = 0;
            if GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) == S_OK {
                // MSDN says that "the values of *dpiX and *dpiY are identical. You only need to
                // record one of the values to determine the DPI and respond appropriately".
                // https://msdn.microsoft.com/en-us/library/windows/desktop/dn280510(v=vs.85).aspx
                return Some(dpi_x as u32);
            }
        }
    }
    None
}

pub const BASE_DPI: u32 = 96;
pub fn dpi_to_scale_factor(dpi: u32) -> f64 {
    dpi as f64 / BASE_DPI as f64
}

pub unsafe fn hwnd_dpi(hwnd: HWND) -> u32 {
    let hdc = winuser::GetDC(hwnd);
    if hdc.is_null() {
        panic!("[winit] `GetDC` returned null!");
    }
    if let Some(GetDpiForWindow) = *GET_DPI_FOR_WINDOW {
        // We are on Windows 10 Anniversary Update (1607) or later.
        match GetDpiForWindow(hwnd) {
            0 => BASE_DPI, // 0 is returned if hwnd is invalid
            dpi => dpi as u32,
        }
    } else if let Some(GetDpiForMonitor) = *GET_DPI_FOR_MONITOR {
        // We are on Windows 8.1 or later.
        let monitor = winuser::MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        if monitor.is_null() {
            return BASE_DPI;
        }

        let mut dpi_x = 0;
        let mut dpi_y = 0;
        if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) == S_OK {
            dpi_x as u32
        } else {
            BASE_DPI
        }
    } else {
        // We are on Vista or later.
        if winuser::IsProcessDPIAware() != FALSE {
            // If the process is DPI aware, then scaling must be handled by the application using
            // this DPI value.
            GetDeviceCaps(hdc, LOGPIXELSX) as u32
        } else {
            // If the process is DPI unaware, then scaling is performed by the OS; we thus return
            // 96 (scale factor 1.0) to prevent the window from being re-scaled by both the
            // application and the WM.
            BASE_DPI
        }
    }
}

pub fn hwnd_scale_factor(hwnd: HWND) -> f64 {
    dpi_to_scale_factor(unsafe { hwnd_dpi(hwnd) })
}
