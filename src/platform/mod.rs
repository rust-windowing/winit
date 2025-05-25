//! Contains traits with platform-specific methods in them.
//!
//! Only the modules corresponding to the platform you're compiling to will be available.

#[cfg(android_platform)]
pub use winit_android as android;
#[cfg(macos_platform)]
pub use winit_appkit as macos;
#[cfg(orbital_platform)]
pub use winit_orbital as orbital;
#[cfg(ios_platform)]
pub use winit_uikit as ios;
#[cfg(any(x11_platform, wayland_platform))]
pub mod startup_notify;
#[cfg(wayland_platform)]
pub use winit_wayland as wayland;
#[cfg(web_platform)]
pub mod web;
#[cfg(windows_platform)]
pub use winit_win32 as windows;
#[cfg(x11_platform)]
pub use winit_x11 as x11;

#[cfg(any(windows_platform, macos_platform, x11_platform, wayland_platform, docsrs))]
pub mod scancode;
