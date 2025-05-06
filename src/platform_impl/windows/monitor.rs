use std::collections::{HashSet, VecDeque};
use std::hash::Hash;
use std::num::{NonZeroU16, NonZeroU32};
use std::{io, iter, mem, ptr};

use scopeguard::{guard, ScopeGuard};
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
            ptr::null_mut(),
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

// Send and Sync are not implemented for HMONITOR, we have to wrap it and implement them manually.

unsafe impl Send for MonitorHandle {}
unsafe impl Sync for MonitorHandle {}

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

use std::ffi::OsStr;
use std::mem::zeroed;
use std::os::windows::ffi::OsStrExt;
use std::ptr::null_mut;

use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::Graphics::Gdi::{
    EnumDisplayDevicesW, DISPLAY_DEVICEW, DISPLAY_DEVICE_ACTIVE, DISPLAY_DEVICE_ATTACHED, DMDO_270,
    DMDO_90,
};
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ,
};

fn to_wide(s: &str) -> Vec<u16> {
    let mut v: Vec<u16> = OsStr::new(s).encode_wide().collect();
    v.push(0);
    v
}

unsafe fn get_monitor_device(adapter_name: *const u16) -> Option<DISPLAY_DEVICEW> {
    let mut dd_mon: DISPLAY_DEVICEW = unsafe { zeroed() };
    dd_mon.cb = mem::size_of::<DISPLAY_DEVICEW>() as u32;

    // 1. find ACTIVE + ATTACHED
    let mut idx = 0;
    loop {
        let ok = unsafe { EnumDisplayDevicesW(adapter_name, idx, &mut dd_mon, 0) } != 0;
        if !ok {
            break;
        }
        if (dd_mon.StateFlags & DISPLAY_DEVICE_ACTIVE) != 0
            && (dd_mon.StateFlags & DISPLAY_DEVICE_ATTACHED) != 0
        {
            break;
        }
        idx += 1;
    }

    // 2. fallback to first if no DeviceString
    if dd_mon.DeviceString[0] == 0 {
        let ok = unsafe { EnumDisplayDevicesW(adapter_name, 0, &mut dd_mon, 0) } != 0;
        if !ok || dd_mon.DeviceString[0] == 0 {
            let def = to_wide("Default Monitor");
            dd_mon.DeviceString[..def.len()].copy_from_slice(&def);
        }
    }

    (dd_mon.DeviceID[0] != 0).then_some(dd_mon)
}

