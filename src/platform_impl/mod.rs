#[cfg(android_platform)]
mod android;
#[cfg(target_vendor = "apple")]
mod apple;
#[cfg(any(x11_platform, wayland_platform))]
mod linux;
#[cfg(orbital_platform)]
mod orbital;
#[cfg(web_platform)]
mod web;
#[cfg(windows_platform)]
mod windows;

#[cfg(android_platform)]
use self::android as platform;
#[cfg(target_vendor = "apple")]
use self::apple as platform;
#[cfg(any(x11_platform, wayland_platform))]
use self::linux as platform;
#[cfg(orbital_platform)]
use self::orbital as platform;
pub use self::platform::*;
#[cfg(web_platform)]
use self::web as platform;
#[cfg(windows_platform)]
use self::windows as platform;

#[cfg(all(
    not(ios_platform),
    not(windows_platform),
    not(macos_platform),
    not(android_platform),
    not(x11_platform),
    not(wayland_platform),
    not(web_platform),
    not(orbital_platform),
))]
compile_error!("The platform you're compiling for is not supported by winit");
