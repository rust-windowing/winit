#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]

use std::convert::TryInto;
use std::ffi::CString;
use std::os::raw::{c_char, c_int};

use objc2::encode::{Encode, Encoding};
use objc2::foundation::{NSInteger, NSUInteger};
use objc2::runtime::Object;
use objc2::{class, msg_send};

use crate::platform::ios::{Idiom, ScreenEdge};

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
