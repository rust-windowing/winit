use std::collections::VecDeque;
use std::sync::Arc;

pub struct WaylandContext;

impl WaylandContext {
    pub fn init() -> Option<WaylandContext> {
        None
    }
}

//
// Monitor stuff
//

pub fn get_primary_monitor(ctxt: &Arc<WaylandContext>) -> MonitorId {
    unimplemented!()
}

pub fn get_available_monitors(ctxt: &Arc<WaylandContext>) -> VecDeque<MonitorId> {
    unimplemented!()
}

#[derive(Clone)]
pub struct MonitorId;

impl MonitorId {
    pub fn get_name(&self) -> Option<String> {
        unimplemented!()
    }

    #[inline]
    pub fn get_native_identifier(&self) -> u32 {
        unimplemented!()
    }

    pub fn get_dimensions(&self) -> (u32, u32) {
        unimplemented!()
    }

    pub fn get_position(&self) -> (i32, i32) {
            unimplemented!()
    }

    #[inline]
    pub fn get_hidpi_factor(&self) -> f32 {
        unimplemented!()
    }
}
