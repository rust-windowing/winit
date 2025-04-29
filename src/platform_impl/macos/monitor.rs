#![allow(clippy::unnecessary_cast)]

use std::collections::VecDeque;
use std::fmt;

use core_foundation::array::{CFArrayGetCount, CFArrayGetValueAtIndex};
use core_foundation::base::{CFRelease, TCFType};
use core_foundation::string::CFString;
use core_foundation::uuid::{CFUUIDGetUUIDBytes, CFUUID};
use core_graphics::display::{
    CGDirectDisplayID, CGDisplay, CGDisplayBounds, CGDisplayCopyDisplayMode,
};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::NSScreen;
use objc2_foundation::{ns_string, run_on_main, MainThreadMarker, NSNumber, NSPoint, NSRect};
use tracing::warn;

use super::ffi;
use crate::dpi::{LogicalPosition, PhysicalPosition, PhysicalSize};

#[derive(Clone)]
pub struct VideoModeHandle {
    size: PhysicalSize<u32>,
    bit_depth: u16,
    refresh_rate_millihertz: u32,
    pub(crate) monitor: MonitorHandle,
    pub(crate) native_mode: NativeDisplayMode,
}

impl PartialEq for VideoModeHandle {
    fn eq(&self, other: &Self) -> bool {
        self.size == other.size
            && self.bit_depth == other.bit_depth
            && self.refresh_rate_millihertz == other.refresh_rate_millihertz
            && self.monitor == other.monitor
    }
}

impl Eq for VideoModeHandle {}

impl std::hash::Hash for VideoModeHandle {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.size.hash(state);
        self.bit_depth.hash(state);
        self.refresh_rate_millihertz.hash(state);
        self.monitor.hash(state);
    }
}

impl std::fmt::Debug for VideoModeHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoModeHandle")
            .field("size", &self.size)
            .field("bit_depth", &self.bit_depth)
            .field("refresh_rate_millihertz", &self.refresh_rate_millihertz)
            .field("monitor", &self.monitor)
            .finish()
    }
}

pub struct NativeDisplayMode(pub ffi::CGDisplayModeRef);

unsafe impl Send for NativeDisplayMode {}
unsafe impl Sync for NativeDisplayMode {}

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

impl VideoModeHandle {
    pub fn size(&self) -> PhysicalSize<u32> {
        self.size
    }

    pub fn bit_depth(&self) -> u16 {
        self.bit_depth
    }

    pub fn refresh_rate_millihertz(&self) -> u32 {
        self.refresh_rate_millihertz
    }

    pub fn monitor(&self) -> MonitorHandle {
        self.monitor.clone()
    }
}

/// `CGDirectDisplayID` is documented as:
/// > a framebuffer, a color correction (gamma) table, and possibly an attached monitor.
///
/// That is, it doesn't actually represent the monitor itself. Instead, we use the UUID of the
/// monitor, as retrieved from `CGDisplayCreateUUIDFromDisplayID` (this makes the monitor ID stable,
/// even across reboots and video mode changes).
///
/// NOTE: I'd be perfectly valid to store `[u8; 16]` in here instead, we only store `CFUUID` to
/// avoid having to re-create it when we want to fetch the display ID.
#[derive(Clone)]
pub struct MonitorHandle(CFUUID);

// SAFETY: CFUUID is immutable.
// FIXME(madsmtm): Upstream this into `objc2-core-foundation`.
unsafe impl Send for MonitorHandle {}
unsafe impl Sync for MonitorHandle {}

type MonitorUuid = [u8; 16];

impl MonitorHandle {
    /// Internal comparisons of [`MonitorHandle`]s are done first requesting a UUID for the handle.
    fn uuid(&self) -> MonitorUuid {
        let uuid = unsafe { CFUUIDGetUUIDBytes(self.0.as_concrete_TypeRef()) };
        MonitorUuid::from([
            uuid.byte0,
            uuid.byte1,
            uuid.byte2,
            uuid.byte3,
            uuid.byte4,
            uuid.byte5,
            uuid.byte6,
            uuid.byte7,
            uuid.byte8,
            uuid.byte9,
            uuid.byte10,
            uuid.byte11,
            uuid.byte12,
            uuid.byte13,
            uuid.byte14,
            uuid.byte15,
        ])
    }

    fn display_id(&self) -> CGDirectDisplayID {
        unsafe { ffi::CGDisplayGetDisplayIDFromUUID(self.0.as_concrete_TypeRef()) }
    }

