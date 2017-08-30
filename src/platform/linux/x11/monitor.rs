use std::collections::VecDeque;
use std::sync::Arc;
use std::ptr;

use super::XConnection;

#[derive(Clone)]
pub struct MonitorId {
  pub x: Arc<XConnection>,
  pub crtc: u64,
}

pub fn get_available_monitors(x: &Arc<XConnection>) -> VecDeque<MonitorId> {
    let mut monitors = VecDeque::new();
    unsafe {
        let root = (x.xlib.XDefaultRootWindow)(x.display);
        let resources = (x.xrandr.XRRGetScreenResources)(x.display, root);

        for i in 0..(*resources).ncrtc {
            let crtcid = ptr::read((*resources).crtcs.offset(i as isize));
            let crtc = (x.xrandr.XRRGetCrtcInfo)(x.display, resources, crtcid);
            if (*crtc).width > 0 && (*crtc).height > 0 && (*crtc).noutput > 0 {
                monitors.push_back(MonitorId{
                    x: x.clone(),
                    crtc: crtcid
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
        Some(format!("CRTC #{}", self.crtc))
    }

    #[inline]
    pub fn get_native_identifier(&self) -> u32 {
        self.crtc as u32
    }

    pub fn get_dimensions(&self) -> (u32, u32) {
        unsafe {
            let root = (self.x.xlib.XDefaultRootWindow)(self.x.display);
            let resources = (self.x.xrandr.XRRGetScreenResources)(self.x.display, root);
            let crtc = (self.x.xrandr.XRRGetCrtcInfo)(self.x.display, resources, self.crtc);
            let width = (*crtc).width;
            let height = (*crtc).height;
            (self.x.xrandr.XRRFreeCrtcInfo)(crtc);
            (self.x.xrandr.XRRFreeScreenResources)(resources);
            (width, height)
        }
    }
}
