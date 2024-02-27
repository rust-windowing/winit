use icrate::Foundation::NSUInteger;
use objc2::encode::{Encode, Encoding};

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UIRectEdge(pub NSUInteger);

impl UIRectEdge {
    pub const NONE: Self = Self(0);
}

unsafe impl Encode for UIRectEdge {
    const ENCODING: Encoding = NSUInteger::ENCODING;
}
