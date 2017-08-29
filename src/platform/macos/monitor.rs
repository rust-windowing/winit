use core_graphics::display;
use std::collections::VecDeque;
use native_monitor::NativeMonitorId;

#[derive(Clone)]
pub struct MonitorId(u32);

pub fn get_available_monitors() -> VecDeque<MonitorId> {
    let mut monitors = VecDeque::new();
    unsafe {
        let max_displays = 10u32;
        let mut active_displays = [0u32; 10];
        let mut display_count = 0;
        display::CGGetActiveDisplayList(max_displays, &mut active_displays[0], &mut display_count);
        for i in 0..display_count as usize {
            monitors.push_back(MonitorId(active_displays[i]));
        }
    }
    monitors
}

#[inline]
pub fn get_primary_monitor() -> MonitorId {
    let id = unsafe { MonitorId(display::CGMainDisplayID()) };
    id
}

impl MonitorId {
    pub fn get_name(&self) -> Option<String> {
        let MonitorId(display_id) = *self;
        let screen_num = unsafe { display::CGDisplayModelNumber(display_id) };
        Some(format!("Monitor #{}", screen_num))
    }

    #[inline]
    pub fn get_native_identifier(&self) -> NativeMonitorId {
        let MonitorId(display_id) = *self;
        NativeMonitorId::Numeric(display_id)
    }

    pub fn get_dimensions(&self) -> (u32, u32) {
        let MonitorId(display_id) = *self;
        let dimension = unsafe {
            let height = display::CGDisplayPixelsHigh(display_id);
            let width = display::CGDisplayPixelsWide(display_id);
            (width as u32, height as u32)
        };
        dimension
    }
}