unsafe fn read_size_from_edid(dd: &DISPLAY_DEVICEW) -> Option<(u32, u32)> {
    // Parse DeviceID: "DISPLAY\\<model>\\<inst>"
    let id_buf = &dd.DeviceID;
    let len = id_buf.iter().position(|&c| c == 0).unwrap_or(id_buf.len());
    let id_str = String::from_utf16_lossy(&id_buf[..len]);
    let mut parts = id_str.split('\\');
    let _ = parts.next();
    let model = parts.next().unwrap_or("");
    let inst = parts.next().unwrap_or("");
    let model = if model.len() > 7 { &model[..7] } else { model };

    // Open HKLM\SYSTEM\CurrentControlSet\Enum\DISPLAY\<model>
    let base = format!("SYSTEM\\CurrentControlSet\\Enum\\DISPLAY\\{model}");
    let base_w = to_wide(&base);
    let mut hkey: ScopeGuard<HKEY, _> = guard(std::ptr::null_mut(), |v| unsafe {
        RegCloseKey(v);
    });
    if unsafe {
        RegOpenKeyExW(HKEY_LOCAL_MACHINE, base_w.as_ptr(), 0, KEY_READ, &mut *hkey) != ERROR_SUCCESS
    } {
        return None;
    }

    // enumerate instances
    let mut i = 0;
    loop {
        let mut name_buf = [0u16; 128];
        let mut name_len = name_buf.len() as u32;
        let mut ft = unsafe { mem::zeroed() };
        let r = unsafe {
            RegEnumKeyExW(
                *hkey,
                i,
                name_buf.as_mut_ptr(),
                &mut name_len,
                null_mut(),
                null_mut(),
                null_mut(),
                &mut ft,
            )
        };
        if r != ERROR_SUCCESS {
            break;
        }
        i += 1;

        let name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
        let subkey = format!("{base}\\{name}");
        let sub_w = to_wide(&subkey);
        let mut hkey2: ScopeGuard<HKEY, _> = guard(std::ptr::null_mut(), |v| unsafe {
            RegCloseKey(v);
        });
        if unsafe {
            RegOpenKeyExW(HKEY_LOCAL_MACHINE, sub_w.as_ptr(), 0, KEY_READ, &mut *hkey2)
                != ERROR_SUCCESS
        } {
            continue;
        }

        // Check Driver == inst
        let drv_w = to_wide("Driver");
        let mut drv_buf = [0u16; 128];
        let mut drv_len = (drv_buf.len() * 2) as u32;

        if !unsafe {
            RegQueryValueExW(
                *hkey2,
                drv_w.as_ptr(),
                null_mut(),
                null_mut(),
                drv_buf.as_mut_ptr() as *mut u8,
                &mut drv_len,
            ) == ERROR_SUCCESS
        } {
            continue;
        }

        let got = String::from_utf16_lossy(&drv_buf[..(drv_len as usize / 2)]);

        if !got.starts_with(inst) {
            continue;
        }

        // Open Device Parameters
        let params = format!("{subkey}\\Device Parameters");
        let params_w = to_wide(&params);
        let mut hkey3: ScopeGuard<HKEY, _> = guard(std::ptr::null_mut(), |v| unsafe {
            RegCloseKey(v);
        });

        if unsafe {
            RegOpenKeyExW(HKEY_LOCAL_MACHINE, params_w.as_ptr(), 0, KEY_READ, &mut *hkey3)
                != ERROR_SUCCESS
        } {
            continue;
        }

        let edid_w = to_wide("EDID");
        let mut edid = [0u8; 256];
        let mut edid_len = edid.len() as u32;
        if unsafe {
            RegQueryValueExW(
                *hkey3,
                edid_w.as_ptr(),
                null_mut(),
                null_mut(),
                edid.as_mut_ptr(),
                &mut edid_len,
            ) != ERROR_SUCCESS
        } || edid_len < 23
        {
            continue;
        }
        let width_mm: u32;
        let height_mm: u32;

        // We want to have more detailed resolution than centimeters,
        // specifically millimeters. EDID provides Detailed Timing
        // Descriptor (DTD) table which can contain desired
        // size. There can be up to 4 DTDs in EDID, but the first one
        // non-zero-clock DTD is the monitor’s preferred (native)
        // mode, so we try only it.
        const DTD0: usize = 54;

        let pixel_clock = (edid[DTD0] as u16) | ((edid[DTD0 + 1] as u16) << 8);

        if pixel_clock != 0 {
            // For mm precision we need 12-14 bits from Detailed
            // Timing Descriptor
            // https://en.wikipedia.org/wiki/Extended_Display_Identification_Data#Detailed_Timing_Descriptor.
            let h_size_lsb = edid[DTD0 + 12] as u16;
            let v_size_lsb = edid[DTD0 + 13] as u16;
            let size_msb = edid[DTD0 + 14] as u16;
            let h_msb = (size_msb >> 4) & 0x0f;
            let v_msb = size_msb & 0x0f;

            width_mm = ((h_msb << 8) | h_size_lsb) as u32;
            height_mm = ((v_msb << 8) | v_size_lsb) as u32;
        } else {
            let width_cm = edid[21] as u32;
            let height_cm = edid[22] as u32;

            width_mm = width_cm * 10;
            height_mm = height_cm * 10;
        }

        return Some((width_mm, height_mm));
    }

    None
}

fn monitor_physical_size(mi: &MONITORINFOEXW) -> Option<(u32, u32)> {
    unsafe {
        // adapter_name is the wide-string in MONITORINFOEXW.szDevice
        let adapter_name = mi.szDevice.as_ptr();
        // find the matching DISPLAY_DEVICEW
        let dd = get_monitor_device(adapter_name)?;
        // read EDID
        read_size_from_edid(&dd)
    }
}

impl MonitorHandleProvider for MonitorHandle {
    fn id(&self) -> u128 {
        self.native_id() as _
    }

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

    fn physical_size(&self) -> Option<(NonZeroU32, NonZeroU32)> {
        let monitor_info_ex_w = get_monitor_info(self.0).ok()?;

        let mut physical_size = monitor_physical_size(&monitor_info_ex_w)
            .and_then(|(width, height)| Some((NonZeroU32::new(width)?, NonZeroU32::new(height)?)));

        let sz_device = monitor_info_ex_w.szDevice.as_ptr();
        let mut dev_mode_w: DEVMODEW = unsafe { mem::zeroed() };
        dev_mode_w.dmSize = mem::size_of_val(&dev_mode_w) as u16;

        if unsafe {
            EnumDisplaySettingsExW(sz_device, ENUM_CURRENT_SETTINGS, &mut dev_mode_w, 0) == 0
        } {
            return None;
        }

        let display_orientation = unsafe { dev_mode_w.Anonymous1.Anonymous2.dmDisplayOrientation };

        if matches!(display_orientation, DMDO_90 | DMDO_270) {
            physical_size = physical_size.map(|(width, height)| (height, width));
        }

        physical_size
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
