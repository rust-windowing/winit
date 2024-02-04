use std::{
    mem::{size_of, MaybeUninit},
    ptr,
};

use windows_sys::Win32::{
    Devices::HumanInterfaceDevice::{
        HidP_GetButtonCaps, HidP_GetCaps, HidP_GetValueCaps, HidP_Input, HidP_MaxDataListLength,
        HIDP_DATA, HIDP_STATUS_SUCCESS, HID_USAGE_GENERIC_GAMEPAD, HID_USAGE_GENERIC_JOYSTICK,
        HID_USAGE_GENERIC_KEYBOARD, HID_USAGE_GENERIC_MOUSE,
        HID_USAGE_GENERIC_MULTI_AXIS_CONTROLLER, HID_USAGE_PAGE_GENERIC,
    },
    Foundation::{HANDLE, HWND},
    UI::{
        Input::{
            GetRawInputData, GetRawInputDeviceInfoW,
            KeyboardAndMouse::{MapVirtualKeyW, MAPVK_VK_TO_VSC_EX, VK_NUMLOCK, VK_SHIFT},
            RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER,
            RAWKEYBOARD, RIDEV_DEVNOTIFY, RIDEV_INPUTSINK, RIDEV_REMOVE, RIDI_DEVICEINFO,
            RIDI_DEVICENAME, RIDI_PREPARSEDDATA, RID_DEVICE_INFO, RID_INPUT, RIM_TYPEHID,
            RIM_TYPEKEYBOARD, RIM_TYPEMOUSE,
        },
        WindowsAndMessaging::{
            RI_KEY_E0, RI_KEY_E1, RI_MOUSE_BUTTON_1_DOWN, RI_MOUSE_BUTTON_1_UP,
            RI_MOUSE_BUTTON_2_DOWN, RI_MOUSE_BUTTON_2_UP, RI_MOUSE_BUTTON_3_DOWN,
            RI_MOUSE_BUTTON_3_UP, RI_MOUSE_BUTTON_4_DOWN, RI_MOUSE_BUTTON_4_UP,
            RI_MOUSE_BUTTON_5_DOWN, RI_MOUSE_BUTTON_5_UP,
        },
    },
};

use super::scancode_to_physicalkey;
use crate::{
    event::{AxisId, ButtonId, DeviceInfo, ElementState},
    event_loop::DeviceEvents,
    keyboard::{KeyCode, PhysicalKey},
    platform_impl::platform::util,
};

#[allow(dead_code)]
pub fn get_raw_input_device_info(handle: HANDLE) -> Option<DeviceInfo> {
    let mut info: MaybeUninit<RID_DEVICE_INFO> = MaybeUninit::uninit();
    let mut info_size = size_of::<RID_DEVICE_INFO>() as _;
    let status = unsafe {
        GetRawInputDeviceInfoW(
            handle,
            RIDI_DEVICEINFO,
            info.as_mut_ptr() as _,
            &mut info_size,
        )
    };
    if status == u32::MAX || status == 0 {
        return None;
    }
    debug_assert_eq!(info_size, status);
    let info = unsafe { info.assume_init() };

    Some(match info.dwType {
        RIM_TYPEMOUSE => DeviceInfo::Mouse,
        RIM_TYPEKEYBOARD => DeviceInfo::Keyboard,
        RIM_TYPEHID => {
            let hid_info = unsafe { info.Anonymous.hid };
            DeviceInfo::Hid {
                vendor_id: hid_info.dwVendorId,
                product_id: hid_info.dwProductId,
            }
        }
        _ => unreachable!(),
    })
}

pub fn get_raw_input_device_name(handle: HANDLE) -> Option<String> {
    let mut name_len = 0;
    let status =
        unsafe { GetRawInputDeviceInfoW(handle, RIDI_DEVICENAME, ptr::null_mut(), &mut name_len) };
    if status != 0 {
        return None;
    }

    let mut name: Vec<u16> = Vec::with_capacity(name_len as _);
    let status = unsafe {
        GetRawInputDeviceInfoW(handle, RIDI_DEVICENAME, name.as_ptr() as _, &mut name_len)
    };
    if status == u32::MAX || status == 0 {
        return None;
    }
    debug_assert_eq!(name_len, status);
    unsafe { name.set_len(name_len as _) };

    util::decode_wide(&name).into_string().ok()
}

