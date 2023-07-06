#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]

use std::convert::TryInto;

use objc2::encode::{Encode, Encoding};
use objc2::foundation::{NSInteger, NSUInteger};

use crate::platform::ios::ScreenEdge;

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

#[allow(dead_code)]
impl UIUserInterfaceIdiom {
    pub const Unspecified: UIUserInterfaceIdiom = UIUserInterfaceIdiom(-1);
    pub const Phone: UIUserInterfaceIdiom = UIUserInterfaceIdiom(0);
    pub const Pad: UIUserInterfaceIdiom = UIUserInterfaceIdiom(1);
    pub const TV: UIUserInterfaceIdiom = UIUserInterfaceIdiom(2);
    pub const CarPlay: UIUserInterfaceIdiom = UIUserInterfaceIdiom(3);
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
