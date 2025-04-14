// TODO: Upstream these

#![allow(dead_code, non_snake_case, non_upper_case_globals)]

use std::ffi::c_void;

use objc2::ffi::NSInteger;
use objc2::runtime::AnyObject;
use objc2_core_foundation::{cf_type, CFString, CFUUID};
use objc2_core_graphics::CGDirectDisplayID;

pub const kCGDisplayBlendNormal: f32 = 0.0;
pub const kCGDisplayBlendSolidColor: f32 = 1.0;

pub type CGDisplayFadeReservationToken = u32;
pub const kCGDisplayFadeReservationInvalidToken: CGDisplayFadeReservationToken = 0;

pub const IO16BitDirectPixels: &str = "-RRRRRGGGGGBBBBB";
pub const IO32BitDirectPixels: &str = "--------RRRRRRRRGGGGGGGGBBBBBBBB";
pub const kIO30BitDirectPixels: &str = "--RRRRRRRRRRGGGGGGGGGGBBBBBBBBBB";
pub const kIO64BitDirectPixels: &str = "-16R16G16B16";

// `CGDisplayCreateUUIDFromDisplayID` comes from the `ColorSync` framework.
// However, that framework was only introduced "publicly" in macOS 10.13.
//
// Since we want to support older versions, we can't link to `ColorSync`
// directly. Fortunately, it has always been available as a subframework of
// `ApplicationServices`, see:
// https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/OSX_Technology_Overview/SystemFrameworks/SystemFrameworks.html#//apple_ref/doc/uid/TP40001067-CH210-BBCFFIEG
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    pub fn CGDisplayCreateUUIDFromDisplayID(display: CGDirectDisplayID) -> *mut CFUUID;
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    // Wildly used private APIs; Apple uses them for their Terminal.app.
    pub fn CGSMainConnectionID() -> *mut AnyObject;
    pub fn CGSSetWindowBackgroundBlurRadius(
        connection_id: *mut AnyObject,
        window_id: NSInteger,
        radius: i64,
    ) -> i32;
}

#[repr(transparent)]
pub struct TISInputSource(std::ffi::c_void);

cf_type!(
    #[encoding_name = "__TISInputSource"]
    unsafe impl TISInputSource {}
);

#[repr(transparent)]
pub struct UCKeyboardLayout(std::ffi::c_void);

pub type OptionBits = u32;
pub type UniCharCount = std::os::raw::c_ulong;
pub type UniChar = std::os::raw::c_ushort;
pub type OSStatus = i32;

#[allow(non_upper_case_globals)]
pub const kUCKeyActionDisplay: u16 = 3;
#[allow(non_upper_case_globals)]
pub const kUCKeyTranslateNoDeadKeysMask: OptionBits = 1;

#[link(name = "Carbon", kind = "framework")]
extern "C" {
    pub static kTISPropertyUnicodeKeyLayoutData: &'static CFString;

    #[allow(non_snake_case)]
    pub fn TISGetInputSourceProperty(
        inputSource: &TISInputSource,
        propertyKey: &CFString,
    ) -> *mut c_void;

    pub fn TISCopyCurrentKeyboardLayoutInputSource() -> *mut TISInputSource;

    pub fn LMGetKbdType() -> u8;

    #[allow(non_snake_case)]
    pub fn UCKeyTranslate(
        keyLayoutPtr: *const UCKeyboardLayout,
        virtualKeyCode: u16,
        keyAction: u16,
        modifierKeyState: u32,
        keyboardType: u32,
        keyTranslateOptions: OptionBits,
        deadKeyState: *mut u32,
        maxStringLength: UniCharCount,
        actualStringLength: *mut UniCharCount,
        unicodeString: *mut UniChar,
    ) -> OSStatus;
}

// CGWindowLevel.h
//
// Note: There are two different things at play in this header:
// `CGWindowLevel` and `CGWindowLevelKey`.
//
// It seems like there was a push towards using "key" values instead of the
// raw window level values, and then you were supposed to use
// `CGWindowLevelForKey` to get the actual level.
//
// But the values that `NSWindowLevel` has are compiled in, and as such has
// to remain ABI compatible, so they're safe for us to hardcode as well.
#[allow(dead_code, non_upper_case_globals)]
mod window_level {
    const kCGNumReservedWindowLevels: i32 = 16;
    const kCGNumReservedBaseWindowLevels: i32 = 5;

    pub const kCGBaseWindowLevel: i32 = i32::MIN;
    pub const kCGMinimumWindowLevel: i32 = kCGBaseWindowLevel + kCGNumReservedBaseWindowLevels;
    pub const kCGMaximumWindowLevel: i32 = i32::MAX - kCGNumReservedWindowLevels;

    pub const kCGDesktopWindowLevel: i32 = kCGMinimumWindowLevel + 20;
    pub const kCGDesktopIconWindowLevel: i32 = kCGDesktopWindowLevel + 20;
    pub const kCGBackstopMenuLevel: i32 = -20;
    pub const kCGNormalWindowLevel: i32 = 0;
    pub const kCGFloatingWindowLevel: i32 = 3;
    pub const kCGTornOffMenuWindowLevel: i32 = 3;
    pub const kCGModalPanelWindowLevel: i32 = 8;
    pub const kCGUtilityWindowLevel: i32 = 19;
    pub const kCGDockWindowLevel: i32 = 20;
    pub const kCGMainMenuWindowLevel: i32 = 24;
    pub const kCGStatusWindowLevel: i32 = 25;
    pub const kCGPopUpMenuWindowLevel: i32 = 101;
    pub const kCGOverlayWindowLevel: i32 = 102;
    pub const kCGHelpWindowLevel: i32 = 200;
    pub const kCGDraggingWindowLevel: i32 = 500;
    pub const kCGScreenSaverWindowLevel: i32 = 1000;
    pub const kCGAssistiveTechHighWindowLevel: i32 = 1500;
    pub const kCGCursorWindowLevel: i32 = kCGMaximumWindowLevel - 1;
}

pub use window_level::*;
