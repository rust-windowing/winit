pub use x11_dl::keysym::*;
pub use x11_dl::xcursor::*;
pub use x11_dl::xf86vmode::*;
pub use x11_dl::xlib::*;
pub use x11_dl::xinput::*;
pub use x11_dl::xinput2::*;

pub use x11_dl::error::OpenError;

pub use self::glx::types::GLXContext;

/// GLX bindings
pub mod glx {
    include!(concat!(env!("OUT_DIR"), "/glx_bindings.rs"));
}

/// Functions that are not necessarly always available
pub mod glx_extra {
    include!(concat!(env!("OUT_DIR"), "/glx_extra_bindings.rs"));
}
