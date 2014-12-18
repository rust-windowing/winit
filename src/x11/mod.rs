#[cfg(feature = "headless")]
pub use self::headless::HeadlessContext;

#[cfg(feature = "window")]
pub use self::window::{Window, WindowProxy, MonitorID, get_available_monitors, get_primary_monitor};

mod ffi;

#[cfg(feature = "headless")]
mod headless;

#[cfg(feature = "window")]
mod window;
