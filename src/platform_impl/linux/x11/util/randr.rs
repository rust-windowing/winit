use std::{env, slice, str, str::FromStr};

use super::*;
use crate::platform_impl::x11::xdisplay::Screen;
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
    dpi_factor
}

impl XConnection {
    // Retrieve DPI from Xft.dpi property
    pub unsafe fn get_xft_dpi(&self, screen: &Screen) -> Option<f64> {
        let resource_manager_str = self
            .get_property::<u8>(
                screen.root,
                ffi::XCB_ATOM_RESOURCE_MANAGER,
                ffi::XCB_ATOM_STRING,
            )
            .ok()?;
        for pair in resource_manager_str.split(|b| *b == b'\n') {
            if let Some(res) = pair.strip_prefix(b"Xft.dpi:\t") {
                return f64::from_str(str::from_utf8(res).ok()?).ok();
            }
        }
        None
    }
    pub unsafe fn get_output_info(
        &self,
        screen: &Screen,
        resources: &ffi::xcb_randr_get_screen_resources_reply_t,
        crtc: &ffi::xcb_randr_get_crtc_info_reply_t,
    ) -> Result<Option<(String, f64, Vec<VideoMode>)>, XcbError> {
        let mut err = ptr::null_mut();

        let first_output = {
            if crtc.num_outputs == 0 {
                return Ok(None);
            }
            *self.randr.xcb_randr_get_crtc_info_outputs(crtc)
        };
        let output_info = {
            let reply = self.randr.xcb_randr_get_output_info_reply(
                self.c,
                self.randr
                    .xcb_randr_get_output_info(self.c, first_output, 0),
                &mut err,
            );
            self.check(reply, err)?
        };
        let output_modes = slice::from_raw_parts(
            self.randr.xcb_randr_get_output_info_modes(&*output_info),
            output_info.num_modes as _,
        );

        let resource_modes = slice::from_raw_parts(
            self.randr.xcb_randr_get_screen_resources_modes(resources),
            resources.num_modes as _,
        );

        let modes = resource_modes
            .iter()
            // XRROutputInfo contains an array of mode ids that correspond to
            // modes in the array in XRRScreenResources
            .filter(|x| output_modes.iter().any(|id| x.id == *id))
            .map(|x| {
                let refresh_rate = if x.dot_clock > 0 && x.htotal > 0 && x.vtotal > 0 {
                    x.dot_clock as u64 * 1000 / (x.htotal as u64 * x.vtotal as u64)
                } else {
                    0
                };

                VideoMode {
                    size: (x.width, x.height),
                    refresh_rate: (refresh_rate as f32 / 1000.0).round() as u16,
                    bit_depth: screen.root_depth as u16,
                    native_mode: x.id,
                    // This is populated in `MonitorHandle::video_modes` as the
                    // video mode is returned to the user
                    monitor: None,
                }
            })
            .collect();

        let name_slice = slice::from_raw_parts(
            self.randr.xcb_randr_get_output_info_name(&*output_info),
            output_info.name_len as usize,
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
                        "`WINIT_X11_SCALE_FACTOR` invalid; DPI factors must be either normal floats greater than 0, or `randr`. Got `{}`",
                        var
                    );
                }
            },
        );

        let scale_factor = match dpi_env {
            EnvVarDPI::Randr => calc_dpi_factor(
                (crtc.width as u32, crtc.height as u32),
                (output_info.mm_width as u64, output_info.mm_height as u64),
            ),
            EnvVarDPI::Scale(dpi_override) => {
                if !validate_scale_factor(dpi_override) {
                    panic!(
                        "`WINIT_X11_SCALE_FACTOR` invalid; DPI factors must be either normal floats greater than 0, or `randr`. Got `{}`",
                        dpi_override,
                    );
                }
                dpi_override
            }
            EnvVarDPI::NotSet => {
                if let Some(dpi) = self.get_xft_dpi(screen) {
                    dpi / 96.
                } else {
                    calc_dpi_factor(
                        (crtc.width as u32, crtc.height as u32),
                        (output_info.mm_width as u64, output_info.mm_height as u64),
                    )
                }
            }
        };

        Ok(Some((name, scale_factor, modes)))
    }
    pub fn set_crtc_config(
        &self,
        crtc_id: ffi::xcb_randr_crtc_t,
        mode_id: ffi::xcb_randr_mode_t,
    ) -> Result<(), XcbError> {
        let info = self.get_crtc_info(crtc_id)?;
        unsafe {
            let cookie = self.randr.xcb_randr_set_crtc_config(
                self.c,
                crtc_id,
                ffi::XCB_TIME_CURRENT_TIME,
                // See the comment in get_crtc_info.
                0,
                info.x,
                info.y,
                mode_id,
                info.rotation,
                info.num_outputs as _,
                self.randr.xcb_randr_get_crtc_info_outputs(&*info),
            );
            let mut err = ptr::null_mut();
            let reply = self
                .randr
                .xcb_randr_set_crtc_config_reply(self.c, cookie, &mut err);
            self.check(reply, err).map(|_| ())
        }
    }

    pub fn get_crtc_mode(
        &self,
        crtc_id: ffi::xcb_randr_crtc_t,
    ) -> Result<ffi::xcb_randr_mode_t, XcbError> {
        self.get_crtc_info(crtc_id).map(|i| i.mode)
    }

    pub fn get_crtc_info(
        &self,
        crtc_id: ffi::xcb_randr_crtc_t,
    ) -> Result<XcbBox<ffi::xcb_randr_get_crtc_info_reply_t>, XcbError> {
        unsafe {
            let mut err = ptr::null_mut();
            let reply = self.randr.xcb_randr_get_crtc_info_reply(
                self.c,
                // NOTE: The randr spec says that the last argument must be equal to the current
                // timestamp on the server but this has never been enforced by the x server.
                // Therefore we can save us a call to retrieve that value.
                self.randr.xcb_randr_get_crtc_info(self.c, crtc_id, 0),
                &mut err,
            );
            self.check(reply, err)
        }
    }
}
