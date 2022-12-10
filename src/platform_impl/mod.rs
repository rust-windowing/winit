use crate::monitor::{MonitorHandle as RootMonitorHandle, VideoMode as RootVideoMode};
use crate::window::Fullscreen as RootFullscreen;

#[cfg(os_windows)]
#[path = "windows/mod.rs"]
mod platform;
#[cfg(os_linux)]
#[path = "linux/mod.rs"]
mod platform;
#[cfg(os_macos)]
#[path = "macos/mod.rs"]
mod platform;
#[cfg(os_android)]
#[path = "android/mod.rs"]
mod platform;
#[cfg(os_ios)]
#[path = "ios/mod.rs"]
mod platform;
#[cfg(arch_wasm)]
#[path = "web/mod.rs"]
mod platform;

pub use self::platform::*;

/// Helper for converting between platform-specific and generic VideoMode/MonitorHandle
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Fullscreen {
    Exclusive(VideoMode),
    Borderless(Option<MonitorHandle>),
}

impl From<RootFullscreen> for Fullscreen {
    fn from(f: RootFullscreen) -> Self {
        match f {
            RootFullscreen::Exclusive(mode) => Self::Exclusive(mode.video_mode),
            RootFullscreen::Borderless(Some(handle)) => Self::Borderless(Some(handle.inner)),
            RootFullscreen::Borderless(None) => Self::Borderless(None),
        }
    }
}

impl From<Fullscreen> for RootFullscreen {
    fn from(f: Fullscreen) -> Self {
        match f {
            Fullscreen::Exclusive(video_mode) => Self::Exclusive(RootVideoMode { video_mode }),
            Fullscreen::Borderless(Some(inner)) => {
                Self::Borderless(Some(RootMonitorHandle { inner }))
            }
            Fullscreen::Borderless(None) => Self::Borderless(None),
        }
    }
}

#[cfg(all(
    not(os_ios),
    not(os_windows),
    not(os_macos),
    not(os_android),
    not(os_linux),
    not(arch_wasm),
))]
compile_error!("The platform you're compiling for is not supported by winit");
