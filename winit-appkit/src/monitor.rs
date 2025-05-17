#![allow(clippy::unnecessary_cast)]

use std::collections::VecDeque;
use std::num::{NonZeroU16, NonZeroU32};
use std::ptr::NonNull;
use std::{fmt, ptr};

use dispatch2::run_on_main;
use dpi::{LogicalPosition, PhysicalPosition, PhysicalSize};
use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_app_kit::NSScreen;
use objc2_core_foundation::{CFArray, CFRetained, CFUUID};
use objc2_core_graphics::{
    CGDirectDisplayID, CGDisplayBounds, CGDisplayCopyAllDisplayModes, CGDisplayCopyDisplayMode,
    CGDisplayMode, CGDisplayModelNumber, CGGetActiveDisplayList, CGMainDisplayID,
};
use objc2_core_video::{kCVReturnSuccess, CVDisplayLink, CVTimeFlags};
use objc2_foundation::{ns_string, NSNumber, NSPoint, NSRect};
use tracing::warn;
use winit_core::monitor::{MonitorHandleProvider, VideoMode};

use super::ffi;
use super::util::cgerr;

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
            #[allow(deprecated)]
            let pixel_encoding =
                CGDisplayMode::pixel_encoding(Some(&native_mode.0)).unwrap().to_string();
            let bit_depth = if pixel_encoding.eq_ignore_ascii_case(ffi::IO32BitDirectPixels) {
                32
            } else if pixel_encoding.eq_ignore_ascii_case(ffi::IO16BitDirectPixels) {
                16
            } else if pixel_encoding.eq_ignore_ascii_case(ffi::kIO30BitDirectPixels) {
                30
            } else {
                unimplemented!()
            };

            let mode = VideoMode::new(
                PhysicalSize::new(
                    CGDisplayMode::pixel_width(Some(&native_mode.0)) as u32,
                    CGDisplayMode::pixel_height(Some(&native_mode.0)) as u32,
                ),
                NonZeroU16::new(bit_depth),
                refresh_rate_millihertz,
            );

            VideoModeHandle { mode, monitor: monitor.clone(), native_mode }
        }
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
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct MonitorHandle(CFRetained<CFUUID>);

impl MonitorHandle {
    /// Internal comparisons of [`MonitorHandle`]s are done first requesting a UUID for the handle.
    fn uuid(&self) -> u128 {
        u128::from_ne_bytes(self.0.uuid_bytes().into())
    }

    fn display_id(&self) -> CGDirectDisplayID {
        unsafe { ffi::CGDisplayGetDisplayIDFromUUID(&self.0) }
    }

    #[track_caller]
    pub(crate) fn new(display_id: CGDirectDisplayID) -> Option<Self> {
        // kCGNullDirectDisplay
        if display_id == 0 {
            // `CGDisplayCreateUUIDFromDisplayID` checks kCGNullDirectDisplay internally.
            warn!("constructing monitor from invalid display ID 0; falling back to main monitor");
        }
        // SAFETY: Valid to call.
        let ptr = unsafe { ffi::CGDisplayCreateUUIDFromDisplayID(display_id) };
        let ptr = NonNull::new(ptr)?;
        // SAFETY: `CGDisplayCreateUUIDFromDisplayID` is a "create" function, so the pointer has
        // +1 retain count.
        let uuid = unsafe { CFRetained::from_raw(ptr) };
        Some(Self(uuid))
    }

    fn refresh_rate_millihertz(&self) -> Option<NonZeroU32> {
        let current_display_mode =
            NativeDisplayMode(unsafe { CGDisplayCopyDisplayMode(self.display_id()) }.unwrap());
        refresh_rate_millihertz(self.display_id(), &current_display_mode)
    }

