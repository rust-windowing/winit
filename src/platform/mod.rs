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
//! And the following platform-specific modules:
//!
//! - `run_ondemand` (available on `windows`, `android`)
//! - `pump_events` (available on `windows`, `android`)
//! - `run_return` (available on `windows`, `unix`, `macos`, and `android`)
//!
//! However only the module corresponding to the platform you're compiling to will be available.

#[cfg(android_platform)]
pub mod android;
#[cfg(ios_platform)]
pub mod ios;
#[cfg(macos_platform)]
pub mod macos;
#[cfg(orbital_platform)]
pub mod orbital;
#[cfg(any(x11_platform, wayland_platform))]
pub mod startup_notify;
#[cfg(wayland_platform)]
pub mod wayland;
#[cfg(wasm_platform)]
pub mod web;
#[cfg(windows_platform)]
pub mod windows;
#[cfg(x11_platform)]
pub mod x11;

#[cfg(any(windows_platform, android_platform))]
pub mod run_ondemand;

#[cfg(any(windows_platform, android_platform,))]
pub mod pump_events;

#[cfg(any(
    windows_platform,
    macos_platform,
    android_platform,
    x11_platform,
    wayland_platform,
    orbital_platform
))]
pub mod run_return;

pub mod modifier_supplement;
pub mod scancode;
