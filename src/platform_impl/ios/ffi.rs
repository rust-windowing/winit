#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]

use std::convert::TryInto;
use std::ffi::CString;
use std::ops::BitOr;
use std::os::raw::{c_char, c_int};

use objc2::encode::{Encode, Encoding};
use objc2::foundation::{CGFloat, NSInteger, NSUInteger};
use objc2::runtime::Object;
use objc2::{class, msg_send};

use crate::platform::ios::{Idiom, ScreenEdge, ValidOrientations};

pub type id = *mut Object;
pub const nil: id = 0 as id;

#[repr(C)]
#[derive(Clone, Debug)]
pub struct NSOperatingSystemVersion {
    pub major: NSInteger,
    pub minor: NSInteger,
    pub patch: NSInteger,
}

unsafe impl Encode for NSOperatingSystemVersion {
    const ENCODING: Encoding = Encoding::Struct(
        "NSOperatingSystemVersion",
        &[
            NSInteger::ENCODING,
            NSInteger::ENCODING,
            NSInteger::ENCODING,
        ],
    );
}

#[derive(Debug)]
#[allow(dead_code)]
#[repr(isize)]
pub enum UITouchPhase {
    Began = 0,
    Moved,
    Stationary,
    Ended,
    Cancelled,
}

unsafe impl Encode for UITouchPhase {
    const ENCODING: Encoding = NSInteger::ENCODING;
}

#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
#[repr(isize)]
pub enum UIForceTouchCapability {
    Unknown = 0,
    Unavailable,
    Available,
}

unsafe impl Encode for UIForceTouchCapability {
    const ENCODING: Encoding = NSInteger::ENCODING;
}

#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
#[repr(isize)]
pub enum UITouchType {
    Direct = 0,
    Indirect,
    Pencil,
}

