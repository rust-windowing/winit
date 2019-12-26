use std::os::raw::*;
use std::{env, slice, str::FromStr};

use winit_types::error::Error;
use super::{
    ffi::{
        RRCrtc, RRCrtcChangeNotifyMask, RRMode, RROutputPropertyNotifyMask,
        RRScreenChangeNotifyMask, True, Window, XRRCrtcInfo, XRRScreenResources,
        CurrentTime, Success
    },
    *,
};
use crate::{
    dpi::validate_hidpi_factor,
    platform_impl::platform::x11::{
        VideoMode,
        monitor::{MonitorHandle, MonitorExt},
    }
};

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
                "[winit] `WINIT_HIDPI_FACTOR` invalid; DPI factors must be normal floats greater than 0. Got `{}`",
                dpi_override,
            );
        }
        return dpi_override;
    }

    // See http://xpra.org/trac/ticket/728 for more information.
    if width_mm == 0 || height_mm == 0 {
        warn!("XRandR reported that the display's 0mm in size, which is certifiably insane");
        return 1.0;
    }

    let ppmm = ((width_px as f64 * height_px as f64) / (width_mm as f64 * height_mm as f64)).sqrt();
    // Quantize 1/12 step size
    let dpi_factor = ((ppmm * (12.0 * 25.4 / 96.0)).round() / 12.0).max(1.0);
    assert!(validate_hidpi_factor(dpi_factor));
    dpi_factor
}

impl XConnection {
    pub fn query_monitor_list_xrandr(&self) -> Vec<MonitorHandle> {
        assert_eq!(self.monitor_ext, MonitorExt::XRandR);
        let (xlib, xrandr) = syms!(XLIB, XRANDR_2_2_0);
        unsafe {
            let mut major = 0;
            let mut minor = 0;
            (xrandr.XRRQueryVersion)(**self.display, &mut major, &mut minor);

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

                    let (name, hidpi_factor, video_modes) = self.get_output_info(resources, crtc).unwrap();
                    let dimensions =((*crtc).width as u32, (*crtc).height as u32);
                    let position = ((*crtc).x as i32, (*crtc).y as i32);
                    let rect = AaRect::new(position, dimensions);
                    available.push(
                        MonitorHandle {
                            id: Some(crtc_id),
                            name,
                            hidpi_factor,
                            dimensions,
                            position,
                            primary,
                            rect,
                            video_modes,
                            screen: Some(screen),
                        }
                    );
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
        assert_eq!(self.monitor_ext, MonitorExt::XRandR);
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

    // Retrieve DPI from Xft.dpi property
    pub unsafe fn get_xft_dpi(&self) -> Option<f64> {
        let xlib = syms!(XLIB);
        (xlib.XrmInitialize)();
        let resource_manager_str = (xlib.XResourceManagerString)(**self.display);
        if resource_manager_str == ptr::null_mut() {
            return None;
        }
        if let Ok(res) = ::std::ffi::CStr::from_ptr(resource_manager_str).to_str() {
            let name: &str = "Xft.dpi:\t";
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
        resources: *mut XRRScreenResources,
        crtc: *mut XRRCrtcInfo,
    ) -> Option<(String, f64, Vec<VideoMode>)> {
        assert_eq!(self.monitor_ext, MonitorExt::XRandR);
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
        let hidpi_factor = if let Some(dpi) = self.get_xft_dpi() {
            dpi / 96.
        } else {
            calc_dpi_factor(
                ((*crtc).width as u32, (*crtc).height as u32),
                (
                    (*output_info).mm_width as u64,
                    (*output_info).mm_height as u64,
                ),
            )
        };

        (xrandr.XRRFreeOutputInfo)(output_info);
        Some((name, hidpi_factor, modes))
    }

    pub fn set_crtc_config(&self, crtc_id: RRCrtc, mode_id: RRMode) -> Result<(), ()> {
        assert_eq!(self.monitor_ext, MonitorExt::XRandR);
        let (xlib, xrandr) = syms!(XLIB, XRANDR_2_2_0);
        unsafe {
            let mut major = 0;
            let mut minor = 0;
            (xrandr.XRRQueryVersion)(**self.display, &mut major, &mut minor);

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
        assert_eq!(self.monitor_ext, MonitorExt::XRandR);
        let (xlib, xrandr) = syms!(XLIB, XRANDR_2_2_0);
        unsafe {
            let mut major = 0;
            let mut minor = 0;
            (xrandr.XRRQueryVersion)(**self.display, &mut major, &mut minor);

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
