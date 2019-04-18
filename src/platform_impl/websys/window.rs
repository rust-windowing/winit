
#[derive(Clone, Copy)]
pub struct DeviceId;

pub struct PlatformSpecificWindowBuilderAttributes;

pub struct MonitorHandle;

pub struct Window {

}

#[derive(Clone, Copy)]
pub struct WindowId {

}

impl WindowId {
    pub fn new() -> WindowId {
        WindowId {}
    }

    pub fn dummy() -> WindowId {
        WindowId {}
    }
}