mod r#async;
mod cursor;

pub use self::{cursor::*, r#async::*};

use std::ops::{BitAnd, Deref};

use cocoa::{
    appkit::{NSApp, NSWindowStyleMask},
    base::{id, nil},
    foundation::{NSPoint, NSRect, NSString, NSUInteger},
};
use core_graphics::display::CGDisplay;
use objc::runtime::{Class, Object, Sel, BOOL, YES};

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

#[derive(Debug, PartialEq)]
pub struct IdRef(id);

impl IdRef {
    pub fn new(inner: id) -> IdRef {
        IdRef(inner)
    }

    #[allow(dead_code)]
    pub fn retain(inner: id) -> IdRef {
        if inner != nil {
            let () = unsafe { msg_send![inner, retain] };
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
                let () = msg_send![self.0, release];
            };
        }
    }
}

impl Deref for IdRef {
    type Target = id;
    fn deref<'a>(&'a self) -> &'a id {
        &self.0
    }
}

impl Clone for IdRef {
    fn clone(&self) -> IdRef {
        if self.0 != nil {
            let _: id = unsafe { msg_send![self.0, retain] };
        }
        IdRef(self.0)
    }
}

// For consistency with other platforms, this will...
// 1. translate the bottom-left window corner into the top-left window corner
// 2. translate the coordinate from a bottom-left origin coordinate system to a top-left one
pub fn bottom_left_to_top_left(rect: NSRect) -> f64 {
    CGDisplay::main().pixels_high() as f64 - (rect.origin.y + rect.size.height)
}

/// Converts from winit screen-coordinates to macOS screen-coordinates.
/// Winit: top-left is (0, 0) and y increasing downwards
/// macOS: bottom-left is (0, 0) and y increasing upwards
pub fn window_position(position: LogicalPosition<f64>) -> NSPoint {
    NSPoint::new(
        position.x,
        CGDisplay::main().pixels_high() as f64 - position.y,
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

pub unsafe fn superclass<'a>(this: &'a Object) -> &'a Class {
    let superclass: id = msg_send![this, superclass];
    &*(superclass as *const _)
}

pub unsafe fn create_input_context(view: id) -> IdRef {
    let input_context: id = msg_send![class!(NSTextInputContext), alloc];
    let input_context: id = msg_send![input_context, initWithClient: view];
    IdRef::new(input_context)
}

#[allow(dead_code)]
pub unsafe fn open_emoji_picker() {
    let () = msg_send![NSApp(), orderFrontCharacterPalette: nil];
}

pub extern "C" fn yes(_: &Object, _: Sel) -> BOOL {
    YES
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
