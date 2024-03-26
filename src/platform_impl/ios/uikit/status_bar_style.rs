use icrate::Foundation::NSInteger;
use objc2::encode::{Encode, Encoding};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(dead_code)]
#[repr(isize)]
pub enum UIStatusBarStyle {
    #[default]
    Default = 0,
    LightContent = 1,
    DarkContent = 3,
}

unsafe impl Encode for UIStatusBarStyle {
    const ENCODING: Encoding = NSInteger::ENCODING;
}
