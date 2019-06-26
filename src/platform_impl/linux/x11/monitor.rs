use std::os::raw::*;

use parking_lot::Mutex;

use super::{
    ffi::{
        RRCrtc, RRCrtcChangeNotifyMask, RRMode, RROutputPropertyNotifyMask,
        RRScreenChangeNotifyMask, True, Window, XRRCrtcInfo, XRRScreenResources,
    },
    util, XConnection, XError,
};
use crate::{
    dpi::{PhysicalPosition, PhysicalSize},
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
    pub(crate) native_mode: RRMode,
    pub(crate) monitor: Option<MonitorHandle>,
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
    /// The actual id
    pub(crate) id: RRCrtc,
    /// The name of the monitor
    pub(crate) name: String,
    /// The size of the monitor
    dimensions: (u32, u32),
    /// The position of the monitor in the X screen
    position: (i32, i32),
    /// If the monitor is the primary one
    primary: bool,
    /// The DPI scale factor
    pub(crate) hidpi_factor: f64,
    /// Used to determine which windows are on this monitor
    pub(crate) rect: util::AaRect,
    /// Supported video modes on this monitor
    video_modes: Vec<VideoMode>,
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
    fn new(
        xconn: &XConnection,
        resources: *mut XRRScreenResources,
        id: RRCrtc,
        crtc: *mut XRRCrtcInfo,
        primary: bool,
    ) -> Option<Self> {
        let (name, hidpi_factor, video_modes) = unsafe { xconn.get_output_info(resources, crtc)? };
        let dimensions = unsafe { ((*crtc).width as u32, (*crtc).height as u32) };
        let position = unsafe { ((*crtc).x as i32, (*crtc).y as i32) };
        let rect = util::AaRect::new(position, dimensions);
        Some(MonitorHandle {
            id,
            name,
            hidpi_factor,
            dimensions,
            position,
            primary,
            rect,
            video_modes,
        })
    }

    pub fn name(&self) -> Option<String> {
        Some(self.name.clone())
    }

    #[inline]
    pub fn native_identifier(&self) -> u32 {
        self.id as u32
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
    pub fn get_monitor_for_window(&self, window_rect: Option<util::AaRect>) -> MonitorHandle {
        let monitors = self.available_monitors();
        let default = monitors
            .get(0)
            .expect("[winit] Failed to find any monitors using XRandR.");

        let window_rect = match window_rect {
            Some(rect) => rect,
            None => return default.to_owned(),
        };

        let mut largest_overlap = 0;
        let mut matched_monitor = default;
        for monitor in &monitors {
            let overlapping_area = window_rect.get_overlapping_area(&monitor.rect);
            if overlapping_area > largest_overlap {
                largest_overlap = overlapping_area;
                matched_monitor = &monitor;
            }
        }

        matched_monitor.to_owned()
    }

    fn query_monitor_list(&self) -> Vec<MonitorHandle> {
        unsafe {
            let mut major = 0;
            let mut minor = 0;
            (self.xrandr.XRRQueryVersion)(self.display, &mut major, &mut minor);

            let root = (self.xlib.XDefaultRootWindow)(self.display);
            let resources = if (major == 1 && minor >= 3) || major > 1 {
                (self.xrandr.XRRGetScreenResourcesCurrent)(self.display, root)
            } else {
                // WARNING: this function is supposedly very slow, on the order of hundreds of ms.
                // Upon failure, `resources` will be null.
                (self.xrandr.XRRGetScreenResources)(self.display, root)
            };

            if resources.is_null() {
                panic!("[winit] `XRRGetScreenResources` returned NULL. That should only happen if the root window doesn't exist.");
            }

            let mut available;
            let mut has_primary = false;

            let primary = (self.xrandr.XRRGetOutputPrimary)(self.display, root);
            available = Vec::with_capacity((*resources).ncrtc as usize);
            for crtc_index in 0..(*resources).ncrtc {
                let crtc_id = *((*resources).crtcs.offset(crtc_index as isize));
                let crtc = (self.xrandr.XRRGetCrtcInfo)(self.display, resources, crtc_id);
                let is_active = (*crtc).width > 0 && (*crtc).height > 0 && (*crtc).noutput > 0;
                if is_active {
                    let is_primary = *(*crtc).outputs.offset(0) == primary;
                    has_primary |= is_primary;
                    MonitorHandle::new(self, resources, crtc_id, crtc, is_primary)
                        .map(|monitor_id| available.push(monitor_id));
                }
                (self.xrandr.XRRFreeCrtcInfo)(crtc);
            }

            // If no monitors were detected as being primary, we just pick one ourselves!
            if !has_primary {
                if let Some(ref mut fallback) = available.first_mut() {
                    // Setting this here will come in handy if we ever add an `is_primary` method.
                    fallback.primary = true;
                }
            }

            (self.xrandr.XRRFreeScreenResources)(resources);
            available
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
            .expect("[winit] Failed to find any monitors using XRandR.")
    }

    pub fn select_xrandr_input(&self, root: Window) -> Result<c_int, XError> {
        let has_xrandr = unsafe {
            let mut major = 0;
            let mut minor = 0;
            (self.xrandr.XRRQueryVersion)(self.display, &mut major, &mut minor)
        };
        assert!(
            has_xrandr == True,
            "[winit] XRandR extension not available."
        );

        let mut event_offset = 0;
        let mut error_offset = 0;
        let status = unsafe {
            (self.xrandr.XRRQueryExtension)(self.display, &mut event_offset, &mut error_offset)
        };

        if status != True {
            self.check_errors()?;
            unreachable!("[winit] `XRRQueryExtension` failed but no error was received.");
        }

        let mask = RRCrtcChangeNotifyMask | RROutputPropertyNotifyMask | RRScreenChangeNotifyMask;
        unsafe { (self.xrandr.XRRSelectInput)(self.display, root, mask) };

        Ok(event_offset)
    }
}
