use winapi::{
    shared::{
        minwindef::{BOOL, DWORD, LPARAM, TRUE, WORD},
        windef::{HDC, HMONITOR, HWND, LPRECT, POINT},
    },
    um::{wingdi, winuser},
};

use std::{
    collections::{BTreeSet, VecDeque},
    io, mem, ptr,
};

use super::{util, EventLoop};
use crate::{
    dpi::{PhysicalPosition, PhysicalSize},
    monitor::{MonitorHandle as RootMonitorHandle, VideoMode as RootVideoMode},
    platform_impl::platform::{
        dpi::{dpi_to_scale_factor, get_monitor_dpi},
        window::Window,
    },
};

#[derive(Derivative)]
#[derivative(Debug, Clone, Eq, PartialEq, Hash)]
pub struct VideoMode {
    pub(crate) size: (u32, u32),
    pub(crate) bit_depth: u16,
    pub(crate) refresh_rate: u16,
    pub(crate) monitor: MonitorHandle,
    #[derivative(Debug = "ignore", PartialEq = "ignore", Hash = "ignore")]
    pub(crate) native_video_mode: wingdi::DEVMODEW,
}

impl VideoMode {
    pub fn size(&self) -> PhysicalSize {
        self.size.into()
    }

    pub fn bit_depth(&self) -> u16 {
        self.bit_depth
    }

    pub fn refresh_rate(&self) -> u16 {
        self.refresh_rate
    }

    pub fn monitor(&self) -> RootMonitorHandle {
        RootMonitorHandle {
            inner: self.monitor.clone(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct MonitorHandle(HMONITOR);

// Send is not implemented for HMONITOR, we have to wrap it and implement it manually.
// For more info see:
// https://github.com/retep998/winapi-rs/issues/360
// https://github.com/retep998/winapi-rs/issues/396

unsafe impl Send for MonitorHandle {}

unsafe extern "system" fn monitor_enum_proc(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _place: LPRECT,
    data: LPARAM,
) -> BOOL {
    let monitors = data as *mut VecDeque<MonitorHandle>;
    (*monitors).push_back(MonitorHandle::new(hmonitor));
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
    let hmonitor = unsafe { winuser::MonitorFromPoint(ORIGIN, winuser::MONITOR_DEFAULTTOPRIMARY) };
    MonitorHandle::new(hmonitor)
}

pub fn current_monitor(hwnd: HWND) -> MonitorHandle {
    let hmonitor = unsafe { winuser::MonitorFromWindow(hwnd, winuser::MONITOR_DEFAULTTONEAREST) };
    MonitorHandle::new(hmonitor)
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
    let mut monitor_info: winuser::MONITORINFOEXW = unsafe { mem::zeroed() };
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
    pub(crate) fn new(hmonitor: HMONITOR) -> Self {
        MonitorHandle(hmonitor)
    }

    pub(crate) fn contains_point(&self, point: &POINT) -> bool {
        let monitor_info = get_monitor_info(self.0).unwrap();
        point.x >= monitor_info.rcMonitor.left
            && point.x <= monitor_info.rcMonitor.right
            && point.y >= monitor_info.rcMonitor.top
            && point.y <= monitor_info.rcMonitor.bottom
    }

    #[inline]
    pub fn name(&self) -> Option<String> {
        let monitor_info = get_monitor_info(self.0).unwrap();
        Some(util::wchar_ptr_to_string(monitor_info.szDevice.as_ptr()))
    }

    #[inline]
    pub fn native_identifier(&self) -> String {
        self.name().unwrap()
    }

    #[inline]
    pub fn hmonitor(&self) -> HMONITOR {
        self.0
    }

    #[inline]
    pub fn size(&self) -> PhysicalSize {
        let monitor_info = get_monitor_info(self.0).unwrap();
        PhysicalSize {
            width: (monitor_info.rcMonitor.right - monitor_info.rcMonitor.left) as f64,
            height: (monitor_info.rcMonitor.bottom - monitor_info.rcMonitor.top) as f64,
        }
    }

    #[inline]
    pub fn position(&self) -> PhysicalPosition {
        let monitor_info = get_monitor_info(self.0).unwrap();
        PhysicalPosition {
            x: monitor_info.rcMonitor.left as f64,
            y: monitor_info.rcMonitor.top as f64,
        }
    }

    #[inline]
    pub fn hidpi_factor(&self) -> f64 {
        dpi_to_scale_factor(get_monitor_dpi(self.0).unwrap_or(96))
    }

    #[inline]
    pub fn video_modes(&self) -> impl Iterator<Item = RootVideoMode> {
        // EnumDisplaySettingsExW can return duplicate values (or some of the
        // fields are probably changing, but we aren't looking at those fields
        // anyway), so we're using a BTreeSet deduplicate
        let mut modes = BTreeSet::new();
        let mut i = 0;

        loop {
            unsafe {
                let monitor_info = get_monitor_info(self.0).unwrap();
                let device_name = monitor_info.szDevice.as_ptr();
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

                modes.insert(RootVideoMode {
                    video_mode: VideoMode {
                        size: (mode.dmPelsWidth, mode.dmPelsHeight),
                        bit_depth: mode.dmBitsPerPel as u16,
                        refresh_rate: mode.dmDisplayFrequency as u16,
                        monitor: self.clone(),
                        native_video_mode: mode,
                    },
                });
            }
        }

        modes.into_iter()
    }
}
