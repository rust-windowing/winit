use std::os::raw;
use std::{env, ptr, str::FromStr};

use super::{
    ffi::{XRRCrtcInfo, XRROutputInfo},
    util, XConnection,
};
use crate::{
    monitor::{MonitorHandle as RootMonitorHandle, VideoMode as RootVideoMode},
    platform_impl::{MonitorHandle as PlatformMonitorHandle, VideoMode as PlatformVideoMode},
};

use parking_lot::Mutex;
use winit_types::dpi::{self, PhysicalPosition, PhysicalSize};
use x11_dl::xrandr::{RRCrtc, RRMode};

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

/// How we are going to get the scale factor.
#[derive(PartialEq, Debug, Copy, Clone)]
pub enum ScaleFactorSource {
    Scale(f64),
    Xft,
    XRandR,
    Xlib,
}

impl From<MonitorInfoSource> for ScaleFactorSource {
    fn from(mis: MonitorInfoSource) -> Self {
        match mis {
            MonitorInfoSource::XRandR => ScaleFactorSource::XRandR,
            MonitorInfoSource::Xinerama => ScaleFactorSource::Xlib,
            MonitorInfoSource::Xlib => ScaleFactorSource::Xlib,
        }
    }
}

impl From<MonitorInfoSource> for Vec<ScaleFactorSource> {
    fn from(mis: MonitorInfoSource) -> Self {
        // Override DPI if `WINIT_X11_SCALE_FACTOR` variable is set
        let deprecated_scale_override = env::var("WINIT_HIDPI_FACTOR").ok();
        if deprecated_scale_override.is_some() {
            warn!(
                "[winit] The WINIT_HIDPI_FACTOR environment variable is deprecated; use WINIT_X11_SCALE_FACTOR"
            )
        }

        let sfc: ScaleFactorSource = mis.into();
        let default = vec![ScaleFactorSource::Xft, sfc];
        env::var("WINIT_X11_SCALE_FACTOR").ok().map_or_else(
            || default.clone(),
            |var| match var.trim() {
                "" => default.clone(),
                var => {
                    var
                        .to_lowercase()
                        .split(",")
                        .map(|var| match var {
                            "randr" => ScaleFactorSource::XRandR,
                            "xlib" => ScaleFactorSource::Xlib,
                            "xft" => ScaleFactorSource::Xft,
                            _ => {
                                if let Ok(scale) = f64::from_str(&var) {
                                    if !dpi::validate_scale_factor(scale) {
                                        panic!(
                                            "[winit] `WINIT_X11_SCALE_FACTOR` invalid; Scale factors must be either normal floats greater than 0, or `randr`. Got `{}`",
                                            scale,
                                        );
                                    }
                                    ScaleFactorSource::Scale(scale)
                                } else {
                                    panic!(
                                        "[winit] `WINIT_X11_SCALE_FACTOR` invalid; Scale factors must be either normal floats greater than 0, or `randr`. Got `{}`",
                                        var
                                    );
                                }
                            }
                        })
                        .collect()
                }
            },
        )
    }
}

/// How we are going to get monitor info.
#[derive(PartialEq, Eq, Debug, Copy, Clone)]
pub enum MonitorInfoSource {
    XRandR,
    Xinerama,
    Xlib,
}

impl VideoMode {
    #[inline]
    pub fn size(&self) -> PhysicalSize<u32> {
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
    pub(crate) scale_factor: f64,
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
            scale_factor: 1.0,
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

    pub fn size(&self) -> PhysicalSize<u32> {
        self.dimensions.into()
    }

    pub fn position(&self) -> PhysicalPosition<i32> {
        self.position.into()
    }

    #[inline]
    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
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
    pub fn get_monitor_for_window(
        &self,
        window_rect: Option<util::AaRect>,
        window_screen: raw::c_int,
    ) -> MonitorHandle {
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
            if monitor.screen != Some(window_screen) {
                continue;
            };
            let overlapping_area = window_rect.get_overlapping_area(&monitor.rect);
            if overlapping_area > largest_overlap {
                largest_overlap = overlapping_area;
                matched_monitor = &monitor;
            }
        }

        matched_monitor.to_owned()
    }

