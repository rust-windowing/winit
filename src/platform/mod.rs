//! Contains traits with platform-specific methods in them.
//!
//! Only the modules corresponding to the platform you're compiling to will be available.

#[cfg(any(android_platform, docsrs))]
pub mod android;
#[cfg(any(ios_platform, docsrs))]
pub mod ios;
#[cfg(any(macos_platform, docsrs))]
pub mod macos;
#[cfg(any(orbital_platform, docsrs))]
pub mod orbital;
#[cfg(any(x11_platform, wayland_platform, docsrs))]
pub mod startup_notify;
#[cfg(any(wayland_platform, docsrs))]
pub mod wayland;
#[cfg(any(wasm_platform, docsrs))]
pub mod web;
#[cfg(any(windows_platform, docsrs))]
pub mod windows;
#[cfg(any(x11_platform, docsrs))]
pub mod x11;

#[cfg(any(
    windows_platform,
    macos_platform,
    android_platform,
    x11_platform,
    wayland_platform,
    docsrs,
))]
pub mod run_on_demand;

#[cfg(any(
    windows_platform,
    macos_platform,
    android_platform,
    x11_platform,
    wayland_platform,
    docsrs,
))]
pub mod pump_events;

#[cfg(any(
    windows_platform,
    macos_platform,
    x11_platform,
    wayland_platform,
    docsrs
))]
pub mod modifier_supplement;

#[cfg(any(
    windows_platform,
    macos_platform,
    x11_platform,
    wayland_platform,
    docsrs
))]
pub mod scancode;
