pub use self::platform::*;

#[cfg(target_os = "windows")]
#[path="windows/mod.rs"]
mod platform;
#[cfg(target_os = "linux")]
#[path="linux/mod.rs"]
mod platform;
#[cfg(target_os = "macos")]
#[path="macos/mod.rs"]
mod platform;
#[cfg(target_os = "android")]
#[path="android/mod.rs"]
mod platform;

#[cfg(all(not(target_os = "windows"), not(target_os = "linux"), not(target_os = "macos"), not(target_os = "android")))]
use this_platform_is_not_supported;
