use winapi;
use user32;

use std::collections::VecDeque;
use std::{mem, ptr};

use super::EventsLoop;

/// Win32 implementation of the main `MonitorId` object.
#[derive(Clone)]
pub struct MonitorId {
    /// The system name of the adapter.
    adapter_name: [winapi::WCHAR; 32],

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
#[derive(Clone)]
struct HMonitor(winapi::HMONITOR);

unsafe impl Send for HMonitor {}

fn wchar_as_string(wchar: &[winapi::WCHAR]) -> String {
    String::from_utf16_lossy(wchar)
        .trim_right_matches(0 as char)
        .to_string()
}

unsafe extern "system" fn monitor_enum_proc(hmonitor: winapi::HMONITOR, _: winapi::HDC, place: winapi::LPRECT, data: winapi::LPARAM) -> winapi::BOOL {
    let monitors = data as *mut VecDeque<MonitorId>;

    let place = *place;
    let position = (place.left as i32, place.top as i32);
    let dimensions = ((place.right - place.left) as u32, (place.bottom - place.top) as u32);

    let mut monitor_info: winapi::MONITORINFOEXW = mem::zeroed();
    monitor_info.cbSize = mem::size_of::<winapi::MONITORINFOEXW>() as winapi::DWORD;
    if user32::GetMonitorInfoW(hmonitor, &mut monitor_info as *mut winapi::MONITORINFOEXW as *mut winapi::MONITORINFO) == 0 {
        // Some error occurred, just skip this monitor and go on.
        return winapi::TRUE;
    }

    (*monitors).push_back(MonitorId {
        adapter_name: monitor_info.szDevice,
        hmonitor: HMonitor(hmonitor),
        monitor_name: wchar_as_string(&monitor_info.szDevice),
        primary: monitor_info.dwFlags & winapi::MONITORINFOF_PRIMARY != 0,
        position,
        dimensions,
        hidpi_factor: 1.0,
    });

    // TRUE means continue enumeration.
    winapi::TRUE
}

impl EventsLoop {
    pub fn get_available_monitors(&self) -> VecDeque<MonitorId> {
        unsafe {
            let mut result: VecDeque<MonitorId> = VecDeque::new();
            user32::EnumDisplayMonitors(ptr::null_mut(), ptr::null_mut(), Some(monitor_enum_proc), &mut result as *mut _ as winapi::LPARAM);
            result
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
    pub fn get_hmonitor(&self) -> winapi::HMONITOR {
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
    pub fn get_adapter_name(&self) -> &[winapi::WCHAR] {
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
