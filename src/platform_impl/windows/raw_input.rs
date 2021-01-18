use std::{
    cmp::max,
    fmt,
    mem::{self, size_of},
    ptr, slice,
};

use winapi::{
    ctypes::wchar_t,
    shared::{
        hidpi::{
            HidP_GetButtonCaps, HidP_GetCaps, HidP_GetScaledUsageValue, HidP_GetUsageValue,
            HidP_GetUsagesEx, HidP_GetValueCaps, HidP_Input, HIDP_STATUS_SUCCESS, HIDP_VALUE_CAPS,
            PHIDP_PREPARSED_DATA,
        },
        hidusage::{
            HID_USAGE_GENERIC_GAMEPAD, HID_USAGE_GENERIC_JOYSTICK, HID_USAGE_GENERIC_KEYBOARD,
            HID_USAGE_GENERIC_MOUSE, HID_USAGE_PAGE_GENERIC,
        },
        minwindef::{INT, TRUE, UINT, USHORT},
        windef::HWND,
    },
    um::{
        winnt::{HANDLE, PCHAR},
        winuser::{
            self, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE, RAWINPUTDEVICELIST, RAWINPUTHEADER,
            RIDEV_DEVNOTIFY, RIDEV_INPUTSINK, RIDI_DEVICEINFO, RIDI_DEVICENAME, RIDI_PREPARSEDDATA,
            RID_DEVICE_INFO, RID_DEVICE_INFO_HID, RID_DEVICE_INFO_KEYBOARD, RID_DEVICE_INFO_MOUSE,
            RID_INPUT, RIM_TYPEHID, RIM_TYPEKEYBOARD, RIM_TYPEMOUSE,
        },
    },
};

use crate::{
    event::{
        device::{GamepadAxis, GamepadEvent},
        ElementState,
    },
    platform_impl::platform::util,
};

#[allow(dead_code)]
pub fn get_raw_input_device_list() -> Option<Vec<RAWINPUTDEVICELIST>> {
    let list_size = size_of::<RAWINPUTDEVICELIST>() as UINT;

    let mut num_devices = 0;
    let status =
        unsafe { winuser::GetRawInputDeviceList(ptr::null_mut(), &mut num_devices, list_size) };

    if status == UINT::max_value() {
        return None;
    }

    let mut buffer = Vec::with_capacity(num_devices as _);

    let num_stored = unsafe {
        winuser::GetRawInputDeviceList(buffer.as_ptr() as _, &mut num_devices, list_size)
    };

    if num_stored == UINT::max_value() {
        return None;
    }

    debug_assert_eq!(num_devices, num_stored);

    unsafe { buffer.set_len(num_devices as _) };

    Some(buffer)
}
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

pub fn get_raw_input_device_info(handle: HANDLE) -> Option<RawDeviceInfo> {
    let mut info: RID_DEVICE_INFO = unsafe { mem::zeroed() };
    let info_size = size_of::<RID_DEVICE_INFO>() as UINT;

    info.cbSize = info_size;

    let mut data_size = info_size;
    let status = unsafe {
        winuser::GetRawInputDeviceInfoW(
            handle,
            RIDI_DEVICEINFO,
            &mut info as *mut _ as _,
            &mut data_size,
        )
    } as INT;

    if status <= 0 {
        return None;
    }

    debug_assert_eq!(info_size, status as _);

    Some(info.into())
}

pub fn get_raw_input_device_name(handle: HANDLE) -> Option<String> {
    let mut minimum_size = 0;
    let status = unsafe {
        winuser::GetRawInputDeviceInfoW(handle, RIDI_DEVICENAME, ptr::null_mut(), &mut minimum_size)
    };

    if status != 0 {
        return None;
    }

    let mut name: Vec<wchar_t> = Vec::with_capacity(minimum_size as _);

    let status = unsafe {
        winuser::GetRawInputDeviceInfoW(
            handle,
            RIDI_DEVICENAME,
            name.as_ptr() as _,
            &mut minimum_size,
        )
    };

    if status == UINT::max_value() || status == 0 {
        return None;
    }

    debug_assert_eq!(minimum_size, status);

    unsafe { name.set_len(minimum_size as _) };

    Some(util::wchar_to_string(&name))
}

