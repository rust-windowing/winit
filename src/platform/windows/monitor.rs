use winapi::ctypes::wchar_t;
use winapi::shared::minwindef::{DWORD, LPARAM, BOOL, TRUE, HKEY};
use winapi::shared::windef::{HMONITOR, HDC, LPRECT};
use winapi::um::winuser;
use winapi::um::wingdi::DISPLAY_DEVICEW;

use std::collections::VecDeque;
use std::{mem, ptr};

use super::EventsLoop;

/// Win32 implementation of the main `MonitorId` object.
#[derive(Clone)]
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

    /// The physical extents of the screen, in millimeter
    extents_mm: Option<(u64, u64)>,
}

// Send is not implemented for HMONITOR, we have to wrap it and implement it manually.
// For more info see:
// https://github.com/retep998/winapi-rs/issues/360
// https://github.com/retep998/winapi-rs/issues/396
#[derive(Clone)]
struct HMonitor(HMONITOR);

unsafe impl Send for HMonitor {}

fn wchar_as_string(wchar: &[wchar_t]) -> String {
    String::from_utf16_lossy(wchar)
        .trim_right_matches(0 as char)
        .to_string()
}

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

    let monitor_name = wchar_as_string(&monitor_info.szDevice); // "\\\\.\\DISPLAY1"
    let physical_size = get_physical_size_for_monitor(&monitor_name).and_then(|(w, h)| Some((w as u64, h as u64)));

    (*monitors).push_back(MonitorId {
        adapter_name: monitor_info.szDevice,
        hmonitor: HMonitor(hmonitor),
        monitor_name,
        primary: monitor_info.dwFlags & winuser::MONITORINFOF_PRIMARY != 0,
        position,
        dimensions,
        hidpi_factor: 1.0,
        extents_mm: physical_size,
    });

    // TRUE means continue enumeration.
    TRUE
}

