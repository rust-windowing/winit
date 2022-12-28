#![allow(clippy::unnecessary_cast)]

use std::{
    collections::{BTreeSet, VecDeque},
    fmt,
    ops::{Deref, DerefMut},
};

use objc2::foundation::{MainThreadMarker, NSInteger};
use objc2::rc::{Id, Shared};

use super::uikit::{UIScreen, UIScreenMode};
use crate::{
    dpi::{PhysicalPosition, PhysicalSize},
    monitor::VideoMode as RootVideoMode,
    platform_impl::platform::app_state,
};

// TODO(madsmtm): Remove or refactor this
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub(crate) struct ScreenModeSendSync(pub(crate) Id<UIScreenMode, Shared>);

unsafe impl Send for ScreenModeSendSync {}
unsafe impl Sync for ScreenModeSendSync {}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct VideoMode {
    pub(crate) size: (u32, u32),
    pub(crate) bit_depth: u16,
    pub(crate) refresh_rate_millihertz: u32,
    pub(crate) screen_mode: ScreenModeSendSync,
    pub(crate) monitor: MonitorHandle,
}

impl VideoMode {
    fn new(uiscreen: Id<UIScreen, Shared>, screen_mode: Id<UIScreenMode, Shared>) -> VideoMode {
        assert_main_thread!("`VideoMode` can only be created on the main thread on iOS");
        let refresh_rate_millihertz = refresh_rate_millihertz(&uiscreen);
        let size = screen_mode.size();
        VideoMode {
            size: (size.width as u32, size.height as u32),
            bit_depth: 32,
            refresh_rate_millihertz,
            screen_mode: ScreenModeSendSync(screen_mode),
            monitor: MonitorHandle::new(uiscreen),
        }
    }

    pub fn size(&self) -> PhysicalSize<u32> {
        self.size.into()
    }

    pub fn bit_depth(&self) -> u16 {
        self.bit_depth
    }

    pub fn refresh_rate_millihertz(&self) -> u32 {
        self.refresh_rate_millihertz
    }

    pub fn monitor(&self) -> MonitorHandle {
        self.monitor.clone()
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Inner {
    uiscreen: Id<UIScreen, Shared>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct MonitorHandle {
    inner: Inner,
}

impl PartialOrd for MonitorHandle {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MonitorHandle {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // TODO: Make a better ordering
        (self as *const Self).cmp(&(other as *const Self))
    }
}

impl Deref for MonitorHandle {
    type Target = Inner;

    fn deref(&self) -> &Inner {
        assert_main_thread!("`MonitorHandle` methods can only be run on the main thread on iOS");
        &self.inner
    }
}

impl DerefMut for MonitorHandle {
    fn deref_mut(&mut self) -> &mut Inner {
        assert_main_thread!("`MonitorHandle` methods can only be run on the main thread on iOS");
        &mut self.inner
    }
}

unsafe impl Send for MonitorHandle {}
unsafe impl Sync for MonitorHandle {}

impl Drop for MonitorHandle {
    fn drop(&mut self) {
        assert_main_thread!("`MonitorHandle` can only be dropped on the main thread on iOS");
    }
}

impl fmt::Debug for MonitorHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // TODO: Do this using the proper fmt API
        #[derive(Debug)]
        #[allow(dead_code)]
        struct MonitorHandle {
            name: Option<String>,
            size: PhysicalSize<u32>,
            position: PhysicalPosition<i32>,
            scale_factor: f64,
        }

        let monitor_id_proxy = MonitorHandle {
            name: self.name(),
            size: self.size(),
            position: self.position(),
            scale_factor: self.scale_factor(),
        };

        monitor_id_proxy.fmt(f)
    }
}

impl MonitorHandle {
    pub(crate) fn new(uiscreen: Id<UIScreen, Shared>) -> Self {
        assert_main_thread!("`MonitorHandle` can only be created on the main thread on iOS");
        Self {
            inner: Inner { uiscreen },
        }
    }
}

impl Inner {
    pub fn name(&self) -> Option<String> {
        let main = UIScreen::main(MainThreadMarker::new().unwrap());
        if self.uiscreen == main {
            Some("Primary".to_string())
        } else if self.uiscreen == main.mirroredScreen() {
            Some("Mirrored".to_string())
        } else {
            UIScreen::screens(MainThreadMarker::new().unwrap())
                .iter()
                .position(|rhs| rhs == &*self.uiscreen)
                .map(|idx| idx.to_string())
        }
    }

    pub fn size(&self) -> PhysicalSize<u32> {
        let bounds = self.uiscreen.nativeBounds();
        PhysicalSize::new(bounds.size.width as u32, bounds.size.height as u32)
    }

    pub fn position(&self) -> PhysicalPosition<i32> {
        let bounds = self.uiscreen.nativeBounds();
        (bounds.origin.x as f64, bounds.origin.y as f64).into()
    }

    pub fn scale_factor(&self) -> f64 {
        self.uiscreen.nativeScale() as f64
    }

    pub fn refresh_rate_millihertz(&self) -> Option<u32> {
        Some(refresh_rate_millihertz(&self.uiscreen))
    }

    pub fn video_modes(&self) -> impl Iterator<Item = VideoMode> {
        // Use Ord impl of RootVideoMode
        let modes: BTreeSet<_> = self
            .uiscreen
            .availableModes()
            .into_iter()
            .map(|mode| {
                let mode: *const UIScreenMode = mode;
                let mode = unsafe { Id::retain(mode as *mut UIScreenMode).unwrap() };

                RootVideoMode {
                    video_mode: VideoMode::new(self.uiscreen.clone(), mode),
                }
            })
            .collect();

        modes.into_iter().map(|mode| mode.video_mode)
    }
}

fn refresh_rate_millihertz(uiscreen: &UIScreen) -> u32 {
    let refresh_rate_millihertz: NSInteger = {
        let os_capabilities = app_state::os_capabilities();
        if os_capabilities.maximum_frames_per_second {
            uiscreen.maximumFramesPerSecond()
        } else {
            // https://developer.apple.com/library/archive/technotes/tn2460/_index.html
            // https://en.wikipedia.org/wiki/IPad_Pro#Model_comparison
            //
            // All iOS devices support 60 fps, and on devices where `maximumFramesPerSecond` is not
            // supported, they are all guaranteed to have 60hz refresh rates. This does not
            // correctly handle external displays. ProMotion displays support 120fps, but they were
            // introduced at the same time as the `maximumFramesPerSecond` API.
            //
            // FIXME: earlier OSs could calculate the refresh rate using
            // `-[CADisplayLink duration]`.
            os_capabilities.maximum_frames_per_second_err_msg("defaulting to 60 fps");
            60
        }
    };

    refresh_rate_millihertz as u32 * 1000
}

// MonitorHandleExtIOS
impl Inner {
    pub(crate) fn ui_screen(&self) -> &Id<UIScreen, Shared> {
        &self.uiscreen
    }

    pub fn preferred_video_mode(&self) -> VideoMode {
        VideoMode::new(
            self.uiscreen.clone(),
            self.uiscreen.preferredMode().unwrap(),
        )
    }
}

pub fn uiscreens(mtm: MainThreadMarker) -> VecDeque<MonitorHandle> {
    UIScreen::screens(mtm)
        .into_iter()
        .map(|screen| {
            let screen: *const UIScreen = screen;
            let screen = unsafe { Id::retain(screen as *mut UIScreen).unwrap() };
            MonitorHandle::new(screen)
        })
        .collect()
}
