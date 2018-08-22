#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]

use std::ffi::CString;
use std::os::raw::*;

use objc::runtime::Object;

pub type id = *mut Object;
pub const nil: id = 0 as id;

pub type CFStringRef = *const c_void;
pub type CFTimeInterval = f64;
pub type Boolean = u32;

pub const kCFRunLoopRunHandledSource: i32 = 4;

#[cfg(target_pointer_width = "32")]
pub type CGFloat = f32;
#[cfg(target_pointer_width = "64")]
pub type CGFloat = f64;

#[repr(C)]
#[derive(Debug, Clone)]
pub struct CGPoint {
    pub x: CGFloat,
    pub y: CGFloat,
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct CGRect {
    pub origin: CGPoint,
    pub size: CGSize,
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct CGSize {
    pub width: CGFloat,
    pub height: CGFloat,
}

#[link(name = "UIKit", kind = "framework")]
#[link(name = "CoreFoundation", kind = "framework")]
#[link(name = "GlKit", kind = "framework")]
extern {
    pub static kCFRunLoopDefaultMode: CFStringRef;

    // int UIApplicationMain ( int argc, char *argv[], NSString *principalClassName, NSString *delegateClassName );
    pub fn UIApplicationMain(
        argc: c_int,
        argv: *const c_char,
        principalClassName: id,
        delegateClassName: id,
    ) -> c_int;

    // SInt32 CFRunLoopRunInMode ( CFStringRef mode, CFTimeInterval seconds, Boolean returnAfterSourceHandled );
    pub fn CFRunLoopRunInMode(
        mode: CFStringRef,
        seconds: CFTimeInterval,
        returnAfterSourceHandled: Boolean,
    ) -> i32;
}

extern {
    pub fn setjmp(env: *mut c_void) -> c_int;
    pub fn longjmp(env: *mut c_void, val: c_int) -> !;
}

// values taken from "setjmp.h" header in xcode iPhoneOS/iPhoneSimulator SDK
#[cfg(any(target_arch = "x86_64"))]
pub const JBLEN: usize = (9 * 2) + 3 + 16;
#[cfg(any(target_arch = "x86"))]
pub const JBLEN: usize = 18;
#[cfg(target_arch = "arm")]
pub const JBLEN: usize = 10 + 16 + 2;
#[cfg(target_arch = "aarch64")]
pub const JBLEN: usize = (14 + 8 + 2) * 2;

pub type JmpBuf = [c_int; JBLEN];

pub trait NSString: Sized {
    unsafe fn alloc(_: Self) -> id {
        msg_send![class!(NSString), alloc]
    }

    unsafe fn initWithUTF8String_(self, c_string: *const c_char) -> id;
    unsafe fn stringByAppendingString_(self, other: id) -> id;
    unsafe fn init_str(self, string: &str) -> Self;
    unsafe fn UTF8String(self) -> *const c_char;
}

impl NSString for id {
    unsafe fn initWithUTF8String_(self, c_string: *const c_char) -> id {
        msg_send![self, initWithUTF8String:c_string as id]
    }

    unsafe fn stringByAppendingString_(self, other: id) -> id {
        msg_send![self, stringByAppendingString:other]
    }

    unsafe fn init_str(self, string: &str) -> id {
        let cstring = CString::new(string).unwrap();
        self.initWithUTF8String_(cstring.as_ptr())
    }

    unsafe fn UTF8String(self) -> *const c_char {
        msg_send![self, UTF8String]
    }
}
