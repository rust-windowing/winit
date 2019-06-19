use std::{collections::VecDeque, fmt};

use cocoa::{
    appkit::NSScreen,
    base::{id, nil},
    foundation::{NSString, NSUInteger},
};
use core_graphics::display::{CGDirectDisplayID, CGDisplay, CGDisplayBounds, CGDisplayMode};
use core_video_sys::{
    kCVReturnSuccess, kCVTimeIsIndefinite, CVDisplayLinkCreateWithCGDisplay,
    CVDisplayLinkGetNominalOutputVideoRefreshPeriod, CVDisplayLinkRelease,
};

use crate::dpi::{PhysicalPosition, PhysicalSize};
use crate::monitor::VideoMode;
use crate::platform_impl::platform::util::IdRef;

#[derive(Clone, PartialEq)]
pub struct MonitorHandle(CGDirectDisplayID);

pub fn available_monitors() -> VecDeque<MonitorHandle> {
    if let Ok(displays) = CGDisplay::active_displays() {
        let mut monitors = VecDeque::with_capacity(displays.len());
        for display in displays {
            monitors.push_back(MonitorHandle(display));
        }
        monitors
    } else {
        VecDeque::with_capacity(0)
    }
}

pub fn primary_monitor() -> MonitorHandle {
    MonitorHandle(CGDisplay::main().id)
}

impl fmt::Debug for MonitorHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // TODO: Do this using the proper fmt API
        #[derive(Debug)]
        struct MonitorHandle {
            name: Option<String>,
            native_identifier: u32,
            size: PhysicalSize,
            position: PhysicalPosition,
            hidpi_factor: f64,
        }

        let monitor_id_proxy = MonitorHandle {
            name: self.name(),
            native_identifier: self.native_identifier(),
            size: self.size(),
            position: self.position(),
            hidpi_factor: self.hidpi_factor(),
        };

        monitor_id_proxy.fmt(f)
    }
}

impl MonitorHandle {
    pub fn new(id: CGDirectDisplayID) -> Self {
        MonitorHandle(id)
    }

    pub fn name(&self) -> Option<String> {
        let MonitorHandle(display_id) = *self;
        let screen_num = CGDisplay::new(display_id).model_number();
        Some(format!("Monitor #{}", screen_num))
    }

    #[inline]
    pub fn native_identifier(&self) -> u32 {
        self.0
    }

    pub fn size(&self) -> PhysicalSize {
        let MonitorHandle(display_id) = *self;
        let display = CGDisplay::new(display_id);
        let height = display.pixels_high();
        let width = display.pixels_wide();
        PhysicalSize::from_logical(
            (width as f64, height as f64),
            self.hidpi_factor(),
        )
    }

    #[inline]
    pub fn position(&self) -> PhysicalPosition {
        let bounds = unsafe { CGDisplayBounds(self.native_identifier()) };
        PhysicalPosition::from_logical(
            (bounds.origin.x as f64, bounds.origin.y as f64),
            self.hidpi_factor(),
        )
    }

    pub fn hidpi_factor(&self) -> f64 {
        let screen = match self.ns_screen() {
            Some(screen) => screen,
            None => return 1.0, // default to 1.0 when we can't find the screen
        };
        unsafe { NSScreen::backingScaleFactor(screen) as f64 }
    }

    pub fn video_modes(&self) -> impl Iterator<Item = VideoMode> {
        let cv_refresh_rate = unsafe {
            let mut display_link = std::ptr::null_mut();
            assert_eq!(
                CVDisplayLinkCreateWithCGDisplay(self.0, &mut display_link),
                kCVReturnSuccess
            );
            let time = CVDisplayLinkGetNominalOutputVideoRefreshPeriod(display_link);
            CVDisplayLinkRelease(display_link);

            // This value is indefinite if an invalid display link was specified
            assert!(time.flags & kCVTimeIsIndefinite == 0);

            time.timeScale as i64 / time.timeValue
        };

        CGDisplayMode::all_display_modes(self.0, std::ptr::null())
            .expect("failed to obtain list of display modes")
            .into_iter()
            .map(move |mode| {
                let cg_refresh_rate = mode.refresh_rate().round() as i64;

                // CGDisplayModeGetRefreshRate returns 0.0 for any display that
                // isn't a CRT
                let refresh_rate = if cg_refresh_rate > 0 {
                    cg_refresh_rate
                } else {
                    cv_refresh_rate
                };

                VideoMode {
                    size: (mode.width() as u32, mode.height() as u32),
                    refresh_rate: refresh_rate as u16,
                    bit_depth: mode.bit_depth() as u16,
                }
            })
    }

    pub(crate) fn ns_screen(&self) -> Option<id> {
        unsafe {
            let native_id = self.native_identifier();
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
