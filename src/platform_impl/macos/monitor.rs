use std::{collections::VecDeque, fmt};

use super::ffi;
use crate::{
    dpi::{PhysicalPosition, PhysicalSize},
    monitor::{MonitorHandle as RootMonitorHandle, VideoMode as RootVideoMode},
    platform_impl::platform::util::IdRef,
};
use cocoa::{
    appkit::NSScreen,
    base::{id, nil},
    foundation::{NSString, NSUInteger},
};
use core_foundation::{
    array::{CFArrayGetCount, CFArrayGetValueAtIndex},
    base::{CFRelease, TCFType},
    string::CFString,
};
use core_graphics::display::{CGDirectDisplayID, CGDisplay, CGDisplayBounds};
use core_video_sys::{
    kCVReturnSuccess, kCVTimeIsIndefinite, CVDisplayLinkCreateWithCGDisplay,
    CVDisplayLinkGetNominalOutputVideoRefreshPeriod, CVDisplayLinkRelease,
};

#[derive(Derivative)]
#[derivative(Debug, Clone, PartialEq, Hash)]
pub struct VideoMode {
    pub(crate) size: (u32, u32),
    pub(crate) bit_depth: u16,
    pub(crate) refresh_rate: u16,
    pub(crate) monitor: MonitorHandle,
    #[derivative(Debug = "ignore", PartialEq = "ignore", Hash = "ignore")]
    pub(crate) native_mode: NativeDisplayMode,
}

pub struct NativeDisplayMode(pub ffi::CGDisplayModeRef);

unsafe impl Send for NativeDisplayMode {}

impl Drop for NativeDisplayMode {
    fn drop(&mut self) {
        unsafe {
            ffi::CGDisplayModeRelease(self.0);
        }
    }
}

impl Clone for NativeDisplayMode {
    fn clone(&self) -> Self {
        unsafe {
            ffi::CGDisplayModeRetain(self.0);
        }
        NativeDisplayMode(self.0)
    }
}

impl VideoMode {
    pub fn size(&self) -> PhysicalSize {
        self.size.into()
    }

    pub fn bit_depth(&self) -> u16 {
        self.bit_depth
    }

    pub fn refresh_rate(&self) -> u16 {
        self.refresh_rate
    }

    pub fn monitor(&self) -> RootMonitorHandle {
        RootMonitorHandle {
            inner: self.monitor.clone(),
        }
    }
}

#[derive(Clone)]
pub struct MonitorHandle(CGDirectDisplayID);

// `CGDirectDisplayID` changes on video mode change, so we cannot rely on that
// for comparisons, but we can use `CGDisplayCreateUUIDFromDisplayID` to get an
// unique identifier that persists even across system reboots
impl PartialEq for MonitorHandle {
    fn eq(&self, other: &Self) -> bool {
        unsafe {
            ffi::CGDisplayCreateUUIDFromDisplayID(self.0)
                == ffi::CGDisplayCreateUUIDFromDisplayID(other.0)
        }
    }
}

impl Eq for MonitorHandle {}

impl PartialOrd for MonitorHandle {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(&other))
    }
}

impl Ord for MonitorHandle {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        unsafe {
            ffi::CGDisplayCreateUUIDFromDisplayID(self.0)
                .cmp(&ffi::CGDisplayCreateUUIDFromDisplayID(other.0))
        }
    }
}

impl std::hash::Hash for MonitorHandle {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        unsafe {
            ffi::CGDisplayCreateUUIDFromDisplayID(self.0).hash(state);
        }
    }
}

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
        PhysicalSize::from_logical((width as f64, height as f64), self.hidpi_factor())
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

    pub fn video_modes(&self) -> impl Iterator<Item = RootVideoMode> {
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

        let monitor = self.clone();

        unsafe {
            let modes = {
                let array = ffi::CGDisplayCopyAllDisplayModes(self.0, std::ptr::null());
                assert!(!array.is_null(), "failed to get list of display modes");
                let array_count = CFArrayGetCount(array);
                let modes: Vec<_> = (0..array_count)
                    .into_iter()
                    .map(move |i| {
                        let mode = CFArrayGetValueAtIndex(array, i) as *mut _;
                        ffi::CGDisplayModeRetain(mode);
                        mode
                    })
                    .collect();
                CFRelease(array as *const _);
                modes
            };

            modes.into_iter().map(move |mode| {
                let cg_refresh_rate = ffi::CGDisplayModeGetRefreshRate(mode).round() as i64;

                // CGDisplayModeGetRefreshRate returns 0.0 for any display that
                // isn't a CRT
                let refresh_rate = if cg_refresh_rate > 0 {
                    cg_refresh_rate
                } else {
                    cv_refresh_rate
                };

                let pixel_encoding =
                    CFString::wrap_under_create_rule(ffi::CGDisplayModeCopyPixelEncoding(mode))
                        .to_string();
                let bit_depth = if pixel_encoding.eq_ignore_ascii_case(ffi::IO32BitDirectPixels) {
                    32
                } else if pixel_encoding.eq_ignore_ascii_case(ffi::IO16BitDirectPixels) {
                    16
                } else if pixel_encoding.eq_ignore_ascii_case(ffi::kIO30BitDirectPixels) {
                    30
                } else {
                    unimplemented!()
                };

                let video_mode = VideoMode {
                    size: (
                        ffi::CGDisplayModeGetPixelWidth(mode) as u32,
                        ffi::CGDisplayModeGetPixelHeight(mode) as u32,
                    ),
                    refresh_rate: refresh_rate as u16,
                    bit_depth,
                    monitor: monitor.clone(),
                    native_mode: NativeDisplayMode(mode),
                };

                RootVideoMode { video_mode }
            })
        }
    }

    pub(crate) fn ns_screen(&self) -> Option<id> {
        unsafe {
            let uuid = ffi::CGDisplayCreateUUIDFromDisplayID(self.0);
            let screens = NSScreen::screens(nil);
            let count: NSUInteger = msg_send![screens, count];
            let key = IdRef::new(NSString::alloc(nil).init_str("NSScreenNumber"));
            for i in 0..count {
                let screen = msg_send![screens, objectAtIndex: i as NSUInteger];
                let device_description = NSScreen::deviceDescription(screen);
                let value: id = msg_send![device_description, objectForKey:*key];
                if value != nil {
                    let other_native_id: NSUInteger = msg_send![value, unsignedIntegerValue];
                    let other_uuid =
                        ffi::CGDisplayCreateUUIDFromDisplayID(other_native_id as CGDirectDisplayID);
                    if uuid == other_uuid {
                        return Some(screen);
                    }
                }
            }
            None
        }
    }
}
