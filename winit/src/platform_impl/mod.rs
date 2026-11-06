#[cfg(android_platform)]
pub(crate) use winit_android as platform;
#[cfg(macos_platform)]
pub(crate) use winit_appkit as platform;
#[cfg(any(x11_platform, wayland_platform))]
mod linux;
#[cfg(orbital_platform)]
pub(crate) use winit_orbital as platform;
#[cfg(ios_platform)]
pub(crate) use winit_uikit as platform;
#[cfg(web_platform)]
pub(crate) use winit_web as platform;
#[cfg(windows_platform)]
pub(crate) use winit_win32 as platform;

#[cfg(any(x11_platform, wayland_platform))]
use self::linux as platform;
#[allow(unused_imports)]
pub use self::platform::*;

#[cfg(not(builtin_platform))]
#[path = "external.rs"]
mod platform;
