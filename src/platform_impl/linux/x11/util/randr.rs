use std::os::raw::*;
use std::slice;

use super::{
    ffi::{
        CurrentTime, RRCrtc, RRCrtcChangeNotifyMask, RRMode, RROutputPropertyNotifyMask,
        RRScreenChangeNotifyMask, Success, True, Window, XRRCrtcInfo, XRRScreenResources,
    },
    *,
};
use crate::platform_impl::platform::x11::{
    monitor::{MonitorHandle, MonitorInfoSource},
    VideoMode,
};

use winit_types::error::Error;

/// Represents values of `WINIT_HIDPI_FACTOR`.

impl XConnection {
    pub fn query_monitor_list_xrandr(&self) -> Vec<MonitorHandle> {
        assert_eq!(self.monitor_info_source, MonitorInfoSource::XRandR);
        let (xlib, xrandr) = syms!(XLIB, XRANDR_2_2_0);
        unsafe {
            let mut major = 0;
            let mut minor = 0;
            (xrandr.XRRQueryVersion)(**self.display, &mut major, &mut minor);

            // With RandR, there will only ever be one screen, so using default
            // here is OK.
            let screen = (xlib.XDefaultScreen)(**self.display);
            let root = (xlib.XRootWindow)(**self.display, screen);
            let resources = if (major == 1 && minor >= 3) || major > 1 {
                (xrandr.XRRGetScreenResourcesCurrent)(**self.display, root)
            } else {
                // WARNING: this function is supposedly very slow, on the order of hundreds of ms.
                // Upon failure, `resources` will be null.
                (xrandr.XRRGetScreenResources)(**self.display, root)
            };

            if resources.is_null() {
                panic!("[winit] `XRRGetScreenResources` returned NULL. That should only happen if the root window doesn't exist.");
            }

            let mut available;
            let mut has_primary = false;

            let primary = (xrandr.XRRGetOutputPrimary)(**self.display, root);
            available = Vec::with_capacity((*resources).ncrtc as usize);
            for crtc_index in 0..(*resources).ncrtc {
                let crtc_id = *((*resources).crtcs.offset(crtc_index as isize));
                let crtc = (xrandr.XRRGetCrtcInfo)(**self.display, resources, crtc_id);
                let is_active = (*crtc).width > 0 && (*crtc).height > 0 && (*crtc).noutput > 0;
                if is_active {
                    let primary = *(*crtc).outputs.offset(0) == primary;
                    has_primary |= primary;

                    let (name, scale_factor, video_modes) =
                        self.get_output_info(resources, crtc).unwrap();
                    let dimensions = ((*crtc).width as u32, (*crtc).height as u32);
                    let position = ((*crtc).x as i32, (*crtc).y as i32);
                    let rect = AaRect::new(position, dimensions);
                    available.push(MonitorHandle {
                        id: Some(crtc_id),
                        name,
                        scale_factor,
                        dimensions,
                        position,
                        primary,
                        rect,
                        video_modes,
                        screen: Some(screen),
                    });
                }
                (xrandr.XRRFreeCrtcInfo)(crtc);
            }

            // If no monitors were detected as being primary, we just pick one ourselves!
            if !has_primary {
                if let Some(ref mut fallback) = available.first_mut() {
                    // Setting this here will come in handy if we ever add an `is_primary` method.
                    fallback.primary = true;
                }
            }

            (xrandr.XRRFreeScreenResources)(resources);
            available
        }
    }

    pub fn select_xrandr_input(&self, root: Window) -> Result<c_int, Error> {
        assert_eq!(self.monitor_info_source, MonitorInfoSource::XRandR);
        let xrandr = syms!(XRANDR_2_2_0);

        let mut event_offset = 0;
        let mut error_offset = 0;
        let status = unsafe {
            (xrandr.XRRQueryExtension)(**self.display, &mut event_offset, &mut error_offset)
        };

        if status != True {
            self.display.check_errors()?;
            unreachable!("[winit] `XRRQueryExtension` failed but no error was received.");
        }

        let mask = RRCrtcChangeNotifyMask | RROutputPropertyNotifyMask | RRScreenChangeNotifyMask;
        unsafe { (xrandr.XRRSelectInput)(**self.display, root, mask) };

        Ok(event_offset)
    }

