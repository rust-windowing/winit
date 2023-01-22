#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]

use std::convert::TryInto;

use objc2::encode::{Encode, Encoding};
use objc2::foundation::{NSInteger, NSUInteger};

use crate::platform::ios::{Idiom, ScreenEdge};

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
