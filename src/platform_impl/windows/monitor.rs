use std::collections::{HashSet, VecDeque};
use std::hash::Hash;
use std::num::{NonZeroU16, NonZeroU32};
use std::{io, iter, mem, ptr};

use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM, POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, EnumDisplaySettingsExW, GetMonitorInfoW, MonitorFromPoint,
    MonitorFromWindow, DEVMODEW, DM_BITSPERPEL, DM_DISPLAYFREQUENCY, DM_PELSHEIGHT, DM_PELSWIDTH,
    ENUM_CURRENT_SETTINGS, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW, MONITOR_DEFAULTTONEAREST,
    MONITOR_DEFAULTTOPRIMARY,
};

use super::util::decode_wide;
use crate::dpi::{PhysicalPosition, PhysicalSize};
use crate::monitor::{MonitorHandleProvider, VideoMode};
use crate::platform_impl::platform::dpi::{dpi_to_scale_factor, get_monitor_dpi};
use crate::platform_impl::platform::util::has_flag;

#[derive(Clone)]
pub struct VideoModeHandle {
    pub(crate) mode: VideoMode,
    // DEVMODEW is huge so we box it to avoid blowing up the size of winit::window::Fullscreen
    pub(crate) native_video_mode: Box<DEVMODEW>,
}

impl PartialEq for VideoModeHandle {
    fn eq(&self, other: &Self) -> bool {
        self.mode == other.mode
    }
}

impl Eq for VideoModeHandle {}

impl std::hash::Hash for VideoModeHandle {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.mode.hash(state);
    }
}

impl std::fmt::Debug for VideoModeHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoMode").field("mode", &self.mode).finish()
    }
}

impl VideoModeHandle {
    fn new(native_video_mode: DEVMODEW) -> Self {
        const REQUIRED_FIELDS: u32 =
            DM_BITSPERPEL | DM_PELSWIDTH | DM_PELSHEIGHT | DM_DISPLAYFREQUENCY;
        assert!(has_flag(native_video_mode.dmFields, REQUIRED_FIELDS));

        let mode = VideoMode {
            size: (native_video_mode.dmPelsWidth, native_video_mode.dmPelsHeight).into(),
            bit_depth: NonZeroU16::new(native_video_mode.dmBitsPerPel as u16),
            refresh_rate_millihertz: NonZeroU32::new(native_video_mode.dmDisplayFrequency * 1000),
        };

        VideoModeHandle { mode, native_video_mode: Box::new(native_video_mode) }
    }
}

unsafe extern "system" fn monitor_enum_proc(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _place: *mut RECT,
    data: LPARAM,
) -> BOOL {
    let monitors = data as *mut VecDeque<MonitorHandle>;
    unsafe { (*monitors).push_back(MonitorHandle::new(hmonitor)) };
    true.into() // continue enumeration
}

pub fn available_monitors() -> VecDeque<MonitorHandle> {
    let mut monitors: VecDeque<MonitorHandle> = VecDeque::new();
    unsafe {
        EnumDisplayMonitors(
            0,
            ptr::null(),
            Some(monitor_enum_proc),
            &mut monitors as *mut _ as LPARAM,
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

pub(crate) fn get_monitor_info(hmonitor: HMONITOR) -> Result<MONITORINFOEXW, io::Error> {
    let mut monitor_info: MONITORINFOEXW = unsafe { mem::zeroed() };
    monitor_info.monitorInfo.cbSize = mem::size_of::<MONITORINFOEXW>() as u32;
    let status = unsafe {
        GetMonitorInfoW(hmonitor, &mut monitor_info as *mut MONITORINFOEXW as *mut MONITORINFO)
    };
    if status == false.into() {
        Err(io::Error::last_os_error())
    } else {
        Ok(monitor_info)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct MonitorHandle(HMONITOR);

// Send is not implemented for HMONITOR, we have to wrap it and implement it manually.
// For more info see:
// https://github.com/retep998/winapi-rs/issues/360
// https://github.com/retep998/winapi-rs/issues/396

unsafe impl Send for MonitorHandle {}

impl MonitorHandle {
    pub(crate) fn new(hmonitor: HMONITOR) -> Self {
        MonitorHandle(hmonitor)
    }

    pub(crate) fn size(&self) -> PhysicalSize<u32> {
        let rc_monitor = get_monitor_info(self.0).unwrap().monitorInfo.rcMonitor;
        PhysicalSize {
            width: (rc_monitor.right - rc_monitor.left) as u32,
            height: (rc_monitor.bottom - rc_monitor.top) as u32,
        }
    }

    pub(crate) fn video_mode_handles(&self) -> Box<dyn Iterator<Item = VideoModeHandle>> {
        // EnumDisplaySettingsExW can return duplicate values (or some of the
        // fields are probably changing, but we aren't looking at those fields
        // anyway), so we're using a BTreeSet deduplicate
        let mut modes = HashSet::<VideoModeHandle>::new();

        let monitor_info = match get_monitor_info(self.0) {
            Ok(monitor_info) => monitor_info,
            Err(error) => {
                tracing::warn!("Error from get_monitor_info: {error}");
                return Box::new(iter::empty());
            },
        };

        let device_name = monitor_info.szDevice.as_ptr();
        let mut i = 0;
        loop {
            let mut mode: DEVMODEW = unsafe { mem::zeroed() };
            mode.dmSize = mem::size_of_val(&mode) as u16;
            if unsafe { EnumDisplaySettingsExW(device_name, i, &mut mode, 0) } == false.into() {
                break;
            }

            // Use Ord impl of RootVideoModeHandle
            modes.insert(VideoModeHandle::new(mode));

            i += 1;
        }

        Box::new(modes.into_iter())
    }
}

impl MonitorHandleProvider for MonitorHandle {
    fn native_id(&self) -> u64 {
        self.0 as _
    }

    fn name(&self) -> Option<std::borrow::Cow<'_, str>> {
        let monitor_info = get_monitor_info(self.0).unwrap();
        Some(decode_wide(&monitor_info.szDevice).to_string_lossy().to_string().into())
    }

    fn position(&self) -> Option<PhysicalPosition<i32>> {
        get_monitor_info(self.0)
            .map(|info| {
                let rc_monitor = info.monitorInfo.rcMonitor;
                PhysicalPosition { x: rc_monitor.left, y: rc_monitor.top }
            })
            .ok()
    }

    fn scale_factor(&self) -> f64 {
        dpi_to_scale_factor(get_monitor_dpi(self.0).unwrap_or(96))
    }

    fn current_video_mode(&self) -> Option<crate::monitor::VideoMode> {
        let monitor_info = get_monitor_info(self.0).ok()?;
        let device_name = monitor_info.szDevice.as_ptr();
        unsafe {
            let mut mode: DEVMODEW = mem::zeroed();
            mode.dmSize = mem::size_of_val(&mode) as u16;
            if EnumDisplaySettingsExW(device_name, ENUM_CURRENT_SETTINGS, &mut mode, 0)
                == false.into()
            {
                None
            } else {
                Some(VideoModeHandle::new(mode).mode)
            }
        }
    }

    fn video_modes(&self) -> Box<dyn Iterator<Item = VideoMode>> {
        Box::new(self.video_mode_handles().map(|mode| mode.mode))
    }
}
