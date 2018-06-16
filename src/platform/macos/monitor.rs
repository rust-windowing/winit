use std::collections::VecDeque;
use std::fmt;

use cocoa::appkit::NSScreen;
use cocoa::base::{id, nil};
use cocoa::foundation::{NSString, NSUInteger};
use core_graphics::display::{CGDirectDisplayID, CGDisplay, CGDisplayBounds};

use {PhysicalPosition, PhysicalSize};
use super::EventsLoop;
use super::window::{IdRef, Window2};

#[derive(Clone, PartialEq)]
pub struct MonitorId(CGDirectDisplayID);

fn get_available_monitors() -> VecDeque<MonitorId> {
    if let Ok(displays) = CGDisplay::active_displays() {
        let mut monitors = VecDeque::with_capacity(displays.len());
        for d in displays {
            monitors.push_back(MonitorId(d));
        }
        monitors
    } else {
        VecDeque::with_capacity(0)
    }
}

pub fn get_primary_monitor() -> MonitorId {
    let id = MonitorId(CGDisplay::main().id);
    id
}

impl EventsLoop {
    #[inline]
    pub fn get_available_monitors(&self) -> VecDeque<MonitorId> {
        get_available_monitors()
    }

    #[inline]
    pub fn get_primary_monitor(&self) -> MonitorId {
        get_primary_monitor()
    }

    pub fn make_monitor_from_display(id: CGDirectDisplayID) -> MonitorId {
        let id = MonitorId(id);
        id
    }
}

impl Window2 {
    #[inline]
    pub fn get_available_monitors(&self) -> VecDeque<MonitorId> {
        get_available_monitors()
    }

    #[inline]
    pub fn get_primary_monitor(&self) -> MonitorId {
        get_primary_monitor()
    }
}

impl fmt::Debug for MonitorId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        #[derive(Debug)]
        struct MonitorId {
            name: Option<String>,
            native_identifier: u32,
            dimensions: PhysicalSize,
            position: PhysicalPosition,
            hidpi_factor: f64,
        }

        let monitor_id_proxy = MonitorId {
            name: self.get_name(),
            native_identifier: self.get_native_identifier(),
            dimensions: self.get_dimensions(),
            position: self.get_position(),
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

    pub fn get_dimensions(&self) -> PhysicalSize {
        let MonitorId(display_id) = *self;
        let display = CGDisplay::new(display_id);
        let height = display.pixels_high();
        let width = display.pixels_wide();
        PhysicalSize::from_logical(
            (width as f64, height as f64),
            self.get_hidpi_factor(),
        )
    }

    #[inline]
    pub fn get_position(&self) -> PhysicalPosition {
        let bounds = unsafe { CGDisplayBounds(self.get_native_identifier()) };
        PhysicalPosition::from_logical(
            (bounds.origin.x as f64, bounds.origin.y as f64),
            self.get_hidpi_factor(),
        )
    }

    pub fn get_hidpi_factor(&self) -> f64 {
        let screen = match self.get_nsscreen() {
            Some(screen) => screen,
            None => return 1.0, // default to 1.0 when we can't find the screen
        };
        unsafe { NSScreen::backingScaleFactor(screen) as f64 }
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
