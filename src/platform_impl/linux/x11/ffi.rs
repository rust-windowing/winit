use x11_dl::xmd::CARD32;
pub use x11_dl::{error::OpenError, xcursor::*, xinput2::*, xlib::*, xlib_xcb::*};

// Isn't defined by x11_dl
#[allow(non_upper_case_globals)]
pub const IconicState: CARD32 = 3;
