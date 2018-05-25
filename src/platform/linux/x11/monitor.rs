use std::os::raw::*;
use std::sync::Arc;

use parking_lot::Mutex;

use super::ffi::{
    RRCrtcChangeNotifyMask,
    RROutputPropertyNotifyMask,
    RRScreenChangeNotifyMask,
    True,
    Window,
    XRRScreenResources,
};
use super::{util, XConnection, XError};

// Used to test XRandR < 1.5 code path. This should always be committed as false.
const FORCE_RANDR_COMPAT: bool = false;
// Also used for testing. This should always be committed as false.
const DISABLE_MONITOR_LIST_CACHING: bool = false;

lazy_static! {
    static ref XRANDR_VERSION: Mutex<Option<(c_int, c_int)>> = Mutex::default();
    static ref MONITORS: Mutex<Option<Vec<MonitorId>>> = Mutex::default();
}

fn version_is_at_least(major: c_int, minor: c_int) -> bool {
    if let Some((avail_major, avail_minor)) = *XRANDR_VERSION.lock() {
        if avail_major == major {
            avail_minor >= minor
        } else {
            avail_major > major
        }
    } else {
        unreachable!();
    }
}

pub fn invalidate_cached_monitor_list() -> Option<Vec<MonitorId>> {
    // We update this lazily.
    (*MONITORS.lock()).take()
}

#[derive(Debug, Clone)]
pub struct MonitorId {
    /// The actual id
    id: u32,
    /// The name of the monitor
    name: String,
    /// The size of the monitor
    dimensions: (u32, u32),
    /// The position of the monitor in the X screen
    position: (i32, i32),
    /// If the monitor is the primary one
    primary: bool,
    /// The DPI scale factor
    pub(crate) hidpi_factor: f32,
    /// Used to determine which windows are on this monitor
    pub(crate) rect: util::Rect,
}

impl MonitorId {
    fn from_repr(
        xconn: &Arc<XConnection>,
        resources: *mut XRRScreenResources,
        id: u32,
        repr: util::MonitorRepr,
        primary: bool,
    ) -> Self {
        let (name, hidpi_factor) = unsafe { xconn.get_output_info(resources, &repr) };
        let (dimensions, position) = unsafe { (repr.get_dimensions(), repr.get_position()) };
        let rect = util::Rect::new(position, dimensions);
        MonitorId {
            id,
            name,
            hidpi_factor,
            dimensions,
            position,
            primary,
            rect,
        }
    }

    pub fn get_name(&self) -> Option<String> {
        Some(self.name.clone())
    }

    #[inline]
    pub fn get_native_identifier(&self) -> u32 {
        self.id as u32
    }

    pub fn get_dimensions(&self) -> (u32, u32) {
        self.dimensions
    }

    pub fn get_position(&self) -> (i32, i32) {
        self.position
    }

    #[inline]
    pub fn get_hidpi_factor(&self) -> f32 {
        self.hidpi_factor
    }
}