unsafe impl Encode for UITouchType {
    const ENCODING: Encoding = NSInteger::ENCODING;
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct UIEdgeInsets {
    pub top: CGFloat,
    pub left: CGFloat,
    pub bottom: CGFloat,
    pub right: CGFloat,
}

unsafe impl Encode for UIEdgeInsets {
    const ENCODING: Encoding = Encoding::Struct(
        "UIEdgeInsets",
        &[
            CGFloat::ENCODING,
            CGFloat::ENCODING,
            CGFloat::ENCODING,
            CGFloat::ENCODING,
        ],
    );
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UIUserInterfaceIdiom(NSInteger);

unsafe impl Encode for UIUserInterfaceIdiom {
    const ENCODING: Encoding = NSInteger::ENCODING;
}

impl UIUserInterfaceIdiom {
    pub const Unspecified: UIUserInterfaceIdiom = UIUserInterfaceIdiom(-1);
    pub const Phone: UIUserInterfaceIdiom = UIUserInterfaceIdiom(0);
    pub const Pad: UIUserInterfaceIdiom = UIUserInterfaceIdiom(1);
    pub const TV: UIUserInterfaceIdiom = UIUserInterfaceIdiom(2);
    pub const CarPlay: UIUserInterfaceIdiom = UIUserInterfaceIdiom(3);
}

impl From<Idiom> for UIUserInterfaceIdiom {
    fn from(idiom: Idiom) -> UIUserInterfaceIdiom {
        match idiom {
            Idiom::Unspecified => UIUserInterfaceIdiom::Unspecified,
            Idiom::Phone => UIUserInterfaceIdiom::Phone,
            Idiom::Pad => UIUserInterfaceIdiom::Pad,
            Idiom::TV => UIUserInterfaceIdiom::TV,
            Idiom::CarPlay => UIUserInterfaceIdiom::CarPlay,
        }
    }
}
impl From<UIUserInterfaceIdiom> for Idiom {
    fn from(ui_idiom: UIUserInterfaceIdiom) -> Idiom {
        match ui_idiom {
            UIUserInterfaceIdiom::Unspecified => Idiom::Unspecified,
            UIUserInterfaceIdiom::Phone => Idiom::Phone,
            UIUserInterfaceIdiom::Pad => Idiom::Pad,
            UIUserInterfaceIdiom::TV => Idiom::TV,
            UIUserInterfaceIdiom::CarPlay => Idiom::CarPlay,
            _ => unreachable!(),
        }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub struct UIInterfaceOrientationMask(NSUInteger);

unsafe impl Encode for UIInterfaceOrientationMask {
    const ENCODING: Encoding = NSUInteger::ENCODING;
}

impl UIInterfaceOrientationMask {
    pub const Portrait: UIInterfaceOrientationMask = UIInterfaceOrientationMask(1 << 1);
    pub const PortraitUpsideDown: UIInterfaceOrientationMask = UIInterfaceOrientationMask(1 << 2);
    pub const LandscapeLeft: UIInterfaceOrientationMask = UIInterfaceOrientationMask(1 << 4);
    pub const LandscapeRight: UIInterfaceOrientationMask = UIInterfaceOrientationMask(1 << 3);
    pub const Landscape: UIInterfaceOrientationMask =
        UIInterfaceOrientationMask(Self::LandscapeLeft.0 | Self::LandscapeRight.0);
    pub const AllButUpsideDown: UIInterfaceOrientationMask =
        UIInterfaceOrientationMask(Self::Landscape.0 | Self::Portrait.0);
    pub const All: UIInterfaceOrientationMask =
        UIInterfaceOrientationMask(Self::AllButUpsideDown.0 | Self::PortraitUpsideDown.0);
}

impl BitOr for UIInterfaceOrientationMask {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        UIInterfaceOrientationMask(self.0 | rhs.0)
    }
}

impl UIInterfaceOrientationMask {
    pub fn from_valid_orientations_idiom(
        valid_orientations: ValidOrientations,
        idiom: Idiom,
    ) -> UIInterfaceOrientationMask {
        match (valid_orientations, idiom) {
            (ValidOrientations::LandscapeAndPortrait, Idiom::Phone) => {
                UIInterfaceOrientationMask::AllButUpsideDown
            }
            (ValidOrientations::LandscapeAndPortrait, _) => UIInterfaceOrientationMask::All,
            (ValidOrientations::Landscape, _) => UIInterfaceOrientationMask::Landscape,
            (ValidOrientations::Portrait, Idiom::Phone) => UIInterfaceOrientationMask::Portrait,
            (ValidOrientations::Portrait, _) => {
                UIInterfaceOrientationMask::Portrait
                    | UIInterfaceOrientationMask::PortraitUpsideDown
            }
        }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UIRectEdge(NSUInteger);

unsafe impl Encode for UIRectEdge {
    const ENCODING: Encoding = NSUInteger::ENCODING;
}

impl From<ScreenEdge> for UIRectEdge {
    fn from(screen_edge: ScreenEdge) -> UIRectEdge {
        assert_eq!(
            screen_edge.bits() & !ScreenEdge::ALL.bits(),
            0,
            "invalid `ScreenEdge`"
        );
        UIRectEdge(screen_edge.bits().into())
    }
}

impl From<UIRectEdge> for ScreenEdge {
    fn from(ui_rect_edge: UIRectEdge) -> ScreenEdge {
        let bits: u8 = ui_rect_edge.0.try_into().expect("invalid `UIRectEdge`");
        ScreenEdge::from_bits(bits).expect("invalid `ScreenEdge`")
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UIScreenOverscanCompensation(NSInteger);

unsafe impl Encode for UIScreenOverscanCompensation {
    const ENCODING: Encoding = NSInteger::ENCODING;
}

#[allow(dead_code)]
impl UIScreenOverscanCompensation {
    pub const Scale: UIScreenOverscanCompensation = UIScreenOverscanCompensation(0);
    pub const InsetBounds: UIScreenOverscanCompensation = UIScreenOverscanCompensation(1);
    pub const None: UIScreenOverscanCompensation = UIScreenOverscanCompensation(2);
}

#[link(name = "UIKit", kind = "framework")]
extern "C" {
    pub fn UIApplicationMain(
        argc: c_int,
        argv: *const c_char,
        principalClassName: id,
        delegateClassName: id,
    ) -> c_int;
}

// This is named NSStringRust rather than NSString because the "Debug View Heirarchy" feature of
// Xcode requires a non-ambiguous reference to NSString for unclear reasons. This makes Xcode happy
// so please test if you change the name back to NSString.
pub trait NSStringRust: Sized {
    unsafe fn alloc(_: Self) -> id {
        msg_send![class!(NSString), alloc]
    }

    unsafe fn initWithUTF8String_(self, c_string: *const c_char) -> id;
    unsafe fn stringByAppendingString_(self, other: id) -> id;
    unsafe fn init_str(self, string: &str) -> Self;
    unsafe fn UTF8String(self) -> *const c_char;
}

impl NSStringRust for id {
    unsafe fn initWithUTF8String_(self, c_string: *const c_char) -> id {
        msg_send![self, initWithUTF8String: c_string]
    }

    unsafe fn stringByAppendingString_(self, other: id) -> id {
        msg_send![self, stringByAppendingString: other]
    }

    unsafe fn init_str(self, string: &str) -> id {
        let cstring = CString::new(string).unwrap();
        self.initWithUTF8String_(cstring.as_ptr())
    }

    unsafe fn UTF8String(self) -> *const c_char {
        msg_send![self, UTF8String]
    }
}