pub fn register_for_raw_input(mut window_handle: HWND, filter: DeviceEvents) -> bool {
    // RIDEV_DEVNOTIFY: receive hotplug events
    // RIDEV_INPUTSINK: receive events even if we're not in the foreground
    // RIDEV_REMOVE: don't receive device events (requires NULL hwndTarget)
    let flags = match filter {
        DeviceEvents::Never => {
            window_handle = 0;
            RIDEV_REMOVE
        }
        DeviceEvents::WhenFocused => RIDEV_DEVNOTIFY,
        DeviceEvents::Always => RIDEV_DEVNOTIFY | RIDEV_INPUTSINK,
    };

    let devices = [
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
            usUsage: HID_USAGE_GENERIC_MULTI_AXIS_CONTROLLER,
            dwFlags: flags,
            hwndTarget: window_handle,
        },
    ];

    let device_size = size_of::<RAWINPUTDEVICE>() as _;
    unsafe {
        RegisterRawInputDevices(devices.as_ptr(), devices.len() as _, device_size) == true.into()
    }
}

pub enum RawInputData {
    MouseOrKeyboard(RAWINPUT),
    Other(Vec<u8>),
}

pub struct HidState {
    pub preparsed_data: Vec<u8>,
    pub data: Vec<HIDP_DATA>,
    pub inputs: Vec<HidStateInput>,
}

#[derive(Clone)]
pub enum HidStateInput {
    None,
    Button(ButtonId, bool, bool),
    Axis(AxisId, u32),
}

impl HidState {
    pub fn new(device: HANDLE) -> Option<Self> {
        let mut preparsed_data_size = 0;
        let status = unsafe {
            GetRawInputDeviceInfoW(
                device,
                RIDI_PREPARSEDDATA,
                ptr::null_mut(),
                &mut preparsed_data_size,
            )
        };
        if status != 0 {
            return None;
        }

        let mut preparsed_data: Vec<u8> = Vec::with_capacity(preparsed_data_size as _);
        let status = unsafe {
            GetRawInputDeviceInfoW(
                device,
                RIDI_PREPARSEDDATA,
                preparsed_data.as_mut_ptr() as _,
                &mut preparsed_data_size,
            )
        };
        if status == 0 || status == u32::MAX {
            return None;
        }
        debug_assert_eq!(preparsed_data_size, status);
        unsafe { preparsed_data.set_len(preparsed_data_size as _) };

        let data_len = unsafe { HidP_MaxDataListLength(HidP_Input, preparsed_data.as_ptr() as _) };

        let mut caps: MaybeUninit<_> = MaybeUninit::uninit();
        let status = unsafe { HidP_GetCaps(preparsed_data.as_ptr() as _, caps.as_mut_ptr()) };
        if status != HIDP_STATUS_SUCCESS {
            return None;
        }
        let caps = unsafe { caps.assume_init() };

        let mut inputs = vec![HidStateInput::None; caps.NumberInputDataIndices as _];

        let mut button_caps_len = caps.NumberInputButtonCaps;
        let mut button_caps = Vec::with_capacity(button_caps_len as _);
        let status = unsafe {
            HidP_GetButtonCaps(
                HidP_Input,
                button_caps.as_mut_ptr(),
                &mut button_caps_len,
                preparsed_data.as_ptr() as _,
            )
        };
        if status != HIDP_STATUS_SUCCESS {
            return None;
        }
        unsafe { button_caps.set_len(button_caps_len as _) };

        for cap in button_caps {
            // All pages beginning with 0xFF are vendor-specific
            if cap.UsagePage >> 8 == 0xFF {
                continue;
            }

            if cap.IsRange != 0 {
                let range = unsafe { cap.Anonymous.Range };
                let mut data_index = range.DataIndexMin;
                for usage in (range.UsageMin - 1)..range.UsageMax {
                    inputs[data_index as usize] = HidStateInput::Button(usage as _, false, false);
                    data_index += 1;
                }
            } else {
                let not_range = unsafe { cap.Anonymous.NotRange };
                inputs[not_range.DataIndex as usize] =
                    HidStateInput::Button((not_range.Usage - 1) as _, false, false);
            }
        }

        let mut value_caps_len = caps.NumberInputValueCaps;
        let mut value_caps = Vec::with_capacity(value_caps_len as _);
        let status = unsafe {
            HidP_GetValueCaps(
                HidP_Input,
                value_caps.as_mut_ptr(),
                &mut value_caps_len,
                preparsed_data.as_ptr() as _,
            )
        };
        if status != HIDP_STATUS_SUCCESS {
            return None;
        }
        unsafe { value_caps.set_len(value_caps_len as _) };

        for cap in value_caps {
            // All pages beginning with 0xFF are vendor-specific
            if cap.UsagePage >> 8 == 0xFF {
                continue;
            }

            if cap.IsRange != 0 {
                let range = unsafe { cap.Anonymous.Range };
                let mut data_index = range.DataIndexMin;
                for usage in (range.UsageMin - 1)..range.UsageMax {
                    inputs[data_index as usize] = HidStateInput::Axis(usage as _, 0);
                    data_index += 1;
                }
            } else {
                let not_range = unsafe { cap.Anonymous.NotRange };
                inputs[not_range.DataIndex as usize] =
                    HidStateInput::Axis((not_range.Usage - 1) as _, 0);
            }
        }

        Some(Self {
            preparsed_data,
            data: Vec::with_capacity(data_len as _),
            inputs,
        })
    }
}

