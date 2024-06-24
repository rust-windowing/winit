//! Apple/Darwin-specific implementations

#[cfg(target_os = "macos")]
mod appkit;
#[cfg(not(target_os = "macos"))]
mod uikit;

#[cfg(target_os = "macos")]
pub use self::appkit::*;
#[cfg(not(target_os = "macos"))]
pub use self::uikit::*;
