use std::{env, slice};
use std::str::FromStr;

use super::*;
use super::ffi::{
    RROutput,
    XRRCrtcInfo,
    XRRMonitorInfo,
    XRRScreenResources,
};

pub enum MonitorRepr {
    Monitor(*mut XRRMonitorInfo),
    Crtc(*mut XRRCrtcInfo),
}

impl MonitorRepr {
    pub unsafe fn get_output(&self) -> RROutput {
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

impl From<*mut XRRMonitorInfo> for MonitorRepr {
    fn from(monitor: *mut XRRMonitorInfo) -> Self {
        MonitorRepr::Monitor(monitor)
    }
}

impl From<*mut XRRCrtcInfo> for MonitorRepr {
    fn from(crtc: *mut XRRCrtcInfo) -> Self {
        MonitorRepr::Crtc(crtc)
    }
}

pub fn calc_dpi_factor(
    (width_px, height_px): (u32, u32),
    (width_mm, height_mm): (u64, u64),
) -> f64 {
    // Override DPI if `WINIT_HIDPI_FACTOR` variable is set
    if let Ok(dpi_factor_str) = env::var("WINIT_HIDPI_FACTOR") {
        if let Ok(dpi_factor) = f64::from_str(&dpi_factor_str) {
            if dpi_factor <= 0. {
                panic!("Expected `WINIT_HIDPI_FACTOR` to be bigger than 0, got '{}'", dpi_factor);
            }

            return dpi_factor;
        }
    }

    // See http://xpra.org/trac/ticket/728 for more information
    if width_mm == 0 || width_mm == 0 {
        return 1.0;
    }

    let ppmm = (
        (width_px as f64 * height_px as f64) / (width_mm as f64 * height_mm as f64)
    ).sqrt();
    // Quantize 1/12 step size
    ((ppmm * (12.0 * 25.4 / 96.0)).round() / 12.0).max(1.0)
}

impl XConnection {
    pub unsafe fn get_output_info(&self, resources: *mut XRRScreenResources, repr: &MonitorRepr) -> (String, f32) {
        let output_info = (self.xrandr.XRRGetOutputInfo)(
            self.display,
            resources,
            repr.get_output(),
        );
        let name_slice = slice::from_raw_parts(
            (*output_info).name as *mut u8,
            (*output_info).nameLen as usize,
        );
        let name = String::from_utf8_lossy(name_slice).into();
        let hidpi_factor = calc_dpi_factor(
            repr.get_dimensions(),
            ((*output_info).mm_width as u64, (*output_info).mm_height as u64),
        ) as f32;
        (self.xrandr.XRRFreeOutputInfo)(output_info);
        (name, hidpi_factor)
    }
}
