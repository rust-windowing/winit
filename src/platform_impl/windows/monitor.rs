use winapi::Windows::Win32::{
    Graphics::Gdi::{
        EnumDisplayMonitors, EnumDisplaySettingsExW, GetMonitorInfoW, MonitorFromPoint,
        MonitorFromWindow, DM_BITSPERPEL, DM_DISPLAYFREQUENCY, DM_PELSHEIGHT, DM_PELSWIDTH,
        ENUM_DISPLAY_SETTINGS_MODE, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
        MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTOPRIMARY,
    },
    System::SystemServices::{BOOL, PWSTR},
    UI::{
        DisplayDevices::{DEVMODEW, POINT, RECT},
        WindowsAndMessaging::{HWND, LPARAM},
    },
};

use std::{
    collections::{BTreeSet, VecDeque},
    ffi::OsString,
    io, mem,
    os::windows::prelude::OsStringExt,
    ptr,
};

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
    pub(crate) native_video_mode: DEVMODEW,
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
pub struct MonitorHandle(isize);

unsafe extern "system" fn monitor_enum_proc(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _place: *mut RECT,
    data: LPARAM,
) -> BOOL {
    let monitors = data.0 as *mut VecDeque<MonitorHandle>;
    (*monitors).push_back(MonitorHandle::new(hmonitor));
    true.into() // continue enumeration
}

pub fn available_monitors() -> VecDeque<MonitorHandle> {
    let mut monitors: VecDeque<MonitorHandle> = VecDeque::new();
    unsafe {
        EnumDisplayMonitors(
            None,
            ptr::null_mut(),
            Some(monitor_enum_proc),
            LPARAM(&mut monitors as *mut _ as isize),
        );
    }
    monitors
}

pub fn primary_monitor() -> MonitorHandle {
    const ORIGIN: POINT = POINT { x: 0, y: 0 };
    let hmonitor = unsafe { MonitorFromPoint(ORIGIN, MONITOR_DEFAULTTOPRIMARY) };
    MonitorHandle::new(hmonitor)
}

pub fn current_monitor(hwnd: HWND) -> MonitorHandle {
    let hmonitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    MonitorHandle::new(hmonitor)
}

impl Window {
    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        available_monitors()
    }

    pub fn primary_monitor(&self) -> Option<RootMonitorHandle> {
        let monitor = primary_monitor();
        Some(RootMonitorHandle { inner: monitor })
    }
}

pub(crate) fn get_monitor_info(hmonitor: HMONITOR) -> Result<MONITORINFOEXW, io::Error> {
    let mut monitor_info: MONITORINFOEXW = unsafe { mem::zeroed() };
    monitor_info.__AnonymousBase_winuser_L13558_C43.cbSize =
        mem::size_of::<MONITORINFOEXW>() as u32;
    let status = unsafe {
        GetMonitorInfoW(
            hmonitor,
            &mut monitor_info as *mut MONITORINFOEXW as *mut MONITORINFO,
        )
    };
    if status.as_bool() {
        Ok(monitor_info)
    } else {
        Err(io::Error::last_os_error())
    }
}

impl MonitorHandle {
    pub(crate) fn new(hmonitor: HMONITOR) -> Self {
        MonitorHandle(hmonitor.0)
    }

    #[inline]
    pub fn name(&self) -> Option<String> {
        let monitor_info = get_monitor_info(self.hmonitor()).unwrap();
        unsafe { OsString::from_wide(&monitor_info.szDevice) }
            .into_string()
            .ok()
    }

    #[inline]
    pub fn native_identifier(&self) -> String {
        self.name().unwrap()
    }

    #[inline]
    pub fn hmonitor(&self) -> HMONITOR {
        HMONITOR(self.0)
    }

    #[inline]
    pub fn size(&self) -> PhysicalSize<u32> {
        let monitor_info = get_monitor_info(self.hmonitor()).unwrap();
        PhysicalSize {
            width: (monitor_info
                .__AnonymousBase_winuser_L13558_C43
                .rcMonitor
                .right
                - monitor_info
                    .__AnonymousBase_winuser_L13558_C43
                    .rcMonitor
                    .left) as u32,
            height: (monitor_info
                .__AnonymousBase_winuser_L13558_C43
                .rcMonitor
                .bottom
                - monitor_info
                    .__AnonymousBase_winuser_L13558_C43
                    .rcMonitor
                    .top) as u32,
        }
    }

    #[inline]
    pub fn position(&self) -> PhysicalPosition<i32> {
        let monitor_info = get_monitor_info(self.hmonitor()).unwrap();
        PhysicalPosition {
            x: monitor_info
                .__AnonymousBase_winuser_L13558_C43
                .rcMonitor
                .left,
            y: monitor_info
                .__AnonymousBase_winuser_L13558_C43
                .rcMonitor
                .top,
        }
    }

    #[inline]
    pub fn scale_factor(&self) -> f64 {
        dpi_to_scale_factor(get_monitor_dpi(self.hmonitor()).unwrap_or(96))
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
                let mut monitor_info = get_monitor_info(self.hmonitor()).unwrap();
                let device_name = monitor_info.szDevice.as_mut_ptr();
                let mut mode: DEVMODEW = mem::zeroed();
                mode.dmSize = mem::size_of_val(&mode) as u16;
                if !EnumDisplaySettingsExW(
                    PWSTR(device_name),
                    ENUM_DISPLAY_SETTINGS_MODE(i),
                    &mut mode,
                    0,
                )
                .as_bool()
                {
                    break;
                }
                i += 1;

                const REQUIRED_FIELDS: u32 =
                    (DM_BITSPERPEL | DM_PELSWIDTH | DM_PELSHEIGHT | DM_DISPLAYFREQUENCY) as u32;
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
