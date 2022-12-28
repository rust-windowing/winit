use std::sync::Mutex;

use once_cell::sync::Lazy;

use super::{util, PlatformError, XConnection};
use crate::{
    dpi::{PhysicalPosition, PhysicalSize},
    platform_impl::{MonitorHandle as PlatformMonitorHandle, VideoMode as PlatformVideoMode},
};

use x11rb::protocol::randr::{self, ConnectionExt as _};

// Used for testing. This should always be committed as false.
const DISABLE_MONITOR_LIST_CACHING: bool = false;

static MONITORS: Lazy<Mutex<Option<Vec<MonitorHandle>>>> = Lazy::new(Mutex::default);

pub fn invalidate_cached_monitor_list() -> Option<Vec<MonitorHandle>> {
    // We update this lazily.
    (*MONITORS.lock().unwrap()).take()
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VideoMode {
    pub(crate) size: (u32, u32),
    pub(crate) bit_depth: u16,
    pub(crate) refresh_rate_millihertz: u32,
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
    pub fn refresh_rate_millihertz(&self) -> u32 {
        self.refresh_rate_millihertz
    }

    #[inline]
    pub fn monitor(&self) -> PlatformMonitorHandle {
        PlatformMonitorHandle::X(self.monitor.clone().unwrap())
    }
}

#[derive(Debug, Clone)]
pub struct MonitorHandle {
    /// The actual id
    pub(crate) id: u32,
    /// The name of the monitor
    pub(crate) name: String,
    /// The size of the monitor
    dimensions: (u32, u32),
    /// The position of the monitor in the X screen
    position: (i32, i32),
    /// If the monitor is the primary one
    primary: bool,
    /// The refresh rate used by monitor.
    refresh_rate_millihertz: Option<u32>,
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
        Some(self.cmp(other))
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

#[inline]
pub fn mode_refresh_rate_millihertz(mode: &randr::ModeInfo) -> Option<u32> {
    if mode.dot_clock > 0 && mode.htotal > 0 && mode.vtotal > 0 {
        #[allow(clippy::unnecessary_cast)]
        Some((mode.dot_clock as u64 * 1000 / (mode.htotal as u64 * mode.vtotal as u64)) as u32)
    } else {
        None
    }
}

impl MonitorHandle {
    fn new(
        xconn: &XConnection,
        resource_modes: &[randr::ModeInfo],
        crtc_id: u32,
        crtc_info: &randr::GetCrtcInfoReply,
        primary: bool,
    ) -> Result<Self, PlatformError> {
        let (name, scale_factor, video_modes) = {
            let output_info = xconn
                .connection
                .randr_get_output_info(crtc_info.outputs[0], crtc_info.timestamp)?
                .reply()?;

            // Get the scale factor for the monitor.
            let scale_factor = xconn.dpi_for_monitor(crtc_info, &output_info);

            let randr::GetOutputInfoReply { name, modes, .. } = output_info;

            // Parse the modes of the monitor.
            let modes = {
                let depth = xconn.default_screen().root_depth;

                resource_modes
                    .iter()
                    .filter(|mode| modes.contains(&mode.id))
                    .map(|mode| {
                        VideoMode {
                            size: (mode.width as _, mode.height as _),
                            refresh_rate_millihertz: mode_refresh_rate_millihertz(mode)
                                .unwrap_or(0),
                            bit_depth: depth.into(),
                            // This is populated in `MonitorHandle::video_modes` as the
                            // video mode is returned to the user
                            native_mode: mode.id as _,
                            monitor: None,
                        }
                    })
                    .collect::<Vec<_>>()
            };

            (
                String::from_utf8_lossy(&name).into_owned(),
                scale_factor,
                modes,
            )
        };

        let dimensions = ((crtc_info).width as u32, (crtc_info).height as u32);
        let position = ((crtc_info).x as i32, (crtc_info).y as i32);

        // Get the refresh rate of the current video mode.
        let refresh_rate_millihertz = {
            let current_mode = (crtc_info).mode;

            resource_modes
                .iter()
                .find(|mode| mode.id == current_mode)
                .and_then(mode_refresh_rate_millihertz)
        };

        let rect = util::AaRect::new(position, dimensions);

        Ok(MonitorHandle {
            id: crtc_id,
            name,
            refresh_rate_millihertz,
            scale_factor,
            dimensions,
            position,
            primary,
            rect,
            video_modes,
        })
    }

    pub fn dummy() -> Self {
        MonitorHandle {
            id: 0,
            name: "<dummy monitor>".into(),
            scale_factor: 1.0,
            dimensions: (1, 1),
            position: (0, 0),
            refresh_rate_millihertz: None,
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
        self.id as _
    }

    pub fn size(&self) -> PhysicalSize<u32> {
        self.dimensions.into()
    }

    pub fn position(&self) -> PhysicalPosition<i32> {
        self.position.into()
    }

    pub fn refresh_rate_millihertz(&self) -> Option<u32> {
        self.refresh_rate_millihertz
    }

    #[inline]
    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    #[inline]
    pub fn video_modes(&self) -> impl Iterator<Item = PlatformVideoMode> {
        let monitor = self.clone();
        self.video_modes.clone().into_iter().map(move |mut x| {
            x.monitor = Some(monitor.clone());
            PlatformVideoMode::X(x)
        })
    }
}

impl XConnection {
    pub fn get_monitor_for_window(&self, window_rect: Option<util::AaRect>) -> MonitorHandle {
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
            let overlapping_area = window_rect.get_overlapping_area(&monitor.rect);
            if overlapping_area > largest_overlap {
                largest_overlap = overlapping_area;
                matched_monitor = monitor;
            }
        }

        matched_monitor.to_owned()
    }

    fn query_monitor_list(&self) -> Result<Vec<MonitorHandle>, PlatformError> {
        let (major, minor) = {
            let extension_info = self.connection.randr_query_version(0, 0)?.reply()?;
            (extension_info.major_version, extension_info.minor_version)
        };
        let root = self.default_screen().root;

        // Start fetching the primary monitor as we fetch the monitor list.
        let primary_token = self.connection.randr_get_output_primary(root)?;

        let (crtc_ids, resource_modes) = {
            if (major == 1 && minor >= 3) || major > 1 {
                let reply = self
                    .connection
                    .randr_get_screen_resources_current(root)?
                    .reply()?;

                let randr::GetScreenResourcesCurrentReply { crtcs, modes, .. } = reply;
                (crtcs, modes)
            } else {
                // WARNING: this function is supposedly very slow, on the order of hundreds of ms.
                // Upon failure, `resources` will be null.
                let reply = self.connection.randr_get_screen_resources(root)?.reply()?;

                let randr::GetScreenResourcesReply { crtcs, modes, .. } = reply;
                (crtcs, modes)
            }
        };

        // Get the CRTC information for each CRTC.
        let crtc_info_tokens = crtc_ids
            .into_iter()
            .map(|crtc| {
                self.connection
                    .randr_get_crtc_info(crtc, 0)
                    .map(move |token| (crtc, token))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut has_primary = false;

        let primary = primary_token.reply()?.output;

        // Get the monitor information for each CRTC.
        let mut available = crtc_info_tokens
            .into_iter()
            .filter_map(|(crtc_id, cookie)| {
                let result = cookie.reply();

                result
                    .map(|crtc_info| {
                        let is_active = crtc_info.width > 0
                            && crtc_info.height > 0
                            && !crtc_info.outputs.is_empty();
                        if is_active {
                            let is_primary = crtc_info.outputs[0] == primary;
                            has_primary |= is_primary;
                            if let Ok(monitor_id) = MonitorHandle::new(
                                self,
                                &resource_modes,
                                crtc_id,
                                &crtc_info,
                                is_primary,
                            ) {
                                return Some(monitor_id);
                            }
                        }

                        None
                    })
                    .transpose()
            })
            .collect::<Result<Vec<_>, _>>()?;

        // If no monitors were detected as being primary, we just pick one ourselves!
        if !has_primary {
            if let Some(ref mut fallback) = available.first_mut() {
                // Setting this here will come in handy if we ever add an `is_primary` method.
                fallback.primary = true;
            }
        }

        Ok(available)
    }

    pub fn available_monitors(&self) -> Vec<MonitorHandle> {
        let mut monitors_lock = MONITORS.lock().unwrap();
        (*monitors_lock)
            .as_ref()
            .cloned()
            .or_else(|| {
                let monitors = Some(
                    self.query_monitor_list()
                        .expect("Failed to load monitors list"),
                );
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

    /// Get the DPI factor for a monitor, considering the environment.
    fn dpi_for_monitor(
        &self,
        crtc: &randr::GetCrtcInfoReply,
        monitor: &randr::GetOutputInfoReply,
    ) -> f64 {
        /// Represents values of `WINIT_HIDPI_FACTOR`.
        enum EnvVarDPI {
            Randr,
            Scale(f64),
            NotSet,
        }

        // Check the environment variable first.
        let dpi_env = std::env::var("WINIT_X11_SCALE_FACTOR").ok().map_or_else(
            || EnvVarDPI::NotSet,
            |var| {
                if var.to_lowercase() == "randr" {
                    EnvVarDPI::Randr
                } else if let Ok(dpi) = var.parse::<f64>() {
                    EnvVarDPI::Scale(dpi)
                } else if var.is_empty() {
                    EnvVarDPI::NotSet
                } else {
                    panic!(
                        "`WINIT_X11_SCALE_FACTOR` invalid; DPI factors must be either normal floats greater than 0, or `randr`. Got `{}`",
                        var
                    );
                }
            },
        );

        // Determine the scale factor.
        match dpi_env {
            EnvVarDPI::Randr => raw_dpi_for_monitor(crtc, monitor),
            EnvVarDPI::Scale(dpi_override) => {
                if !crate::dpi::validate_scale_factor(dpi_override) {
                    panic!(
                        "`WINIT_X11_SCALE_FACTOR` invalid; DPI factors must be either normal floats greater than 0, or `randr`. Got `{}`",
                        dpi_override,
                    );
                }
                dpi_override
            }
            EnvVarDPI::NotSet => {
                if let Some(dpi) = self.xft_dpi() {
                    dpi / 96.
                } else {
                    raw_dpi_for_monitor(crtc, monitor)
                }
            }
        }
    }

    /// Get the DPI property from `Xft.dpi`.
    pub fn xft_dpi(&self) -> Option<f64> {
        self.database.get_value("Xft.dpi", "").ok().flatten()
    }
}

/// Get the raw DPI factor for a monitor.
fn raw_dpi_for_monitor(crtc: &randr::GetCrtcInfoReply, monitor: &randr::GetOutputInfoReply) -> f64 {
    calc_dpi_factor(
        (crtc.width as _, crtc.height as _),
        (monitor.mm_width as _, monitor.mm_height as _),
    )
}

pub fn calc_dpi_factor(
    (width_px, height_px): (u32, u32),
    (width_mm, height_mm): (u64, u64),
) -> f64 {
    // See http://xpra.org/trac/ticket/728 for more information.
    if width_mm == 0 || height_mm == 0 {
        warn!("XRandR reported that the display's 0mm in size, which is certifiably insane");
        return 1.0;
    }

    let ppmm = ((width_px as f64 * height_px as f64) / (width_mm as f64 * height_mm as f64)).sqrt();
    // Quantize 1/12 step size
    let dpi_factor = ((ppmm * (12.0 * 25.4 / 96.0)).round() / 12.0).max(1.0);
    assert!(crate::dpi::validate_scale_factor(dpi_factor));
    if dpi_factor <= 20. {
        dpi_factor
    } else {
        1.
    }
}
