//! Safe bindings for the AppKit framework.
//!
//! These are split out from the rest of `winit` to make safety easier to review.
//! In the future, these should probably live in another crate like `cacao`.
//!
//! TODO: Main thread safety.
// Objective-C methods have different conventions, and it's much easier to
// understand if we just use the same names
#![allow(non_snake_case)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::enum_variant_names)]
#![allow(non_upper_case_globals)]

mod application;
mod tab_group;
mod window;

pub(crate) use self::application::{
    NSApp, NSApplication, NSApplicationActivationPolicy, NSApplicationPresentationOptions,
    NSRequestUserAttentionType,
};
pub(crate) use self::tab_group::NSWindowTabGroup;
pub(crate) use self::window::{
    NSBackingStoreType, NSWindow, NSWindowButton, NSWindowLevel, NSWindowOcclusionState,
    NSWindowSharingType, NSWindowStyleMask, NSWindowTabbingMode, NSWindowTitleVisibility,
};

#[link(name = "AppKit", kind = "framework")]
extern "C" {}
