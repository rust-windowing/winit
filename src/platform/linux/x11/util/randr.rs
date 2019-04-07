use std::{env, slice};
use std::str::FromStr;

use validate_hidpi_factor;
use super::*;

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
                "`WINIT_HIDPI_FACTOR` invalid; DPI factors must be normal floats greater than 0. Got `{}`",
                dpi_override,
            );
        }
        return dpi_override;
    }

    // See http://xpra.org/trac/ticket/728 for more information.
    if width_mm == 0 || width_mm == 0 {
        warn!("XRandR reported that the display's 0mm in size, which is certifiably insane");
        return 1.0;
    }

    let ppmm = (
        (width_px as f64 * height_px as f64) / (width_mm as f64 * height_mm as f64)
    ).sqrt();
    // Quantize 1/12 step size
    let dpi_factor = ((ppmm * (12.0 * 25.4 / 96.0)).round() / 12.0).max(1.0);
    assert!(validate_hidpi_factor(dpi_factor));
    dpi_factor
}

pub enum MonitorRepr {
    Monitor(*mut ffi::XRRMonitorInfo),
    Crtc(*mut ffi::XRRCrtcInfo),
}

impl MonitorRepr {
    pub unsafe fn get_output(&self) -> ffi::RROutput {
        match *self {
            // Same member names, but different locations within the struct...
            MonitorRepr::Monitor(monitor) => *((*monitor).outputs.offset(0)),
            MonitorRepr::Crtc(crtc) => *((*crtc).outputs.offset(0)),
        }
    }

    pub unsafe fn get_dimensions(&self) -> (u32, u32) {
        match *self {
            MonitorRepr::Monitor(monitor) => ((*monitor).width as u32, (*monitor).height as u32),
            MonitorRepr::Crtc(crtc) => ((*crtc).width as u32, (*crtc).height as u32),
        }
    }

    pub unsafe fn get_position(&self) -> (i32, i32) {
        match *self {
            MonitorRepr::Monitor(monitor) => ((*monitor).x as i32, (*monitor).y as i32),
            MonitorRepr::Crtc(crtc) => ((*crtc).x as i32, (*crtc).y as i32),
        }
    }
}

impl From<*mut ffi::XRRMonitorInfo> for MonitorRepr {
    fn from(monitor: *mut ffi::XRRMonitorInfo) -> Self {
        MonitorRepr::Monitor(monitor)
    }
}

impl From<*mut ffi::XRRCrtcInfo> for MonitorRepr {
    fn from(crtc: *mut ffi::XRRCrtcInfo) -> Self {
        MonitorRepr::Crtc(crtc)
    }
}

impl XConnection {
    // Retrieve DPI from Xft.dpi property
    pub unsafe fn get_xft_dpi(&self) -> Option<f64> {
        (self.xlib.XrmInitialize)();
        let resource_manager_str = (self.xlib.XResourceManagerString)(self.display);
        if resource_manager_str == ptr::null_mut() {
            return None;
        }
        if let Ok(res) = ::std::ffi::CStr::from_ptr(resource_manager_str).to_str() {
            let name : &str = "Xft.dpi:\t";
            for pair in res.split("\n") {
                if pair.starts_with(&name) {
                    let res = &pair[name.len()..];
                    return f64::from_str(&res).ok();
                }
            }
        }
        None
    }
    pub unsafe fn get_output_info(
        &self,
        resources: *mut ffi::XRRScreenResources,
        repr: &MonitorRepr,
    ) -> Option<(String, f64)> {
        let output_info = (self.xrandr.XRRGetOutputInfo)(
            self.display,
            resources,
            repr.get_output(),
        );
        if output_info.is_null() {
            // When calling `XRRGetOutputInfo` on a virtual monitor (versus a physical display)
            // it's possible for it to return null.
            // https://bugs.debian.org/cgi-bin/bugreport.cgi?bug=816596
            let _ = self.check_errors(); // discard `BadRROutput` error
            return None;
        }
        let name_slice = slice::from_raw_parts(
            (*output_info).name as *mut u8,
            (*output_info).nameLen as usize,
        );
        let name = String::from_utf8_lossy(name_slice).into();
        let hidpi_factor = if let Some(dpi) = self.get_xft_dpi() {
            dpi / 96.
        } else {
            calc_dpi_factor(
                repr.get_dimensions(),
                ((*output_info).mm_width as u64, (*output_info).mm_height as u64),
            )
        };

        (self.xrandr.XRRFreeOutputInfo)(output_info);
        Some((name, hidpi_factor))
    }
}
