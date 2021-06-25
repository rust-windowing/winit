use x11_dl::xmd::CARD32;
pub use x11_dl::{
    error::OpenError, keysym::*, xcursor::*, xinput::*, xinput2::*, xlib::*, xlib_xcb::*,
    xrandr::*, xrender::*,
};

// Isn't defined by x11_dl
#[allow(non_upper_case_globals)]
pub const IconicState: CARD32 = 3;
