//! Safe bindings for the AppKit framework.
//!
//! These are split out from the rest of `winit` to make safety easier to review.
//! In the future, these should probably live in another crate like `cacao`.
//!
//! TODO: Main thread safety.
#![deny(unsafe_op_in_unsafe_fn)]
// Objective-C methods have different conventions, and it's much easier to
// understand if we just use the same names
#![allow(non_snake_case)]
#![allow(clippy::enum_variant_names)]
#![allow(non_upper_case_globals)]

mod application;
mod cursor;
mod event;
mod image;
mod responder;
mod text_input_context;
mod view;
mod window;

pub(crate) use self::application::{NSApp, NSApplication};
pub(crate) use self::cursor::NSCursor;
pub(crate) use self::event::{NSEvent, NSEventModifierFlags, NSEventPhase};
pub(crate) use self::image::NSImage;
pub(crate) use self::responder::NSResponder;
pub(crate) use self::text_input_context::NSTextInputContext;
pub(crate) use self::view::{NSTrackingRectTag, NSView};
pub(crate) use self::window::NSWindow;
