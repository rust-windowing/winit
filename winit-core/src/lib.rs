//! # Core types for Winit
//!
//! Platform-agnostic types and traits useful when implementing Winit backends,
//! or otherwise interfacing with Winit from library code.
//!
//! See the [`winit`] crate for the full user-facing API.
//!
//! [`winit`]: https://docs.rs/winit

// Every newly exported enum should either be `#[non_exhaustive]`, or carry an `#[allow]` of
// this lint when the set of variants can never grow.
// `clippy::exhaustive_structs` is deliberately not enabled: event structs must stay
// constructible by the backend crates, which `#[non_exhaustive]` would forbid.
#![warn(clippy::exhaustive_enums)]

#[macro_use]
pub mod casting;
pub mod cursor;
#[macro_use]
pub mod error;
pub mod application;
pub mod data_transfer;
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
