use winapi;

/// Win32 implementation of the main `MonitorID` object.
pub struct MonitorID {
    /// The system name of the monitor.
    name: [winapi::WCHAR, ..32],

    /// Name to give to the user.
    readable_name: String,

    /// See the `StateFlags` element here:
    /// http://msdn.microsoft.com/en-us/library/dd183569(v=vs.85).aspx
    flags: winapi::DWORD,

    /// The position of the monitor in pixels on the desktop.
    ///
    /// A window that is positionned at these coordinates will overlap the monitor.
    position: (uint, uint),

    /// The current resolution in pixels on the monitor.
    dimensions: (uint, uint),
}

/// Win32 implementation of the main `get_available_monitors` function.
pub fn get_available_monitors() -> Vec<MonitorID> {
    use std::{iter, mem, ptr};

    // return value
    let mut result = Vec::new();

    // enumerating the devices is done by querying device 0, then device 1, then device 2, etc.
    //  until the query function returns null
    for id in iter::count(0u, 1) {
        // getting the DISPLAY_DEVICEW object of the current device
        let output = {
            let mut output: winapi::DISPLAY_DEVICEW = unsafe { mem::zeroed() };
            output.cb = mem::size_of::<winapi::DISPLAY_DEVICEW>() as winapi::DWORD;

            if unsafe { winapi::EnumDisplayDevicesW(ptr::null(),
                id as winapi::DWORD, &mut output, 0) } == 0
            {
                // the device doesn't exist, which means we have finished enumerating
                break;
            }

            if  (output.StateFlags & winapi::DISPLAY_DEVICE_ACTIVE) == 0 ||
                (output.StateFlags & winapi::DISPLAY_DEVICE_MIRRORING_DRIVER) != 0
            {
                // the device is not active
                // the Win32 api usually returns a lot of inactive devices
                continue;
            }

            output
        };

        // computing the human-friendly name
        let readable_name = String::from_utf16_lossy(output.DeviceString.as_slice());
        let readable_name = readable_name.as_slice().trim_right_chars(0 as char).to_string();

        // getting the position
        let (position, dimensions) = unsafe {
            let mut dev: winapi::DEVMODEW = mem::zeroed();
            dev.dmSize = mem::size_of::<winapi::DEVMODEW>() as winapi::WORD;

            if winapi::EnumDisplaySettingsExW(output.DeviceName.as_ptr(), winapi::ENUM_CURRENT_SETTINGS,
                &mut dev, 0) == 0
            {
                continue;
            }

            let point: &winapi::POINTL = mem::transmute(&dev.union1);
            let position = (point.x as uint, point.y as uint);

            let dimensions = (dev.dmPelsWidth as uint, dev.dmPelsHeight as uint);

            (position, dimensions)
        };

        // adding to the resulting list
        result.push(MonitorID {
            name: output.DeviceName,
            readable_name: readable_name,
            flags: output.StateFlags,
            position: position,
            dimensions: dimensions,
        });
    }

    result
}

/// Win32 implementation of the main `get_primary_monitor` function.
pub fn get_primary_monitor() -> MonitorID {
    // we simply get all available monitors and return the one with the `PRIMARY_DEVICE` flag
    // TODO: it is possible to query the win32 API for the primary monitor, this should be done
    //  instead
    for monitor in get_available_monitors().into_iter() {
        if (monitor.flags & winapi::DISPLAY_DEVICE_PRIMARY_DEVICE) != 0 {
            return monitor
        }
    }

    panic!("Failed to find the primary monitor")
}

impl MonitorID {
    /// See the docs if the crate root file.
    pub fn get_name(&self) -> Option<String> {
        Some(self.readable_name.clone())
    }

    /// See the docs if the crate root file.
    pub fn get_dimensions(&self) -> (uint, uint) {
        // TODO: retreive the dimensions every time this is called
        self.dimensions
    }

    /// This is a Win32-only function for `MonitorID` that returns the system name of the device.
    pub fn get_system_name(&self) -> &[winapi::WCHAR] {
        // TODO: retreive the position every time this is called
        self.name.as_slice()
    }

    /// This is a Win32-only function for `MonitorID` that returns the position of the
    ///  monitor on the desktop. 
    /// A window that is positionned at these coordinates will overlap the monitor.
    pub fn get_position(&self) -> (uint, uint) {
        self.position
    }
}
