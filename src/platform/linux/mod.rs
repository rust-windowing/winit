#![cfg(target_os = "linux")]

#[cfg(feature = "headless")]
pub use api::osmesa::OsMesaContext as HeadlessContext;

#[cfg(feature = "window")]
pub use api::x11::{Window, WindowProxy, MonitorID, get_available_monitors, get_primary_monitor};
#[cfg(feature = "window")]
pub use api::x11::{WaitEventsIterator, PollEventsIterator};

#[cfg(not(feature = "window"))]
pub type Window = ();       // TODO: hack to make things work
#[cfg(not(feature = "window"))]
pub type MonitorID = ();       // TODO: hack to make things work
