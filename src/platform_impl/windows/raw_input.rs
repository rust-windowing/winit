use std::{
    mem::{self, size_of},
    ptr,
};

use windows_sys::Win32::{
    Devices::HumanInterfaceDevice::{
        HID_USAGE_GENERIC_KEYBOARD, HID_USAGE_GENERIC_MOUSE, HID_USAGE_PAGE_GENERIC,
    },
    Foundation::{HANDLE, HWND},
    UI::{
        Input::{
            GetRawInputData, GetRawInputDeviceInfoW, GetRawInputDeviceList,
            RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE, RAWINPUTDEVICELIST,
            RAWINPUTHEADER, RIDEV_DEVNOTIFY, RIDEV_INPUTSINK, RIDEV_REMOVE, RIDI_DEVICEINFO,
            RIDI_DEVICENAME, RID_DEVICE_INFO, RID_DEVICE_INFO_HID, RID_DEVICE_INFO_KEYBOARD,
            RID_DEVICE_INFO_MOUSE, RID_INPUT, RIM_TYPEHID, RIM_TYPEKEYBOARD, RIM_TYPEMOUSE,
        },
        WindowsAndMessaging::{
            RI_MOUSE_LEFT_BUTTON_DOWN, RI_MOUSE_LEFT_BUTTON_UP, RI_MOUSE_MIDDLE_BUTTON_DOWN,
            RI_MOUSE_MIDDLE_BUTTON_UP, RI_MOUSE_RIGHT_BUTTON_DOWN, RI_MOUSE_RIGHT_BUTTON_UP,
        },
    },
};

use crate::{event::ElementState, event_loop::DeviceEventFilter, platform_impl::platform::util};

#[allow(dead_code)]
pub fn get_raw_input_device_list() -> Option<Vec<RAWINPUTDEVICELIST>> {
    let list_size = size_of::<RAWINPUTDEVICELIST>() as u32;

    let mut num_devices = 0;
    let status = unsafe { GetRawInputDeviceList(ptr::null_mut(), &mut num_devices, list_size) };

    if status == u32::MAX {
        return None;
    }

    let mut buffer = Vec::with_capacity(num_devices as _);

    let num_stored =
        unsafe { GetRawInputDeviceList(buffer.as_mut_ptr(), &mut num_devices, list_size) };

    if num_stored == u32::MAX {
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
                RIM_TYPEMOUSE => RawDeviceInfo::Mouse(info.Anonymous.mouse),
                RIM_TYPEKEYBOARD => RawDeviceInfo::Keyboard(info.Anonymous.keyboard),
                RIM_TYPEHID => RawDeviceInfo::Hid(info.Anonymous.hid),
                _ => unreachable!(),
            }
        }
    }
}

#[allow(dead_code)]
pub fn get_raw_input_device_info(handle: HANDLE) -> Option<RawDeviceInfo> {
    let mut info: RID_DEVICE_INFO = unsafe { mem::zeroed() };
    let info_size = size_of::<RID_DEVICE_INFO>() as u32;

    info.cbSize = info_size;

    let mut minimum_size = 0;
    let status = unsafe {
        GetRawInputDeviceInfoW(
            handle,
            RIDI_DEVICEINFO,
            &mut info as *mut _ as _,
            &mut minimum_size,
        )
    };

    if status == u32::MAX || status == 0 {
        return None;
    }

    debug_assert_eq!(info_size, status);

    Some(info.into())
}

pub fn get_raw_input_device_name(handle: HANDLE) -> Option<String> {
    let mut minimum_size = 0;
    let status = unsafe {
        GetRawInputDeviceInfoW(handle, RIDI_DEVICENAME, ptr::null_mut(), &mut minimum_size)
    };

    if status != 0 {
        return None;
    }

    let mut name: Vec<u16> = Vec::with_capacity(minimum_size as _);

    let status = unsafe {
        GetRawInputDeviceInfoW(
            handle,
            RIDI_DEVICENAME,
            name.as_ptr() as _,
            &mut minimum_size,
        )
    };

    if status == u32::MAX || status == 0 {
        return None;
    }

    debug_assert_eq!(minimum_size, status);

    unsafe { name.set_len(minimum_size as _) };

    util::decode_wide(&name).into_string().ok()
}

pub fn register_raw_input_devices(devices: &[RAWINPUTDEVICE]) -> bool {
    let device_size = size_of::<RAWINPUTDEVICE>() as u32;

    unsafe {
        RegisterRawInputDevices(devices.as_ptr(), devices.len() as u32, device_size) == true.into()
    }
}

pub fn register_all_mice_and_keyboards_for_raw_input(
    mut window_handle: HWND,
    filter: DeviceEventFilter,
) -> bool {
    // RIDEV_DEVNOTIFY: receive hotplug events
    // RIDEV_INPUTSINK: receive events even if we're not in the foreground
    // RIDEV_REMOVE: don't receive device events (requires NULL hwndTarget)
    let flags = match filter {
        DeviceEventFilter::Always => {
            window_handle = 0;
            RIDEV_REMOVE
        }
        DeviceEventFilter::Unfocused => RIDEV_DEVNOTIFY,
        DeviceEventFilter::Never => RIDEV_DEVNOTIFY | RIDEV_INPUTSINK,
    };

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
    let mut data: RAWINPUT = unsafe { mem::zeroed() };
    let mut data_size = size_of::<RAWINPUT>() as u32;
    let header_size = size_of::<RAWINPUTHEADER>() as u32;

    let status = unsafe {
        GetRawInputData(
            handle,
            RID_INPUT,
            &mut data as *mut _ as _,
            &mut data_size,
            header_size,
        )
    };

    if status == u32::MAX || status == 0 {
        return None;
    }

    Some(data)
}

fn button_flags_to_element_state(
    button_flags: u32,
    down_flag: u32,
    up_flag: u32,
) -> Option<ElementState> {
    // We assume the same button won't be simultaneously pressed and released.
    if util::has_flag(button_flags, down_flag) {
        Some(ElementState::Pressed)
    } else if util::has_flag(button_flags, up_flag) {
        Some(ElementState::Released)
    } else {
        None
    }
}

pub fn get_raw_mouse_button_state(button_flags: u32) -> [Option<ElementState>; 3] {
    [
        button_flags_to_element_state(
            button_flags,
            RI_MOUSE_LEFT_BUTTON_DOWN,
            RI_MOUSE_LEFT_BUTTON_UP,
        ),
        button_flags_to_element_state(
            button_flags,
            RI_MOUSE_MIDDLE_BUTTON_DOWN,
            RI_MOUSE_MIDDLE_BUTTON_UP,
        ),
        button_flags_to_element_state(
            button_flags,
            RI_MOUSE_RIGHT_BUTTON_DOWN,
            RI_MOUSE_RIGHT_BUTTON_UP,
        ),
    ]
}
