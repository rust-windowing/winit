use winapi::ctypes::wchar_t;
use winapi::shared::minwindef::{DWORD, LPARAM, BOOL, TRUE};
use winapi::shared::windef::{HMONITOR, HDC, LPRECT, HWND};
use winapi::um::winuser;

use std::collections::VecDeque;
use std::{mem, ptr};

use super::{EventsLoop, util};

/// Win32 implementation of the main `MonitorId` object.
#[derive(Debug, Clone)]
pub struct MonitorId {
    /// The system name of the adapter.
    adapter_name: [wchar_t; 32],

    /// Monitor handle.
    hmonitor: HMonitor,

    /// The system name of the monitor.
    monitor_name: String,

    /// True if this is the primary monitor.
    primary: bool,

    /// The position of the monitor in pixels on the desktop.
    ///
    /// A window that is positioned at these coordinates will overlap the monitor.
    position: (i32, i32),

    /// The current resolution in pixels on the monitor.
    dimensions: (u32, u32),

    /// DPI scaling factor.
    hidpi_factor: f32,
}

// Send is not implemented for HMONITOR, we have to wrap it and implement it manually.
// For more info see:
// https://github.com/retep998/winapi-rs/issues/360
// https://github.com/retep998/winapi-rs/issues/396
#[derive(Debug, Clone)]
struct HMonitor(HMONITOR);

unsafe impl Send for HMonitor {}

unsafe extern "system" fn monitor_enum_proc(hmonitor: HMONITOR, _: HDC, place: LPRECT, data: LPARAM) -> BOOL {
    let monitors = data as *mut VecDeque<MonitorId>;

    let place = *place;
    let position = (place.left as i32, place.top as i32);
    let dimensions = ((place.right - place.left) as u32, (place.bottom - place.top) as u32);

    let mut monitor_info: winuser::MONITORINFOEXW = mem::zeroed();
    monitor_info.cbSize = mem::size_of::<winuser::MONITORINFOEXW>() as DWORD;
    if winuser::GetMonitorInfoW(hmonitor, &mut monitor_info as *mut winuser::MONITORINFOEXW as *mut winuser::MONITORINFO) == 0 {
        // Some error occurred, just skip this monitor and go on.
        return TRUE;
    }

    (*monitors).push_back(MonitorId {
        adapter_name: monitor_info.szDevice,
        hmonitor: HMonitor(hmonitor),
        monitor_name: util::wchar_to_string(&monitor_info.szDevice),
        primary: monitor_info.dwFlags & winuser::MONITORINFOF_PRIMARY != 0,
        position,
        dimensions,
        hidpi_factor: 1.0,
    });

    // TRUE means continue enumeration.
    TRUE
}

impl EventsLoop {
    pub fn get_available_monitors(&self) -> VecDeque<MonitorId> {
        unsafe {
            let mut result: VecDeque<MonitorId> = VecDeque::new();
            winuser::EnumDisplayMonitors(ptr::null_mut(), ptr::null_mut(), Some(monitor_enum_proc), &mut result as *mut _ as LPARAM);
            result
        }
    }

    pub fn get_current_monitor(handle: HWND) -> MonitorId {
        unsafe {
            let mut monitor_info: winuser::MONITORINFOEXW = mem::zeroed();
            monitor_info.cbSize = mem::size_of::<winuser::MONITORINFOEXW>() as DWORD;

            let hmonitor = winuser::MonitorFromWindow(handle, winuser::MONITOR_DEFAULTTONEAREST);

            winuser::GetMonitorInfoW(
                hmonitor,
                &mut monitor_info as *mut winuser::MONITORINFOEXW as *mut winuser::MONITORINFO,
            );

            let place = monitor_info.rcMonitor;
            let position = (place.left as i32, place.top as i32);
            let dimensions = (
                (place.right - place.left) as u32,
                (place.bottom - place.top) as u32,
            );

            MonitorId {
                adapter_name: monitor_info.szDevice,
                hmonitor: super::monitor::HMonitor(hmonitor),
                monitor_name: util::wchar_to_string(&monitor_info.szDevice),
                primary: monitor_info.dwFlags & winuser::MONITORINFOF_PRIMARY != 0,
                position,
                dimensions,
                hidpi_factor: 1.0,
            }
        }
    }

    pub fn get_primary_monitor(&self) -> MonitorId {
        // we simply get all available monitors and return the one with the `MONITORINFOF_PRIMARY` flag
        // TODO: it is possible to query the win32 API for the primary monitor, this should be done
        //  instead
        for monitor in self.get_available_monitors().into_iter() {
            if monitor.primary {
                return monitor;
            }
        }

        panic!("Failed to find the primary monitor")
    }
}

impl MonitorId {
    /// See the docs if the crate root file.
    #[inline]
    pub fn get_name(&self) -> Option<String> {
        Some(self.monitor_name.clone())
    }

    /// See the docs of the crate root file.
    #[inline]
    pub fn get_native_identifier(&self) -> String {
        self.monitor_name.clone()
    }

    /// See the docs of the crate root file.
    #[inline]
    pub fn get_hmonitor(&self) -> HMONITOR {
        self.hmonitor.0
    }

    /// See the docs of the crate root file.
    #[inline]
    pub fn get_dimensions(&self) -> (u32, u32) {
        // TODO: retrieve the dimensions every time this is called
        self.dimensions
    }

    /// This is a Win32-only function for `MonitorId` that returns the system name of the adapter
    /// device.
    #[inline]
    pub fn get_adapter_name(&self) -> &[wchar_t] {
        &self.adapter_name
    }

    /// A window that is positioned at these coordinates will overlap the monitor.
    #[inline]
    pub fn get_position(&self) -> (i32, i32) {
        self.position
    }

    #[inline]
    pub fn get_hidpi_factor(&self) -> f32 {
        self.hidpi_factor
    }
}
