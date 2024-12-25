#![allow(clippy::unnecessary_cast)]

use std::collections::VecDeque;
use std::fmt;
use std::num::{NonZeroU16, NonZeroU32};

use core_foundation::array::{CFArrayGetCount, CFArrayGetValueAtIndex};
use core_foundation::base::{CFRelease, TCFType};
use core_foundation::string::CFString;
use core_graphics::display::{
    CGDirectDisplayID, CGDisplay, CGDisplayBounds, CGDisplayCopyDisplayMode,
};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::NSScreen;
use objc2_foundation::{ns_string, run_on_main, MainThreadMarker, NSNumber, NSPoint, NSRect};

use super::ffi;
use crate::dpi::{LogicalPosition, PhysicalPosition, PhysicalSize};
use crate::monitor::VideoMode;

#[derive(Clone)]
pub struct VideoModeHandle {
    pub(crate) mode: VideoMode,
    pub(crate) monitor: MonitorHandle,
    pub(crate) native_mode: NativeDisplayMode,
}

impl PartialEq for VideoModeHandle {
    fn eq(&self, other: &Self) -> bool {
        self.monitor == other.monitor && self.mode == other.mode
    }
}

impl Eq for VideoModeHandle {}

impl std::hash::Hash for VideoModeHandle {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.monitor.hash(state);
    }
}

impl std::fmt::Debug for VideoModeHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoMode")
            .field("mode", &self.mode)
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
    fn new(
        monitor: MonitorHandle,
        native_mode: NativeDisplayMode,
        refresh_rate_millihertz: Option<NonZeroU32>,
    ) -> Self {
        unsafe {
            let pixel_encoding = CFString::wrap_under_create_rule(
                ffi::CGDisplayModeCopyPixelEncoding(native_mode.0),
            )
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

            let mode = VideoMode {
                size: PhysicalSize::new(
                    ffi::CGDisplayModeGetPixelWidth(native_mode.0) as u32,
                    ffi::CGDisplayModeGetPixelHeight(native_mode.0) as u32,
                ),
                refresh_rate_millihertz,
                bit_depth: NonZeroU16::new(bit_depth),
            };

            VideoModeHandle { mode, monitor: monitor.clone(), native_mode }
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
        Some(self.cmp(other))
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
        f.debug_struct("MonitorHandle")
            .field("name", &self.name())
            .field("native_identifier", &self.native_identifier())
            .field("position", &self.position())
            .field("scale_factor", &self.scale_factor())
            .finish_non_exhaustive()
    }
}

impl MonitorHandle {
    pub fn new(id: CGDirectDisplayID) -> Self {
        MonitorHandle(id)
    }

    // TODO: Be smarter about this:
    // <https://github.com/glfw/glfw/blob/57cbded0760a50b9039ee0cb3f3c14f60145567c/src/cocoa_monitor.m#L44-L126>
    pub fn name(&self) -> Option<String> {
        let MonitorHandle(display_id) = *self;
        let screen_num = CGDisplay::new(display_id).model_number();
        Some(format!("Monitor #{screen_num}"))
    }

    #[inline]
    pub fn native_identifier(&self) -> u32 {
        self.0
    }

    #[inline]
    pub fn position(&self) -> Option<PhysicalPosition<i32>> {
        // This is already in screen coordinates. If we were using `NSScreen`,
        // then a conversion would've been needed:
        // flip_window_screen_coordinates(self.ns_screen(mtm)?.frame())
        let bounds = unsafe { CGDisplayBounds(self.native_identifier()) };
        let position = LogicalPosition::new(bounds.origin.x, bounds.origin.y);
        Some(position.to_physical(self.scale_factor()))
    }

    pub fn scale_factor(&self) -> f64 {
        run_on_main(|mtm| {
            match self.ns_screen(mtm) {
                Some(screen) => screen.backingScaleFactor() as f64,
                None => 1.0, // default to 1.0 when we can't find the screen
            }
        })
    }

    fn refresh_rate_millihertz(&self) -> Option<NonZeroU32> {
        let current_display_mode =
            NativeDisplayMode(unsafe { CGDisplayCopyDisplayMode(self.0) } as _);
        refresh_rate_millihertz(self.0, &current_display_mode)
    }

    pub fn current_video_mode(&self) -> Option<VideoMode> {
        let mode = NativeDisplayMode(unsafe { CGDisplayCopyDisplayMode(self.0) } as _);
        let refresh_rate_millihertz = refresh_rate_millihertz(self.0, &mode);
        Some(VideoModeHandle::new(self.clone(), mode, refresh_rate_millihertz).mode)
    }

    pub fn video_modes(&self) -> impl Iterator<Item = VideoMode> {
        self.video_modes_handles().map(|handle| handle.mode)
    }

    pub(crate) fn video_modes_handles(&self) -> impl Iterator<Item = VideoModeHandle> {
        let refresh_rate_millihertz = self.refresh_rate_millihertz();
        let monitor = self.clone();

        unsafe {
            let modes = {
                let array = ffi::CGDisplayCopyAllDisplayModes(self.0, std::ptr::null());
                assert!(!array.is_null(), "failed to get list of display modes");
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
            };

            modes.into_iter().map(move |mode| {
                let cg_refresh_rate_hertz = ffi::CGDisplayModeGetRefreshRate(mode).round() as i64;

                // CGDisplayModeGetRefreshRate returns 0.0 for any display that
                // isn't a CRT
                let refresh_rate_millihertz = if cg_refresh_rate_hertz > 0 {
                    NonZeroU32::new((cg_refresh_rate_hertz * 1000) as u32)
                } else {
                    refresh_rate_millihertz
                };

                VideoModeHandle::new(
                    monitor.clone(),
                    NativeDisplayMode(mode),
                    refresh_rate_millihertz,
                )
            })
        }
    }

    pub(crate) fn ns_screen(&self, mtm: MainThreadMarker) -> Option<Retained<NSScreen>> {
        let uuid = unsafe { ffi::CGDisplayCreateUUIDFromDisplayID(self.0) };
        NSScreen::screens(mtm).into_iter().find(|screen| {
            let other_native_id = get_display_id(screen);
            let other_uuid = unsafe {
                ffi::CGDisplayCreateUUIDFromDisplayID(other_native_id as CGDirectDisplayID)
            };
            uuid == other_uuid
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

fn refresh_rate_millihertz(id: CGDirectDisplayID, mode: &NativeDisplayMode) -> Option<NonZeroU32> {
    unsafe {
        let refresh_rate = ffi::CGDisplayModeGetRefreshRate(mode.0);
        if refresh_rate > 0.0 {
            return NonZeroU32::new((refresh_rate * 1000.0).round() as u32);
        }

        let mut display_link = std::ptr::null_mut();
        if ffi::CVDisplayLinkCreateWithCGDisplay(id, &mut display_link) != ffi::kCVReturnSuccess {
            return None;
        }
        let time = ffi::CVDisplayLinkGetNominalOutputVideoRefreshPeriod(display_link);
        ffi::CVDisplayLinkRelease(display_link);

        // This value is indefinite if an invalid display link was specified
        if time.flags & ffi::kCVTimeIsIndefinite != 0 {
            return None;
        }

        (time.time_scale as i64)
            .checked_div(time.time_value)
            .map(|v| (v * 1000) as u32)
            .and_then(NonZeroU32::new)
    }
}