    #[track_caller]
    pub(crate) fn new(display_id: CGDirectDisplayID) -> Option<Self> {
        // kCGNullDirectDisplay
        if display_id == 0 {
            // `CGDisplayCreateUUIDFromDisplayID` checks kCGNullDirectDisplay internally.
            warn!("constructing monitor from invalid display ID 0; falling back to main monitor");
        }
        let ptr = unsafe { ffi::CGDisplayCreateUUIDFromDisplayID(display_id) };
        if ptr.is_null() {
            return None;
        }
        Some(Self(unsafe { CFUUID::wrap_under_create_rule(ptr) }))
    }
}

impl PartialEq for MonitorHandle {
    fn eq(&self, other: &Self) -> bool {
        self.uuid() == other.uuid()
    }
}

impl Eq for MonitorHandle {}

impl PartialOrd for MonitorHandle {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MonitorHandle {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.uuid().cmp(&other.uuid())
    }
}

impl std::hash::Hash for MonitorHandle {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.uuid().hash(state);
    }
}

pub fn available_monitors() -> VecDeque<MonitorHandle> {
    if let Ok(displays) = CGDisplay::active_displays() {
        let mut monitors = VecDeque::with_capacity(displays.len());
        for display in displays {
            // Display ID just fetched from `CGGetActiveDisplayList`, should be fine to unwrap.
            monitors.push_back(MonitorHandle::new(display).expect("invalid display ID"));
        }
        monitors
    } else {
        VecDeque::with_capacity(0)
    }
}

pub fn primary_monitor() -> MonitorHandle {
    // Display ID just fetched from `CGMainDisplayID`, should be fine to unwrap.
    MonitorHandle::new(CGDisplay::main().id).expect("invalid display ID")
}

impl fmt::Debug for MonitorHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MonitorHandle")
            .field("name", &self.name())
            .field("native_identifier", &self.native_identifier())
            .field("size", &self.size())
            .field("position", &self.position())
            .field("scale_factor", &self.scale_factor())
            .field("refresh_rate_millihertz", &self.refresh_rate_millihertz())
            .finish_non_exhaustive()
    }
}

impl MonitorHandle {
    // TODO: Be smarter about this:
    // <https://github.com/glfw/glfw/blob/57cbded0760a50b9039ee0cb3f3c14f60145567c/src/cocoa_monitor.m#L44-L126>
    pub fn name(&self) -> Option<String> {
        let screen_num = CGDisplay::new(self.display_id()).model_number();
        Some(format!("Monitor #{screen_num}"))
    }

    #[inline]
    pub fn native_identifier(&self) -> u32 {
        self.display_id()
    }

    pub fn size(&self) -> PhysicalSize<u32> {
        let display = CGDisplay::new(self.display_id());
        let height = display.pixels_high();
        let width = display.pixels_wide();
        PhysicalSize::from_logical::<_, f64>((width as f64, height as f64), self.scale_factor())
    }

    #[inline]
    pub fn position(&self) -> PhysicalPosition<i32> {
        // This is already in screen coordinates. If we were using `NSScreen`,
        // then a conversion would've been needed:
        // flip_window_screen_coordinates(self.ns_screen(mtm)?.frame())
        let bounds = unsafe { CGDisplayBounds(self.native_identifier()) };
        let position = LogicalPosition::new(bounds.origin.x, bounds.origin.y);
        position.to_physical(self.scale_factor())
    }

    pub fn scale_factor(&self) -> f64 {
        run_on_main(|mtm| {
            match self.ns_screen(mtm) {
                Some(screen) => screen.backingScaleFactor() as f64,
                None => 1.0, // default to 1.0 when we can't find the screen
            }
        })
    }

    pub fn refresh_rate_millihertz(&self) -> Option<u32> {
        unsafe {
            let current_display_mode =
                NativeDisplayMode(CGDisplayCopyDisplayMode(self.display_id()) as _);
            let refresh_rate = ffi::CGDisplayModeGetRefreshRate(current_display_mode.0);
            if refresh_rate > 0.0 {
                return Some((refresh_rate * 1000.0).round() as u32);
            }

            let mut display_link = std::ptr::null_mut();
            if ffi::CVDisplayLinkCreateWithCGDisplay(self.display_id(), &mut display_link)
                != ffi::kCVReturnSuccess
            {
                return None;
            }
            let time = ffi::CVDisplayLinkGetNominalOutputVideoRefreshPeriod(display_link);
            ffi::CVDisplayLinkRelease(display_link);

            // This value is indefinite if an invalid display link was specified
            if time.flags & ffi::kCVTimeIsIndefinite != 0 {
                return None;
            }

            (time.time_scale as i64).checked_div(time.time_value).map(|v| (v * 1000) as u32)
        }
    }

