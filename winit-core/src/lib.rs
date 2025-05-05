//! # Core types for Winit
//!
//! Platform-agnostic types and traits useful when implementing Winit backends,
//! or otherwise interfacing with Winit from library code.
//!
//! See the [`winit`] crate for the full user-facing API.
//!
//! [`winit`]: https://docs.rs/winit

#[macro_use]
pub mod as_any;
pub mod cursor;
#[macro_use]
pub mod error;
pub mod application;
pub mod event;
pub mod event_loop;
pub mod icon;
pub mod keyboard;
pub mod monitor;
pub mod window;
