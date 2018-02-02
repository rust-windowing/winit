use std::sync::Arc;
use std::slice;

use super::XConnection;

#[derive(Clone)]
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
    /// The DPI scaling factor
    hidpi_factor: f32,
}

pub fn get_available_monitors(x: &Arc<XConnection>) -> Vec<MonitorId> {
    let mut available = Vec::new();
    unsafe {
        let root = (x.xlib.XDefaultRootWindow)(x.display);
        let resources = (x.xrandr.XRRGetScreenResources)(x.display, root);

        if let Some(ref xrandr_1_5) = x.xrandr_1_5 {
            // We're in XRandR >= 1.5, enumerate Monitors to handle things like MST and videowalls
            let mut nmonitors = 0;
            let monitors = (xrandr_1_5.XRRGetMonitors)(x.display, root, 1, &mut nmonitors);
            for i in 0..nmonitors {
                let monitor = *(monitors.offset(i as isize));
                let output = (xrandr_1_5.XRRGetOutputInfo)(x.display, resources, *(monitor.outputs.offset(0)));
                let nameslice = slice::from_raw_parts((*output).name as *mut u8, (*output).nameLen as usize);
                let name = String::from_utf8_lossy(nameslice).into_owned();
                let hidpi_factor = {
                    let x_mm = (*output).mm_width as f32;
                    let y_mm = (*output).mm_height as f32;
                    let x_px = monitor.width as f32;
                    let y_px = monitor.height as f32;
                    let ppmm = ((x_px * y_px) / (x_mm * y_mm)).sqrt();

                    // Quantize 1/12 step size
                    ((ppmm * (12.0 * 25.4 / 96.0)).round() / 12.0).max(1.0)
                };
                (xrandr_1_5.XRRFreeOutputInfo)(output);
                available.push(MonitorId{
                    id: i as u32,
                    name,
                    hidpi_factor,
                    dimensions: (monitor.width as u32, monitor.height as u32),
                    position: (monitor.x as i32, monitor.y as i32),
                    primary: (monitor.primary != 0),
                });
            }
            (xrandr_1_5.XRRFreeMonitors)(monitors);
        } else {
            // We're in XRandR < 1.5, enumerate CRTCs. Everything will work but MST and
            // videowall setups will show more monitors than the logical groups the user
            // cares about
            for i in 0..(*resources).ncrtc {
                let crtcid = *((*resources).crtcs.offset(i as isize));
                let crtc = (x.xrandr.XRRGetCrtcInfo)(x.display, resources, crtcid);
                if (*crtc).width > 0 && (*crtc).height > 0 && (*crtc).noutput > 0 {
                    let output = (x.xrandr.XRRGetOutputInfo)(x.display, resources, *((*crtc).outputs.offset(0)));
                    let nameslice = slice::from_raw_parts((*output).name as *mut u8, (*output).nameLen as usize);
                    let name = String::from_utf8_lossy(nameslice).into_owned();

                    let hidpi_factor = {
                        let x_mm = (*output).mm_width as f32;
                        let y_mm = (*output).mm_height as f32;
                        let x_px = (*crtc).width as f32;
                        let y_px = (*crtc).height as f32;
                        let ppmm = ((x_px * y_px) / (x_mm * y_mm)).sqrt();

                        // Quantize 1/12 step size
                        ((ppmm * (12.0 * 25.4 / 96.0)).round() / 12.0).max(1.0)
                    };

                    (x.xrandr.XRRFreeOutputInfo)(output);
                    available.push(MonitorId{
                        id: crtcid as u32,
                        name,
                        hidpi_factor,
                        dimensions: ((*crtc).width as u32, (*crtc).height as u32),
                        position: ((*crtc).x as i32, (*crtc).y as i32),
                        primary: true,
                    });
                }
                (x.xrandr.XRRFreeCrtcInfo)(crtc);
            }
        }
        (x.xrandr.XRRFreeScreenResources)(resources);
    }
    available
}

#[inline]
pub fn get_primary_monitor(x: &Arc<XConnection>) -> MonitorId {
    get_available_monitors(x).into_iter().find(|m| m.primary)
        // 'no primary' case is better handled picking some existing monitor
        .or_else(|| get_available_monitors(x).into_iter().next())
        .expect("[winit] Failed to find any x11 monitor")
}

impl MonitorId {
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