pub fn get_raw_input_pre_parse_info(handle: HANDLE) -> Option<Vec<u8>> {
    let mut minimum_size = 0;
    let status = unsafe {
        winuser::GetRawInputDeviceInfoW(
            handle,
            RIDI_PREPARSEDDATA,
            ptr::null_mut(),
            &mut minimum_size,
        )
    };

    if status != 0 {
        return None;
    }

    let mut buf: Vec<u8> = Vec::with_capacity(minimum_size as _);

    let status = unsafe {
        winuser::GetRawInputDeviceInfoW(
            handle,
            RIDI_PREPARSEDDATA,
            buf.as_ptr() as _,
            &mut minimum_size,
        )
    };

    if status == UINT::max_value() || status == 0 {
        return None;
    }

    debug_assert_eq!(minimum_size, status);

    unsafe { buf.set_len(minimum_size as _) };

    Some(buf)
}

pub fn register_raw_input_devices(devices: &[RAWINPUTDEVICE]) -> bool {
    let device_size = size_of::<RAWINPUTDEVICE>() as UINT;

    let success = unsafe {
        winuser::RegisterRawInputDevices(devices.as_ptr() as _, devices.len() as _, device_size)
    };

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

pub enum RawInputData {
    Mouse {
        device_handle: HANDLE,
        raw_mouse: winuser::RAWMOUSE,
    },
    Keyboard {
        device_handle: HANDLE,
        raw_keyboard: winuser::RAWKEYBOARD,
    },
    Hid {
        device_handle: HANDLE,
        raw_hid: RawHidData,
    },
}

pub struct RawHidData {
    pub hid_input_size: u32,
    pub hid_input_count: u32,
    pub raw_input: Box<[u8]>,
}

pub fn get_raw_input_data(handle: HRAWINPUT) -> Option<RawInputData> {
    let mut data_size = 0;
    let header_size = size_of::<RAWINPUTHEADER>() as UINT;

    // There are two classes of data this function can output:
    // - Raw mouse and keyboard data
    // - Raw HID data
    // The first class (mouse and keyboard) is always going to write data formatted like the
    // `RAWINPUT` struct, with no other data, and can be placed on the stack into `RAWINPUT`.
    // The second class (raw HID data) writes the struct, and then a buffer of data appended to
    // the end. That data needs to be heap-allocated so we can store all of it.
    unsafe {
        winuser::GetRawInputData(
            handle,
            RID_INPUT,
            ptr::null_mut(),
            &mut data_size,
            header_size,
        )
    };

    let (status, data): (INT, RawInputData);

    if data_size <= size_of::<RAWINPUT>() as UINT {
        // Since GetRawInputData is going to write... well, a buffer that's `RAWINPUT` bytes long
        // and structured like `RAWINPUT`, we're just going to cut to the chase and write directly into
        // a `RAWINPUT` struct.
        let mut rawinput_data: RAWINPUT = unsafe { mem::zeroed() };

        status = unsafe {
            winuser::GetRawInputData(
                handle,
                RID_INPUT,
                &mut rawinput_data as *mut RAWINPUT as *mut _,
                &mut data_size,
                header_size,
            )
        } as INT;

        assert_ne!(-1, status);

        let device_handle = rawinput_data.header.hDevice;

        data = match rawinput_data.header.dwType {
            winuser::RIM_TYPEMOUSE => {
                let raw_mouse = unsafe { rawinput_data.data.mouse().clone() };
                RawInputData::Mouse {
                    device_handle,
                    raw_mouse,
                }
            }
            winuser::RIM_TYPEKEYBOARD => {
                let raw_keyboard = unsafe { rawinput_data.data.keyboard().clone() };
                RawInputData::Keyboard {
                    device_handle,
                    raw_keyboard,
                }
            }
            winuser::RIM_TYPEHID => {
                let hid_data = unsafe { rawinput_data.data.hid() };
                let buf_len = hid_data.dwSizeHid as usize * hid_data.dwCount as usize;
                let data = unsafe { slice::from_raw_parts(hid_data.bRawData.as_ptr(), buf_len) };
                RawInputData::Hid {
                    device_handle,
                    raw_hid: RawHidData {
                        hid_input_size: hid_data.dwSizeHid,
                        hid_input_count: hid_data.dwCount,
                        raw_input: Box::from(data),
                    },
                }
            }
            _ => unreachable!(),
        };
    } else {
        let mut buf = vec![0u8; data_size as usize];

        status = unsafe {
            winuser::GetRawInputData(
                handle,
                RID_INPUT,
                buf.as_mut_ptr() as *mut _,
                &mut data_size,
                header_size,
            )
        } as INT;

        let rawinput_data = buf.as_ptr() as *const RAWINPUT;

        let device_handle = unsafe { (&*rawinput_data).header.hDevice };

        data = match unsafe { *rawinput_data }.header.dwType {
            winuser::RIM_TYPEMOUSE => {
                let raw_mouse = unsafe { (&*rawinput_data).data.mouse().clone() };
                RawInputData::Mouse {
                    device_handle,
                    raw_mouse,
                }
            }
            winuser::RIM_TYPEKEYBOARD => {
                let raw_keyboard = unsafe { (&*rawinput_data).data.keyboard().clone() };
                RawInputData::Keyboard {
                    device_handle,
                    raw_keyboard,
                }
            }
            winuser::RIM_TYPEHID => {
                let hid_data: winuser::RAWHID = unsafe { (&*rawinput_data).data.hid().clone() };

                let hid_data_index = {
                    let hid_data_start =
                        unsafe { &((&*rawinput_data).data.hid().bRawData) } as *const _;
                    hid_data_start as usize - buf.as_ptr() as usize
                };

                buf.drain(..hid_data_index);

                RawInputData::Hid {
                    device_handle,
                    raw_hid: RawHidData {
                        hid_input_size: hid_data.dwSizeHid,
                        hid_input_count: hid_data.dwCount,
                        raw_input: buf.into_boxed_slice(),
                    },
                }
            }
            _ => unreachable!(),
        };

        assert_ne!(-1, status);
    }

    if status == 0 {
        return None;
    }

    Some(data)
}

fn button_flags_to_element_state(
    button_flags: USHORT,
    down_flag: USHORT,
    up_flag: USHORT,
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

pub fn get_raw_mouse_button_state(button_flags: USHORT) -> [Option<ElementState>; 5] {
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
        button_flags_to_element_state(
            button_flags,
            winuser::RI_MOUSE_BUTTON_4_DOWN,
            winuser::RI_MOUSE_BUTTON_4_UP,
        ),
        button_flags_to_element_state(
            button_flags,
            winuser::RI_MOUSE_BUTTON_5_DOWN,
            winuser::RI_MOUSE_BUTTON_5_UP,
        ),
    ]
}

pub struct Axis {
    caps: HIDP_VALUE_CAPS,
    value: f64,
    prev_value: f64,
    axis: Option<GamepadAxis>,
}

impl fmt::Debug for Axis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[derive(Debug)]
        struct Axis {
            value: f64,
            prev_value: f64,
            axis: Option<GamepadAxis>,
        }

        let axis_proxy = Axis {
            value: self.value,
            prev_value: self.prev_value,
            axis: self.axis,
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
        let mut caps = unsafe { mem::zeroed() };
        let status = unsafe { HidP_GetCaps(data_ptr, &mut caps) };
        if status != HIDP_STATUS_SUCCESS {
            return None;
        }
        let mut button_caps_len = caps.NumberInputButtonCaps;
        let mut button_caps = Vec::with_capacity(button_caps_len as _);
        let status = unsafe {
            HidP_GetButtonCaps(
                HidP_Input,
                button_caps.as_mut_ptr(),
                &mut button_caps_len,
                data_ptr,
            )
        };
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
        let status = unsafe {
            HidP_GetValueCaps(
                HidP_Input,
                axis_caps.as_mut_ptr(),
                &mut axis_caps_len,
                data_ptr,
            )
        };
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
                axis: None,
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

    fn update_button_state(&mut self, raw_input_report: &mut [u8]) -> Option<()> {
        let pre_parsed_data_ptr = self.pre_parsed_data_ptr();
        self.prev_button_state =
            mem::replace(&mut self.button_state, vec![false; self.button_count]);
        let mut usages_len = 0;
        // This is the officially documented way to get the required length, but it nonetheless returns
        // `HIDP_STATUS_BUFFER_TOO_SMALL`...
        unsafe {
            HidP_GetUsagesEx(
                HidP_Input,
                0,
                ptr::null_mut(),
                &mut usages_len,
                pre_parsed_data_ptr,
                raw_input_report.as_mut_ptr() as PCHAR,
                raw_input_report.len() as _,
            )
        };
        let mut usages = Vec::with_capacity(usages_len as _);
        let status = unsafe {
            HidP_GetUsagesEx(
                HidP_Input,
                0,
                usages.as_mut_ptr(),
                &mut usages_len,
                pre_parsed_data_ptr,
                raw_input_report.as_mut_ptr() as PCHAR,
                raw_input_report.len() as _,
            )
        };
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

    fn update_axis_state(&mut self, raw_input_report: &mut [u8]) -> Option<()> {
        let pre_parsed_data_ptr = self.pre_parsed_data_ptr();
        for axis in &mut self.axis_state {
            let (status, axis_value) = if axis.caps.LogicalMin < 0 {
                let mut scaled_axis_value = 0;
                let status = unsafe {
                    HidP_GetScaledUsageValue(
                        HidP_Input,
                        axis.caps.UsagePage,
                        0,
                        axis.caps.u.Range().UsageMin,
                        &mut scaled_axis_value,
                        pre_parsed_data_ptr,
                        raw_input_report.as_mut_ptr() as PCHAR,
                        raw_input_report.len() as _,
                    )
                };
                (status, scaled_axis_value as f64)
            } else {
                let mut axis_value = 0;
                let status = unsafe {
                    HidP_GetUsageValue(
                        HidP_Input,
                        axis.caps.UsagePage,
                        0,
                        axis.caps.u.Range().UsageMin,
                        &mut axis_value,
                        pre_parsed_data_ptr,
                        raw_input_report.as_mut_ptr() as PCHAR,
                        raw_input_report.len() as _,
                    )
                };
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

    pub unsafe fn update_state(&mut self, raw_input_report: &mut [u8]) -> Option<()> {
        self.update_button_state(raw_input_report)?;
        self.update_axis_state(raw_input_report)?;
        Some(())
    }

    pub fn get_changed_buttons(&self) -> impl '_ + Iterator<Item = GamepadEvent> {
        self.button_state
            .iter()
            .zip(self.prev_button_state.iter())
            .enumerate()
            .filter(|&(_, (button, prev_button))| button != prev_button)
            .map(|(index, (button, _))| {
                let state = if *button {
                    ElementState::Pressed
                } else {
                    ElementState::Released
                };
                GamepadEvent::Button {
                    button_id: index as _,
                    button: None,
                    state,
                }
            })
    }

    pub fn get_changed_axes(&self) -> impl '_ + Iterator<Item = GamepadEvent> {
        self.axis_state
            .iter()
            .enumerate()
            .filter(|&(_, axis)| axis.value != axis.prev_value)
            .map(|(index, axis)| GamepadEvent::Axis {
                axis_id: index as _,
                axis: axis.axis,
                value: axis.value,
                stick: false,
            })
    }

    pub fn get_gamepad_events(&self) -> Vec<GamepadEvent> {
        self.get_changed_axes()
            .chain(self.get_changed_buttons())
            .collect()
    }

    // pub fn rumble(&mut self, _left_speed: u16, _right_speed: u16) {
    //     // Even though I can't read German, this is still the most useful resource I found:
    //     // https://zfx.info/viewtopic.php?t=3574&f=7
    //     // I'm not optimistic about it being possible to implement this.
    // }
}
