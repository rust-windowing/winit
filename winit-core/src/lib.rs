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

// `Instant` is not actually available on `wasm32-unknown-unknown`, the `std` implementation there
// is a stub. And `wasm32-none` doesn't even have `std`. Instead, we use `web_time::Instant`.
#[cfg(not(all(target_family = "wasm", any(target_os = "unknown", target_os = "none"))))]
pub(crate) use std::time::Instant;

#[cfg(all(target_family = "wasm", any(target_os = "unknown", target_os = "none")))]
pub(crate) use web_time::Instant;
