#![cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd"))]
#![allow(dead_code)]

use libc;

pub const RTLD_LAZY: libc::c_int = 0x001;
pub const RTLD_NOW: libc::c_int = 0x002;

#[link="dl"]
extern {
    pub fn dlopen(filename: *const libc::c_char, flag: libc::c_int) -> *mut libc::c_void;
    pub fn dlerror() -> *mut libc::c_char;
    pub fn dlsym(handle: *mut libc::c_void, symbol: *const libc::c_char) -> *mut libc::c_void;
    pub fn dlclose(handle: *mut libc::c_void) -> libc::c_int;
}