pub fn get_monitor_for_window(
    xconn: &Arc<XConnection>,
    window_rect: Option<util::Rect>,
) -> MonitorId {
    let monitors = get_available_monitors(xconn);
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

fn query_monitor_list(xconn: &Arc<XConnection>) -> Vec<MonitorId> {
    unsafe {
        let root = (xconn.xlib.XDefaultRootWindow)(xconn.display);
        // WARNING: this function is supposedly very slow, on the order of hundreds of ms.
        // Upon failure, `resources` will be null.
        let resources = (xconn.xrandr.XRRGetScreenResources)(xconn.display, root);
        if resources.is_null() {
            panic!("[winit] `XRRGetScreenResources` returned NULL. That should only happen if the root window doesn't exist.");
        }

        let mut available;
        let mut has_primary = false;

        if xconn.xrandr_1_5.is_some() && version_is_at_least(1, 5) && !FORCE_RANDR_COMPAT {
            // We're in XRandR >= 1.5, enumerate monitors. This supports things like MST and
            // videowalls.
            let xrandr_1_5 = xconn.xrandr_1_5.as_ref().unwrap();
            let mut monitor_count = 0;
            let monitors = (xrandr_1_5.XRRGetMonitors)(xconn.display, root, 1, &mut monitor_count);
            assert!(monitor_count >= 0);
            available = Vec::with_capacity(monitor_count as usize);
            for monitor_index in 0..monitor_count {
                let monitor = monitors.offset(monitor_index as isize);
                let is_primary = (*monitor).primary != 0;
                has_primary |= is_primary;
                available.push(MonitorId::from_repr(
                    xconn,
                    resources,
                    monitor_index as u32,
                    monitor.into(),
                    is_primary,
                ));
            }
            (xrandr_1_5.XRRFreeMonitors)(monitors);
        } else {
            // We're in XRandR < 1.5, enumerate CRTCs. Everything will work except MST and
            // videowall setups will also show monitors that aren't in the logical groups the user
            // cares about.
            let primary = (xconn.xrandr.XRRGetOutputPrimary)(xconn.display, root);
            available = Vec::with_capacity((*resources).ncrtc as usize);
            for crtc_index in 0..(*resources).ncrtc {
                let crtc_id = *((*resources).crtcs.offset(crtc_index as isize));
                let crtc = (xconn.xrandr.XRRGetCrtcInfo)(xconn.display, resources, crtc_id);
                let is_active = (*crtc).width > 0 && (*crtc).height > 0 && (*crtc).noutput > 0;
                if is_active {
                    let crtc = util::MonitorRepr::from(crtc);
                    let is_primary = crtc.get_output() == primary;
                    has_primary |= is_primary;
                    available.push(MonitorId::from_repr(
                        xconn,
                        resources,
                        crtc_id as u32,
                        crtc,
                        is_primary,
                    ));
                }
                (xconn.xrandr.XRRFreeCrtcInfo)(crtc);
            }
        }

        // If no monitors were detected as being primary, we just pick one ourselves!
        if !has_primary {
            if let Some(ref mut fallback) = available.first_mut() {
                // Setting this here will come in handy if we ever add an `is_primary` method.
                fallback.primary = true;
            }
        }

        (xconn.xrandr.XRRFreeScreenResources)(resources);
        available
    }
}

pub fn get_available_monitors(xconn: &Arc<XConnection>) -> Vec<MonitorId> {
    let mut monitors_lock = MONITORS.lock();
    (*monitors_lock)
        .as_ref()
        .cloned()
        .or_else(|| {
            let monitors = Some(query_monitor_list(xconn));
            if !DISABLE_MONITOR_LIST_CACHING {
                (*monitors_lock) = monitors.clone();
            }
            monitors
        })
        .unwrap()
}

#[inline]
pub fn get_primary_monitor(xconn: &Arc<XConnection>) -> MonitorId {
    get_available_monitors(xconn)
        .into_iter()
        .find(|monitor| monitor.primary)
        .expect("[winit] Failed to find any monitors using XRandR.")
}

pub fn select_input(xconn: &Arc<XConnection>, root: Window) -> Result<c_int, XError> {
    {
        let mut version_lock = XRANDR_VERSION.lock();
        if version_lock.is_none() {
            let mut major = 0;
            let mut minor = 0;
            let has_extension = unsafe {
                (xconn.xrandr.XRRQueryVersion)(
                    xconn.display,
                    &mut major,
                    &mut minor,
                )
            };
            if has_extension != True {
                panic!("[winit] XRandR extension not available.");
            }
            *version_lock = Some((major, minor));
        }
    }

    let mut event_offset = 0;
    let mut error_offset = 0;
    let status = unsafe {
        (xconn.xrandr.XRRQueryExtension)(
            xconn.display,
            &mut event_offset,
            &mut error_offset,
        )
    };

    if status != True {
        xconn.check_errors()?;
        unreachable!("[winit] `XRRQueryExtension` failed but no error was received.");
    }

    let mask = RRCrtcChangeNotifyMask
        | RROutputPropertyNotifyMask
        | RRScreenChangeNotifyMask;
    unsafe { (xconn.xrandr.XRRSelectInput)(xconn.display, root, mask) };

    Ok(event_offset)
}
