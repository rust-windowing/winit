use std::{fmt, ptr};
use std::cmp::max;
use std::mem::{self, size_of};

use winapi::ctypes::wchar_t;
use winapi::shared::minwindef::{BYTE, TRUE, UINT, USHORT};
use winapi::shared::hidpi::{
    HidP_GetButtonCaps,
    HidP_GetCaps,
    HidP_GetScaledUsageValue,
    HidP_GetUsagesEx,
    HidP_GetUsageValue,
    HidP_GetValueCaps,
    HidP_Input,
    /*HIDP_STATUS_BUFFER_TOO_SMALL,
    HIDP_STATUS_INCOMPATIBLE_REPORT_ID,
    HIDP_STATUS_INVALID_PREPARSED_DATA,
    HIDP_STATUS_INVALID_REPORT_LENGTH,
    HIDP_STATUS_INVALID_REPORT_TYPE,*/
    HIDP_STATUS_SUCCESS,
    HIDP_VALUE_CAPS,
    PHIDP_PREPARSED_DATA,
};
use winapi::shared::hidusage::{
    HID_USAGE_PAGE_GENERIC,
    HID_USAGE_GENERIC_MOUSE,
    HID_USAGE_GENERIC_KEYBOARD,
    HID_USAGE_GENERIC_JOYSTICK,
    HID_USAGE_GENERIC_GAMEPAD,
};
use winapi::shared::windef::HWND;
use winapi::um::winnt::{HANDLE, PCHAR};
use winapi::um::winuser::{
    self,
    HRAWINPUT,
    RAWHID,
    RAWINPUT,
    RAWINPUTDEVICE,
    RAWINPUTDEVICELIST,
    RAWINPUTHEADER,
    RID_INPUT,
    RID_DEVICE_INFO,
    RID_DEVICE_INFO_HID,
    RID_DEVICE_INFO_KEYBOARD,
    RID_DEVICE_INFO_MOUSE,
    RIDEV_DEVNOTIFY,
    RIDEV_INPUTSINK,
    RIDI_DEVICEINFO,
    RIDI_DEVICENAME,
    RIDI_PREPARSEDDATA,
    RIM_TYPEMOUSE,
    RIM_TYPEKEYBOARD,
    RIM_TYPEHID,
};

use events::{AxisHint, ButtonHint, ElementState};
use platform::platform::util;

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

