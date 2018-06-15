use cocoa::appkit::NSWindowStyleMask;
use cocoa::base::{class, id, nil};
use cocoa::foundation::{NSRect, NSUInteger};
use core_graphics::display::CGDisplay;

use platform::platform::ffi;
use platform::platform::window::IdRef;

pub const EMPTY_RANGE: ffi::NSRange = ffi::NSRange {
    location: ffi::NSNotFound as NSUInteger,
    length: 0,
};

// For consistency with other platforms, this will...
// 1. translate the bottom-left window corner into the top-left window corner
// 2. translate the coordinate from a bottom-left origin coordinate system to a top-left one
pub fn bottom_left_to_top_left(rect: NSRect) -> f64 {
    CGDisplay::main().pixels_high() as f64 - (rect.origin.y + rect.size.height)
}

pub unsafe fn set_style_mask(window: id, view: id, mask: NSWindowStyleMask) {
    use cocoa::appkit::NSWindow;
    window.setStyleMask_(mask);
    // If we don't do this, key handling will break. Therefore, never call `setStyleMask` directly!
    window.makeFirstResponder_(view);
}

pub unsafe fn create_input_context(view: id) -> IdRef {
    let input_context: id = msg_send![class("NSTextInputContext"), alloc];
    let input_context: id = msg_send![input_context, initWithClient:view];
    IdRef::new(input_context)
}

#[allow(dead_code)]
pub unsafe fn open_emoji_picker() {
    let app: id = msg_send![class("NSApplication"), sharedApplication];
    let _: () = msg_send![app, orderFrontCharacterPalette:nil];
}
