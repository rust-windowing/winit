//! Contains traits with platform-specific methods in them.
//!
//! Contains the follow OS-specific modules:
//!
//!  - `android`
//!  - `ios`
//!  - `macos`
//!  - `unix`
//!  - `windows`
//!  - `web`
//!
//! And the following platform-specific module:
//!
//! - `run_return` (available on `windows`, `unix`, `macos`, and `android`)
//!
//! However only the module corresponding to the platform you're compiling to will be available.

#[cfg(target_os = "android")]
pub mod android;
#[cfg(target_os = "ios")]
pub mod ios;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(wayland)]
pub mod wayland;
#[cfg(wasm)]
pub mod web;
#[cfg(windows)]
pub mod windows;
#[cfg(x11)]
pub mod x11;

#[cfg(any(windows, x11, wayland, target_os = "macos", target_os = "android"))]
pub mod run_return;
