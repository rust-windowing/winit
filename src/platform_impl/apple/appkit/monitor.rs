#![allow(clippy::unnecessary_cast)]

use std::collections::VecDeque;
use std::num::{NonZeroU16, NonZeroU32};
use std::ptr::NonNull;
use std::{fmt, ptr};

use dispatch2::run_on_main;
use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_app_kit::NSScreen;
use objc2_core_foundation::{
    CFArrayGetCount, CFArrayGetValueAtIndex, CFRetained, CFUUIDGetUUIDBytes,
};
#[allow(deprecated)]
use objc2_core_graphics::{
    CGDirectDisplayID, CGDisplayBounds, CGDisplayCopyAllDisplayModes, CGDisplayCopyDisplayMode,
    CGDisplayMode, CGDisplayModeCopyPixelEncoding, CGDisplayModeGetPixelHeight,
    CGDisplayModeGetPixelWidth, CGDisplayModeGetRefreshRate, CGDisplayModelNumber,
    CGGetActiveDisplayList, CGMainDisplayID,
};
#[allow(deprecated)]
use objc2_core_video::{
    kCVReturnSuccess, CVDisplayLinkCreateWithCGDisplay,
    CVDisplayLinkGetNominalOutputVideoRefreshPeriod, CVTimeFlags,
};
use objc2_foundation::{ns_string, NSNumber, NSPoint, NSRect};
use tracing::warn;

use super::ffi;
use super::util::cgerr;
use crate::dpi::{LogicalPosition, PhysicalPosition, PhysicalSize};
use crate::monitor::{MonitorHandleProvider, VideoMode};

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
        f.debug_struct("VideoModeHandle")
            .field("mode", &self.mode)
            .field("monitor", &self.monitor)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NativeDisplayMode(pub CFRetained<CGDisplayMode>);

unsafe impl Send for NativeDisplayMode {}
unsafe impl Sync for NativeDisplayMode {}

impl VideoModeHandle {
    fn new(
        monitor: MonitorHandle,
        native_mode: NativeDisplayMode,
        refresh_rate_millihertz: Option<NonZeroU32>,
    ) -> Self {
        unsafe {
            // The bit-depth is basically always 32 since macOS 10.12.
            #[allow(deprecated)]
            let pixel_encoding =
                CGDisplayModeCopyPixelEncoding(Some(&native_mode.0)).unwrap().to_string();
            let bit_depth = if pixel_encoding.eq_ignore_ascii_case(ffi::IO32BitDirectPixels) {
                NonZeroU16::new(32)
            } else if pixel_encoding.eq_ignore_ascii_case(ffi::IO16BitDirectPixels) {
                NonZeroU16::new(16)
            } else if pixel_encoding.eq_ignore_ascii_case(ffi::kIO30BitDirectPixels) {
                NonZeroU16::new(30)
            } else if pixel_encoding.eq_ignore_ascii_case(ffi::kIO64BitDirectPixels) {
                NonZeroU16::new(64)
            } else {
                warn!(?pixel_encoding, "unknown bit depth");
                None
            };

            let mode = VideoMode {
                size: PhysicalSize::new(
                    CGDisplayModeGetPixelWidth(Some(&native_mode.0)) as u32,
                    CGDisplayModeGetPixelHeight(Some(&native_mode.0)) as u32,
                ),
                refresh_rate_millihertz,
                bit_depth,
            };

            VideoModeHandle { mode, monitor: monitor.clone(), native_mode }
        }
    }
}

#[derive(Clone)]
pub struct MonitorHandle(CGDirectDisplayID);

impl MonitorHandle {
    /// Internal comparisons of [`MonitorHandle`]s are done first requesting a UUID for the handle.
    fn uuid(&self) -> u128 {
        let ptr = unsafe { ffi::CGDisplayCreateUUIDFromDisplayID(self.0) };
        let cf_uuid = unsafe { CFRetained::from_raw(NonNull::new(ptr).unwrap()) };
        u128::from_ne_bytes(unsafe { CFUUIDGetUUIDBytes(&cf_uuid) }.into())
    }

    pub fn new(id: CGDirectDisplayID) -> Self {
        MonitorHandle(id)
    }

    fn refresh_rate_millihertz(&self) -> Option<NonZeroU32> {
        let current_display_mode =
            NativeDisplayMode(unsafe { CGDisplayCopyDisplayMode(self.0) }.unwrap());
        refresh_rate_millihertz(self.0, &current_display_mode)
    }

