#![allow(clippy::unnecessary_cast)]

use std::{
    collections::{BTreeSet, VecDeque},
    fmt, hash, ptr,
};

use icrate::Foundation::{MainThreadBound, MainThreadMarker, NSInteger};
use objc2::mutability::IsRetainable;
use objc2::rc::Id;

use super::uikit::{UIScreen, UIScreenMode};
use crate::{
    dpi::{PhysicalPosition, PhysicalSize},
    monitor::VideoMode as RootVideoMode,
    platform_impl::platform::app_state,
};

// Workaround for `MainThreadBound` implementing almost no traits
#[derive(Debug)]
struct MainThreadBoundDelegateImpls<T>(MainThreadBound<Id<T>>);

impl<T: IsRetainable> Clone for MainThreadBoundDelegateImpls<T> {
    fn clone(&self) -> Self {
        Self(
            self.0
                .get_on_main(|inner, mtm| MainThreadBound::new(Id::clone(inner), mtm)),
        )
    }
}

impl<T: IsRetainable> hash::Hash for MainThreadBoundDelegateImpls<T> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        // SAFETY: Marker only used to get the pointer
        let mtm = unsafe { MainThreadMarker::new_unchecked() };
        Id::as_ptr(self.0.get(mtm)).hash(state);
    }
}

impl<T: IsRetainable> PartialEq for MainThreadBoundDelegateImpls<T> {
    fn eq(&self, other: &Self) -> bool {
        // SAFETY: Marker only used to get the pointer
        let mtm = unsafe { MainThreadMarker::new_unchecked() };
        Id::as_ptr(self.0.get(mtm)) == Id::as_ptr(other.0.get(mtm))
    }
}

impl<T: IsRetainable> Eq for MainThreadBoundDelegateImpls<T> {}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct VideoMode {
    pub(crate) size: (u32, u32),
    pub(crate) bit_depth: u16,
    pub(crate) refresh_rate_millihertz: u32,
    screen_mode: MainThreadBoundDelegateImpls<UIScreenMode>,
    pub(crate) monitor: MonitorHandle,
}

impl VideoMode {
    fn new(
        uiscreen: Id<UIScreen>,
        screen_mode: Id<UIScreenMode>,
        mtm: MainThreadMarker,
    ) -> VideoMode {
        let refresh_rate_millihertz = refresh_rate_millihertz(&uiscreen);
        let size = screen_mode.size();
        VideoMode {
            size: (size.width as u32, size.height as u32),
            bit_depth: 32,
            refresh_rate_millihertz,
            screen_mode: MainThreadBoundDelegateImpls(MainThreadBound::new(screen_mode, mtm)),
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

    pub(super) fn screen_mode(&self, mtm: MainThreadMarker) -> &Id<UIScreenMode> {
        self.screen_mode.0.get(mtm)
    }
}

pub struct MonitorHandle {
    ui_screen: MainThreadBound<Id<UIScreen>>,
}

impl Clone for MonitorHandle {
    fn clone(&self) -> Self {
        Self {
            ui_screen: self
                .ui_screen
                .get_on_main(|inner, mtm| MainThreadBound::new(inner.clone(), mtm)),
        }
    }
}

impl hash::Hash for MonitorHandle {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        (self as *const Self).hash(state);
    }
}

impl PartialEq for MonitorHandle {
    fn eq(&self, other: &Self) -> bool {
        ptr::eq(self, other)
    }
}

impl Eq for MonitorHandle {}

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
    pub(crate) fn new(ui_screen: Id<UIScreen>) -> Self {
        // Holding `Id<UIScreen>` implies we're on the main thread.
        let mtm = MainThreadMarker::new().unwrap();
        Self {
            ui_screen: MainThreadBound::new(ui_screen, mtm),
        }
    }

    pub fn name(&self) -> Option<String> {
        self.ui_screen.get_on_main(|ui_screen, mtm| {
            let main = UIScreen::main(mtm);
            if *ui_screen == main {
                Some("Primary".to_string())
            } else if *ui_screen == main.mirroredScreen() {
                Some("Mirrored".to_string())
            } else {
                UIScreen::screens(mtm)
                    .iter()
                    .position(|rhs| rhs == &**ui_screen)
                    .map(|idx| idx.to_string())
            }
        })
    }

    pub fn size(&self) -> PhysicalSize<u32> {
        let bounds = self
            .ui_screen
            .get_on_main(|ui_screen, _| ui_screen.nativeBounds());
        PhysicalSize::new(bounds.size.width as u32, bounds.size.height as u32)
    }

    pub fn position(&self) -> PhysicalPosition<i32> {
        let bounds = self
            .ui_screen
            .get_on_main(|ui_screen, _| ui_screen.nativeBounds());
        (bounds.origin.x as f64, bounds.origin.y as f64).into()
    }

    pub fn scale_factor(&self) -> f64 {
        self.ui_screen
            .get_on_main(|ui_screen, _| ui_screen.nativeScale()) as f64
    }

    pub fn refresh_rate_millihertz(&self) -> Option<u32> {
        Some(
            self.ui_screen
                .get_on_main(|ui_screen, _| refresh_rate_millihertz(ui_screen)),
        )
    }

    pub fn video_modes(&self) -> impl Iterator<Item = VideoMode> {
        self.ui_screen.get_on_main(|ui_screen, mtm| {
            // Use Ord impl of RootVideoMode

            let modes: BTreeSet<_> = ui_screen
                .availableModes()
                .into_iter()
                .map(|mode| RootVideoMode {
                    video_mode: VideoMode::new(ui_screen.clone(), mode, mtm),
                })
                .collect();

            modes.into_iter().map(|mode| mode.video_mode)
        })
    }

    pub(crate) fn ui_screen(&self, mtm: MainThreadMarker) -> &Id<UIScreen> {
        self.ui_screen.get(mtm)
    }

    pub fn preferred_video_mode(&self) -> VideoMode {
        self.ui_screen.get_on_main(|ui_screen, mtm| {
            VideoMode::new(ui_screen.clone(), ui_screen.preferredMode().unwrap(), mtm)
        })
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

pub fn uiscreens(mtm: MainThreadMarker) -> VecDeque<MonitorHandle> {
    UIScreen::screens(mtm)
        .into_iter()
        .map(MonitorHandle::new)
        .collect()
}