    fn query_monitor_list(&self) -> Vec<MonitorHandle> {
        match self.monitor_info_source {
            MonitorInfoSource::XRandR => self.query_monitor_list_xrandr(),
            MonitorInfoSource::Xinerama => self.query_monitor_list_xinerama(),
            MonitorInfoSource::Xlib => self.query_monitor_list_none(),
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

pub fn calc_scale_factor(
    (width_px, height_px): (u32, u32),
    (width_mm, height_mm): (u64, u64),
) -> f64 {
    // See http://xpra.org/trac/ticket/728 for more information.
    if width_mm == 0 || height_mm == 0 {
        warn!(
            "[winit] XRandR reported that the display's 0mm in size, which is certifiably insane"
        );
        return 1.0;
    }

    let ppmm = ((width_px as f64 * height_px as f64) / (width_mm as f64 * height_mm as f64)).sqrt();
    // Quantize 1/12 step size
    let scale_factor = ((ppmm * (12.0 * 25.4 / 96.0)).round() / 12.0).max(1.0);
    assert!(dpi::validate_scale_factor(scale_factor));
    scale_factor
}

impl XConnection {
    // Retrieve DPI from Xft.dpi property
    pub fn get_xft_scale(&self) -> Option<f64> {
        unsafe {
            let xlib = syms!(XLIB);
            (xlib.XrmInitialize)();
            let resource_manager_str = (xlib.XResourceManagerString)(**self.display);
            if resource_manager_str == ptr::null_mut() {
                return None;
            }
            if let Ok(res) = ::std::ffi::CStr::from_ptr(resource_manager_str).to_str() {
                let name: &str = "Xft.dpi:\t";
                for pair in res.split("\n") {
                    if pair.starts_with(&name) {
                        let res = &pair[name.len()..];
                        return f64::from_str(&res).ok();
                    }
                }
            }
            None
        }
    }

    pub fn get_xlib_dims(&self, screen: raw::c_int) -> ((u32, u32), (u64, u64)) {
        unsafe {
            let xlib = syms!(XLIB);
            let screen_ptr = (xlib.XScreenOfDisplay)(**self.display, screen);
            let dimensions = (
                (xlib.XWidthOfScreen)(screen_ptr) as u32,
                (xlib.XHeightOfScreen)(screen_ptr) as u32,
            );
            let dimensions_mm = (
                (xlib.XWidthMMOfScreen)(screen_ptr) as u64,
                (xlib.XHeightMMOfScreen)(screen_ptr) as u64,
            );
            (dimensions, dimensions_mm)
        }
    }

    pub fn acquire_scale_factor(
        &self,
        xrandr_parts: Option<(*mut XRROutputInfo, *mut XRRCrtcInfo)>,
        screen: raw::c_int,
    ) -> Option<f64> {
        let scale_factor_sources: Vec<ScaleFactorSource> = self.monitor_info_source.into();

        for sfc in scale_factor_sources {
            match sfc {
                ScaleFactorSource::Scale(s) => return Some(s),
                ScaleFactorSource::Xft => {
                    if let Some(s) = self.get_xft_scale() {
                        return Some(s / 96.0);
                    }
                }
                ScaleFactorSource::Xlib => {
                    let (dimensions, dimensions_mm) = self.get_xlib_dims(screen);
                    return Some(calc_scale_factor(dimensions, dimensions_mm));
                }
                ScaleFactorSource::XRandR => unsafe {
                    if let Some((output_info, crtc)) = xrandr_parts {
                        return Some(calc_scale_factor(
                            ((*crtc).width as u32, (*crtc).height as u32),
                            (
                                (*output_info).mm_width as u64,
                                (*output_info).mm_height as u64,
                            ),
                        ));
                    } else {
                        panic!(
                            "[winit] `WINIT_X11_SCALE_FACTOR` had `randr`, but system does not have RANDR ext.",
                        );
                    }
                },
            }
        }

        None
    }
}
