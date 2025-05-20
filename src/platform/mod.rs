//! Contains traits with platform-specific methods in them.
//!
//! Only the modules corresponding to the platform you're compiling to will be available.

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
#[cfg(web_platform)]
pub mod web;
#[cfg(windows_platform)]
pub mod windows;
#[cfg(x11_platform)]
pub mod x11;

#[cfg(any(windows_platform, macos_platform, x11_platform, wayland_platform, docsrs))]
pub mod scancode;
