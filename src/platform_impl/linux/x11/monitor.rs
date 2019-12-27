use std::os::raw;
use std::{env, str::FromStr};

use parking_lot::Mutex;
use x11_dl::xrandr::{RRMode, RRCrtc};
use super::{
    util, XConnection
};
use crate::{
    dpi::{PhysicalPosition, PhysicalSize, validate_hidpi_factor},
    monitor::{MonitorHandle as RootMonitorHandle, VideoMode as RootVideoMode},
    platform_impl::{MonitorHandle as PlatformMonitorHandle, VideoMode as PlatformVideoMode},
};

// Used for testing. This should always be committed as false.
const DISABLE_MONITOR_LIST_CACHING: bool = false;

lazy_static! {
    static ref MONITORS: Mutex<Option<Vec<MonitorHandle>>> = Mutex::default();
}

pub fn invalidate_cached_monitor_list() -> Option<Vec<MonitorHandle>> {
    // We update this lazily.
    (*MONITORS.lock()).take()
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VideoMode {
    pub(crate) size: (u32, u32),
    pub(crate) bit_depth: u16,
    pub(crate) refresh_rate: u16,
    pub(crate) monitor: Option<MonitorHandle>,
    /// RandR only. None otherwise.
    pub(crate) native_mode: Option<RRMode>,
}

/// Which monitor extention we are going to try to use. XRandR is best of
/// course, but if we can't find it we will try to use Xinerama. And if that is
/// not present, we use nothing.
#[derive(PartialEq, Eq, Debug, Copy, Clone)]
pub enum MonitorExt {
    XRandR,
    Xinerama,
    None,
}

impl VideoMode {
    #[inline]
    pub fn size(&self) -> PhysicalSize {
        self.size.into()
    }

    #[inline]
    pub fn bit_depth(&self) -> u16 {
        self.bit_depth
    }

    #[inline]
    pub fn refresh_rate(&self) -> u16 {
        self.refresh_rate
    }

    #[inline]
    pub fn monitor(&self) -> RootMonitorHandle {
        RootMonitorHandle {
            inner: PlatformMonitorHandle::X(self.monitor.clone().unwrap()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MonitorHandle {
    /// The actual id, RandR only.
    pub(crate) id: Option<RRCrtc>,
    /// X11 screen. None if dummy.
    pub(crate) screen: Option<raw::c_int>,
    /// The name of the monitor
    pub(crate) name: String,
    /// The size of the monitor
    pub(crate) dimensions: (u32, u32),
    /// The position of the monitor in the X screen
    pub(crate) position: (i32, i32),
    /// If the monitor is the primary one
    pub(crate) primary: bool,
    /// The DPI scale factor
    pub(crate) hidpi_factor: f64,
    /// Used to determine which windows are on this monitor
    pub(crate) rect: util::AaRect,
    /// Supported video modes on this monitor
    pub(crate) video_modes: Vec<VideoMode>,
}

impl PartialEq for MonitorHandle {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for MonitorHandle {}

impl PartialOrd for MonitorHandle {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(&other))
    }
}

impl Ord for MonitorHandle {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl std::hash::Hash for MonitorHandle {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl MonitorHandle {
    pub fn dummy() -> Self {
        MonitorHandle {
            id: Some(0),
            name: "<dummy monitor>".into(),
            hidpi_factor: 1.0,
            dimensions: (1, 1),
            position: (0, 0),
            primary: true,
            rect: util::AaRect::new((0, 0), (1, 1)),
            video_modes: Vec::new(),
            screen: None,
        }
    }

    pub(crate) fn is_dummy(&self) -> bool {
        // Zero is an invalid XID value; no real monitor will have it
        self.id == Some(0)
    }

    pub fn name(&self) -> Option<String> {
        Some(self.name.clone())
    }

    #[inline]
    pub fn native_id(&self) -> Option<u32> {
        self.id.map(|id| id as u32)
    }

    #[inline]
    pub fn x11_screen(&self) -> Option<raw::c_int> {
        self.screen
    }

    pub fn size(&self) -> PhysicalSize {
        self.dimensions.into()
    }

    pub fn position(&self) -> PhysicalPosition {
        self.position.into()
    }

    #[inline]
    pub fn hidpi_factor(&self) -> f64 {
        self.hidpi_factor
    }

    #[inline]
    pub fn video_modes(&self) -> impl Iterator<Item = RootVideoMode> {
        let monitor = self.clone();
        self.video_modes.clone().into_iter().map(move |mut x| {
            x.monitor = Some(monitor.clone());
            RootVideoMode {
                video_mode: PlatformVideoMode::X(x),
            }
        })
    }
}

impl XConnection {
    pub fn get_monitor_for_window(&self, window_rect: Option<util::AaRect>, window_screen: raw::c_int) -> MonitorHandle {
        let monitors = self.available_monitors();

        if monitors.is_empty() {
            // Return a dummy monitor to avoid panicking
            return MonitorHandle::dummy();
        }

        let default = monitors.get(0).unwrap();

        let window_rect = match window_rect {
            Some(rect) => rect,
            None => return default.to_owned(),
        };

        let mut largest_overlap = 0;
        let mut matched_monitor = default;
        for monitor in &monitors {
            if monitor.screen != Some(window_screen) { continue };
            let overlapping_area = window_rect.get_overlapping_area(&monitor.rect);
            if overlapping_area > largest_overlap {
                largest_overlap = overlapping_area;
                matched_monitor = &monitor;
            }
        }

        matched_monitor.to_owned()
    }

    fn query_monitor_list(&self) -> Vec<MonitorHandle> {
        match self.monitor_ext {
            MonitorExt::XRandR => self.query_monitor_list_xrandr(),
            MonitorExt::Xinerama => self.query_monitor_list_xinerama(),
            MonitorExt::None => self.query_monitor_list_none(),
        }
    }

    pub fn available_monitors(&self) -> Vec<MonitorHandle> {
        let mut monitors_lock = MONITORS.lock();
        (*monitors_lock)
            .as_ref()
            .cloned()
            .or_else(|| {
                let monitors = Some(self.query_monitor_list());
                if !DISABLE_MONITOR_LIST_CACHING {
                    (*monitors_lock) = monitors.clone();
                }
                monitors
            })
            .unwrap()
    }

    #[inline]
    pub fn primary_monitor(&self) -> MonitorHandle {
        self.available_monitors()
            .into_iter()
            .find(|monitor| monitor.primary)
            .unwrap_or_else(MonitorHandle::dummy)
    }
}

pub fn calc_dpi_factor(
    (width_px, height_px): (u32, u32),
    (width_mm, height_mm): (u64, u64),
) -> f64 {
    // Override DPI if `WINIT_HIDPI_FACTOR` variable is set
    let dpi_override = env::var("WINIT_HIDPI_FACTOR")
        .ok()
        .and_then(|var| f64::from_str(&var).ok());
    if let Some(dpi_override) = dpi_override {
        if !validate_hidpi_factor(dpi_override) {
            panic!(
                "[winit] `WINIT_HIDPI_FACTOR` invalid; DPI factors must be normal floats greater than 0. Got `{}`",
                dpi_override,
            );
        }
        return dpi_override;
    }

    // See http://xpra.org/trac/ticket/728 for more information.
    if width_mm == 0 || height_mm == 0 {
        warn!("XRandR reported that the display's 0mm in size, which is certifiably insane");
        return 1.0;
    }

    let ppmm = ((width_px as f64 * height_px as f64) / (width_mm as f64 * height_mm as f64)).sqrt();
    // Quantize 1/12 step size
    let dpi_factor = ((ppmm * (12.0 * 25.4 / 96.0)).round() / 12.0).max(1.0);
    assert!(validate_hidpi_factor(dpi_factor));
    dpi_factor
}
