use cocoa::appkit::NSScreen;
use cocoa::base::{id, nil};
use cocoa::foundation::{NSString, NSUInteger};
use core_graphics::display::{CGDirectDisplayID, CGDisplay};
use std::collections::VecDeque;
use std::fmt;
use super::EventsLoop;
use super::window::IdRef;

#[derive(Clone, PartialEq)]
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

    pub fn make_monitor_from_display(id: CGDirectDisplayID) -> MonitorId {
        let id = MonitorId(id);
        id
    }
}

impl fmt::Debug for MonitorId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        #[derive(Debug)]
        struct MonitorId {
            name: Option<String>,
            native_identifier: u32,
            dimensions: (u32, u32),
            position: &'static str,
            hidpi_factor: f32,
        }

        let monitor_id_proxy = MonitorId {
            name: self.get_name(),
            native_identifier: self.get_native_identifier(),
            dimensions: self.get_dimensions(),
            position: "WARNING: `MonitorId::get_position` is unimplemented on macOS!",
            hidpi_factor: self.get_hidpi_factor(),
        };

        monitor_id_proxy.fmt(f)
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

    pub fn get_hidpi_factor(&self) -> f32 {
        let screen = match self.get_nsscreen() {
            Some(screen) => screen,
            None => return 1.0, // default to 1.0 when we can't find the screen
        };

        unsafe { NSScreen::backingScaleFactor(screen) as f32 }
    }

    pub(crate) fn get_nsscreen(&self) -> Option<id> {
        unsafe {
            let native_id = self.get_native_identifier();
            let screens = NSScreen::screens(nil);
            let count: NSUInteger = msg_send![screens, count];
            let key = IdRef::new(NSString::alloc(nil).init_str("NSScreenNumber"));
            let mut matching_screen: Option<id> = None;
            for i in 0..count {
                let screen = msg_send![screens, objectAtIndex: i as NSUInteger];
                let device_description = NSScreen::deviceDescription(screen);
                let value: id = msg_send![device_description, objectForKey:*key];
                if value != nil {
                    let screen_number: NSUInteger = msg_send![value, unsignedIntegerValue];
                    if screen_number as u32 == native_id {
                        matching_screen = Some(screen);
                        break;
                    }
                }
            }
            matching_screen
        }
    }
}
