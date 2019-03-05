use winapi::shared::minwindef::{BOOL, DWORD, LPARAM, TRUE};
use winapi::shared::windef::{HDC, HMONITOR, HWND, LPRECT, POINT};
use winapi::um::winnt::LONG;
use winapi::um::winuser;

use std::{mem, ptr};
use std::collections::VecDeque;

use super::{EventsLoop, util};
use dpi::{PhysicalPosition, PhysicalSize};
use platform::platform::dpi::{dpi_to_scale_factor, get_monitor_dpi};
use platform::platform::window::Window;

/// Win32 implementation of the main `MonitorId` object.
#[derive(Debug, Clone)]
pub struct MonitorId {
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
    /// DPI scale factor.
    hidpi_factor: f64,
}

// Send is not implemented for HMONITOR, we have to wrap it and implement it manually.
// For more info see:
// https://github.com/retep998/winapi-rs/issues/360
// https://github.com/retep998/winapi-rs/issues/396
#[derive(Debug, Clone)]
struct HMonitor(HMONITOR);

unsafe impl Send for HMonitor {}

unsafe extern "system" fn monitor_enum_proc(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _place: LPRECT,
    data: LPARAM,
) -> BOOL {
    let monitors = data as *mut VecDeque<MonitorId>;
    (*monitors).push_back(MonitorId::from_hmonitor(hmonitor));
    TRUE // continue enumeration
}

pub fn get_available_monitors() -> VecDeque<MonitorId> {
    let mut monitors: VecDeque<MonitorId> = VecDeque::new();
    unsafe {
        winuser::EnumDisplayMonitors(
            ptr::null_mut(),
            ptr::null_mut(),
            Some(monitor_enum_proc),
            &mut monitors as *mut _ as LPARAM,
        );
    }
    monitors
}

pub fn get_primary_monitor() -> MonitorId {
    const ORIGIN: POINT = POINT { x: 0, y: 0 };
    let hmonitor = unsafe {
        winuser::MonitorFromPoint(ORIGIN, winuser::MONITOR_DEFAULTTOPRIMARY)
    };
    MonitorId::from_hmonitor(hmonitor)
}

impl EventsLoop {
    // TODO: Investigate opportunities for caching
    pub fn get_available_monitors(&self) -> VecDeque<MonitorId> {
        get_available_monitors()
    }

    pub fn get_current_monitor(hwnd: HWND) -> MonitorId {
        let hmonitor = unsafe {
            winuser::MonitorFromWindow(hwnd, winuser::MONITOR_DEFAULTTONEAREST)
        };
        MonitorId::from_hmonitor(hmonitor)
    }

    pub fn get_primary_monitor(&self) -> MonitorId {
        get_primary_monitor()
    }
}

impl Window {
    pub fn get_available_monitors(&self) -> VecDeque<MonitorId> {
        get_available_monitors()
    }

    pub fn get_primary_monitor(&self) -> MonitorId {
        get_primary_monitor()
    }
}

pub(crate) fn get_monitor_info(hmonitor: HMONITOR) -> Result<winuser::MONITORINFOEXW, util::WinError> {
    let mut monitor_info: winuser::MONITORINFOEXW = unsafe { mem::uninitialized() };
    monitor_info.cbSize = mem::size_of::<winuser::MONITORINFOEXW>() as DWORD;
    let status = unsafe {
        winuser::GetMonitorInfoW(
            hmonitor,
            &mut monitor_info as *mut winuser::MONITORINFOEXW as *mut winuser::MONITORINFO,
        )
    };
    if status == 0 {
        Err(util::WinError::from_last_error())
    } else {
        Ok(monitor_info)
    }
}

impl MonitorId {
    pub(crate) fn from_hmonitor(hmonitor: HMONITOR) -> Self {
        let monitor_info = get_monitor_info(hmonitor).expect("`GetMonitorInfoW` failed");
        let place = monitor_info.rcMonitor;
        let dimensions = (
            (place.right - place.left) as u32,
            (place.bottom - place.top) as u32,
        );
        MonitorId {
            hmonitor: HMonitor(hmonitor),
            monitor_name: util::wchar_ptr_to_string(monitor_info.szDevice.as_ptr()),
            primary: util::has_flag(monitor_info.dwFlags, winuser::MONITORINFOF_PRIMARY),
            position: (place.left as i32, place.top as i32),
            dimensions,
            hidpi_factor: dpi_to_scale_factor(get_monitor_dpi(hmonitor).unwrap_or(96)),
        }
    }

    pub(crate) fn contains_point(&self, point: &POINT) -> bool {
        let left = self.position.0 as LONG;
        let right = left + self.dimensions.0 as LONG;
        let top = self.position.1 as LONG;
        let bottom = top + self.dimensions.1 as LONG;
        point.x >= left && point.x <= right && point.y >= top && point.y <= bottom
    }

    #[inline]
    pub fn get_name(&self) -> Option<String> {
        Some(self.monitor_name.clone())
    }

    #[inline]
    pub fn get_native_identifier(&self) -> String {
        self.monitor_name.clone()
    }

    #[inline]
    pub fn get_hmonitor(&self) -> HMONITOR {
        self.hmonitor.0
    }

    #[inline]
    pub fn get_dimensions(&self) -> PhysicalSize {
        self.dimensions.into()
    }

    #[inline]
    pub fn get_position(&self) -> PhysicalPosition {
        self.position.into()
    }

    #[inline]
    pub fn get_hidpi_factor(&self) -> f64 {
        self.hidpi_factor
    }
}
