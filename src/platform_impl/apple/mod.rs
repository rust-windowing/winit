#[cfg(target_os = "macos")]
mod appkit;
#[cfg(not(target_os = "macos"))]
mod uikit;

#[allow(unused_imports)]
#[cfg(target_os = "macos")]
pub use self::appkit::*;
#[allow(unused_imports)]
#[cfg(not(target_os = "macos"))]
pub use self::uikit::*;