    pub fn video_mode_handles(&self) -> impl Iterator<Item = VideoModeHandle> {
        let refresh_rate_millihertz = self.refresh_rate_millihertz();
        let monitor = self.clone();

        let array = unsafe { CGDisplayCopyAllDisplayModes(self.display_id(), None) };
        let modes = if let Some(array) = array {
            // SAFETY: `CGDisplayCopyAllDisplayModes` is documented to return an array of
            // display modes.
            unsafe { CFRetained::cast_unchecked::<CFArray<CGDisplayMode>>(array) }
        } else {
            // Occasionally, certain CalDigit Thunderbolt Hubs report a spurious monitor during
            // sleep/wake/cycling monitors. It tends to have null or 1 video mode only.
            // See <https://github.com/bevyengine/bevy/issues/17827>.
            warn!(monitor = ?self, "failed to get a list of display modes");
            CFArray::empty()
        };

        modes.into_iter().map(move |mode| {
            let cg_refresh_rate_hertz = unsafe { CGDisplayMode::refresh_rate(Some(&mode)) };

            // CGDisplayModeGetRefreshRate returns 0.0 for any display that
            // isn't a CRT
            let refresh_rate_millihertz = if cg_refresh_rate_hertz > 0.0 {
                NonZeroU32::new((cg_refresh_rate_hertz * 1000.0).round() as u32)
            } else {
                refresh_rate_millihertz
            };

            VideoModeHandle::new(monitor.clone(), NativeDisplayMode(mode), refresh_rate_millihertz)
        })
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

impl MonitorHandleProvider for MonitorHandle {
    fn id(&self) -> u128 {
        self.uuid()
    }

    fn native_id(&self) -> u64 {
        self.display_id() as _
    }

    // TODO: Be smarter about this:
    //
    // <https://github.com/glfw/glfw/blob/57cbded0760a50b9039ee0cb3f3c14f60145567c/src/cocoa_monitor.m#L44-L126>
    fn name(&self) -> Option<std::borrow::Cow<'_, str>> {
        let screen_num = unsafe { CGDisplayModelNumber(self.display_id()) };
        Some(format!("Monitor #{screen_num}").into())
    }

    fn position(&self) -> Option<PhysicalPosition<i32>> {
        // This is already in screen coordinates. If we were using `NSScreen`,
        // then a conversion would've been needed:
        // flip_window_screen_coordinates(self.ns_screen(mtm)?.frame())
        let bounds = unsafe { CGDisplayBounds(self.display_id()) };
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
        let mode =
            NativeDisplayMode(unsafe { CGDisplayCopyDisplayMode(self.display_id()) }.unwrap());
        let refresh_rate_millihertz = refresh_rate_millihertz(self.display_id(), &mode);
        Some(VideoModeHandle::new(self.clone(), mode, refresh_rate_millihertz).mode)
    }

    fn video_modes(&self) -> Box<dyn Iterator<Item = VideoMode>> {
        Box::new(self.video_mode_handles().map(|mode| mode.mode))
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
        // Display ID just fetched from `CGGetActiveDisplayList`, should be fine to unwrap.
        monitors.push_back(MonitorHandle::new(display).expect("invalid display ID"));
    }
    monitors
}

pub fn primary_monitor() -> MonitorHandle {
    // Display ID just fetched from `CGMainDisplayID`, should be fine to unwrap.
    MonitorHandle::new(unsafe { CGMainDisplayID() }).expect("invalid display ID")
}

impl fmt::Debug for MonitorHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MonitorHandle")
            .field("name", &self.name())
            .field("uuid", &self.uuid())
            .field("display_id", &self.display_id())
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
        let refresh_rate = CGDisplayMode::refresh_rate(Some(&mode.0));
        if refresh_rate > 0.0 {
            return NonZeroU32::new((refresh_rate * 1000.0).round() as u32);
        }

        let mut display_link = std::ptr::null_mut();
        #[allow(deprecated)]
        if CVDisplayLink::create_with_cg_display(id, NonNull::from(&mut display_link))
            != kCVReturnSuccess
        {
            return None;
        }
        let display_link = CFRetained::from_raw(NonNull::new(display_link).unwrap());
        #[allow(deprecated)]
        let time = display_link.nominal_output_video_refresh_period();

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_stable() {
        let handle_a = MonitorHandle::new(1).unwrap();
        let handle_b = MonitorHandle::new(1).unwrap();
        assert_eq!(handle_a, handle_b);
        assert_eq!(handle_a.display_id(), handle_b.display_id());
        assert_eq!(handle_a.uuid(), handle_b.uuid());

        let handle_a = primary_monitor();
        let handle_b = primary_monitor();
        assert_eq!(handle_a, handle_b);
        assert_eq!(handle_a.display_id(), handle_b.display_id());
        assert_eq!(handle_a.uuid(), handle_b.uuid());
    }

    /// Test the MonitorHandle::new fallback.
    #[test]
    fn monitorhandle_from_zero() {
        let handle0 = MonitorHandle::new(0).unwrap();
        let handle1 = MonitorHandle::new(1).unwrap();
        assert_eq!(handle0, handle1);
        assert_eq!(handle0.display_id(), handle1.display_id());
        assert_eq!(handle0.uuid(), handle1.uuid());
    }

    #[test]
    fn from_invalid_id() {
        // Assume there are never this many monitors connected.
        assert!(MonitorHandle::new(10000).is_none());
    }

    /// Test that calling `CGDisplayGetDisplayIDFromUUID` on an invalid UUID returns an invalid
    /// display ID.
    #[test]
    fn invalid_monitor_handle() {
        // `CGMainDisplayID` must be called to avoid:
        // ```
        // Assertion failed: (did_initialize), function CGS_REQUIRE_INIT, file CGInitialization.c, line 44.
        // ```
        // See https://github.com/JXA-Cookbook/JXA-Cookbook/issues/27#issuecomment-277517668
        let _ = unsafe { CGMainDisplayID() };

        let handle = MonitorHandle(CFUUID::new(None).unwrap());
        assert_eq!(handle.display_id(), 0);
    }
}
