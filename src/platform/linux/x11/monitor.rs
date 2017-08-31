use std::sync::Arc;
use std::slice;

use super::XConnection;

#[derive(Clone)]
pub struct MonitorId {
  /// This is the actual ID but isn't really used as name/dimensions/position are cached
  crtc: u64,
  /// The name of the monitor
  name: String,
  /// The size of the monitor
  dimensions: (u32, u32),
  /// The position of the monitor in the X screen
  position: (u32, u32),
}

pub fn get_available_monitors(x: &Arc<XConnection>) -> Vec<MonitorId> {
    let mut monitors = Vec::new();
    // For simplicity we just enumerate CRTCs and use those as monitors. XRandR 1.5 adds
    // the concept of a monitor that aggregates several CRTCs which is useful from Multi
    // Stream Transport monitors (same screen, fed as if 2) and for video wall kinds of
    // situations.
    // Given test hardware it would be easy to support these cases by testing if we have
    // XRandR 1.5 and if yes, enumerating monitors instead of CRTCs
    unsafe {
        let root = (x.xlib.XDefaultRootWindow)(x.display);
        let resources = (x.xrandr.XRRGetScreenResources)(x.display, root);

        for i in 0..(*resources).ncrtc {
            let crtcid = *((*resources).crtcs.offset(i as isize));
            let crtc = (x.xrandr.XRRGetCrtcInfo)(x.display, resources, crtcid);
            if (*crtc).width > 0 && (*crtc).height > 0 && (*crtc).noutput > 0 {
                let output = (x.xrandr.XRRGetOutputInfo)(x.display, resources, *((*crtc).outputs.offset(0)));
                let nameslice = slice::from_raw_parts((*output).name as *mut u8, (*output).nameLen as usize);
                let name = String::from_utf8_lossy(nameslice).into_owned();
                (x.xrandr.XRRFreeOutputInfo)(output);
                monitors.push(MonitorId{
                    crtc: crtcid,
                    name,
                    dimensions: ((*crtc).width as u32, (*crtc).height as u32),
                    position: ((*crtc).x as u32, (*crtc).y as u32),
                });
            }
            (x.xrandr.XRRFreeCrtcInfo)(crtc);
        }
        (x.xrandr.XRRFreeScreenResources)(resources);
    }
    monitors
}

#[inline]
pub fn get_primary_monitor(x: &Arc<XConnection>) -> MonitorId {
    get_available_monitors(x)[0].clone()
}

impl MonitorId {
    pub fn get_name(&self) -> Option<String> {
        Some(self.name.clone())
    }

    #[inline]
    pub fn get_native_identifier(&self) -> u32 {
        self.crtc as u32
    }

    pub fn get_dimensions(&self) -> (u32, u32) {
        self.dimensions
    }

    pub fn get_position(&self) -> (u32, u32) {
        self.position
    }
}
