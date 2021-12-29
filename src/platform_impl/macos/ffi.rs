// TODO: Upstream these

#![allow(dead_code, non_snake_case, non_upper_case_globals)]

use std::ffi::c_void;

use cocoa::{
    base::id,
    foundation::{NSInteger, NSUInteger},
};
use core_foundation::{
    array::CFArrayRef, dictionary::CFDictionaryRef, string::CFStringRef, uuid::CFUUIDRef,
};
use core_graphics::{
    base::CGError,
    display::{CGDirectDisplayID, CGDisplayConfigRef},
};
use objc;

pub const NSNotFound: NSInteger = NSInteger::max_value();

#[repr(C)]
pub struct NSRange {
    pub location: NSUInteger,
    pub length: NSUInteger,
}

impl NSRange {
    #[inline]
    pub fn new(location: NSUInteger, length: NSUInteger) -> NSRange {
        NSRange { location, length }
    }
}

unsafe impl objc::Encode for NSRange {
    fn encode() -> objc::Encoding {
        let encoding = format!(
            // TODO: Verify that this is correct
            "{{NSRange={}{}}}",
            NSUInteger::encode().as_str(),
            NSUInteger::encode().as_str(),
        );
        unsafe { objc::Encoding::from_str(&encoding) }
    }
}

pub trait NSMutableAttributedString: Sized {
    unsafe fn alloc(_: Self) -> id {
        msg_send![class!(NSMutableAttributedString), alloc]
    }

    unsafe fn init(self) -> id; // *mut NSMutableAttributedString
    unsafe fn initWithString(self, string: id) -> id;
    unsafe fn initWithAttributedString(self, string: id) -> id;

    unsafe fn string(self) -> id; // *mut NSString
    unsafe fn mutableString(self) -> id; // *mut NSMutableString
    unsafe fn length(self) -> NSUInteger;
}

impl NSMutableAttributedString for id {
    unsafe fn init(self) -> id {
        msg_send![self, init]
    }

    unsafe fn initWithString(self, string: id) -> id {
        msg_send![self, initWithString: string]
    }

    unsafe fn initWithAttributedString(self, string: id) -> id {
        msg_send![self, initWithAttributedString: string]
    }

    unsafe fn string(self) -> id {
        msg_send![self, string]
    }

    unsafe fn mutableString(self) -> id {
        msg_send![self, mutableString]
    }

    unsafe fn length(self) -> NSUInteger {
        msg_send![self, length]
    }
}

pub const kCGBaseWindowLevelKey: NSInteger = 0;
pub const kCGMinimumWindowLevelKey: NSInteger = 1;
pub const kCGDesktopWindowLevelKey: NSInteger = 2;
pub const kCGBackstopMenuLevelKey: NSInteger = 3;
pub const kCGNormalWindowLevelKey: NSInteger = 4;
pub const kCGFloatingWindowLevelKey: NSInteger = 5;
pub const kCGTornOffMenuWindowLevelKey: NSInteger = 6;
pub const kCGDockWindowLevelKey: NSInteger = 7;
pub const kCGMainMenuWindowLevelKey: NSInteger = 8;
pub const kCGStatusWindowLevelKey: NSInteger = 9;
pub const kCGModalPanelWindowLevelKey: NSInteger = 10;
pub const kCGPopUpMenuWindowLevelKey: NSInteger = 11;
pub const kCGDraggingWindowLevelKey: NSInteger = 12;
pub const kCGScreenSaverWindowLevelKey: NSInteger = 13;
pub const kCGMaximumWindowLevelKey: NSInteger = 14;
pub const kCGOverlayWindowLevelKey: NSInteger = 15;
pub const kCGHelpWindowLevelKey: NSInteger = 16;
pub const kCGUtilityWindowLevelKey: NSInteger = 17;
pub const kCGDesktopIconWindowLevelKey: NSInteger = 18;
pub const kCGCursorWindowLevelKey: NSInteger = 19;
pub const kCGNumberOfWindowLevelKeys: NSInteger = 20;

#[derive(Debug, Clone, Copy)]
pub enum NSWindowLevel {
    NSNormalWindowLevel = kCGBaseWindowLevelKey as _,
    NSFloatingWindowLevel = kCGFloatingWindowLevelKey as _,
    NSTornOffMenuWindowLevel = kCGTornOffMenuWindowLevelKey as _,
    NSModalPanelWindowLevel = kCGModalPanelWindowLevelKey as _,
    NSMainMenuWindowLevel = kCGMainMenuWindowLevelKey as _,
    NSStatusWindowLevel = kCGStatusWindowLevelKey as _,
    NSPopUpMenuWindowLevel = kCGPopUpMenuWindowLevelKey as _,
    NSScreenSaverWindowLevel = kCGScreenSaverWindowLevelKey as _,
}

pub type CGDisplayFadeInterval = f32;
pub type CGDisplayReservationInterval = f32;
pub type CGDisplayBlendFraction = f32;

