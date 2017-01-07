#![cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "openbsd"))]

pub use self::api_dispatch::{Window, WindowProxy, MonitorId, get_available_monitors, get_primary_monitor};
pub use self::api_dispatch::{WaitEventsIterator, PollEventsIterator};
pub use self::api_dispatch::PlatformSpecificWindowBuilderAttributes;
pub use self::api_dispatch::Backend as UnixBackend;
pub use self::api_dispatch::BACKEND as UNIX_BACKEND;

mod api_dispatch;
