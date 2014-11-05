use core_graphics::display;

pub struct MonitorID(u32);

pub fn get_available_monitors() -> Vec<MonitorID> {
    let mut monitors = Vec::new();
    unsafe {
        let max_displays = 10u32;
        let mut active_displays = [0u32, ..10];
        let mut display_count = 0;
        display::CGGetActiveDisplayList(max_displays,
                                                        &mut active_displays[0],
                                                        &mut display_count);
        for i in range(0u, display_count as uint) {
            monitors.push(MonitorID(active_displays[i]));
        }
    }
    monitors
}

pub fn get_primary_monitor() -> MonitorID {
    let id = unsafe {
        MonitorID(display::CGMainDisplayID())
    };
    id
}

impl MonitorID {
    pub fn get_name(&self) -> Option<String> {
        let MonitorID(display_id) = *self;
        let screen_num = unsafe {
            display::CGDisplayModelNumber(display_id)
        };
        Some(format!("Monitor #{}", screen_num))
    }

    pub fn get_dimensions(&self) -> (uint, uint) {
        let MonitorID(display_id) = *self;
        let dimension = unsafe {
            let height = display::CGDisplayPixelsHigh(display_id);
            let width = display::CGDisplayPixelsWide(display_id);
            (width as uint, height as uint)
        };
        dimension
    }
}
