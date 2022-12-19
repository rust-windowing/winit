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

#[cfg(android)]
pub mod android;
#[cfg(os_ios)]
pub mod ios;
#[cfg(macos)]
pub mod macos;
#[cfg(wayland_platform)]
pub mod wayland;
#[cfg(wasm)]
pub mod web;
#[cfg(windows)]
pub mod windows;
#[cfg(all(x11_platform, free_unix))]
pub mod x11;

#[cfg(any(windows, macos, android, free_unix))]
pub mod run_return;
