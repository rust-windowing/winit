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
    position: (u32, u32),
    /// If the monitor is the primary one
    primary: bool,
}

pub fn get_available_monitors(x: &Arc<XConnection>) -> Vec<MonitorId> {
    let mut available = Vec::new();
    unsafe {
        let root = (x.xlib.XDefaultRootWindow)(x.display);
        let resources = (x.xrandr.XRRGetScreenResources)(x.display, root);

        let mut major = 0;
        let mut minor = 0;
        (x.xrandr.XRRQueryVersion)(x.display, &mut major, &mut minor);
        if ((major as u64)<<32)+(minor as u64) >= (1<<32)+5 {
            // We're in XRandR >= 1.5, enumerate Monitors to handle things like MST and videowalls
            let mut nmonitors = 0;
            let monitors = (x.xrandr.XRRGetMonitors)(x.display, root, 1, &mut nmonitors);
            for i in 0..nmonitors {
                let monitor = *(monitors.offset(i as isize));
                let output = (x.xrandr.XRRGetOutputInfo)(x.display, resources, *(monitor.outputs.offset(0)));
                let nameslice = slice::from_raw_parts((*output).name as *mut u8, (*output).nameLen as usize);
                let name = String::from_utf8_lossy(nameslice).into_owned();
                (x.xrandr.XRRFreeOutputInfo)(output);
                available.push(MonitorId{
                    id: i as u32,
                    name,
                    dimensions: (monitor.width as u32, monitor.height as u32),
                    position: (monitor.x as u32, monitor.y as u32),
                    primary: (monitor.primary != 0),
                });
            }
            (x.xrandr.XRRFreeMonitors)(monitors);
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
                    (x.xrandr.XRRFreeOutputInfo)(output);
                    available.push(MonitorId{
                        id: crtcid as u32,
                        name,
                        dimensions: ((*crtc).width as u32, (*crtc).height as u32),
                        position: ((*crtc).x as u32, (*crtc).y as u32),
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
    for monitor in get_available_monitors(x) {
        if monitor.primary {
            return monitor.clone()
        }
    }

    panic!("[winit] Failed to find the primary monitor")
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

    pub fn get_position(&self) -> (u32, u32) {
        self.position
    }
}
