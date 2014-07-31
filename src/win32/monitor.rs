use super::ffi;

pub struct MonitorID {
    name: [ffi::WCHAR, ..32],
    readable_name: String,
    flags: ffi::DWORD,
    position: (uint, uint),
}

pub fn get_available_monitors() -> Vec<MonitorID> {
    use std::{iter, mem, ptr};

    let mut result = Vec::new();

    for id in iter::count(0u, 1) {
        let mut output: ffi::DISPLAY_DEVICEW = unsafe { mem::zeroed() };
        output.cb = mem::size_of::<ffi::DISPLAY_DEVICEW>() as ffi::DWORD;

        if unsafe { ffi::EnumDisplayDevicesW(ptr::null(), id as ffi::DWORD, &mut output, 0) } == 0 {
            break
        }

        if  (output.StateFlags & ffi::DISPLAY_DEVICE_ACTIVE) == 0 ||
            (output.StateFlags & ffi::DISPLAY_DEVICE_MIRRORING_DRIVER) != 0
        {
            continue
        }

        let readable_name = String::from_utf16_lossy(output.DeviceString.as_slice());
        let readable_name = readable_name.as_slice().trim_right_chars(0 as char).to_string();

        let position = unsafe {
            let mut dev: ffi::DEVMODE = mem::zeroed();
            dev.dmSize = mem::size_of::<ffi::DEVMODE>() as ffi::WORD;

            if ffi::EnumDisplaySettingsExW(output.DeviceName.as_ptr(), ffi::ENUM_CURRENT_SETTINGS,
                &mut dev, 0) == 0
            {
                continue
            }

            let point: &ffi::POINTL = mem::transmute(&dev.union1);
            (point.x as uint, point.y as uint)
        };

        result.push(MonitorID {
            name: output.DeviceName,
            readable_name: readable_name,
            flags: output.StateFlags,
            position: position,
        });
    }

    result
}

pub fn get_primary_monitor() -> MonitorID {
    for monitor in get_available_monitors().move_iter() {
        if (monitor.flags & ffi::DISPLAY_DEVICE_PRIMARY_DEVICE) != 0 {
            return monitor
        }
    }

    fail!("Failed to find the primary monitor")
}

impl MonitorID {
    pub fn get_name(&self) -> Option<String> {
        Some(self.readable_name.clone())
    }

    pub fn get_system_name(&self) -> &[ffi::WCHAR] {
        self.name.as_slice()
    }

    pub fn get_position(&self) -> (uint, uint) {
        self.position
    }
}
