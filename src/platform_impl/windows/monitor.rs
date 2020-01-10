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

#[derive(Clone)]
pub struct VideoMode {
    pub(crate) size: (u32, u32),
    pub(crate) bit_depth: u16,
    pub(crate) refresh_rate: u16,
    pub(crate) monitor: MonitorHandle,
    pub(crate) native_video_mode: wingdi::DEVMODEW,
}

impl PartialEq for VideoMode {
    fn eq(&self, other: &Self) -> bool {
        self.size == other.size
            && self.bit_depth == other.bit_depth
            && self.refresh_rate == other.refresh_rate
            && self.monitor == other.monitor
    }
}

impl Eq for VideoMode {}

impl std::hash::Hash for VideoMode {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.size.hash(state);
        self.bit_depth.hash(state);
        self.refresh_rate.hash(state);
        self.monitor.hash(state);
    }
}

impl std::fmt::Debug for VideoMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoMode")
            .field("size", &self.size)
            .field("bit_depth", &self.bit_depth)
            .field("refresh_rate", &self.refresh_rate)
            .field("monitor", &self.monitor)
            .finish()
    }
}

impl VideoMode {
    pub fn size(&self) -> PhysicalSize<u32> {
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
    pub fn size(&self) -> PhysicalSize<u32> {
        let monitor_info = get_monitor_info(self.0).unwrap();
        PhysicalSize {
            width: (monitor_info.rcMonitor.right - monitor_info.rcMonitor.left) as u32,
            height: (monitor_info.rcMonitor.bottom - monitor_info.rcMonitor.top) as u32,
        }
    }

    #[inline]
    pub fn position(&self) -> PhysicalPosition<i32> {
        let monitor_info = get_monitor_info(self.0).unwrap();
        PhysicalPosition {
            x: monitor_info.rcMonitor.left,
            y: monitor_info.rcMonitor.top,
        }
    }

    #[inline]
    pub fn scale_factor(&self) -> f64 {
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
