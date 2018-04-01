use libc::int32_t;

use std::os::raw::c_void;

#[link(name = "android")]
#[link(name = "EGL")]
#[link(name = "GLESv2")]
extern {}

pub type ANativeWindow = c_void;

extern {
    pub fn ANativeWindow_getHeight(window: *const ANativeWindow) -> int32_t;
    pub fn ANativeWindow_getWidth(window: *const ANativeWindow) -> int32_t;
}