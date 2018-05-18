// TODO: Upstream these

#![allow(non_snake_case, non_upper_case_globals)]

use cocoa::base::{class, id};
use cocoa::foundation::{NSInteger, NSUInteger};
use objc;

pub const NSNotFound: NSInteger = NSInteger::max_value();

#[repr(C)]
pub struct NSRange {
    pub location: NSUInteger,
    pub length: NSUInteger,
}

impl NSRange {
    #[inline]
    pub fn new(location: NSUInteger, length: NSUInteger) -> NSRange {
        NSRange { location, length }
    }
}

unsafe impl objc::Encode for NSRange {
    fn encode() -> objc::Encoding {
        let encoding = format!(
            // TODO: Verify that this is correct
            "{{NSRange={}{}}}",
            NSUInteger::encode().as_str(),
            NSUInteger::encode().as_str(),
        );
        unsafe { objc::Encoding::from_str(&encoding) }
    }
}

pub trait NSMutableAttributedString: Sized {
    unsafe fn alloc(_: Self) -> id {
        msg_send![class("NSMutableAttributedString"), alloc]
    }

    unsafe fn init(self) -> id; // *mut NSMutableAttributedString
    unsafe fn initWithString(self, string: id) -> id;
    unsafe fn initWithAttributedString(self, string: id) -> id;

    unsafe fn string(self) -> id; // *mut NSString
    unsafe fn mutableString(self) -> id; // *mut NSMutableString
    unsafe fn length(self) -> NSUInteger;
}

impl NSMutableAttributedString for id {
    unsafe fn init(self) -> id {
        msg_send![self, init]
    }

    unsafe fn initWithString(self, string: id) -> id {
        msg_send![self, initWithString:string]
    }

    unsafe fn initWithAttributedString(self, string: id) -> id {
        msg_send![self, initWithAttributedString:string]
    }

    unsafe fn string(self) -> id {
        msg_send![self, string]
    }

    unsafe fn mutableString(self) -> id {
        msg_send![self, mutableString]
    }

    unsafe fn length(self) -> NSUInteger {
        msg_send![self, length]
    }
}