    pub fn video_modes(&self) -> impl Iterator<Item = VideoModeHandle> {
        let refresh_rate_millihertz = self.refresh_rate_millihertz().unwrap_or(0);
        let monitor = self.clone();

        unsafe {
            let modes = {
                let array = ffi::CGDisplayCopyAllDisplayModes(self.display_id(), std::ptr::null());
                if array.is_null() {
                    // Occasionally, certain CalDigit Thunderbolt Hubs report a spurious monitor
                    // during sleep/wake/cycling monitors. It tends to have null
                    // or 1 video mode only. See <https://github.com/bevyengine/bevy/issues/17827>.
                    warn!(monitor = ?self, "failed to get a list of display modes");
                    Vec::new()
                } else {
                    let array_count = CFArrayGetCount(array);
                    let modes: Vec<_> = (0..array_count)
                        .map(move |i| {
                            let mode = CFArrayGetValueAtIndex(array, i) as *mut _;
                            ffi::CGDisplayModeRetain(mode);
                            mode
                        })
                        .collect();
                    CFRelease(array as *const _);
                    modes
                }
            };

            modes.into_iter().map(move |mode| {
                let cg_refresh_rate_hertz = ffi::CGDisplayModeGetRefreshRate(mode);

                // CGDisplayModeGetRefreshRate returns 0.0 for any display that
                // isn't a CRT
                let refresh_rate_millihertz = if cg_refresh_rate_hertz > 0.0 {
                    (cg_refresh_rate_hertz * 1000.0).round() as u32
                } else {
                    refresh_rate_millihertz
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

                VideoModeHandle {
                    size: PhysicalSize::new(
                        ffi::CGDisplayModeGetPixelWidth(mode) as u32,
                        ffi::CGDisplayModeGetPixelHeight(mode) as u32,
                    ),
                    refresh_rate_millihertz,
                    bit_depth,
                    monitor: monitor.clone(),
                    native_mode: NativeDisplayMode(mode),
                }
            })
        }
    }

    pub(crate) fn ns_screen(&self, mtm: MainThreadMarker) -> Option<Retained<NSScreen>> {
        let uuid = self.uuid();
        NSScreen::screens(mtm).into_iter().find(|screen| {
            let other_native_id = get_display_id(screen);
            if let Some(other) = MonitorHandle::new(other_native_id) {
                uuid == other.uuid()
            } else {
                // Display ID was just fetched from live NSScreen, but can still result in `None`
                // with certain Thunderbolt docked monitors.
                warn!(other_native_id, "comparing against screen with invalid display ID");
                false
            }
        })
    }
}

pub(crate) fn get_display_id(screen: &NSScreen) -> u32 {
    let key = ns_string!("NSScreenNumber");

    objc2::rc::autoreleasepool(|_| {
        let device_description = screen.deviceDescription();

        // Retrieve the CGDirectDisplayID associated with this screen
        //
        // SAFETY: The value from @"NSScreenNumber" in deviceDescription is guaranteed
        // to be an NSNumber. See documentation for `deviceDescription` for details:
        // <https://developer.apple.com/documentation/appkit/nsscreen/1388360-devicedescription?language=objc>
        let obj = device_description
            .get(key)
            .expect("failed getting screen display id from device description");
        let obj: *const AnyObject = obj;
        let obj: *const NSNumber = obj.cast();
        let obj: &NSNumber = unsafe { &*obj };

        obj.as_u32()
    })
}

/// Core graphics screen coordinates are relative to the top-left corner of
/// the so-called "main" display, with y increasing downwards - which is
/// exactly what we want in Winit.
///
/// However, `NSWindow` and `NSScreen` changes these coordinates to:
/// 1. Be relative to the bottom-left corner of the "main" screen.
/// 2. Be relative to the bottom-left corner of the window/screen itself.
/// 3. Have y increasing upwards.
///
/// This conversion happens to be symmetric, so we only need this one function
/// to convert between the two coordinate systems.
pub(crate) fn flip_window_screen_coordinates(frame: NSRect) -> NSPoint {
    // It is intentional that we use `CGMainDisplayID` (as opposed to
    // `NSScreen::mainScreen`), because that's what the screen coordinates
    // are relative to, no matter which display the window is currently on.
    let main_screen_height = CGDisplay::main().bounds().size.height;

    let y = main_screen_height - frame.size.height - frame.origin.y;
    NSPoint::new(frame.origin.x, y)
}
