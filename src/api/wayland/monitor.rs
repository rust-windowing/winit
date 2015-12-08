use std::collections::VecDeque;

#[derive(Clone)]
pub struct MonitorId;

#[inline]
pub fn get_available_monitors() -> VecDeque<MonitorId> {
    unimplemented!()
}
#[inline]
pub fn get_primary_monitor() -> MonitorId {
    unimplemented!()
}

impl MonitorId {
    pub fn get_name(&self) -> Option<String> {
        unimplemented!()
    }

    #[inline]
    pub fn get_native_identifier(&self) -> ::native_monitor::NativeMonitorId {
        unimplemented!()
    }

    pub fn get_dimensions(&self) -> (u32, u32) {
        unimplemented!()
    }
}