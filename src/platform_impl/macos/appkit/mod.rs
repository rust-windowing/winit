#![deny(unsafe_op_in_unsafe_fn)]
// Objective-C methods have different conventions, and it's much easier to
// understand if we just use the same names
#![allow(non_snake_case)]
#![allow(clippy::enum_variant_names)]
#![allow(non_upper_case_globals)]

mod application;
mod button;
mod color;
mod control;
mod cursor;
mod event;
mod image;
mod pasteboard;
mod responder;
mod screen;
mod text_input_context;
mod view;
mod window;

pub(crate) use self::application::{
    NSApp, NSApplication, NSApplicationActivationPolicy, NSApplicationPresentationOptions,
    NSRequestUserAttentionType,
};
pub(crate) use self::button::NSButton;
pub(crate) use self::color::NSColor;
pub(crate) use self::control::NSControl;
pub(crate) use self::cursor::NSCursor;
#[allow(unused_imports)]
pub(crate) use self::event::{
    NSEvent, NSEventModifierFlags, NSEventPhase, NSEventSubtype, NSEventType,
};
pub(crate) use self::image::NSImage;
pub(crate) use self::pasteboard::{NSFilenamesPboardType, NSPasteboardType};
pub(crate) use self::responder::NSResponder;
#[allow(unused_imports)]
pub(crate) use self::screen::{NSDeviceDescriptionKey, NSScreen};
pub(crate) use self::text_input_context::NSTextInputContext;
pub(crate) use self::view::{NSTrackingRectTag, NSView};
pub(crate) use self::window::{
    NSWindow, NSWindowButton, NSWindowLevel, NSWindowOcclusionState, NSWindowStyleMask,
    NSWindowTitleVisibility,
};
