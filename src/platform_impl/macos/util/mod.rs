mod r#async;
mod cursor;

pub use self::{cursor::*, r#async::*};

use std::ops::{BitAnd, Deref};
use std::os::raw::c_uchar;

use cocoa::{
    appkit::{CGFloat, NSApp, NSWindowStyleMask},
    base::{id, nil},
    foundation::{NSPoint, NSRect, NSString, NSUInteger},
};
use core_graphics::display::CGDisplay;
use objc::runtime::{Class, Object, BOOL, NO};

use crate::dpi::LogicalPosition;
use crate::platform_impl::platform::ffi;

// Replace with `!` once stable
#[derive(Debug)]
pub enum Never {}

pub fn has_flag<T>(bitset: T, flag: T) -> bool
where
    T: Copy + PartialEq + BitAnd<T, Output = T>,
{
    bitset & flag == flag
}

pub const EMPTY_RANGE: ffi::NSRange = ffi::NSRange {
    location: ffi::NSNotFound as NSUInteger,
    length: 0,
};

#[derive(Debug, PartialEq, Eq)]
pub struct IdRef(id);

impl IdRef {
    pub fn new(inner: id) -> IdRef {
        IdRef(inner)
    }

    pub fn retain(inner: id) -> IdRef {
        if inner != nil {
            let _: id = unsafe { msg_send![inner, retain] };
        }
        IdRef(inner)
    }

    pub fn non_nil(self) -> Option<IdRef> {
        if self.0 == nil {
            None
        } else {
            Some(self)
        }
    }
}

impl Drop for IdRef {
    fn drop(&mut self) {
        if self.0 != nil {
            unsafe {
                let _: () = msg_send![self.0, release];
            };
        }
    }
}

impl Deref for IdRef {
    type Target = id;
    fn deref(&self) -> &id {
        &self.0
    }
}

impl Clone for IdRef {
    fn clone(&self) -> IdRef {
        IdRef::retain(self.0)
    }
}

macro_rules! trace_scope {
    ($s:literal) => {
        let _crate = $crate::platform_impl::platform::util::TraceGuard::new(module_path!(), $s);
    };
}

pub(crate) struct TraceGuard {
    module_path: &'static str,
    called_from_fn: &'static str,
}

impl TraceGuard {
    #[inline]
    pub(crate) fn new(module_path: &'static str, called_from_fn: &'static str) -> Self {
        trace!(target: module_path, "Triggered `{}`", called_from_fn);
        Self {
            module_path,
            called_from_fn,
        }
    }
}

impl Drop for TraceGuard {
    #[inline]
    fn drop(&mut self) {
        trace!(target: self.module_path, "Completed `{}`", self.called_from_fn);
    }
}

// For consistency with other platforms, this will...
// 1. translate the bottom-left window corner into the top-left window corner
// 2. translate the coordinate from a bottom-left origin coordinate system to a top-left one
pub fn bottom_left_to_top_left(rect: NSRect) -> f64 {
    CGDisplay::main().pixels_high() as f64 - (rect.origin.y + rect.size.height) as f64
}

/// Converts from winit screen-coordinates to macOS screen-coordinates.
/// Winit: top-left is (0, 0) and y increasing downwards
/// macOS: bottom-left is (0, 0) and y increasing upwards
pub fn window_position(position: LogicalPosition<f64>) -> NSPoint {
    NSPoint::new(
        position.x as CGFloat,
        CGDisplay::main().pixels_high() as CGFloat - position.y as CGFloat,
    )
}

pub unsafe fn ns_string_id_ref(s: &str) -> IdRef {
    IdRef::new(NSString::alloc(nil).init_str(s))
}

#[allow(dead_code)] // In case we want to use this function in the future
pub unsafe fn app_name() -> Option<id> {
    let bundle: id = msg_send![class!(NSBundle), mainBundle];
    let dict: id = msg_send![bundle, infoDictionary];
    let key = ns_string_id_ref("CFBundleName");
    let app_name: id = msg_send![dict, objectForKey:*key];
    if app_name != nil {
        Some(app_name)
    } else {
        None
    }
}

pub unsafe fn superclass(this: &Object) -> &Class {
    let superclass: *const Class = msg_send![this, superclass];
    &*superclass
}

#[allow(dead_code)]
pub unsafe fn open_emoji_picker() {
    let _: () = msg_send![NSApp(), orderFrontCharacterPalette: nil];
}

pub unsafe fn toggle_style_mask(window: id, view: id, mask: NSWindowStyleMask, on: bool) {
    use cocoa::appkit::NSWindow;

    let current_style_mask = window.styleMask();
    if on {
        window.setStyleMask_(current_style_mask | mask);
    } else {
        window.setStyleMask_(current_style_mask & (!mask));
    }

    // If we don't do this, key handling will break. Therefore, never call `setStyleMask` directly!
    window.makeFirstResponder_(view);
}

/// For invalid utf8 sequences potentially returned by `UTF8String`,
/// it behaves identically to `String::from_utf8_lossy`
///
/// Safety: Assumes that `string` is an instance of `NSAttributedString` or `NSString`
pub unsafe fn id_to_string_lossy(string: id) -> String {
    let has_attr: BOOL = msg_send![string, isKindOfClass: class!(NSAttributedString)];
    let characters = if has_attr != NO {
        // This is a *mut NSAttributedString
        msg_send![string, string]
    } else {
        // This is already a *mut NSString
        string
    };
    let utf8_sequence =
        std::slice::from_raw_parts(characters.UTF8String() as *const c_uchar, characters.len());
    String::from_utf8_lossy(utf8_sequence).into_owned()
}