    pub unsafe fn get_output_info(
        &self,
        resources: *mut XRRScreenResources,
        crtc: *mut XRRCrtcInfo,
    ) -> Option<(String, f64, Vec<VideoMode>)> {
        assert_eq!(self.monitor_info_source, MonitorInfoSource::XRandR);
        let (xlib, xrandr) = syms!(XLIB, XRANDR_2_2_0);
        let output_info =
            (xrandr.XRRGetOutputInfo)(**self.display, resources, *(*crtc).outputs.offset(0));
        if output_info.is_null() {
            // When calling `XRRGetOutputInfo` on a virtual monitor (versus a physical display)
            // it's possible for it to return null.
            // https://bugs.debian.org/cgi-bin/bugreport.cgi?bug=816596
            let _ = self.display.check_errors(); // discard `BadRROutput` error
            return None;
        }

        // With RandR, there will only ever be one screen, so using default
        // here is OK.
        let screen = (xlib.XDefaultScreen)(**self.display);
        let bit_depth = (xlib.XDefaultDepth)(**self.display, screen);

        let output_modes =
            slice::from_raw_parts((*output_info).modes, (*output_info).nmode as usize);
        let resource_modes = slice::from_raw_parts((*resources).modes, (*resources).nmode as usize);

        let modes = resource_modes
            .iter()
            // XRROutputInfo contains an array of mode ids that correspond to
            // modes in the array in XRRScreenResources
            .filter(|x| output_modes.iter().any(|id| x.id == *id))
            .map(|x| {
                let refresh_rate = if x.dotClock > 0 && x.hTotal > 0 && x.vTotal > 0 {
                    x.dotClock as u64 * 1000 / (x.hTotal as u64 * x.vTotal as u64)
                } else {
                    0
                };

                VideoMode {
                    size: (x.width, x.height),
                    refresh_rate: (refresh_rate as f32 / 1000.0).round() as u16,
                    bit_depth: bit_depth as u16,
                    native_mode: Some(x.id),
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
        let scale_factor = self
            .acquire_scale_factor(Some((output_info, crtc)), screen)
            .unwrap();

        (xrandr.XRRFreeOutputInfo)(output_info);
        Some((name, scale_factor, modes))
    }

    pub fn set_crtc_config(&self, crtc_id: RRCrtc, mode_id: RRMode) -> Result<(), ()> {
        assert_eq!(self.monitor_info_source, MonitorInfoSource::XRandR);
        let (xlib, xrandr) = syms!(XLIB, XRANDR_2_2_0);
        unsafe {
            let mut major = 0;
            let mut minor = 0;
            (xrandr.XRRQueryVersion)(**self.display, &mut major, &mut minor);

            // With RandR, there will only ever be one screen, so using default
            // here is OK.
            let root = (xlib.XDefaultRootWindow)(**self.display);
            let resources = if (major == 1 && minor >= 3) || major > 1 {
                (xrandr.XRRGetScreenResourcesCurrent)(**self.display, root)
            } else {
                (xrandr.XRRGetScreenResources)(**self.display, root)
            };

            let crtc = (xrandr.XRRGetCrtcInfo)(**self.display, resources, crtc_id);
            let status = (xrandr.XRRSetCrtcConfig)(
                **self.display,
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

            (xrandr.XRRFreeCrtcInfo)(crtc);
            (xrandr.XRRFreeScreenResources)(resources);

            if status == Success as i32 {
                Ok(())
            } else {
                Err(())
            }
        }
    }

    pub fn get_crtc_mode(&self, crtc_id: RRCrtc) -> RRMode {
        assert_eq!(self.monitor_info_source, MonitorInfoSource::XRandR);
        let (xlib, xrandr) = syms!(XLIB, XRANDR_2_2_0);
        unsafe {
            let mut major = 0;
            let mut minor = 0;
            (xrandr.XRRQueryVersion)(**self.display, &mut major, &mut minor);

            // With RandR, there will only ever be one screen, so using default
            // here is OK.
            let root = (xlib.XDefaultRootWindow)(**self.display);
            let resources = if (major == 1 && minor >= 3) || major > 1 {
                (xrandr.XRRGetScreenResourcesCurrent)(**self.display, root)
            } else {
                (xrandr.XRRGetScreenResources)(**self.display, root)
            };

            let crtc = (xrandr.XRRGetCrtcInfo)(**self.display, resources, crtc_id);
            let mode = (*crtc).mode;
            (xrandr.XRRFreeCrtcInfo)(crtc);
            (xrandr.XRRFreeScreenResources)(resources);
            mode
        }
    }
}
