// TODO: Upstream these

#![allow(non_upper_case_globals)]

use std::ffi::c_void;

use objc2::ffi::NSInteger;
use objc2::runtime::AnyObject;
use objc2_core_foundation::{cf_type, CFString, CFUUID};
use objc2_core_graphics::CGDirectDisplayID;

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

    pub fn CGDisplayGetDisplayIDFromUUID(uuid: &CFUUID) -> CGDirectDisplayID;
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
