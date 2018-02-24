use core_graphics::display::{CGDirectDisplayID, CGDisplay};
use std::collections::VecDeque;
use super::EventsLoop;

#[derive(Clone)]
pub struct MonitorId(CGDirectDisplayID);

impl EventsLoop {
    pub fn get_available_monitors(&self) -> VecDeque<MonitorId> {
        let mut monitors = VecDeque::new();
        if let Ok(displays) = CGDisplay::active_displays() {
            for d in displays {
                monitors.push_back(MonitorId(d));
            }
        }
        monitors
    }

    #[inline]
    pub fn get_primary_monitor(&self) -> MonitorId {
        let id = MonitorId(CGDisplay::main().id);
        id
    }
}

impl MonitorId {
    pub fn get_name(&self) -> Option<String> {
        let MonitorId(display_id) = *self;
        let screen_num = CGDisplay::new(display_id).model_number();
        Some(format!("Monitor #{}", screen_num))
    }

    #[inline]
    pub fn get_native_identifier(&self) -> u32 {
        self.0
    }

    pub fn get_dimensions(&self) -> (u32, u32) {
        let MonitorId(display_id) = *self;
        let display = CGDisplay::new(display_id);
        let dimension = {
            let height = display.pixels_high();
            let width = display.pixels_wide();
            (width as u32, height as u32)
        };
        dimension
    }

    #[inline]
    pub fn get_position(&self) -> (i32, i32) {
        unimplemented!()
    }

    #[inline]
    pub fn get_hidpi_factor(&self) -> f32 {
        1.0
    }
}
