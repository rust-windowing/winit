use super::{ffi, util, XConnection};
use crate::platform_impl::x11::xdisplay::Screen;
use crate::{
    dpi::{PhysicalPosition, PhysicalSize},
    monitor::{MonitorHandle as RootMonitorHandle, VideoMode as RootVideoMode},
    platform_impl::{MonitorHandle as PlatformMonitorHandle, VideoMode as PlatformVideoMode},
};
use std::ptr;
use std::sync::Arc;
use xcb_dl_util::error::XcbError;
use xcb_dl_util::xcb_box::XcbBox;

// Used for testing. This should always be committed as false.
const DISABLE_MONITOR_LIST_CACHING: bool = false;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VideoMode {
    pub(crate) size: (u16, u16),
    pub(crate) bit_depth: u16,
    pub(crate) refresh_rate: u16,
    pub(crate) native_mode: u32,
    pub(crate) monitor: Option<MonitorHandle>,
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
    /// The actual id
    pub(crate) id: ffi::xcb_randr_crtc_t,
    /// The name of the monitor
    pub(crate) name: String,
    /// The screen this monitor is attached to
    pub(crate) screen: Option<Arc<Screen>>,
    /// The size of the monitor
    dimensions: (u32, u32),
    /// The position of the monitor in the X screen
    position: (i32, i32),
    /// If the monitor is the primary one
    primary: bool,
    /// The DPI scale factor
    pub(crate) scale_factor: f64,
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
        screen: &Arc<Screen>,
        resources: &ffi::xcb_randr_get_screen_resources_reply_t,
        id: ffi::xcb_randr_crtc_t,
        crtc: &ffi::xcb_randr_get_crtc_info_reply_t,
        primary: bool,
    ) -> Result<Option<Self>, XcbError> {
        let output_info = unsafe { xconn.get_output_info(screen, resources, crtc)? };
        let (name, scale_factor, video_modes) = match output_info {
            Some(o) => o,
            _ => return Ok(None),
        };
        let dimensions = (crtc.width as u32, crtc.height as u32);
        let position = (crtc.x as i32, crtc.y as i32);
        let rect = util::AaRect::new(position, dimensions);
        Ok(Some(MonitorHandle {
            id,
            name,
            screen: Some(screen.clone()),
            scale_factor,
            dimensions,
            position,
            primary,
            rect,
            video_modes,
        }))
    }

    pub fn dummy() -> Self {
        MonitorHandle {
            id: 0,
            name: "<dummy monitor>".into(),
            screen: None,
            scale_factor: 1.0,
            dimensions: (1, 1),
            position: (0, 0),
            primary: true,
            rect: util::AaRect::new((0, 0), (1, 1)),
            video_modes: Vec::new(),
        }
    }

    pub(crate) fn is_dummy(&self) -> bool {
        // Zero is an invalid XID value; no real monitor will have it
        self.id == 0
    }

    pub fn name(&self) -> Option<String> {
        Some(self.name.clone())
    }

    #[inline]
    pub fn native_identifier(&self) -> u32 {
        self.id as u32
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
    pub fn get_monitor_for_window(&self, window_rect: Option<util::AaRect>) -> MonitorHandle {
        let monitors = match self.available_monitors_inner() {
            Ok(m) => m,
            Err(e) => {
                log::error!("Could not retrieve monitors: {}", e);
                return MonitorHandle::dummy();
            }
        };

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
            let overlapping_area = window_rect.get_overlapping_area(&monitor.rect);
            if overlapping_area > largest_overlap {
                largest_overlap = overlapping_area;
                matched_monitor = &monitor;
            }
        }

        matched_monitor.to_owned()
    }

    unsafe fn screen_resources(
        &self,
        screen: &Screen,
    ) -> Result<XcbBox<ffi::xcb_randr_get_screen_resources_reply_t>, XcbError> {
        let cookie = if self.randr_version >= (1, 3) {
            self.randr
                .xcb_randr_get_screen_resources_current(self.c, screen.root)
                .sequence
        } else {
            self.randr
                .xcb_randr_get_screen_resources(self.c, screen.root)
                .sequence
        };
        let mut err = ptr::null_mut();
        let reply = self.xcb.xcb_wait_for_reply(self.c, cookie, &mut err) as *mut _;
        self.check(reply, err)
    }

    fn query_monitor_list(&self, screen: &Arc<Screen>) -> Result<Vec<MonitorHandle>, XcbError> {
        unsafe {
            let resources = self.screen_resources(screen)?;
            let crtcs = std::slice::from_raw_parts(
                self.randr.xcb_randr_get_screen_resources_crtcs(&*resources),
                resources.num_crtcs as _,
            );

            let mut has_primary = false;

            let mut err = ptr::null_mut();
            let primary = self.randr.xcb_randr_get_output_primary_reply(
                self.c,
                self.randr.xcb_randr_get_output_primary(self.c, screen.root),
                &mut err,
            );
            let primary = self.check(primary, err)?.output;
            let mut available = Vec::with_capacity(crtcs.len());
            for &crtc_id in crtcs {
                let crtc = self.get_crtc_info(crtc_id)?;
                let is_active = crtc.width > 0 && crtc.height > 0 && crtc.num_outputs > 0;
                if is_active {
                    let is_primary = *self.randr.xcb_randr_get_crtc_info_outputs(&*crtc) == primary;
                    has_primary |= is_primary;
                    MonitorHandle::new(self, screen, &resources, crtc_id, &crtc, is_primary)?
                        .map(|monitor_id| available.push(monitor_id));
                }
            }

            // If no monitors were detected as being primary, we just pick one ourselves!
            if !has_primary {
                if let Some(ref mut fallback) = available.first_mut() {
                    // Setting this here will come in handy if we ever add an `is_primary` method.
                    fallback.primary = true;
                }
            }

            Ok(available)
        }
    }

    pub fn available_monitors(&self) -> Vec<MonitorHandle> {
        match self.available_monitors_inner() {
            Ok(m) => m,
            Err(e) => {
                log::error!("Could not retrive monitors: {}", e);
                vec![]
            }
        }
    }

    pub fn available_monitors_inner(&self) -> Result<Vec<MonitorHandle>, XcbError> {
        let mut monitors_lock = self.monitors.lock();
        if let Some(monitors) = &*monitors_lock {
            return Ok(monitors.clone());
        }
        let mut monitors = vec![];
        for screen in &self.screens {
            monitors.extend(self.query_monitor_list(screen)?);
        }
        if !DISABLE_MONITOR_LIST_CACHING {
            (*monitors_lock) = Some(monitors.clone());
        }
        Ok(monitors)
    }

    #[inline]
    pub fn primary_monitor(&self) -> MonitorHandle {
        match self.available_monitors_inner() {
            Ok(m) => m
                .into_iter()
                .find(|monitor| monitor.primary)
                .unwrap_or_else(MonitorHandle::dummy),
            _ => MonitorHandle::dummy(),
        }
    }

    pub fn invalidate_cached_monitor_list(&self) -> Option<Vec<MonitorHandle>> {
        // We update this lazily.
        (*self.monitors.lock()).take()
    }
}
