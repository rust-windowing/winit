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

#[cfg(android_platform)]
pub mod android;
#[cfg(ios_platform)]
pub mod ios;
#[cfg(macos_platform)]
pub mod macos;
#[cfg(orbital_platform)]
pub mod orbital;
#[cfg(wayland_platform)]
pub mod wayland;
#[cfg(wasm_platform)]
pub mod web;
#[cfg(windows_platform)]
pub mod windows;
#[cfg(x11_platform)]
pub mod x11;

#[cfg(any(
    windows_platform,
    macos_platform,
    android_platform,
    x11_platform,
    wayland_platform,
    orbital_platform
))]
pub mod run_return;

/// Enumeration of platforms
///
/// Each option is compile-time enabled only if that platform is possible.
/// Methods like [`Self::is_wayland`] are available on all platforms.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Platform {
    #[cfg(android_platform)]
    Android,

    #[cfg(ios_platform)]
    IOS,

    #[cfg(macos_platform)]
    MacOS,

    #[cfg(orbital_platform)]
    Orbital,

    #[cfg(wasm_platform)]
    Web,

    #[cfg(windows_platform)]
    Windows,

    #[cfg(any(x11_platform, wayland_platform))]
    Wayland,
    #[cfg(any(x11_platform, wayland_platform))]
    X11,
}

impl Platform {
    /// True if the platform is Android
    pub fn is_android(&self) -> bool {
        cfg_if::cfg_if! {
            if #[cfg(android_platform)] {
                true
            } else {
                false
            }
        }
    }

    /// True if the platform is IOS
    pub fn is_ios(&self) -> bool {
        cfg_if::cfg_if! {
            if #[cfg(ios_platform)] {
                true
            } else {
                false
            }
        }
    }

    /// True if the platform is MacOS
    pub fn is_macos(&self) -> bool {
        cfg_if::cfg_if! {
            if #[cfg(macos_platform)] {
                true
            } else {
                false
            }
        }
    }

    /// True if the platform is Orbital
    pub fn is_orbital(&self) -> bool {
        cfg_if::cfg_if! {
            if #[cfg(orbital_platform)] {
                true
            } else {
                false
            }
        }
    }

    /// True if the platform is Web
    pub fn is_web(&self) -> bool {
        cfg_if::cfg_if! {
            if #[cfg(wasm_platform)] {
                true
            } else {
                false
            }
        }
    }

    /// True if the platform is Windows
    pub fn is_windows(&self) -> bool {
        cfg_if::cfg_if! {
            if #[cfg(windows_platform)] {
                true
            } else {
                false
            }
        }
    }

    /// True if the platform is Wayland
    pub fn is_wayland(&self) -> bool {
        cfg_if::cfg_if! {
            if #[cfg(any(x11_platform, wayland_platform))] {
                matches!(self, Platform::Wayland)
            } else {
                false
            }
        }
    }

    /// True if the platform is X11
    pub fn is_x11(&self) -> bool {
        cfg_if::cfg_if! {
            if #[cfg(any(x11_platform, wayland_platform))] {
                matches!(self, Platform::X11)
            } else {
                false
            }
        }
    }
}