pub const kCGDisplayBlendNormal: f32 = 0.0;
pub const kCGDisplayBlendSolidColor: f32 = 1.0;

pub type CGDisplayFadeReservationToken = u32;
pub const kCGDisplayFadeReservationInvalidToken: CGDisplayFadeReservationToken = 0;

pub type Boolean = u8;
pub const FALSE: Boolean = 0;
pub const TRUE: Boolean = 1;

pub const kCGErrorSuccess: i32 = 0;
pub const kCGErrorFailure: i32 = 1000;
pub const kCGErrorIllegalArgument: i32 = 1001;
pub const kCGErrorInvalidConnection: i32 = 1002;
pub const kCGErrorInvalidContext: i32 = 1003;
pub const kCGErrorCannotComplete: i32 = 1004;
pub const kCGErrorNotImplemented: i32 = 1006;
pub const kCGErrorRangeCheck: i32 = 1007;
pub const kCGErrorTypeCheck: i32 = 1008;
pub const kCGErrorInvalidOperation: i32 = 1010;
pub const kCGErrorNoneAvailable: i32 = 1011;

pub const IO1BitIndexedPixels: &str = "P";
pub const IO2BitIndexedPixels: &str = "PP";
pub const IO4BitIndexedPixels: &str = "PPPP";
pub const IO8BitIndexedPixels: &str = "PPPPPPPP";
pub const IO16BitDirectPixels: &str = "-RRRRRGGGGGBBBBB";
pub const IO32BitDirectPixels: &str = "--------RRRRRRRRGGGGGGGGBBBBBBBB";

pub const kIO30BitDirectPixels: &str = "--RRRRRRRRRRGGGGGGGGGGBBBBBBBBBB";
pub const kIO64BitDirectPixels: &str = "-16R16G16B16";

pub const kIO16BitFloatPixels: &str = "-16FR16FG16FB16";
pub const kIO32BitFloatPixels: &str = "-32FR32FG32FB32";

pub const IOYUV422Pixels: &str = "Y4U2V2";
pub const IO8BitOverlayPixels: &str = "O8";

pub type CGWindowLevel = i32;
pub type CGDisplayModeRef = *mut c_void;

#[cfg_attr(
    not(use_colorsync_cgdisplaycreateuuidfromdisplayid),
    link(name = "CoreGraphics", kind = "framework")
)]
#[cfg_attr(
    use_colorsync_cgdisplaycreateuuidfromdisplayid,
    link(name = "ColorSync", kind = "framework")
)]
extern "C" {
    pub fn CGDisplayCreateUUIDFromDisplayID(display: CGDirectDisplayID) -> CFUUIDRef;
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    pub fn CGRestorePermanentDisplayConfiguration();
    pub fn CGDisplayCapture(display: CGDirectDisplayID) -> CGError;
    pub fn CGDisplayRelease(display: CGDirectDisplayID) -> CGError;
    pub fn CGConfigureDisplayFadeEffect(
        config: CGDisplayConfigRef,
        fadeOutSeconds: CGDisplayFadeInterval,
        fadeInSeconds: CGDisplayFadeInterval,
        fadeRed: f32,
        fadeGreen: f32,
        fadeBlue: f32,
    ) -> CGError;
    pub fn CGAcquireDisplayFadeReservation(
        seconds: CGDisplayReservationInterval,
        token: *mut CGDisplayFadeReservationToken,
    ) -> CGError;
    pub fn CGDisplayFade(
        token: CGDisplayFadeReservationToken,
        duration: CGDisplayFadeInterval,
        startBlend: CGDisplayBlendFraction,
        endBlend: CGDisplayBlendFraction,
        redBlend: f32,
        greenBlend: f32,
        blueBlend: f32,
        synchronous: Boolean,
    ) -> CGError;
    pub fn CGReleaseDisplayFadeReservation(token: CGDisplayFadeReservationToken) -> CGError;
    pub fn CGShieldingWindowLevel() -> CGWindowLevel;
    pub fn CGDisplaySetDisplayMode(
        display: CGDirectDisplayID,
        mode: CGDisplayModeRef,
        options: CFDictionaryRef,
    ) -> CGError;
    pub fn CGDisplayCopyAllDisplayModes(
        display: CGDirectDisplayID,
        options: CFDictionaryRef,
    ) -> CFArrayRef;
    pub fn CGDisplayModeGetPixelWidth(mode: CGDisplayModeRef) -> usize;
    pub fn CGDisplayModeGetPixelHeight(mode: CGDisplayModeRef) -> usize;
    pub fn CGDisplayModeGetRefreshRate(mode: CGDisplayModeRef) -> f64;
    pub fn CGDisplayModeCopyPixelEncoding(mode: CGDisplayModeRef) -> CFStringRef;
    pub fn CGDisplayModeRetain(mode: CGDisplayModeRef);
    pub fn CGDisplayModeRelease(mode: CGDisplayModeRef);
}
