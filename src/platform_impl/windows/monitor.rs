use winapi::shared::minwindef::{BOOL, DWORD, LPARAM, TRUE, WORD};
use winapi::shared::windef::{HDC, HMONITOR, HWND, LPRECT, POINT};
use winapi::um::winnt::LONG;
use winapi::um::{wingdi, winuser};

use std::collections::{HashSet, VecDeque};
use std::{io, mem, ptr};

use super::{util, EventLoop};
use crate::dpi::{PhysicalPosition, PhysicalSize};
use crate::monitor::VideoMode;
use crate::platform_impl::platform::dpi::{dpi_to_scale_factor, get_monitor_dpi};
use crate::platform_impl::platform::window::Window;

/// Win32 implementation of the main `MonitorHandle` object.
#[derive(Derivative)]
#[derivative(Debug, Clone)]
pub struct MonitorHandle {
    /// Monitor handle.
    hmonitor: HMonitor,
    #[derivative(Debug = "ignore")]
    monitor_info: winuser::MONITORINFOEXW,
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
    let monitors = data as *mut VecDeque<MonitorHandle>;
    (*monitors).push_back(MonitorHandle::from_hmonitor(hmonitor));
    TRUE // continue enumeration
}

pub fn available_monitors() -> VecDeque<MonitorHandle> {
    let mut monitors: VecDeque<MonitorHandle> = VecDeque::new();
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

pub fn primary_monitor() -> MonitorHandle {
    const ORIGIN: POINT = POINT { x: 0, y: 0 };
    let hmonitor = unsafe {
        winuser::MonitorFromPoint(ORIGIN, winuser::MONITOR_DEFAULTTOPRIMARY)
    };
    MonitorHandle::from_hmonitor(hmonitor)
}

pub fn current_monitor(hwnd: HWND) -> MonitorHandle {
    let hmonitor = unsafe {
        winuser::MonitorFromWindow(hwnd, winuser::MONITOR_DEFAULTTONEAREST)
    };
    MonitorHandle::from_hmonitor(hmonitor)
}

impl<T> EventLoop<T> {
    // TODO: Investigate opportunities for caching
    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        available_monitors()
    }

    pub fn primary_monitor(&self) -> MonitorHandle {
        primary_monitor()
    }
}

impl Window {
    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        available_monitors()
    }

    pub fn primary_monitor(&self) -> MonitorHandle {
        primary_monitor()
    }
}

pub(crate) fn get_monitor_info(hmonitor: HMONITOR) -> Result<winuser::MONITORINFOEXW, io::Error> {
    let mut monitor_info: winuser::MONITORINFOEXW = unsafe { mem::uninitialized() };
    monitor_info.cbSize = mem::size_of::<winuser::MONITORINFOEXW>() as DWORD;
    let status = unsafe {
        winuser::GetMonitorInfoW(
            hmonitor,
            &mut monitor_info as *mut winuser::MONITORINFOEXW as *mut winuser::MONITORINFO,
        )
    };
    if status == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(monitor_info)
    }
}

impl MonitorHandle {
    pub(crate) fn from_hmonitor(hmonitor: HMONITOR) -> Self {
        let monitor_info = get_monitor_info(hmonitor).expect("`GetMonitorInfoW` failed");
        let place = monitor_info.rcMonitor;
        let dimensions = (
            (place.right - place.left) as u32,
            (place.bottom - place.top) as u32,
        );
        MonitorHandle {
            hmonitor: HMonitor(hmonitor),
            monitor_name: util::wchar_ptr_to_string(monitor_info.szDevice.as_ptr()),
            primary: util::has_flag(monitor_info.dwFlags, winuser::MONITORINFOF_PRIMARY),
            position: (place.left as i32, place.top as i32),
            dimensions,
            hidpi_factor: dpi_to_scale_factor(get_monitor_dpi(hmonitor).unwrap_or(96)),
            monitor_info,
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
    pub fn name(&self) -> Option<String> {
        Some(self.monitor_name.clone())
    }

    #[inline]
    pub fn native_identifier(&self) -> String {
        self.monitor_name.clone()
    }

    #[inline]
    pub fn hmonitor(&self) -> HMONITOR {
        self.hmonitor.0
    }

    #[inline]
    pub fn size(&self) -> PhysicalSize {
        self.dimensions.into()
    }

    #[inline]
    pub fn position(&self) -> PhysicalPosition {
        self.position.into()
    }

    #[inline]
    pub fn hidpi_factor(&self) -> f64 {
        self.hidpi_factor
    }

    #[inline]
    pub fn video_modes(&self) -> impl Iterator<Item = VideoMode> {
        // EnumDisplaySettingsExW can return duplicate values (or some of the
        // fields are probably changing, but we aren't looking at those fields
        // anyway), so we're using a HashSet deduplicate
        let mut modes = HashSet::new();
        let mut i = 0;

        loop {
            unsafe {
                let device_name = self.monitor_info.szDevice.as_ptr();
                let mut mode: wingdi::DEVMODEW = mem::zeroed();
                mode.dmSize = mem::size_of_val(&mode) as WORD;
                if winuser::EnumDisplaySettingsExW(device_name, i, &mut mode, 0) == 0 {
                    break;
                }
                i += 1;

                const REQUIRED_FIELDS: DWORD = wingdi::DM_BITSPERPEL
                    | wingdi::DM_PELSWIDTH
                    | wingdi::DM_PELSHEIGHT
                    | wingdi::DM_DISPLAYFREQUENCY;
                assert!(mode.dmFields & REQUIRED_FIELDS == REQUIRED_FIELDS);

                modes.insert(VideoMode {
                    size: (mode.dmPelsWidth, mode.dmPelsHeight),
                    bit_depth: mode.dmBitsPerPel as u16,
                    refresh_rate: mode.dmDisplayFrequency as u16,
                });
            }
        }

        modes.into_iter()
    }
}
