use winapi;
use user32;

use std::collections::VecDeque;
use std::mem;

use native_monitor::NativeMonitorId;

/// Win32 implementation of the main `MonitorID` object.
pub struct MonitorID {
    /// The system name of the adapter.
    adapter_name: [winapi::WCHAR; 32],

    /// The system name of the monitor.
    monitor_name: String,

    /// Name to give to the user.
    readable_name: String,

    /// See the `StateFlags` element here:
    /// http://msdn.microsoft.com/en-us/library/dd183569(v=vs.85).aspx
    flags: winapi::DWORD,

    /// True if this is the primary monitor.
    primary: bool,

    /// The position of the monitor in pixels on the desktop.
    ///
    /// A window that is positionned at these coordinates will overlap the monitor.
    position: (u32, u32),

    /// The current resolution in pixels on the monitor.
    dimensions: (u32, u32),
}

struct DeviceEnumerator {
    parent_device: *const winapi::WCHAR,
    current_index: u32,
}

impl DeviceEnumerator {
    fn adapters() -> DeviceEnumerator {
        use std::ptr;
        DeviceEnumerator {
            parent_device: ptr::null(),
            current_index: 0
        }
    }

    fn monitors(adapter_name: *const winapi::WCHAR) -> DeviceEnumerator {
        DeviceEnumerator {
            parent_device: adapter_name,
            current_index: 0
        }
    }
}

impl Iterator for DeviceEnumerator {
    type Item = winapi::DISPLAY_DEVICEW;
    fn next(&mut self) -> Option<winapi::DISPLAY_DEVICEW> {
        use std::mem;
        loop {
            let mut output: winapi::DISPLAY_DEVICEW = unsafe { mem::zeroed() };
            output.cb = mem::size_of::<winapi::DISPLAY_DEVICEW>() as winapi::DWORD;

            if unsafe { user32::EnumDisplayDevicesW(self.parent_device,
                self.current_index as winapi::DWORD, &mut output, 0) } == 0
            {
                // the device doesn't exist, which means we have finished enumerating
                break;
            }
            self.current_index += 1;

            if  (output.StateFlags & winapi::DISPLAY_DEVICE_ACTIVE) == 0 ||
                (output.StateFlags & winapi::DISPLAY_DEVICE_MIRRORING_DRIVER) != 0
            {
                // the device is not active
                // the Win32 api usually returns a lot of inactive devices
                continue;
            }

            return Some(output);
        }
        None
    }
}

fn wchar_as_string(wchar: &[winapi::WCHAR]) -> String {
    String::from_utf16_lossy(wchar)
        .trim_right_matches(0 as char)
        .to_string()
}

/// Win32 implementation of the main `get_available_monitors` function.
pub fn get_available_monitors() -> VecDeque<MonitorID> {
    // return value
    let mut result = VecDeque::new();

    for adapter in DeviceEnumerator::adapters() {
        // getting the position
        let (position, dimensions) = unsafe {
            let mut dev: winapi::DEVMODEW = mem::zeroed();
            dev.dmSize = mem::size_of::<winapi::DEVMODEW>() as winapi::WORD;

            if user32::EnumDisplaySettingsExW(adapter.DeviceName.as_ptr(), 
                winapi::ENUM_CURRENT_SETTINGS,
                &mut dev, 0) == 0
            {
                continue;
            }

            let point: &winapi::POINTL = mem::transmute(&dev.union1);
            let position = (point.x as u32, point.y as u32);

            let dimensions = (dev.dmPelsWidth as u32, dev.dmPelsHeight as u32);

            (position, dimensions)
        };

        for (num, monitor) in DeviceEnumerator::monitors(adapter.DeviceName.as_ptr()).enumerate() {
            // adding to the resulting list
            result.push_back(MonitorID {
                adapter_name: adapter.DeviceName,
                monitor_name: wchar_as_string(&monitor.DeviceName),
                readable_name: wchar_as_string(&monitor.DeviceString),
                flags: monitor.StateFlags,
                primary: (adapter.StateFlags & winapi::DISPLAY_DEVICE_PRIMARY_DEVICE) != 0 &&
                         num == 0,
                position: position,
                dimensions: dimensions,
            });
        }
    }
    result
}

/// Win32 implementation of the main `get_primary_monitor` function.
pub fn get_primary_monitor() -> MonitorID {
    // we simply get all available monitors and return the one with the `PRIMARY_DEVICE` flag
    // TODO: it is possible to query the win32 API for the primary monitor, this should be done
    //  instead
    for monitor in get_available_monitors().into_iter() {
        if monitor.primary {
            return monitor;
        }
    }

    panic!("Failed to find the primary monitor")
}

impl MonitorID {
    /// See the docs if the crate root file.
    pub fn get_name(&self) -> Option<String> {
        Some(self.readable_name.clone())
    }

    /// See the docs of the crate root file.
    pub fn get_native_identifier(&self) -> NativeMonitorId {
        NativeMonitorId::Name(self.monitor_name.clone())
    }

    /// See the docs if the crate root file.
    pub fn get_dimensions(&self) -> (u32, u32) {
        // TODO: retreive the dimensions every time this is called
        self.dimensions
    }

    /// This is a Win32-only function for `MonitorID` that returns the system name of the adapter
    /// device.
    pub fn get_adapter_name(&self) -> &[winapi::WCHAR] {
        &self.adapter_name
    }

    /// This is a Win32-only function for `MonitorID` that returns the position of the
    ///  monitor on the desktop.
    /// A window that is positionned at these coordinates will overlap the monitor.
    pub fn get_position(&self) -> (u32, u32) {
        self.position
    }
}