    pub fn video_mode_handles(&self) -> impl Iterator<Item = VideoModeHandle> {
        let refresh_rate_millihertz = self.refresh_rate_millihertz();
        let monitor = self.clone();

        unsafe {
            let modes = {
                let array = CGDisplayCopyAllDisplayModes(self.0, None)
                    .expect("failed to get list of display modes");
                let array_count = CFArrayGetCount(&array);
                let modes: Vec<_> = (0..array_count)
                    .map(move |i| {
                        let mode = CFArrayGetValueAtIndex(&array, i) as *mut CGDisplayMode;
                        CFRetained::retain(NonNull::new(mode).unwrap())
                    })
                    .collect();
                modes
            };

            modes.into_iter().map(move |mode| {
                let cg_refresh_rate_hertz = CGDisplayModeGetRefreshRate(Some(&mode)).round() as i64;

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
        let uuid = self.uuid();
        NSScreen::screens(mtm).into_iter().find(|screen| {
            let other_native_id = get_display_id(screen);
            let other = MonitorHandle::new(other_native_id);
            uuid == other.uuid()
        })
    }
}

impl MonitorHandleProvider for MonitorHandle {
    fn id(&self) -> u128 {
        self.uuid()
    }

    fn native_id(&self) -> u64 {
        self.0 as _
    }

    // TODO: Be smarter about this:
    //
    // <https://github.com/glfw/glfw/blob/57cbded0760a50b9039ee0cb3f3c14f60145567c/src/cocoa_monitor.m#L44-L126>
    fn name(&self) -> Option<std::borrow::Cow<'_, str>> {
        let screen_num = unsafe { CGDisplayModelNumber(self.0) };
        Some(format!("Monitor #{screen_num}").into())
    }

    fn position(&self) -> Option<PhysicalPosition<i32>> {
        // This is already in screen coordinates. If we were using `NSScreen`,
        // then a conversion would've been needed:
        // flip_window_screen_coordinates(self.ns_screen(mtm)?.frame())
        let bounds = unsafe { CGDisplayBounds(self.0) };
        let position = LogicalPosition::new(bounds.origin.x, bounds.origin.y);
        Some(position.to_physical(self.scale_factor()))
    }

    fn scale_factor(&self) -> f64 {
        run_on_main(|mtm| {
            match self.ns_screen(mtm) {
                Some(screen) => screen.backingScaleFactor() as f64,
                None => 1.0, // default to 1.0 when we can't find the screen
            }
        })
    }

    fn current_video_mode(&self) -> Option<VideoMode> {
        let mode = NativeDisplayMode(unsafe { CGDisplayCopyDisplayMode(self.0) }.unwrap());
        let refresh_rate_millihertz = refresh_rate_millihertz(self.0, &mode);
        Some(VideoModeHandle::new(self.clone(), mode, refresh_rate_millihertz).mode)
    }

    fn video_modes(&self) -> Box<dyn Iterator<Item = VideoMode>> {
        Box::new(self.video_mode_handles().map(|mode| mode.mode))
    }
}

// `CGDirectDisplayID` changes on video mode change, so we cannot rely on that
// for comparisons, but we can use `CGDisplayCreateUUIDFromDisplayID` to get an
// unique identifier that persists even across system reboots
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
    let mut expected_count = 0;
    let res = cgerr(unsafe { CGGetActiveDisplayList(0, ptr::null_mut(), &mut expected_count) });
    if res.is_err() {
        return VecDeque::with_capacity(0);
    }

    let mut displays: Vec<CGDirectDisplayID> = vec![0; expected_count as usize];
    let mut actual_count = 0;
    let res = cgerr(unsafe {
        CGGetActiveDisplayList(expected_count, displays.as_mut_ptr(), &mut actual_count)
    });
    displays.truncate(actual_count as usize);

    if res.is_err() {
        return VecDeque::with_capacity(0);
    }

    let mut monitors = VecDeque::with_capacity(displays.len());
    for display in displays {
        monitors.push_back(MonitorHandle(display));
    }
    monitors
}

pub fn primary_monitor() -> MonitorHandle {
    MonitorHandle(unsafe { CGMainDisplayID() })
}

impl fmt::Debug for MonitorHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MonitorHandle")
            .field("name", &self.name())
            .field("native_id", &self.native_id())
            .field("position", &self.position())
            .field("scale_factor", &self.scale_factor())
            .finish_non_exhaustive()
    }
}

pub(crate) fn get_display_id(screen: &NSScreen) -> u32 {
    let key = ns_string!("NSScreenNumber");

    objc2::rc::autoreleasepool(|_| {
        let device_description = screen.deviceDescription();

        // Retrieve the CGDirectDisplayID associated with this screen
        //
        // The value from @"NSScreenNumber" in deviceDescription is guaranteed
        // to be an NSNumber. See documentation for details:
        // <https://developer.apple.com/documentation/appkit/nsscreen/1388360-devicedescription?language=objc>
        let obj = device_description
            .objectForKey(key)
            .expect("failed getting screen display id from device description")
            .downcast::<NSNumber>()
            .expect("NSScreenNumber must be NSNumber");

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
    let main_screen_height = unsafe { CGDisplayBounds(CGMainDisplayID()) }.size.height;

    let y = main_screen_height - frame.size.height - frame.origin.y;
    NSPoint::new(frame.origin.x, y)
}

fn refresh_rate_millihertz(id: CGDirectDisplayID, mode: &NativeDisplayMode) -> Option<NonZeroU32> {
    unsafe {
        let refresh_rate = CGDisplayModeGetRefreshRate(Some(&mode.0));
        if refresh_rate > 0.0 {
            return NonZeroU32::new((refresh_rate * 1000.0).round() as u32);
        }

        let mut display_link = std::ptr::null_mut();
        #[allow(deprecated)]
        if CVDisplayLinkCreateWithCGDisplay(id, NonNull::from(&mut display_link))
            != kCVReturnSuccess
        {
            return None;
        }
        let display_link = CFRetained::from_raw(NonNull::new(display_link).unwrap());
        #[allow(deprecated)]
        let time = CVDisplayLinkGetNominalOutputVideoRefreshPeriod(&display_link);

        // This value is indefinite if an invalid display link was specified
        if time.flags & CVTimeFlags::IsIndefinite.0 != 0 {
            return None;
        }

        (time.timeScale as i64)
            .checked_div(time.timeValue)
            .map(|v| (v * 1000) as u32)
            .and_then(NonZeroU32::new)
    }
}
