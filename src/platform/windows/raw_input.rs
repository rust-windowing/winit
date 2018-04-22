use std::mem::{self, size_of};
use std::ptr;

use winapi::ctypes::wchar_t;
use winapi::shared::minwindef::{UINT, USHORT, TRUE};
use winapi::shared::hidusage::{
    HID_USAGE_PAGE_GENERIC,
    HID_USAGE_GENERIC_MOUSE,
    HID_USAGE_GENERIC_KEYBOARD,
};
use winapi::shared::windef::HWND;
use winapi::um::winnt::HANDLE;
use winapi::um::winuser::{
    self,
    RAWINPUTDEVICELIST,
    RID_DEVICE_INFO,
    RID_DEVICE_INFO_MOUSE,
    RID_DEVICE_INFO_KEYBOARD,
    RID_DEVICE_INFO_HID,
    RIM_TYPEMOUSE,
    RIM_TYPEKEYBOARD,
    RIM_TYPEHID,
    RIDI_DEVICEINFO,
    RIDI_DEVICENAME,
    RAWINPUTDEVICE,
    RIDEV_DEVNOTIFY,
    RIDEV_INPUTSINK,
    HRAWINPUT,
    RAWINPUT,
    RAWINPUTHEADER,
    RID_INPUT,
};

use platform::platform::util;
use events::ElementState;

#[allow(dead_code)]
pub fn get_raw_input_device_list() -> Option<Vec<RAWINPUTDEVICELIST>> {
    let list_size = size_of::<RAWINPUTDEVICELIST>() as UINT;

    let mut num_devices = 0;
    let status = unsafe { winuser::GetRawInputDeviceList(
        ptr::null_mut(),
        &mut num_devices,
        list_size,
    ) };

    if status == UINT::max_value() {
        return None;
    }

    let mut buffer = Vec::with_capacity(num_devices as _);

    let num_stored = unsafe { winuser::GetRawInputDeviceList(
        buffer.as_ptr() as _,
        &mut num_devices,
        list_size,
    ) };

    if num_stored == UINT::max_value() {
        return None;
    }

    debug_assert_eq!(num_devices, num_stored);

    unsafe { buffer.set_len(num_devices as _) };

    Some(buffer)
}

#[allow(dead_code)]
pub enum RawDeviceInfo {
    Mouse(RID_DEVICE_INFO_MOUSE),
    Keyboard(RID_DEVICE_INFO_KEYBOARD),
    Hid(RID_DEVICE_INFO_HID),
}

impl From<RID_DEVICE_INFO> for RawDeviceInfo {
    fn from(info: RID_DEVICE_INFO) -> Self {
        unsafe {
            match info.dwType {
                RIM_TYPEMOUSE => RawDeviceInfo::Mouse(*info.u.mouse()),
                RIM_TYPEKEYBOARD => RawDeviceInfo::Keyboard(*info.u.keyboard()),
                RIM_TYPEHID => RawDeviceInfo::Hid(*info.u.hid()),
                _ => unreachable!(),
            }
        }
    }
}

#[allow(dead_code)]
pub fn get_raw_input_device_info(handle: HANDLE) -> Option<RawDeviceInfo> {
    let mut info: RID_DEVICE_INFO = unsafe { mem::uninitialized() };
    let info_size = size_of::<RID_DEVICE_INFO>() as UINT;

    info.cbSize = info_size;

    let mut minimum_size = 0;
    let status = unsafe { winuser::GetRawInputDeviceInfoW(
        handle,
        RIDI_DEVICEINFO,
        &mut info as *mut _ as _,
        &mut minimum_size,
    ) };

    if status == UINT::max_value() || status == 0 {
        return None;
    }

    debug_assert_eq!(info_size, status);

    Some(info.into())
}

pub fn get_raw_input_device_name(handle: HANDLE) -> Option<String> {
    let mut minimum_size = 0;
    let status = unsafe { winuser::GetRawInputDeviceInfoW(
        handle,
        RIDI_DEVICENAME,
        ptr::null_mut(),
        &mut minimum_size,
    ) };

    if status != 0 {
        return None;
    }

    let mut name: Vec<wchar_t> = Vec::with_capacity(minimum_size as _);

    let status = unsafe { winuser::GetRawInputDeviceInfoW(
        handle,
        RIDI_DEVICENAME,
        name.as_ptr() as _,
        &mut minimum_size,
    ) };

    if status == UINT::max_value() || status == 0 {
        return None;
    }

    debug_assert_eq!(minimum_size, status);

    unsafe { name.set_len(minimum_size as _) };

    Some(util::wchar_to_string(&name))
}

pub fn register_raw_input_devices(devices: &[RAWINPUTDEVICE]) -> bool {
    let device_size = size_of::<RAWINPUTDEVICE>() as UINT;

    let success = unsafe { winuser::RegisterRawInputDevices(
        devices.as_ptr() as _,
        devices.len() as _,
        device_size,
    ) };

    success == TRUE
}

pub fn register_all_mice_and_keyboards_for_raw_input(window_handle: HWND) -> bool {
    // RIDEV_DEVNOTIFY: receive hotplug events
    // RIDEV_INPUTSINK: receive events even if we're not in the foreground
    let flags = RIDEV_DEVNOTIFY | RIDEV_INPUTSINK;

    let devices: [RAWINPUTDEVICE; 2] = [
        RAWINPUTDEVICE {
            usUsagePage: HID_USAGE_PAGE_GENERIC,
            usUsage: HID_USAGE_GENERIC_MOUSE,
            dwFlags: flags,
            hwndTarget: window_handle,
        },
        RAWINPUTDEVICE {
            usUsagePage: HID_USAGE_PAGE_GENERIC,
            usUsage: HID_USAGE_GENERIC_KEYBOARD,
            dwFlags: flags,
            hwndTarget: window_handle,
        },
    ];

    register_raw_input_devices(&devices)
}

pub fn get_raw_input_data(handle: HRAWINPUT) -> Option<RAWINPUT> {
    let mut data: RAWINPUT = unsafe { mem::uninitialized() };
    let mut data_size = size_of::<RAWINPUT>() as UINT;
    let header_size = size_of::<RAWINPUTHEADER>() as UINT;

    let status = unsafe { winuser::GetRawInputData(
        handle,
        RID_INPUT,
        &mut data as *mut _  as _,
        &mut data_size,
        header_size,
    ) };

    if status == UINT::max_value() || status == 0 {
        return None;
    }

    Some(data)
}


fn button_flags_to_element_state(button_flags: USHORT, down_flag: USHORT, up_flag: USHORT)
    -> Option<ElementState>
{
    // We assume the same button won't be simultaneously pressed and released.
    if util::has_flag(button_flags, down_flag) {
        Some(ElementState::Pressed)
    } else if util::has_flag(button_flags, up_flag) {
        Some(ElementState::Released)
    } else {
        None
    }
}

pub fn get_raw_mouse_button_state(button_flags: USHORT) -> [Option<ElementState>; 3] {
    [
        button_flags_to_element_state(
            button_flags,
            winuser::RI_MOUSE_LEFT_BUTTON_DOWN,
            winuser::RI_MOUSE_LEFT_BUTTON_UP,
        ),
        button_flags_to_element_state(
            button_flags,
            winuser::RI_MOUSE_MIDDLE_BUTTON_DOWN,
            winuser::RI_MOUSE_MIDDLE_BUTTON_UP,
        ),
        button_flags_to_element_state(
            button_flags,
            winuser::RI_MOUSE_RIGHT_BUTTON_DOWN,
            winuser::RI_MOUSE_RIGHT_BUTTON_UP,
        ),
    ]
}
