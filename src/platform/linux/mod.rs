#![cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd"))]

pub use self::api_dispatch::{Window, WindowProxy, MonitorId, get_available_monitors, get_primary_monitor};
pub use self::api_dispatch::{WaitEventsIterator, PollEventsIterator};
pub use self::api_dispatch::PlatformSpecificWindowBuilderAttributes;

mod api_dispatch;
