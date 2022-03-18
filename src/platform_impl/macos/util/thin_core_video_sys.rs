#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![allow(non_snake_case)]
#![allow(unused)]

pub type CVReturn = i32;

pub const kCVReturnSuccess: CVReturn = 0;

pub const kCVTimeIsIndefinite: CVTimeFlags = 1 << 0;

pub type CFTypeID = usize;

#[derive(Debug, Copy, Clone)]
pub enum __CVDisplayLink {}
pub type CVDisplayLinkRef = *mut __CVDisplayLink;

extern "C" {
    pub fn CVDisplayLinkGetTypeID() -> CFTypeID;
    pub fn CVDisplayLinkCreateWithCGDisplay(
        displayID: CGDirectDisplayID,
        displayLinkOut: *mut CVDisplayLinkRef,
    ) -> CVReturn;
    pub fn CVDisplayLinkGetNominalOutputVideoRefreshPeriod(displayLink: CVDisplayLinkRef)
        -> CVTime;
    pub fn CVDisplayLinkRelease(displayLink: CVDisplayLinkRef);
}

pub type CGDirectDisplayID = u32;

#[repr(C)]
#[derive(Debug, Clone)]
pub struct CVTime {
    pub timeValue: i64,
    pub timeScale: i32,
    pub flags: i32,
}
pub type CVTimeFlags = i32;