/// Get the physical size of a monitor from the Windows registry
///
/// The `monitor_name` is obtained from the `MONITORINFOEXQ::szDevice` field.
fn get_physical_size_for_monitor(monitor_name: &str) -> Option<(u16, u16)> {

    use winapi::shared::minwindef::HKEY;
    use winapi::shared::winerror::ERROR_NO_MORE_ITEMS;
    use winapi::shared::guiddef::GUID;
    use winapi::um::setupapi::{
        SetupDiGetClassDevsExW, SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInfo,
        SetupDiGetDeviceInstanceIdW, SetupDiOpenDevRegKey, DICS_FLAG_GLOBAL, DIREG_DEV};
    use winapi::um::errhandlingapi::GetLastError;
    use winapi::um::handleapi::INVALID_HANDLE_VALUE;
    use winapi::um::setupapi::{DIGCF_PRESENT, DIGCF_PROFILE, SP_DEVINFO_DATA};
    use winapi::um::winnt::KEY_READ;
    use winapi::um::winreg::RegCloseKey;
    use winapi::um::cfgmgr32::MAX_DEVICE_ID_LEN;

    // Find the display_device with the correct monitor name
    let display_device = get_display_device_from_monitor_name(monitor_name)?;
    let device_id = wchar_as_string(&display_device.DeviceID[..]);
    println!("device id - {:?}, monitor name: {:?}", device_id, monitor_name);
    let device_id = get_2nd_slash_block(&device_id)?;

    // Monitor device class GUID, see:
    // https://github.com/tpn/winsdk-10/blob/master/Include/10.0.10240.0/shared/devguid.h
    // NOTE: For some reason this isn't defined in the winapi crate. TODO: report this
    let guid_devclass_monitor = GUID {
        Data1: 0x4d36e96e,
        Data2: 0xe325,
        Data3: 0x11ce,
        Data4: [0xbf, 0xc1, 0x08, 0x00, 0x2b, 0xe1, 0x03, 0x18],
    };

    // HKEY_LOCAL_MACHINE\SYSTEM\ControlSet001\Enum\DISPLAY
    let device_info = unsafe { SetupDiGetClassDevsExW(
        &guid_devclass_monitor, // device setup class, select all monitors
        ptr::null_mut(),        // enumerator, used to select devices
        ptr::null_mut(),        // parent HWND, NULL
        DIGCF_PRESENT | DIGCF_PROFILE,  // return only devices currently present in the system
        ptr::null_mut(),        // device info, create a new device
        ptr::null_mut(),        // machine name = local machine
        ptr::null_mut()) };     // reserved

    if device_info.is_null() {
        // no need to free the display info
        return None;
    }

    let mut display_size = None;
    let mut i = 0;
    while ERROR_NO_MORE_ITEMS != unsafe { GetLastError() } {
        let mut device_info_data: SP_DEVINFO_DATA = unsafe { mem::zeroed() };
        device_info_data.cbSize = mem::size_of::<SP_DEVINFO_DATA>() as u32;

        if unsafe { SetupDiEnumDeviceInfo(device_info, i, &mut device_info_data) } != 0 {
            let mut device_instance = [0_u16; MAX_DEVICE_ID_LEN];
            unsafe { SetupDiGetDeviceInstanceIdW(device_info, &mut device_info_data, &mut device_instance[0], MAX_DEVICE_ID_LEN as u32, ptr::null_mut()) };
            let device_instance_str = wchar_as_string(&device_instance[..]);
            if !device_instance_str.contains(device_id) {
                continue;
            }

            // open registry key for this device (i.e. monitor)
            let h_device_registry_key = unsafe { SetupDiOpenDevRegKey(
                device_info,
                &mut device_info_data,
                DICS_FLAG_GLOBAL,
                0,
                DIREG_DEV,
                KEY_READ) };

            // well, basically we are comparing void* here ... h_device_registry_key should not be 0 or -1
            if h_device_registry_key.is_null() || h_device_registry_key == INVALID_HANDLE_VALUE as HKEY {
                // no key opened, so we don't need to close the key
                i += 1;
                continue;
            }

            display_size = get_monitor_size_from_edid(h_device_registry_key);
            unsafe {  RegCloseKey(h_device_registry_key) };
        }

        i += 1;
    }

    unsafe { SetupDiDestroyDeviceInfoList(device_info) };

    display_size
}

/// Finds the correct DISPLAY_DEVICE for an HMONITOR
///
/// HMONITOR –> DISPLAY_DEVICE –> DeviceID
fn get_display_device_from_monitor_name(monitor_name: &str) -> Option<DISPLAY_DEVICEW> {
    use winapi::um::winuser::EnumDisplayDevicesW;
    use std::ptr;

    let mut display_device = unsafe { mem::zeroed::<DISPLAY_DEVICEW>() };
    display_device.cb = mem::size_of::<DISPLAY_DEVICEW>() as DWORD;

    let mut device_idx: DWORD = 0;

    while unsafe { EnumDisplayDevicesW(ptr::null_mut(), device_idx, &mut display_device, 0) } != 0 {
        device_idx += 1;
        let display_device_name = wchar_as_string(&display_device.DeviceName);
        println!("get_display_device_from_monitor_name - {:?}, monitor_name: {:?}", display_device_name, monitor_name);
        if display_device_name != monitor_name {
            continue;
        }

        let mut display_device_monitor = unsafe { mem::zeroed::<DISPLAY_DEVICEW>() };
        display_device.cb = mem::size_of::<DISPLAY_DEVICEW>() as DWORD;

        // something is wrong with this thing - ported incorrectly
        if unsafe { EnumDisplayDevicesW(display_device.DeviceName.as_ptr(), 0, &mut display_device_monitor, 0) } != 0 {
            return Some(display_device_monitor);
        }
    }

    None
}

fn str_to_wide_vec_u16(input: &str) -> Vec<u16> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStrExt;
    let mut s: Vec<u16> = OsString::from(input).as_os_str().encode_wide().into_iter().collect();
    s.push(0);
    s
}

