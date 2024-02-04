use crate::platform::ios::StatusBarStyle;
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

impl From<StatusBarStyle> for UIStatusBarStyle {
    fn from(value: StatusBarStyle) -> Self {
        match value {
            StatusBarStyle::Default => Self::Default,
            StatusBarStyle::LightContent => Self::LightContent,
            StatusBarStyle::DarkContent => Self::DarkContent,
        }
    }
}

unsafe impl Encode for UIStatusBarStyle {
    const ENCODING: Encoding = NSInteger::ENCODING;
}
