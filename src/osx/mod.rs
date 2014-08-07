//! Dummy implementation for OS/X to make gl-init-rs compile on this platform

use {Event, WindowBuilder};

pub struct Window;

pub struct MonitorID;

pub fn get_available_monitors() -> Vec<MonitorID> {
    unimplemented!()
}

pub fn get_primary_monitor() -> MonitorID {
    unimplemented!()
}

impl MonitorID {
    pub fn get_name(&self) -> Option<String> {
        unimplemented!()
    }

    pub fn get_dimensions(&self) -> (uint, uint) {
        unimplemented!()
    }
}

impl Window {
    pub fn new(_builder: WindowBuilder) -> Result<Window, String> {
        unimplemented!()
    }

    pub fn is_closed(&self) -> bool {
        unimplemented!()
    }

    pub fn set_title(&self, _title: &str) {
        unimplemented!()
    }

    pub fn get_position(&self) -> Option<(int, int)> {
        unimplemented!()
    }

    pub fn set_position(&self, _x: int, _y: int) {
        unimplemented!()
    }

    pub fn get_inner_size(&self) -> Option<(uint, uint)> {
        unimplemented!()
    }

    pub fn get_outer_size(&self) -> Option<(uint, uint)> {
        unimplemented!()
    }

    pub fn set_inner_size(&self, _x: uint, _y: uint) {
        unimplemented!()
    }

    pub fn poll_events(&self) -> Vec<Event> {
        unimplemented!()
    }

    pub fn wait_events(&self) -> Vec<Event> {
        unimplemented!()
    }

    pub unsafe fn make_current(&self) {
        unimplemented!()
    }

    pub fn get_proc_address(&self, _addr: &str) -> *const () {
        unimplemented!()
    }

    pub fn swap_buffers(&self) {
        unimplemented!()
    }
}