pub fn get_raw_input_data(handle: HRAWINPUT) -> Option<RawInputData> {
    let mut data: MaybeUninit<_> = MaybeUninit::uninit();
    let mut data_size = size_of::<RAWINPUT>() as _;
    let header_size = size_of::<RAWINPUTHEADER>() as _;
    let status = unsafe {
        GetRawInputData(
            handle,
            RID_INPUT,
            data.as_mut_ptr() as _,
            &mut data_size,
            header_size,
        )
    };
    if status != u32::MAX {
        let data = unsafe { data.assume_init() };
        return Some(RawInputData::MouseOrKeyboard(data));
    }

    let mut data = Vec::with_capacity(data_size as _);
    let status = unsafe {
        GetRawInputData(
            handle,
            RID_INPUT,
            data.as_mut_ptr() as _,
            &mut data_size,
            header_size,
        )
    };
    if status != u32::MAX {
        unsafe { data.set_len(data_size as _) };
        return Some(RawInputData::Other(data));
    }

    None
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

pub fn get_raw_mouse_button_state(button_flags: u32) -> [Option<ElementState>; 5] {
    [
        button_flags_to_element_state(button_flags, RI_MOUSE_BUTTON_1_DOWN, RI_MOUSE_BUTTON_1_UP),
        button_flags_to_element_state(button_flags, RI_MOUSE_BUTTON_2_DOWN, RI_MOUSE_BUTTON_2_UP),
        button_flags_to_element_state(button_flags, RI_MOUSE_BUTTON_3_DOWN, RI_MOUSE_BUTTON_3_UP),
        button_flags_to_element_state(button_flags, RI_MOUSE_BUTTON_4_DOWN, RI_MOUSE_BUTTON_4_UP),
        button_flags_to_element_state(button_flags, RI_MOUSE_BUTTON_5_DOWN, RI_MOUSE_BUTTON_5_UP),
    ]
}

pub fn get_keyboard_physical_key(keyboard: RAWKEYBOARD) -> Option<PhysicalKey> {
    let extension = {
        if util::has_flag(keyboard.Flags, RI_KEY_E0 as _) {
            0xE000
        } else if util::has_flag(keyboard.Flags, RI_KEY_E1 as _) {
            0xE100
        } else {
            0x0000
        }
    };
    let scancode = if keyboard.MakeCode == 0 {
        // In some cases (often with media keys) the device reports a scancode of 0 but a
        // valid virtual key. In these cases we obtain the scancode from the virtual key.
        unsafe { MapVirtualKeyW(keyboard.VKey as u32, MAPVK_VK_TO_VSC_EX) as u16 }
    } else {
        keyboard.MakeCode | extension
    };
    if scancode == 0xE11D || scancode == 0xE02A {
        // At the hardware (or driver?) level, pressing the Pause key is equivalent to pressing
        // Ctrl+NumLock.
        // This equvalence means that if the user presses Pause, the keyboard will emit two
        // subsequent keypresses:
        // 1, 0xE11D - Which is a left Ctrl (0x1D) with an extension flag (0xE100)
        // 2, 0x0045 - Which on its own can be interpreted as Pause
        //
        // There's another combination which isn't quite an equivalence:
        // PrtSc used to be Shift+Asterisk. This means that on some keyboards, presssing
        // PrtSc (print screen) produces the following sequence:
        // 1, 0xE02A - Which is a left shift (0x2A) with an extension flag (0xE000)
        // 2, 0xE037 - Which is a numpad multiply (0x37) with an exteion flag (0xE000). This on
        //             its own it can be interpreted as PrtSc
        //
        // For this reason, if we encounter the first keypress, we simply ignore it, trusting
        // that there's going to be another event coming, from which we can extract the
        // appropriate key.
        // For more on this, read the article by Raymond Chen, titled:
        // "Why does Ctrl+ScrollLock cancel dialogs?"
        // https://devblogs.microsoft.com/oldnewthing/20080211-00/?p=23503
        return None;
    }
    let physical_key = if keyboard.VKey == VK_NUMLOCK {
        // Historically, the NumLock and the Pause key were one and the same physical key.
        // The user could trigger Pause by pressing Ctrl+NumLock.
        // Now these are often physically separate and the two keys can be differentiated by
        // checking the extension flag of the scancode. NumLock is 0xE045, Pause is 0x0045.
        //
        // However in this event, both keys are reported as 0x0045 even on modern hardware.
        // Therefore we use the virtual key instead to determine whether it's a NumLock and
        // set the KeyCode accordingly.
        //
        // For more on this, read the article by Raymond Chen, titled:
        // "Why does Ctrl+ScrollLock cancel dialogs?"
        // https://devblogs.microsoft.com/oldnewthing/20080211-00/?p=23503
        PhysicalKey::Code(KeyCode::NumLock)
    } else {
        scancode_to_physicalkey(scancode as u32)
    };
    if keyboard.VKey == VK_SHIFT {
        if let PhysicalKey::Code(code) = physical_key {
            match code {
                KeyCode::NumpadDecimal
                | KeyCode::Numpad0
                | KeyCode::Numpad1
                | KeyCode::Numpad2
                | KeyCode::Numpad3
                | KeyCode::Numpad4
                | KeyCode::Numpad5
                | KeyCode::Numpad6
                | KeyCode::Numpad7
                | KeyCode::Numpad8
                | KeyCode::Numpad9 => {
                    // On Windows, holding the Shift key makes numpad keys behave as if NumLock
                    // wasn't active. The way this is exposed to applications by the system is that
                    // the application receives a fake key release event for the shift key at the
                    // moment when the numpad key is pressed, just before receiving the numpad key
                    // as well.
                    //
                    // The issue is that in the raw device event (here), the fake shift release
                    // event reports the numpad key as the scancode. Unfortunately, the event doesn't
                    // have any information to tell whether it's the left shift or the right shift
                    // that needs to get the fake release (or press) event so we don't forward this
                    // event to the application at all.
                    //
                    // For more on this, read the article by Raymond Chen, titled:
                    // "The shift key overrides NumLock"
                    // https://devblogs.microsoft.com/oldnewthing/20040906-00/?p=37953
                    return None;
                }
                _ => (),
            }
        }
    }

    Some(physical_key)
}