pub fn get_raw_input_pre_parse_info(handle: HANDLE) -> Option<Vec<u8>> {
    let mut minimum_size = 0;
    let status = unsafe { winuser::GetRawInputDeviceInfoW(
        handle,
        RIDI_PREPARSEDDATA,
        ptr::null_mut(),
        &mut minimum_size,
    ) };

    if status != 0 {
        return None;
    }

    let mut buf: Vec<u8> = Vec::with_capacity(minimum_size as _);

    let status = unsafe { winuser::GetRawInputDeviceInfoW(
        handle,
        RIDI_PREPARSEDDATA,
        buf.as_ptr() as _,
        &mut minimum_size,
    ) };

    if status == UINT::max_value() || status == 0 {
        return None;
    }

    debug_assert_eq!(minimum_size, status);

    unsafe { buf.set_len(minimum_size as _) };

    Some(buf)
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

pub fn register_for_raw_input(window_handle: HWND) -> bool {
    // `RIDEV_DEVNOTIFY`: receive hotplug events
    // `RIDEV_INPUTSINK`: receive events even if we're not in the foreground
    let flags = RIDEV_DEVNOTIFY | RIDEV_INPUTSINK;

    let devices: [RAWINPUTDEVICE; 5] = [
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
        RAWINPUTDEVICE {
            usUsagePage: HID_USAGE_PAGE_GENERIC,
            usUsage: HID_USAGE_GENERIC_JOYSTICK,
            dwFlags: flags,
            hwndTarget: window_handle,
        },
        RAWINPUTDEVICE {
            usUsagePage: HID_USAGE_PAGE_GENERIC,
            usUsage: HID_USAGE_GENERIC_GAMEPAD,
            dwFlags: flags,
            hwndTarget: window_handle,
        },
        RAWINPUTDEVICE {
            usUsagePage: HID_USAGE_PAGE_GENERIC,
            usUsage: 0x08, // multi-axis
            dwFlags: flags,
            hwndTarget: window_handle,
        },
    ];

    register_raw_input_devices(&devices)
}

pub fn get_raw_input_data(handle: HRAWINPUT) -> Option<Vec<BYTE>> {
    let mut data_size = 0;
    let header_size = size_of::<RAWINPUTHEADER>() as UINT;

    unsafe { winuser::GetRawInputData(
        handle,
        RID_INPUT,
        ptr::null_mut(),
        &mut data_size,
        header_size,
    ) };

    let alignment_remainder = data_size % 8;
    if alignment_remainder != 0 {
        data_size += 8 - alignment_remainder;
    }

    let mut data = Vec::with_capacity(data_size as _);

    let status = unsafe { winuser::GetRawInputData(
        handle,
        RID_INPUT,
        data.as_mut_ptr() as _,
        &mut data_size,
        header_size,
    ) };

    if status == UINT::max_value() || status == 0 {
        return None;
    }

    unsafe { data.set_len(data_size as _) };

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

pub struct Axis {
    caps: HIDP_VALUE_CAPS,
    pub value: f64,
    pub prev_value: f64,
    //active: bool,
}

impl fmt::Debug for Axis {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        #[derive(Debug)]
        struct Axis {
            value: f64,
            prev_value: f64,
        }

        let axis_proxy = Axis {
            value: self.value,
            prev_value: self.prev_value,
        };

        axis_proxy.fmt(f)
    }
}

#[derive(Debug)]
pub struct RawGamepad {
    handle: HANDLE,
    pre_parsed_data: Vec<u8>,
    button_count: usize,
    pub button_state: Vec<bool>,
    pub prev_button_state: Vec<bool>,
    axis_count: usize,
    pub axis_state: Vec<Axis>,
}

// Reference: https://chromium.googlesource.com/chromium/chromium/+/trunk/content/browser/gamepad/raw_input_data_fetcher_win.cc
impl RawGamepad {
    pub fn new(handle: HANDLE) -> Option<Self> {
        let pre_parsed_data = get_raw_input_pre_parse_info(handle)?;
        let data_ptr = pre_parsed_data.as_ptr() as PHIDP_PREPARSED_DATA;
        let mut caps = unsafe { mem::uninitialized() };
        let status = unsafe { HidP_GetCaps(data_ptr, &mut caps) };
        if status != HIDP_STATUS_SUCCESS {
            return None;
        }
        let mut button_caps_len = caps.NumberInputButtonCaps;
        let mut button_caps = Vec::with_capacity(button_caps_len as _);
        let status = unsafe { HidP_GetButtonCaps(
            HidP_Input,
            button_caps.as_mut_ptr(),
            &mut button_caps_len,
            data_ptr,
        ) };
        if status != HIDP_STATUS_SUCCESS {
            return None;
        }
        unsafe { button_caps.set_len(button_caps_len as _) };
        let mut button_count = 0;
        for button_cap in button_caps {
            let range = unsafe { button_cap.u.Range() };
            button_count = max(button_count, range.UsageMax);
        }
        let button_state = vec![false; button_count as usize];
        let mut axis_caps_len = caps.NumberInputValueCaps;
        let mut axis_caps = Vec::with_capacity(axis_caps_len as _);
        let status = unsafe { HidP_GetValueCaps(
            HidP_Input,
            axis_caps.as_mut_ptr(),
            &mut axis_caps_len,
            data_ptr,
        ) };
        if status != HIDP_STATUS_SUCCESS {
            return None;
        }
        unsafe { axis_caps.set_len(axis_caps_len as _) };
        let mut axis_state = Vec::with_capacity(axis_caps_len as _);
        let mut axis_count = 0;
        for (axis_index, axis_cap) in axis_caps.drain(0..).enumerate() {
            axis_state.push(Axis {
                caps: axis_cap,
                value: 0.0,
                prev_value: 0.0,
                //active: true,
            });
            axis_count = max(axis_count, axis_index + 1);
        }
        Some(RawGamepad {
            handle,
            pre_parsed_data,
            button_count: button_count as usize,
            button_state: button_state.clone(),
            prev_button_state: button_state,
            axis_count,
            axis_state,
        })
    }

    fn pre_parsed_data_ptr(&mut self) -> PHIDP_PREPARSED_DATA {
        self.pre_parsed_data.as_mut_ptr() as PHIDP_PREPARSED_DATA
    }

    fn update_button_state(&mut self, hid: &mut RAWHID) -> Option<()> {
        let pre_parsed_data_ptr = self.pre_parsed_data_ptr();
        self.prev_button_state = mem::replace(
            &mut self.button_state,
            vec![false; self.button_count],
        );
        let mut usages_len = 0;
        // This is the officially documented way to get the required length, but it nonetheless returns
        // `HIDP_STATUS_BUFFER_TOO_SMALL`...
        unsafe { HidP_GetUsagesEx(
            HidP_Input,
            0,
            ptr::null_mut(),
            &mut usages_len,
            pre_parsed_data_ptr,
            hid.bRawData.as_mut_ptr() as PCHAR,
            hid.dwSizeHid,
        ) };
        let mut usages = Vec::with_capacity(usages_len as _);
        let status = unsafe { HidP_GetUsagesEx(
            HidP_Input,
            0,
            usages.as_mut_ptr(),
            &mut usages_len,
            pre_parsed_data_ptr,
            hid.bRawData.as_mut_ptr() as PCHAR,
            hid.dwSizeHid,
        ) };
        if status != HIDP_STATUS_SUCCESS {
            return None;
        }
        unsafe { usages.set_len(usages_len as _) };
        for usage in usages {
            if usage.UsagePage != 0xFF << 8 {
                let button_index = (usage.Usage - 1) as usize;
                self.button_state[button_index] = true;
            }
        }
        Some(())
    }

    fn update_axis_state(&mut self, hid: &mut RAWHID) -> Option<()> {
        let pre_parsed_data_ptr = self.pre_parsed_data_ptr();
        for axis in &mut self.axis_state {
            let (status, axis_value) = if axis.caps.LogicalMin < 0 {
                let mut scaled_axis_value = 0;
                let status = unsafe { HidP_GetScaledUsageValue(
                    HidP_Input,
                    axis.caps.UsagePage,
                    0,
                    axis.caps.u.Range().UsageMin,
                    &mut scaled_axis_value,
                    pre_parsed_data_ptr,
                    hid.bRawData.as_mut_ptr() as PCHAR,
                    hid.dwSizeHid,
                ) };
                (status, scaled_axis_value as f64)
            } else {
                let mut axis_value = 0;
                let status = unsafe { HidP_GetUsageValue(
                    HidP_Input,
                    axis.caps.UsagePage,
                    0,
                    axis.caps.u.Range().UsageMin,
                    &mut axis_value,
                    pre_parsed_data_ptr,
                    hid.bRawData.as_mut_ptr() as PCHAR,
                    hid.dwSizeHid,
                ) };
                (status, axis_value as f64)
            };
            if status != HIDP_STATUS_SUCCESS {
                return None;
            }
            axis.prev_value = axis.value;
            axis.value = util::normalize_symmetric(
                axis_value,
                axis.caps.LogicalMin as f64,
                axis.caps.LogicalMax as f64,
            );
        }
        Some(())
    }

    pub unsafe fn update_state(&mut self, input: *mut RAWINPUT) -> Option<()> {
        if (*input).header.dwType != winuser::RIM_TYPEHID {
            return None;
        }
        let hid = (*input).data.hid_mut();
        self.update_button_state(hid)?;
        self.update_axis_state(hid)?;
        Some(())
    }

    pub fn get_changed_buttons(&self) -> Vec<(u32, Option<ButtonHint>, ElementState)> {
        self.button_state
            .iter()
            .zip(self.prev_button_state.iter())
            .enumerate()
            .filter(|&(_, (button, prev_button))| button != prev_button)
            .map(|(index, (button, _))| {
                let state = if *button { ElementState::Pressed } else { ElementState::Released };
                (index as _, None, state)
            })
            .collect()
    }

    pub fn get_changed_axes(&self) -> Vec<(u32, Option<AxisHint>, f64)> {
        // TODO: hints
        self.axis_state
            .iter()
            .enumerate()
            .filter(|&(_, axis)| axis.value != axis.prev_value)
            .map(|(index, axis)| (index as _, None, axis.value))
            .collect()
    }

    pub fn rumble(&mut self, _left_speed: u16, _right_speed: u16) {
        // Even though I can't read German, this is still the most useful resource I found:
        // https://zfx.info/viewtopic.php?t=3574&f=7
        // I'm not optimistic about it being possible to implement this.
    }
}
