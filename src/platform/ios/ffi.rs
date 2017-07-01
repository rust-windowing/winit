use std::ffi::CString;

use libc;
use objc::runtime::{ Object, Class };

#[allow(non_camel_case_types)]
pub type id = *mut Object;

#[allow(non_camel_case_types)]
#[allow(non_upper_case_globals)]
pub const nil: id = 0 as id;

pub type CFStringRef = *const libc::c_void;
pub type CFTimeInterval = f64;
pub type Boolean = u32;

#[allow(non_upper_case_globals)]
pub const kCFRunLoopRunHandledSource: i32 = 4;

#[cfg(target_pointer_width = "32")]
pub type CGFloat = f32;
#[cfg(target_pointer_width = "64")]
pub type CGFloat = f64;

#[cfg(target_pointer_width = "32")]
pub type NSUInteger = u32;
#[cfg(target_pointer_width = "64")]
pub type NSUInteger = u64;

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
    pub size: CGSize
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct CGSize {
    pub width: CGFloat,
    pub height: CGFloat
}

#[link(name = "UIKit", kind = "framework")]
#[link(name = "CoreFoundation", kind = "framework")]
#[link(name = "GlKit", kind = "framework")]
extern {
    pub static kCFRunLoopDefaultMode: CFStringRef;

    // int UIApplicationMain ( int argc, char *argv[], NSString *principalClassName, NSString *delegateClassName );
    pub fn UIApplicationMain(argc: libc::c_int, argv: *const libc::c_char, principalClassName: id, delegateClassName: id) -> libc::c_int;

    // SInt32 CFRunLoopRunInMode ( CFStringRef mode, CFTimeInterval seconds, Boolean returnAfterSourceHandled );
    pub fn CFRunLoopRunInMode(mode: CFStringRef, seconds: CFTimeInterval, returnAfterSourceHandled: Boolean) -> i32;
}

extern {
    pub fn setjmp(env: *mut libc::c_void) -> libc::c_int;
    pub fn longjmp(env: *mut libc::c_void, val: libc::c_int);
}

pub trait NSString: Sized {
    unsafe fn alloc(_: Self) -> id {
        msg_send![class("NSString"), alloc]
    }

    #[allow(non_snake_case)]
    unsafe fn initWithUTF8String_(self, c_string: *const i8) -> id;
    #[allow(non_snake_case)]
    unsafe fn stringByAppendingString_(self, other: id) -> id;
    unsafe fn init_str(self, string: &str) -> Self;
    #[allow(non_snake_case)]
    unsafe fn UTF8String(self) -> *const libc::c_char;
}

impl NSString for id {
    unsafe fn initWithUTF8String_(self, c_string: *const i8) -> id {
        msg_send![self, initWithUTF8String:c_string as id]
    }

    unsafe fn stringByAppendingString_(self, other: id) -> id {
        msg_send![self, stringByAppendingString:other]
    }

    unsafe fn init_str(self, string: &str) -> id {
        let cstring = CString::new(string).unwrap();
        self.initWithUTF8String_(cstring.as_ptr())
    }

    unsafe fn UTF8String(self) -> *const libc::c_char {
        msg_send![self, UTF8String]
    }
}

#[inline]
pub fn class(name: &str) -> *mut Class {
    unsafe {
        ::std::mem::transmute(Class::get(name))
    }
}