// Test two C-based widestrings for equivalence.
// Length of the two arrays is not important
fn widestring_equal(a: &[u16], b: &[u16]) -> bool {
    for (a, b) in a.iter().zip(b.iter()) {
        if *a == 0 || *b == 0 {
            return *a == *b;
        } else {
            if *a != *b {
                return false;
            }
        }
    }
    true
}

#[test]
fn test_compare_c_widestring() {
    let a = str_to_wide_vec_u16("EDID");
    let b = str_to_wide_vec_u16("EDID");
    assert_eq!(widestring_equal(&a, &b), true);
}

/// Windows standars "HORZSIZE" and "VERTSIZE" do not always work correctly
///
/// This solution reads the monitor size from the Windows Registry
/// input: the registry key for the monitor to lookup the size for.
fn get_monitor_size_from_edid(hdev_reg_key: HKEY) -> Option<(u16, u16)> {

    use winapi::shared::winerror::{ERROR_SUCCESS, ERROR_NO_MORE_ITEMS};
    use winapi::um::winreg::RegEnumValueW;
    use winapi::um::winnt::WCHAR;
    use winapi::shared::minwindef::BYTE;

    const NAME_SIZE: DWORD = 128;
    const EDID_DATA_SIZE: DWORD = 1024;

    let edid_id = str_to_wide_vec_u16("EDID");

    let mut dw_type: DWORD = unsafe { mem::uninitialized() };

    let mut value_name: [WCHAR; NAME_SIZE as usize] = unsafe { mem::zeroed() };
    let mut value_name_len = value_name.len() as u32;

    let mut edid_data: [BYTE; EDID_DATA_SIZE as usize] = unsafe { mem::zeroed() };
    let mut edid_data_len = edid_data.len() as u32;

    let mut i = 0;
    let mut ret_value = ERROR_SUCCESS;
    while ret_value != ERROR_NO_MORE_ITEMS {
        ret_value = unsafe { RegEnumValueW(
            hdev_reg_key,
            i,
            &mut value_name[0],
            &mut value_name_len,
            ptr::null_mut(),
            &mut dw_type,
            &mut edid_data[0],
            &mut edid_data_len) } as u32; // windows errors are unsigned anyway

        i += 1;

        if ret_value != ERROR_SUCCESS ||
           value_name[0] == 0 || /* prevent error if value_name is empty */
           !widestring_equal(&value_name, &edid_id) {
            continue;
        }

        let width_mm  = ((edid_data[68] as u16 & 0xF0) << 4) + edid_data[66] as u16;
        let height_mm = ((edid_data[68] as u16 & 0x0F) << 8) + edid_data[67] as u16;
        return Some((width_mm, height_mm))
    }

    None
}

/// input: "MONITOR\GSM4B85\{4d36e96e-e325-11ce-bfc1-08002be10318}\ 0011"
/// output: Some("GSM4B85")
fn get_2nd_slash_block(input: &str) -> Option<&str> {
    let mut iterator = input.splitn(3, "\\");
    iterator.next();
    iterator.next()
}

#[test]
fn test_get_2n_slash_block() {
    assert_eq!(get_2nd_slash_block("MONITOR\\GSM4B85\\{4d36e96e-e325-11ce-bfc1-08002be10318}\\ 0011"), Some("GSM4B85"));
}


impl EventsLoop {
    pub fn get_available_monitors(&self) -> VecDeque<MonitorId> {
        let mut result: VecDeque<MonitorId> = VecDeque::new();
        unsafe {
            use winapi::um::winuser::{GetDC, ReleaseDC};
            let fake_hdc = GetDC(ptr::null_mut());
            winuser::EnumDisplayMonitors(fake_hdc, ptr::null_mut(), Some(monitor_enum_proc), &mut result as *mut _ as LPARAM);
            ReleaseDC(ptr::null_mut(), fake_hdc);
        }
        result
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

    #[inline]
    pub fn get_physical_extents(&self) -> Option<(u64, u64)> {
        self.extents_mm
    }
}
