use std::{env, slice, str::FromStr};

use super::{
    ffi::{CurrentTime, RRCrtc, RRMode, Success, XRRCrtcInfo, XRRScreenResources},
    *,
};
use crate::platform_impl::platform::x11::monitor;
use crate::{dpi::validate_scale_factor, platform_impl::platform::x11::VideoMode};

/// Represents values of `WINIT_HIDPI_FACTOR`.
pub enum EnvVarDPI {
    Randr,
    Scale(f64),
    NotSet,
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
    assert!(validate_scale_factor(dpi_factor));
    if dpi_factor <= 20. {
        dpi_factor
    } else {
        1.
    }
}

impl XConnection {
    // Retrieve DPI from Xft.dpi property
    pub unsafe fn get_xft_dpi(&self) -> Option<f64> {
        (self.xlib.XrmInitialize)();
        let resource_manager_str = (self.xlib.XResourceManagerString)(self.display);
        if resource_manager_str.is_null() {
            return None;
        }
        if let Ok(res) = ::std::ffi::CStr::from_ptr(resource_manager_str).to_str() {
            let name: &str = "Xft.dpi:\t";
            for pair in res.split('\n') {
                if let Some(stripped) = pair.strip_prefix(name) {
                    return f64::from_str(stripped).ok();
                }
            }
        }
        None
    }
    pub unsafe fn get_output_info(
        &self,
        resources: *mut XRRScreenResources,
        crtc: *mut XRRCrtcInfo,
    ) -> Option<(String, f64, Vec<VideoMode>)> {
        let output_info =
            (self.xrandr.XRRGetOutputInfo)(self.display, resources, *(*crtc).outputs.offset(0));
        if output_info.is_null() {
            // When calling `XRRGetOutputInfo` on a virtual monitor (versus a physical display)
            // it's possible for it to return null.
            // https://bugs.debian.org/cgi-bin/bugreport.cgi?bug=816596
            let _ = self.check_errors(); // discard `BadRROutput` error
            return None;
        }

        let screen = (self.xlib.XDefaultScreen)(self.display);
        let bit_depth = (self.xlib.XDefaultDepth)(self.display, screen);

        let output_modes =
            slice::from_raw_parts((*output_info).modes, (*output_info).nmode as usize);
        let resource_modes = slice::from_raw_parts((*resources).modes, (*resources).nmode as usize);

        let modes = resource_modes
            .iter()
            // XRROutputInfo contains an array of mode ids that correspond to
            // modes in the array in XRRScreenResources
            .filter(|x| output_modes.iter().any(|id| x.id == *id))
            .map(|mode| {
                VideoMode {
                    size: (mode.width, mode.height),
                    refresh_rate_millihertz: monitor::mode_refresh_rate_millihertz(mode)
                        .unwrap_or(0),
                    bit_depth: bit_depth as u16,
                    native_mode: mode.id,
                    // This is populated in `MonitorHandle::video_modes` as the
                    // video mode is returned to the user
                    monitor: None,
                }
            })
            .collect();

        let name_slice = slice::from_raw_parts(
            (*output_info).name as *mut u8,
            (*output_info).nameLen as usize,
        );
        let name = String::from_utf8_lossy(name_slice).into();
        // Override DPI if `WINIT_X11_SCALE_FACTOR` variable is set
        let deprecated_dpi_override = env::var("WINIT_HIDPI_FACTOR").ok();
        if deprecated_dpi_override.is_some() {
            warn!(
	            "The WINIT_HIDPI_FACTOR environment variable is deprecated; use WINIT_X11_SCALE_FACTOR"
	        )
        }
        let dpi_env = env::var("WINIT_X11_SCALE_FACTOR").ok().map_or_else(
            || EnvVarDPI::NotSet,
            |var| {
                if var.to_lowercase() == "randr" {
                    EnvVarDPI::Randr
                } else if let Ok(dpi) = f64::from_str(&var) {
                    EnvVarDPI::Scale(dpi)
                } else if var.is_empty() {
                    EnvVarDPI::NotSet
                } else {
                    panic!(
                        "`WINIT_X11_SCALE_FACTOR` invalid; DPI factors must be either normal floats greater than 0, or `randr`. Got `{var}`"
                    );
                }
            },
        );

        let scale_factor = match dpi_env {
            EnvVarDPI::Randr => calc_dpi_factor(
                ((*crtc).width, (*crtc).height),
                ((*output_info).mm_width as _, (*output_info).mm_height as _),
            ),
            EnvVarDPI::Scale(dpi_override) => {
                if !validate_scale_factor(dpi_override) {
                    panic!(
                        "`WINIT_X11_SCALE_FACTOR` invalid; DPI factors must be either normal floats greater than 0, or `randr`. Got `{dpi_override}`",
                    );
                }
                dpi_override
            }
            EnvVarDPI::NotSet => {
                if let Some(dpi) = self.get_xft_dpi() {
                    dpi / 96.
                } else {
                    calc_dpi_factor(
                        ((*crtc).width, (*crtc).height),
                        ((*output_info).mm_width as _, (*output_info).mm_height as _),
                    )
                }
            }
        };

        (self.xrandr.XRRFreeOutputInfo)(output_info);
        Some((name, scale_factor, modes))
    }

    #[must_use]
    pub fn set_crtc_config(&self, crtc_id: RRCrtc, mode_id: RRMode) -> Option<()> {
        unsafe {
            let mut major = 0;
            let mut minor = 0;
            (self.xrandr.XRRQueryVersion)(self.display, &mut major, &mut minor);

            let root = (self.xlib.XDefaultRootWindow)(self.display);
            let resources = if (major == 1 && minor >= 3) || major > 1 {
                (self.xrandr.XRRGetScreenResourcesCurrent)(self.display, root)
            } else {
                (self.xrandr.XRRGetScreenResources)(self.display, root)
            };

            let crtc = (self.xrandr.XRRGetCrtcInfo)(self.display, resources, crtc_id);
            let status = (self.xrandr.XRRSetCrtcConfig)(
                self.display,
                resources,
                crtc_id,
                CurrentTime,
                (*crtc).x,
                (*crtc).y,
                mode_id,
                (*crtc).rotation,
                (*crtc).outputs.offset(0),
                1,
            );

            (self.xrandr.XRRFreeCrtcInfo)(crtc);
            (self.xrandr.XRRFreeScreenResources)(resources);

            if status == Success as i32 {
                Some(())
            } else {
                None
            }
        }
    }

    pub fn get_crtc_mode(&self, crtc_id: RRCrtc) -> RRMode {
        unsafe {
            let mut major = 0;
            let mut minor = 0;
            (self.xrandr.XRRQueryVersion)(self.display, &mut major, &mut minor);

            let root = (self.xlib.XDefaultRootWindow)(self.display);
            let resources = if (major == 1 && minor >= 3) || major > 1 {
                (self.xrandr.XRRGetScreenResourcesCurrent)(self.display, root)
            } else {
                (self.xrandr.XRRGetScreenResources)(self.display, root)
            };

            let crtc = (self.xrandr.XRRGetCrtcInfo)(self.display, resources, crtc_id);
            let mode = (*crtc).mode;
            (self.xrandr.XRRFreeCrtcInfo)(crtc);
            (self.xrandr.XRRFreeScreenResources)(resources);
            mode
        }
    }
}
