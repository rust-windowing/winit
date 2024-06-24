use crate::monitor::{MonitorHandle as RootMonitorHandle, VideoModeHandle as RootVideoModeHandle};
use crate::window::Fullscreen as RootFullscreen;

#[cfg(android_platform)]
mod android;
#[cfg(ios_platform)]
mod ios;
#[cfg(any(x11_platform, wayland_platform))]
mod linux;
#[cfg(macos_platform)]
mod macos;
#[cfg(orbital_platform)]
mod orbital;
#[cfg(web_platform)]
mod web;
#[cfg(windows_platform)]
mod windows;

#[cfg(android_platform)]
use android as platform;
#[cfg(ios_platform)]
use ios as platform;
#[cfg(any(x11_platform, wayland_platform))]
use linux as platform;
#[cfg(macos_platform)]
use macos as platform;
#[cfg(orbital_platform)]
use orbital as platform;
#[cfg(web_platform)]
use web as platform;
#[cfg(windows_platform)]
use windows as platform;

pub use self::platform::*;

/// Helper for converting between platform-specific and generic
/// [`VideoModeHandle`]/[`MonitorHandle`]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Fullscreen {
    Exclusive(VideoModeHandle),
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
            Fullscreen::Exclusive(video_mode) => {
                Self::Exclusive(RootVideoModeHandle { video_mode })
            },
            Fullscreen::Borderless(Some(inner)) => {
                Self::Borderless(Some(RootMonitorHandle { inner }))
            },
            Fullscreen::Borderless(None) => Self::Borderless(None),
        }
    }
}

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
