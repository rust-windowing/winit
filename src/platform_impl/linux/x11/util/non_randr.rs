//! For use when RandR is not supported.
use std::os::raw;

use super::*;
use crate::platform_impl::platform::x11::{monitor::MonitorHandle, VideoMode};

fn get_depths(xconn: &XConnection, screen: raw::c_int) -> Vec<raw::c_int> {
    let xlib = syms!(XLIB);
    unsafe {
        let mut num_depths = 0;
        let depths = (xlib.XListDepths)(**xconn.display, screen, &mut num_depths);

        if depths.is_null() {
            let default_depth = (xlib.XDefaultDepth)(**xconn.display, screen);
            return vec![default_depth];
        }

        assert!(num_depths != 0);

        let ret: Vec<_> = (0..num_depths)
            .into_iter()
            .map(|offset| *depths.offset(offset as isize))
            .collect();

        (xlib.XFree)(depths as *mut _);
        ret
    }
}

impl XConnection {
    pub fn query_monitor_list_xinerama(&self) -> Vec<MonitorHandle> {
        let (xlib, xinerama) = syms!(XLIB, XINERAMA);
        // Alright, so we got the list of screens, however, Xinerama also
        // exposes a list of monitors and how they were stitched together to
        // make the screens.
        //
        // Well, or at least that's how it should work. My AMD GPU's new shiny
        // modern driver, amdgpu, did not support Xinerama at all, it would just
        // SEGV. Xinerama is, after all, very much in the category of legacy shit.
        //
        // Meanwhile, my GPU's older driver, radeon, successfully enabled
        // Xinerama and merged all my monitors into a single screen, however,
        // despite how hard I tried, refused to expose the XINERAMA extension
        // to X11 clients.
        //
        // We add these to the list in case applications want to borderless
        // fullscreen onto just _one_ monitor, not the whole screen.
        //
        // FIXME: As it currently stands, if a user passes in one of these
        // monitors for fullscreening, it will just fullscreen the whole screen.
        // To fix this, we need to use the `_NET_WM_FULLSCREEN_MONITORS` atom,
        // an atom specifically made for use with Xinerama.
        //
        // The issue is that I don't know of any WM that supports this atom
        // because _it is specifically made for use with Xinerama,_ and what
        // modern WM supports Xinerama?
        //
        // I (Freya) could barely get Xinerama working, so I'm not even going to
        // bother supported this silly atom.
        let mut monitors = self.query_monitor_list_none();
        let num_screens = unsafe { (xlib.XScreenCount)(**self.display) };

        unsafe {
            let mut number_of_screens = 0;
            // Should return NULL and set number_of_screens to 0 only if
            // Xinerama is not active.
            //
            // Now, in xdisplay.rs we check to make sure that Xinerama _is_
            // active, so this should never happen, however, we check this stuff
            // here again because I really **really** don't trust the people who
            // implemented these legacy X11 extensions, especially given how
            // buggy it was to setup on my rig.
            let xinerama_screens =
                (xinerama.XineramaQueryScreens)(**self.display, &mut number_of_screens);

            if xinerama_screens.is_null() {
                return monitors;
            }

            for xinerama_screen_index in 0..number_of_screens {
                let xinerama_screen = *xinerama_screens.offset(xinerama_screen_index as isize);

                // Just a sanity check
                assert!(xinerama_screen.screen_number < num_screens);

                // These should be ordered, so this should be alright.
                let mut new_monitor = monitors[xinerama_screen.screen_number as usize].clone();
                // Just a sanity check
                assert_eq!(xinerama_screen.screen_number, new_monitor.screen.unwrap());

                // Our new monitor is never the primary.
                new_monitor.primary = false;

                // And of course it's position & size, which will overlap with the screen's.
                let dimensions = (xinerama_screen.width as u32, xinerama_screen.height as u32);
                new_monitor.dimensions = dimensions;
                new_monitor.position = (xinerama_screen.x_org as i32, xinerama_screen.y_org as i32);
                new_monitor.rect = AaRect::new(new_monitor.position, new_monitor.dimensions);
                new_monitor
                    .video_modes
                    .iter_mut()
                    .for_each(|video_mode| video_mode.size = dimensions);

                monitors.push(new_monitor);
            }

            (xlib.XFree)(xinerama_screens as *mut _);
        }

        monitors
    }

    pub fn query_monitor_list_none(&self) -> Vec<MonitorHandle> {
        let xlib = syms!(XLIB);
        let num_screens = unsafe { (xlib.XScreenCount)(**self.display) };
        let default_screen = unsafe { (xlib.XDefaultScreen)(**self.display) };

        (0..num_screens)
            .into_iter()
            .map(|screen| {
                // Every screen starts at the start, relative to itself...
                let position = (0, 0);
                let (dimensions, _) = self.get_xlib_dims(screen);
                let rect = AaRect::new(position, dimensions);

                let depths = get_depths(self, screen);
                let video_modes: Vec<_> = depths
                    .into_iter()
                    .map(|bit_depth| VideoMode {
                        size: dimensions,
                        bit_depth: bit_depth as u16,
                        // Does it matter? Not like they can change the video mode
                        // easily, especially if they are using Xinerama which makes
                        // it literally impossible.
                        //
                        // The only alternative to RandR for getting the refresh
                        // rate is using something like XF86VMODE, which is
                        // undocumented and glitchy as hell.
                        refresh_rate: 60,
                        // This is populated in `MonitorHandle::video_modes` as the
                        // video mode is returned to the user
                        monitor: None,
                        // RandR only.
                        native_mode: None,
                    })
                    .collect();

                MonitorHandle {
                    position,
                    id: None,
                    screen: Some(screen),
                    // We will treat the default screen as the primary.
                    primary: screen == default_screen,
                    name: format!("Unknown - Screen {}", screen),
                    scale_factor: self.acquire_scale_factor(None, screen).unwrap(),
                    dimensions,
                    rect,
                    video_modes,
                }
            })
            .collect()
    }
}
