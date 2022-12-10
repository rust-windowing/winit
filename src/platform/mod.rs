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

#[cfg(os_android)]
pub mod android;
#[cfg(os_ios)]
pub mod ios;
#[cfg(os_macos)]
pub mod macos;
#[cfg(all(wayland, os_linux))]
pub mod wayland;
#[cfg(arch_wasm)]
pub mod web;
#[cfg(windows)]
pub mod windows;
#[cfg(all(x11, os_linux))]
pub mod x11;

#[cfg(any(os_windows, os_macos, os_android, os_linux))]
pub mod run_return;
