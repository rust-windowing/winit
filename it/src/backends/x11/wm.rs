use super::XInstanceData;
use crate::backend::BackendIcon;
use crate::backends::x11::{Protocols, WindowState, XConnection, XWindow};
use std::future::Future;
use std::ptr;
use std::sync::{Arc, Weak};
use tokio::io::unix::AsyncFd;
use tokio::io::Interest;
use xcb_dl::ffi;
use xcb_dl_util::error::XcbErrorType;
use xcb_dl_util::hint::{XcbHints, XcbHintsFlags, XcbSizeHints, XcbSizeHintsFlags};
use xcb_dl_util::property::XcbGetPropertyError;

pub(super) fn run(instance: Arc<XInstanceData>) -> impl Future<Output = ()> {
    unsafe {
        let xcb = &instance.backend.xcb;
        let xrandr = &instance.backend.xrandr;
        let c = XConnection::new(&instance.backend, instance.display);
        let mut err = ptr::null_mut();
        let reply = xrandr.xcb_randr_query_version_reply(
            c.c,
            xrandr.xcb_randr_query_version(c.c, 1, 3),
            &mut err,
        );
        if let Err(e) = c.errors.check(xcb, reply, err) {
            panic!("Could not enable randr: {}", e);
        }
        let first_randr_event =
            (*xcb.xcb_get_extension_data(c.c, xrandr.xcb_randr_id())).first_event;
        let cookie = xrandr.xcb_randr_select_input_checked(c.c, c.screen.root, 0xff);
        if let Err(e) = c.errors.check_cookie(xcb, cookie) {
            panic!("Can't listen for randr events: {}", e);
        }
        let events = ffi::XCB_EVENT_MASK_SUBSTRUCTURE_REDIRECT
            | ffi::XCB_EVENT_MASK_SUBSTRUCTURE_NOTIFY
            | ffi::XCB_EVENT_MASK_PROPERTY_CHANGE
            | ffi::XCB_EVENT_MASK_BUTTON_RELEASE
            | ffi::XCB_EVENT_MASK_POINTER_MOTION;
        let cookie = xcb.xcb_change_window_attributes_checked(
            c.c,
            c.screen.root,
            ffi::XCB_CW_EVENT_MASK,
            &events as *const ffi::xcb_event_mask_t as _,
        );
        if let Err(e) = c.errors.check_cookie(xcb, cookie) {
            panic!("Could not select wm events: {}", e);
        }
        let supported = [
            instance.atoms.net_client_list,
            instance.atoms.net_supporting_wm_check,
        ];
        let cookie = xcb.xcb_change_property_checked(
            c.c,
            ffi::XCB_PROP_MODE_REPLACE as _,
            c.screen.root,
            instance.atoms.net_supported,
            ffi::XCB_ATOM_ATOM,
            32,
            supported.len() as _,
            supported.as_ptr() as _,
        );
        if let Err(e) = c.errors.check_cookie(xcb, cookie) {
            panic!("Could not set _NET_SUPPORTED property: {}", e);
        }
        let window_id = xcb.xcb_generate_id(c.c);
        let cookie = xcb.xcb_create_window_checked(
            c.c,
            c.screen.root_depth,
            window_id,
            c.screen.root,
            0,
            0,
            1,
            1,
            0,
            ffi::XCB_WINDOW_CLASS_INPUT_OUTPUT as _,
            c.screen.root_visual,
            0,
            ptr::null(),
        );
        if let Err(e) = c.errors.check_cookie(xcb, cookie) {
            panic!("Could not create child window: {}", e);
        }
        const WM_NAME: &str = "UNKNOWN WM";
        let cookie = xcb.xcb_change_property_checked(
            c.c,
            ffi::XCB_PROP_MODE_REPLACE as _,
            window_id,
            instance.atoms.net_wm_name,
            instance.atoms.utf8_string,
            8,
            WM_NAME.len() as _,
            WM_NAME.as_ptr() as *const _,
        );
        if let Err(e) = c.errors.check_cookie(xcb, cookie) {
            panic!("Could not set _NET_WM_NAME property on window: {}", e);
        }
        let cookie = xcb.xcb_change_property_checked(
            c.c,
            ffi::XCB_PROP_MODE_REPLACE as _,
            c.screen.root,
            instance.atoms.net_supporting_wm_check,
            ffi::XCB_ATOM_ATOM,
            32,
            1,
            &window_id as *const _ as _,
        );
        if let Err(e) = c.errors.check_cookie(xcb, cookie) {
            panic!(
                "Could not set _NET_SUPPORTING_WM_CHECK property on root: {}",
                e
            );
        }
        let cookie = xcb.xcb_change_property_checked(
            c.c,
            ffi::XCB_PROP_MODE_REPLACE as _,
            window_id,
            instance.atoms.net_supporting_wm_check,
            ffi::XCB_ATOM_ATOM,
            32,
            1,
            &window_id as *const _ as _,
        );
        if let Err(e) = c.errors.check_cookie(xcb, cookie) {
            panic!(
                "Could not set _NET_SUPPORTING_WM_CHECK property on child: {}",
                e
            );
        }
        let wm = Wm {
            c,
            instance,
            window_id,
            first_randr_event,
            moving: None,
            crtcs: vec![],
        };

        wm.run()
    }
}

struct Wm {
    c: XConnection,
    instance: Arc<XInstanceData>,
    window_id: ffi::xcb_window_t,
    first_randr_event: u8,
    moving: Option<Moving>,
    crtcs: Vec<Crtc>,
}

struct Crtc {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

struct Moving {
    win: Weak<XWindow>,
    start_pointer_x: i32,
    start_pointer_y: i32,
    start_window_x: i32,
    start_window_y: i32,
}

impl Drop for Wm {
    fn drop(&mut self) {
        unsafe {
            self.instance
                .backend
                .xcb
                .xcb_destroy_window(self.c.c, self.window_id);
        }
    }
}

pub const TITLE_HEIGHT: u16 = 10;

impl Wm {
    async fn run(mut self) {
        self.update_crtcs();
        self.update_client_list();
        let fd = AsyncFd::with_interest(self.c.fd, Interest::READABLE).unwrap();
        loop {
            self.handle_events();
            fd.readable().await.unwrap().clear_ready();
        }
    }

    fn update_crtcs(&mut self) {
        unsafe {
            let xrandr = &self.instance.backend.xrandr;
            let xcb = &self.instance.backend.xcb;
            let mut err = ptr::null_mut();
            let reply = xrandr.xcb_randr_get_screen_resources_current_reply(
                self.c.c,
                xrandr.xcb_randr_get_screen_resources_current(self.c.c, self.c.screen.root),
                &mut err,
            );
            let reply = match self.c.errors.check(xcb, reply, err) {
                Ok(r) => r,
                Err(e) => panic!("Can't list crtcs: {}", e),
            };
            let crtcs = std::slice::from_raw_parts(
                xrandr.xcb_randr_get_screen_resources_current_crtcs(&*reply),
                reply.num_crtcs as _,
            );
            self.crtcs.clear();
            for crtc in crtcs {
                let reply = xrandr.xcb_randr_get_crtc_info_reply(
                    self.c.c,
                    xrandr.xcb_randr_get_crtc_info(self.c.c, *crtc, 0),
                    &mut err,
                );
                let reply = match self.c.errors.check(xcb, reply, err) {
                    Ok(r) => r,
                    Err(e) => panic!("Can't get crtc info: {}", e),
                };
                self.crtcs.push(Crtc {
                    x: reply.x as _,
                    y: reply.y as _,
                    width: reply.width as _,
                    height: reply.height as _,
                });
            }
        }
    }

    fn handle_events(&mut self) {
        unsafe {
            loop {
                let event = self.instance.backend.xcb.xcb_poll_for_event(self.c.c);
                let event = match self.c.errors.check_val(&self.instance.backend.xcb, event) {
                    Ok(e) => e,
                    Err(e) => {
                        if matches!(e.ty, XcbErrorType::MissingReply) {
                            break;
                        }
                        panic!("The connection is in error: {}", e);
                    }
                };
                self.handle_event(&event);
            }
            self.instance.backend.xcb.xcb_flush(self.c.c);
        }
    }

    fn handle_event(&mut self, event: &ffi::xcb_generic_event_t) {
        match event.response_type & 0x7f {
            ffi::XCB_CREATE_NOTIFY => self.handle_create_notify(event),
            ffi::XCB_MAP_REQUEST => self.handle_map_request(event),
            ffi::XCB_CONFIGURE_REQUEST => self.handle_configure_request(event),
            ffi::XCB_PROPERTY_NOTIFY => self.handle_property_notify(event),
            ffi::XCB_MAP_NOTIFY => self.handle_map_notify(event),
            ffi::XCB_UNMAP_NOTIFY => self.handle_unmap_notify(event),
            ffi::XCB_DESTROY_NOTIFY => self.handle_destroy_notify(event),
            ffi::XCB_REPARENT_NOTIFY => self.handle_reparent_notify(event),
            ffi::XCB_CLIENT_MESSAGE => self.handle_client_message(event),
            ffi::XCB_CONFIGURE_NOTIFY => self.handle_configure_notify(event),
            ffi::XCB_MOTION_NOTIFY => self.handle_motion_notify(event),
            ffi::XCB_BUTTON_RELEASE => self.handle_button_release(event),
            ffi::XCB_MAPPING_NOTIFY => {}
            n if n == self.first_randr_event + ffi::XCB_RANDR_SCREEN_CHANGE_NOTIFY => {
                self.handle_randr_screen_change_notify(event);
            }
            n if n == self.first_randr_event + ffi::XCB_RANDR_NOTIFY => {
                self.handle_randr_notify(event);
            }
            _ => {
                log::warn!("Received unexpected event: {:?}", event);
            }
        }
    }

    fn handle_randr_screen_change_notify(&mut self, event: &ffi::xcb_generic_event_t) {
        self.update_crtcs();
        let event =
            unsafe { &*(event as *const _ as *const ffi::xcb_randr_screen_change_notify_event_t) };
        log::info!("{:?}", event);
    }

    fn handle_randr_notify(&mut self, event: &ffi::xcb_generic_event_t) {
        self.update_crtcs();
        let event = unsafe { &*(event as *const _ as *const ffi::xcb_randr_notify_event_t) };
        match event.sub_code as u32 {
            ffi::XCB_RANDR_NOTIFY_CRTC_CHANGE => self.handle_randr_notify_crtc_change(event),
            ffi::XCB_RANDR_NOTIFY_OUTPUT_CHANGE => self.handle_randr_notify_output_change(event),
            ffi::XCB_RANDR_NOTIFY_OUTPUT_PROPERTY => {
                self.handle_randr_notify_output_property(event)
            }
            ffi::XCB_RANDR_NOTIFY_PROVIDER_CHANGE => {
                self.handle_randr_notify_provider_change(event)
            }
            ffi::XCB_RANDR_NOTIFY_PROVIDER_PROPERTY => {
                self.handle_randr_notify_provider_property(event)
            }
            ffi::XCB_RANDR_NOTIFY_RESOURCE_CHANGE => {
                self.handle_randr_notify_resource_change(event)
            }
            ffi::XCB_RANDR_NOTIFY_LEASE => self.handle_randr_notify_lease(event),
            _ => {
                log::warn!("Unexpected randr event: {:?}", event);
            }
        }
    }

    fn handle_randr_notify_crtc_change(&mut self, event: &ffi::xcb_randr_notify_event_t) {
        let cc = unsafe { &event.u.cc };
        log::info!("{:?}", cc);
    }

    fn handle_randr_notify_output_change(&mut self, event: &ffi::xcb_randr_notify_event_t) {
        let cc = unsafe { &event.u.oc };
        log::info!("{:?}", cc);
    }

    fn handle_randr_notify_output_property(&mut self, event: &ffi::xcb_randr_notify_event_t) {
        let cc = unsafe { &event.u.op };
        log::info!("{:?}", cc);
    }

    fn handle_randr_notify_provider_change(&mut self, event: &ffi::xcb_randr_notify_event_t) {
        let cc = unsafe { &event.u.pc };
        log::info!("{:?}", cc);
    }

    fn handle_randr_notify_provider_property(&mut self, event: &ffi::xcb_randr_notify_event_t) {
        let cc = unsafe { &event.u.pp };
        log::info!("{:?}", cc);
    }

    fn handle_randr_notify_resource_change(&mut self, event: &ffi::xcb_randr_notify_event_t) {
        let cc = unsafe { &event.u.rc };
        log::info!("{:?}", cc);
    }

    fn handle_randr_notify_lease(&mut self, event: &ffi::xcb_randr_notify_event_t) {
        let cc = unsafe { &event.u.lc };
        log::info!("{:?}", cc);
    }

    fn handle_property_notify(&mut self, event: &ffi::xcb_generic_event_t) {
        let event = unsafe { &*(event as *const _ as *const ffi::xcb_property_notify_event_t) };
        if event.atom == self.instance.atoms.motif_wm_hints {
            log::info!("MOTIF_WM_HINTS changed: {:?}", event.window);
            self.handle_motif_wm_hints(event.window);
        } else if event.atom == ffi::XCB_ATOM_WM_NAME {
            log::info!("WM_NAME changed: {:?}", event.window);
            self.handle_wm_name(event.window);
        } else if event.atom == self.instance.atoms.wm_hints {
            log::info!("WM_HINTS changed: {:?}", event.window);
            self.handle_wm_hints(event.window);
        } else if event.atom == self.instance.atoms.wm_normal_hints {
            log::info!("WM_NORMAL_HINTS changed: {:?}", event.window);
            self.handle_wm_normal_hints(event.window);
        } else if event.atom == self.instance.atoms.net_wm_name {
            log::info!("NET_WM_NAME changed: {:?}", event.window);
            self.handle_net_wm_name(event.window);
        } else if event.atom == self.instance.atoms.net_wm_icon {
            log::info!("NET_WM_ICON changed: {:?}", event.window);
            self.handle_net_wm_icon(event.window);
        } else if event.atom == ffi::XCB_ATOM_WM_CLASS {
            log::info!("WM_CLASS changed: {:?}", event.window);
            self.handle_wm_class(event.window);
        } else if event.atom == self.instance.atoms.wm_protocols {
            log::info!("WM_PROTOCOLS changed: {:?}", event.window);
            self.handle_wm_protocols(event.window);
        } else if event.atom == self.instance.atoms.net_supporting_wm_check {
            // ignored
        } else if event.atom == self.instance.atoms.net_supported {
            // ignored
        } else if event.atom == self.instance.atoms.net_client_list {
            // ignored
        } else if event.atom == self.instance.atoms.wm_state {
            // ignored
        } else {
            unsafe {
                let xcb = &self.instance.backend.xcb;
                let mut err = ptr::null_mut();
                let s = xcb.xcb_get_atom_name_reply(
                    self.c.c,
                    xcb.xcb_get_atom_name(self.c.c, event.atom),
                    &mut err,
                );
                let name = match self.c.errors.check(xcb, s, err) {
                    Ok(name) => name,
                    Err(e) => {
                        log::warn!("Cannot retrieve property name: {}", e);
                        return;
                    }
                };
                let name = std::slice::from_raw_parts(
                    xcb.xcb_get_atom_name_name(&*name) as *const u8,
                    name.name_len as _,
                );
                log::warn!("Unknown property change: {}", String::from_utf8_lossy(name));
            }
        }
    }

    fn handle_net_wm_icon(&mut self, window: ffi::xcb_window_t) {
        let mut data = self.instance.wm_data.lock();
        let win = match data.window(window) {
            Some(win) => win,
            None => {
                return;
            }
        };
        let prop = unsafe {
            xcb_dl_util::property::get_property::<u32>(
                &self.instance.backend.xcb,
                &self.c.errors,
                window,
                self.instance.atoms.net_wm_icon,
                ffi::XCB_ATOM_CARDINAL,
                false,
                10000,
            )
        };
        let mut unset = || {
            log::info!("NET_WM_ICON unset");
            *win.icon.borrow_mut() = None;
            win.upgade();
            data.changed();
        };
        let prop = match prop {
            Err(XcbGetPropertyError::Unset) => {
                unset();
                return;
            }
            Ok(p) if p.is_empty() => {
                unset();
                return;
            }
            Ok(p) => p,
            Err(e) => {
                log::warn!("Could not retrieve NET_WM_ICON property: {}", e);
                return;
            }
        };
        if prop.len() < 2 {
            log::warn!("NET_WM_ICON property has length < 2");
            return;
        }
        let width = prop[0];
        let height = prop[1];
        let prop = &prop[2..];
        if prop.len() != (width * height) as usize {
            log::warn!("NET_WM_ICON property invalid length");
            return;
        }
        let mut rgba = vec![];
        for &pixel in prop {
            rgba.push((pixel >> 16) as u8);
            rgba.push((pixel >> 8) as u8);
            rgba.push((pixel >> 0) as u8);
            rgba.push((pixel >> 24) as u8);
        }
        log::info!("NET_WM_ICON set to {}x{}", width, height);
        *win.icon.borrow_mut() = Some(BackendIcon {
            rgba,
            width,
            height,
        });
        win.upgade();
        data.changed();
    }

    fn handle_net_wm_name(&mut self, window: ffi::xcb_window_t) {
        if let Some(n) = self.handle_wm_name_(
            window,
            "net wm name",
            self.instance.atoms.net_wm_name,
            self.instance.atoms.utf8_string,
        ) {
            let mut data = self.instance.wm_data.lock();
            let win = match data.window(window) {
                Some(win) => win,
                None => return,
            };
            *win.utf8_title.borrow_mut() = n;
            win.upgade();
            data.changed();
        }
    }

    fn handle_wm_hints(&mut self, window: ffi::xcb_window_t) {
        let mut data = self.instance.wm_data.lock();
        let win = match data.window(window) {
            Some(win) => win,
            None => {
                return;
            }
        };
        let res = unsafe {
            xcb_dl_util::property::get_property::<u32>(
                &self.instance.backend.xcb,
                &self.c.errors,
                window,
                ffi::XCB_ATOM_WM_HINTS,
                ffi::XCB_ATOM_WM_HINTS,
                false,
                10000,
            )
        };
        let res = match res {
            Ok(res) => res,
            Err(e) => {
                log::warn!("Could not retrieve hints property: {}", e);
                return;
            }
        };
        let res = match XcbHints::try_from(&*res) {
            Ok(res) => res,
            Err(e) => {
                log::warn!("Could not covert hints property to normal hints: {}", e);
                return;
            }
        };
        win.urgency.set(res.flags.contains(XcbHintsFlags::URGENCY));
        log::info!("Hints updated for {}: {:?}", win.id, res);
        win.upgade();
        data.changed();
    }

    fn handle_wm_class(&mut self, window: ffi::xcb_window_t) {
        let mut data = self.instance.wm_data.lock();
        let win = match data.window(window) {
            Some(win) => win,
            None => {
                return;
            }
        };
        let res = unsafe {
            xcb_dl_util::property::get_property::<u8>(
                &self.instance.backend.xcb,
                &self.c.errors,
                window,
                ffi::XCB_ATOM_WM_CLASS,
                ffi::XCB_ATOM_STRING,
                false,
                10000,
            )
        };
        let res = match res {
            Ok(res) => res,
            Err(e) => {
                log::warn!("Could not retrieve WM_CLASS property: {}", e);
                return;
            }
        };
        let mut parts = res.split(|b| *b == 0);
        let instance = parts.next().and_then(|b| std::str::from_utf8(b).ok());
        let class = parts.next().and_then(|b| std::str::from_utf8(b).ok());
        *win.instance.borrow_mut() = instance.map(|s| s.to_owned());
        *win.class.borrow_mut() = class.map(|s| s.to_owned());
        log::info!("Class updated: {:?}", class);
        log::info!("Instance updated: {:?}", instance);
        win.upgade();
        data.changed();
    }

    fn handle_wm_protocols(&mut self, window: ffi::xcb_window_t) {
        let mut data = self.instance.wm_data.lock();
        let win = match data.window(window) {
            Some(win) => win,
            None => {
                return;
            }
        };
        let res = unsafe {
            xcb_dl_util::property::get_property::<u32>(
                &self.instance.backend.xcb,
                &self.c.errors,
                window,
                self.instance.atoms.wm_protocols,
                ffi::XCB_ATOM_ATOM,
                false,
                10000,
            )
        };
        let res = match res {
            Ok(res) => res,
            Err(e) => {
                log::warn!("Could not retrieve WM_PROTOCOLS property: {}", e);
                return;
            }
        };
        let mut protocols = Protocols::empty();
        for protocol in res {
            if protocol == self.instance.atoms.net_wm_ping {
                protocols |= Protocols::PING;
            } else if protocol == self.instance.atoms.wm_delete_window {
                protocols |= Protocols::DELETE_WINDOW;
            }
        }
        log::info!("WM_PROTOCOLS updated: {:?}", protocols);
        win.protocols.set(protocols);
        win.upgade();
        data.changed();
    }

    fn handle_wm_normal_hints(&mut self, window: ffi::xcb_window_t) {
        let mut data = self.instance.wm_data.lock();
        let win = match data.window(window) {
            Some(win) => win,
            None => {
                return;
            }
        };
        let res = unsafe {
            xcb_dl_util::property::get_property::<u32>(
                &self.instance.backend.xcb,
                &self.c.errors,
                window,
                ffi::XCB_ATOM_WM_NORMAL_HINTS,
                ffi::XCB_ATOM_WM_SIZE_HINTS,
                false,
                10000,
            )
        };
        let res = match res {
            Ok(res) => res,
            Err(e) => {
                log::warn!("Could not retrieve normal hints property: {}", e);
                return;
            }
        };
        let res = match XcbSizeHints::try_from(&*res) {
            Ok(res) => res,
            Err(e) => {
                log::warn!(
                    "Could not covert normal hints property to normal hints: {}",
                    e
                );
                return;
            }
        };
        if res.flags.contains(XcbSizeHintsFlags::P_MIN_SIZE) {
            win.min_size.set(Some((res.min_width, res.min_height)));
        } else {
            win.min_size.set(None);
        }
        if res.flags.contains(XcbSizeHintsFlags::P_MAX_SIZE) {
            win.max_size.set(Some((res.max_width, res.max_height)));
        } else {
            win.max_size.set(None);
        }
        log::info!("Normal hints updated for {}: {:?}", win.id, res);
        win.upgade();
        data.changed();
    }

    fn handle_wm_name(&mut self, window: ffi::xcb_window_t) {
        if let Some(n) = self.handle_wm_name_(
            window,
            "wm name",
            ffi::XCB_ATOM_WM_NAME,
            ffi::XCB_ATOM_STRING,
        ) {
            let mut data = self.instance.wm_data.lock();
            let win = match data.window(window) {
                Some(win) => win,
                None => return,
            };
            *win.wm_name.borrow_mut() = n;
            win.upgade();
            data.changed();
        }
    }

    fn handle_wm_name_(
        &mut self,
        window: ffi::xcb_window_t,
        atom_name: &str,
        atom: ffi::xcb_atom_t,
        ty: ffi::xcb_atom_t,
    ) -> Option<String> {
        let res = unsafe {
            xcb_dl_util::property::get_property::<u8>(
                &self.instance.backend.xcb,
                &self.c.errors,
                window,
                atom,
                ty,
                false,
                10000,
            )
        };
        let name = match res {
            Ok(h) => h,
            Err(e) => {
                log::warn!("Could not retrieve {}: {}", atom_name, e);
                return None;
            }
        };
        match String::from_utf8(name) {
            Ok(n) => Some(n),
            _ => {
                log::warn!("{} is not utf8", atom_name);
                None
            }
        }
    }

    fn handle_motif_wm_hints(&mut self, window: ffi::xcb_window_t) {
        let mut data = self.instance.wm_data.lock();
        let win = match data.window(window) {
            Some(win) => win,
            None => {
                return;
            }
        };
        let res = unsafe {
            xcb_dl_util::property::get_property::<u32>(
                &self.instance.backend.xcb,
                &self.c.errors,
                window,
                self.instance.atoms.motif_wm_hints,
                self.instance.atoms.motif_wm_hints,
                false,
                10000,
            )
        };
        let hints = match res {
            Ok(h) => h,
            Err(e) => {
                log::warn!("Could not retrieve motif wm hints: {}", e);
                return;
            }
        };
        if hints.len() < 5 {
            log::warn!("Motif hints property is too small");
            return;
        }
        const MWM_HINTS_FUNCTIONS: u32 = 1 << 0;
        // const MWM_HINTS_DECORATIONS: u32 = 1 << 1;
        const MWM_FUNC_ALL: u32 = 1 << 0;
        // const MWM_FUNC_RESIZE: u32 = 1 << 1;
        // const MWM_FUNC_MOVE: u32 = 1 << 2;
        // const MWM_FUNC_MINIMIZE: u32 = 1 << 3;
        const MWM_FUNC_MAXIMIZE: u32 = 1 << 4;
        // const MWM_FUNC_CLOSE: u32 = 1 << 5;
        let flags = hints[0];
        let functions = hints[1];
        let decorations = hints[2];
        win.decorations.set(flags & 2 == 0 || decorations != 0);
        win.maximizable.set(
            flags & MWM_HINTS_FUNCTIONS == 0
                || (functions & MWM_FUNC_ALL != 0) != (functions & MWM_FUNC_MAXIMIZE != 0),
        );
        win.upgade();
        data.changed();
    }

    fn handle_motion_notify(&mut self, event: &ffi::xcb_generic_event_t) {
        let event = unsafe { &*(event as *const _ as *const ffi::xcb_motion_notify_event_t) };
        log::info!("Got motion event: {:?}", event);
        let moving = match &self.moving {
            Some(win) => win,
            _ => return,
        };
        let win = match moving.win.upgrade() {
            Some(win) => win,
            _ => return,
        };
        unsafe {
            let list = ffi::xcb_configure_window_value_list_t {
                x: (event.root_x as i32 - moving.start_pointer_x) + moving.start_window_x,
                y: (event.root_y as i32 - moving.start_pointer_y) + moving.start_window_y,
                ..Default::default()
            };
            let xcb = &self.instance.backend.xcb;
            let cookie = xcb.xcb_configure_window_aux_checked(
                self.c.c,
                win.parent_id.get(),
                (ffi::XCB_CONFIG_WINDOW_X | ffi::XCB_CONFIG_WINDOW_Y) as _,
                &list,
            );
            let error = self.c.errors.check_cookie(xcb, cookie);
            if let Err(e) = error {
                log::warn!("Could not drag parent window: {}", e);
            }
            win.x_to_be.set(list.x);
            win.y_to_be.set(list.y);
        }
    }

    fn handle_button_release(&mut self, event: &ffi::xcb_generic_event_t) {
        let event = unsafe { &*(event as *const _ as *const ffi::xcb_button_release_event_t) };
        log::info!("Got button release event: {:?}", event);
        if event.detail != 1 {
            return;
        }
        let mut data = self.instance.wm_data.lock();
        let moving = match self.moving.take() {
            Some(win) => win,
            _ => return,
        };
        let win = match moving.win.upgrade() {
            Some(win) => win,
            _ => return,
        };
        win.dragging.set(false);
        win.upgade();
        data.changed();
        unsafe {
            let xcb = &self.instance.backend.xcb;
            let cookie = xcb.xcb_ungrab_pointer_checked(self.c.c, 0);
            let error = self.c.errors.check_cookie(xcb, cookie);
            if let Err(e) = error {
                log::warn!("Could not ungrab pointer: {}", e);
            }
        }
    }

    fn handle_configure_notify(&mut self, event: &ffi::xcb_generic_event_t) {
        let event = unsafe { &*(event as *const _ as *const ffi::xcb_configure_notify_event_t) };
        let mut data = self.instance.wm_data.lock();
        log::info!(
            "Window {} configured: {}x{} + {}x{} (border: {})",
            event.window,
            event.x,
            event.y,
            event.width,
            event.height,
            event.border_width,
        );
        if let Some(win) = data.window(event.window) {
            win.width.set(event.width as _);
            win.height.set(event.height as _);
            win.upgade();
            data.changed();
        } else if let Some(win) = data.parent(event.window) {
            unsafe {
                let event = ffi::xcb_configure_notify_event_t {
                    event: win.id,
                    window: win.id,
                    x: event.x + event.border_width as i16,
                    y: event.y + (event.border_width + TITLE_HEIGHT) as i16,
                    width: event.width,
                    height: event.height - TITLE_HEIGHT,
                    border_width: 0,
                    ..*event
                };
                let xcb = &self.instance.backend.xcb;
                let cookie =
                    xcb.xcb_send_event_checked(self.c.c, 0, win.id, 0, &event as *const _ as _);
                if let Err(e) = self.c.errors.check_cookie(xcb, cookie) {
                    log::warn!("Could not send configure event to child: {}", e);
                }
            }
            win.x.set(event.x as _);
            win.y.set(event.y as _);
            win.border.set(event.border_width as _);
            win.upgade();
            data.changed();
        }
    }

    fn handle_configure_request(&mut self, event: &ffi::xcb_generic_event_t) {
        let event = unsafe { &*(event as *const _ as *const ffi::xcb_configure_request_event_t) };
        log::info!(
            "Window {} configure request: {}x{} + {}x{} ({:b})",
            event.window,
            event.x,
            event.y,
            event.width,
            event.height,
            event.value_mask,
        );
        let data = self.instance.wm_data.lock();
        let mut list = ffi::xcb_configure_window_value_list_t {
            x: event.x as _,
            y: event.y as _,
            width: event.width as _,
            height: event.height as _,
            border_width: event.border_width as _,
            sibling: event.sibling as _,
            stack_mode: event.stack_mode as _,
        };
        let xcb = &self.instance.backend.xcb;
        let win = match data.window(event.window) {
            Some(w) => w,
            _ => unsafe {
                let cookie = xcb.xcb_configure_window_aux_checked(
                    self.c.c,
                    event.window,
                    event.value_mask,
                    &list,
                );
                if let Err(e) = self.c.errors.check_cookie(xcb, cookie) {
                    log::warn!("Could not configure freestanding window: {}", e);
                }
                return;
            },
        };
        unsafe {
            list.height += TITLE_HEIGHT as u32;
            let cookie = xcb.xcb_configure_window_aux_checked(
                self.c.c,
                win.parent_id.get(),
                event.value_mask,
                &list,
            );
            let error = self.c.errors.check_cookie(xcb, cookie);
            if let Err(e) = error {
                log::warn!("Could not configure parent window: {}", e);
            }
            let list = ffi::xcb_configure_window_value_list_t {
                width: event.width as _,
                height: event.height as _,
                ..Default::default()
            };
            let cookie = xcb.xcb_configure_window_aux_checked(
                self.c.c,
                event.window,
                event.value_mask
                    & (ffi::XCB_CONFIG_WINDOW_WIDTH | ffi::XCB_CONFIG_WINDOW_HEIGHT) as u16,
                &list,
            );
            let error = self.c.errors.check_cookie(xcb, cookie);
            if let Err(e) = error {
                log::warn!("Could not configure window: {}", e);
            }
            if event.value_mask & ffi::XCB_CONFIG_WINDOW_X as u16 != 0 {
                win.x_to_be.set(event.x as _);
            }
            if event.value_mask & ffi::XCB_CONFIG_WINDOW_Y as u16 != 0 {
                win.y_to_be.set(event.y as _);
            }
            if event.value_mask & ffi::XCB_CONFIG_WINDOW_WIDTH as u16 != 0 {
                win.width_to_be.set(event.width as _);
            }
            if event.value_mask & ffi::XCB_CONFIG_WINDOW_HEIGHT as u16 != 0 {
                win.height_to_be.set(event.width as _);
            }
            if event.value_mask & ffi::XCB_CONFIG_WINDOW_BORDER_WIDTH as u16 != 0 {
                win.border_to_be.set(event.border_width as _);
            }
        }
    }

    fn handle_map_request(&mut self, event: &ffi::xcb_generic_event_t) {
        let event = unsafe { &*(event as *const _ as *const ffi::xcb_map_request_event_t) };
        log::info!("Map request: {}", event.window);
        let data = self.instance.wm_data.lock();
        let win = match data.window(event.window) {
            Some(w) => w,
            _ => return,
        };
        win.desired_state.set(WindowState::Normal);
        unsafe {
            for w in [win.parent_id.get(), event.window] {
                let cookie = self
                    .instance
                    .backend
                    .xcb
                    .xcb_map_window_checked(self.c.c, w);
                let error = self
                    .c
                    .errors
                    .check_cookie(&self.instance.backend.xcb, cookie);
                if let Err(e) = error {
                    log::warn!("Could not map window: {}", e);
                }
            }
        }
    }

    fn handle_map_notify(&mut self, event: &ffi::xcb_generic_event_t) {
        let event = unsafe { &*(event as *const _ as *const ffi::xcb_map_notify_event_t) };
        log::info!("Window mapped: {}", event.window);
        let mut data = self.instance.wm_data.lock();
        if let Some(win) = data.window(event.window) {
            win.current_state.set(WindowState::Normal);
            win.update_wm_state(&self.c);
            if win.desired_state.get() != WindowState::Normal {
                unsafe {
                    self.instance.backend.xcb.xcb_unmap_window(self.c.c, win.id);
                }
            }
            win.mapped.set(true);
            win.upgade();
            data.changed();
        }
    }

    fn handle_unmap_notify(&mut self, event: &ffi::xcb_generic_event_t) {
        let event = unsafe { &*(event as *const _ as *const ffi::xcb_unmap_notify_event_t) };
        log::info!("Window unmapped: {}", event.window);
        let mut data = self.instance.wm_data.lock();
        if let Some(win) = data.window(event.window) {
            if win.desired_state.get() == WindowState::Iconic {
                win.current_state.set(WindowState::Iconic);
            } else {
                win.current_state.set(WindowState::Withdrawn);
            }
            win.update_wm_state(&self.c);
            win.mapped.set(false);
            win.upgade();
            data.changed();
        }
    }

    fn handle_net_wm_moveresize(&mut self, event: &ffi::xcb_client_message_event_t) {
        let mut data = self.instance.wm_data.lock();
        let data32 = unsafe { event.data.data32 };
        let win = match data.window(event.window) {
            Some(w) => w,
            _ => return,
        };
        let x_root = data32[0];
        let y_root = data32[1];
        let direction = data32[2];
        if direction != 8 {
            return;
        }
        unsafe {
            let xcb = &self.instance.backend.xcb;
            let mut err = ptr::null_mut();
            let reply = xcb.xcb_grab_pointer_reply(
                self.c.c,
                xcb.xcb_grab_pointer(
                    self.c.c,
                    0,
                    self.c.screen.root,
                    (ffi::XCB_EVENT_MASK_BUTTON_RELEASE | ffi::XCB_EVENT_MASK_POINTER_MOTION) as _,
                    1,
                    1,
                    0,
                    0,
                    0,
                ),
                &mut err,
            );
            let reply = match self.c.errors.check(xcb, reply, err) {
                Ok(r) => r,
                Err(e) => {
                    log::warn!("Could not grab the pointer: {}", e);
                    return;
                }
            };
            if reply.status != 0 {
                log::warn!("Could not grab the pointer: status: {}", reply.status);
                return;
            }
            log::info!("Grabbed pointer");
        }
        win.dragging.set(true);
        win.upgade();
        data.changed();
        assert!(self.moving.is_none());
        self.moving = Some(Moving {
            start_pointer_x: x_root as i32,
            start_pointer_y: y_root as i32,
            start_window_x: win.x.get(),
            start_window_y: win.y.get(),
            win: Arc::downgrade(&win),
        });
    }

    fn handle_wm_change_state(&mut self, event: &ffi::xcb_client_message_event_t) {
        let mut data = self.instance.wm_data.lock();
        let data32 = unsafe { event.data.data32 };
        let win = match data.window(event.window) {
            Some(w) => w,
            _ => return,
        };
        if data32[0] == 3 {
            win.desired_state.set(WindowState::Iconic);
            if win.mapped.get() {
                unsafe {
                    self.instance.backend.xcb.xcb_unmap_window(self.c.c, win.id);
                }
            }
        }
        win.upgade();
        data.changed();
    }

    fn handle_create_notify(&mut self, event: &ffi::xcb_generic_event_t) {
        let event = unsafe { &*(event as *const _ as *const ffi::xcb_create_notify_event_t) };
        log::info!(
            "Window created: {}: {}x{} + {}x{}",
            event.window,
            event.x,
            event.y,
            event.width,
            event.height
        );
        let mut data = self.instance.wm_data.lock();
        let win = match data.window(event.window) {
            Some(win) => win,
            _ => return,
        };
        let c = self.c.c;
        let xcb = &self.instance.backend.xcb;
        unsafe {
            win.parent_id.set(xcb.xcb_generate_id(c));
            let em =
                ffi::XCB_EVENT_MASK_SUBSTRUCTURE_NOTIFY | ffi::XCB_EVENT_MASK_SUBSTRUCTURE_REDIRECT;
            let cookie = xcb.xcb_create_window_checked(
                c,
                self.c.screen.root_depth,
                win.parent_id.get(),
                self.c.screen.root,
                event.x,
                event.y,
                event.width,
                event.height + TITLE_HEIGHT,
                event.border_width,
                ffi::XCB_WINDOW_CLASS_INPUT_OUTPUT as _,
                self.c.screen.root_visual,
                ffi::XCB_CW_EVENT_MASK,
                &em as *const _ as _,
            );
            if let Err(e) = self.c.errors.check_cookie(xcb, cookie) {
                log::error!("Could not create parent window: {}", e);
                return;
            }
            log::info!("Reparenting {} under {}", event.window, win.parent_id.get());
            let cookie = xcb.xcb_reparent_window_checked(
                c,
                event.window,
                win.parent_id.get(),
                0,
                TITLE_HEIGHT as i16,
            );
            if let Err(e) = self.c.errors.check_cookie(xcb, cookie) {
                log::error!("Could not reparent window: {}", e);
                return;
            }
            let events = ffi::XCB_EVENT_MASK_PROPERTY_CHANGE;
            let cookie = xcb.xcb_change_window_attributes_checked(
                c,
                event.window,
                ffi::XCB_CW_EVENT_MASK,
                &events as *const _ as _,
            );
            if let Err(e) = self.c.errors.check_cookie(xcb, cookie) {
                log::warn!("Could not select events on window {}: {}", event.window, e);
            }
            data.parents
                .insert(win.parent_id.get(), Arc::downgrade(&win));
            data.window_to_parent.insert(win.id, win.parent_id.get());
            drop(data);
            self.handle_wm_name(event.window);
            self.handle_net_wm_name(event.window);
            self.handle_net_wm_icon(event.window);
            self.handle_motif_wm_hints(event.window);
            self.handle_wm_normal_hints(event.window);
            self.handle_wm_hints(event.window);
            self.handle_wm_class(event.window);
            self.handle_wm_protocols(event.window);
        }
        win.x.set(event.x as _);
        win.y.set(event.y as _);
        win.border.set(event.border_width as _);
        win.width.set(event.width as _);
        win.height.set(event.height as _);
        win.x_to_be.set(event.x as _);
        win.y_to_be.set(event.y as _);
        win.border_to_be.set(event.border_width as _);
        win.width_to_be.set(event.width as _);
        win.height_to_be.set(event.height as _);
        win.created.set(true);
        win.upgade();
        self.instance.wm_data.lock().changed();
        self.update_client_list();
    }

    fn update_client_list(&mut self) {
        let data = self.instance.wm_data.lock();
        let mut windows = vec![];
        for win in data.windows.values() {
            if let Some(win) = win.upgrade() {
                if win.created.get() && !win.destroyed.get() {
                    windows.push(win.id);
                }
            }
        }
        unsafe {
            let cookie = self.instance.backend.xcb.xcb_change_property_checked(
                self.c.c,
                ffi::XCB_PROP_MODE_REPLACE as _,
                self.c.screen.root,
                self.instance.atoms.net_client_list,
                ffi::XCB_ATOM_WINDOW,
                32,
                windows.len() as _,
                windows.as_ptr() as *const _,
            );
            if let Err(e) = self
                .c
                .errors
                .check_cookie(&self.instance.backend.xcb, cookie)
            {
                log::warn!("Could not update _NET_CLIENT_LIST: {}", e);
            }
        }
    }

    fn handle_reparent_notify(&mut self, event: &ffi::xcb_generic_event_t) {
        let event = unsafe { &*(event as *const _ as *const ffi::xcb_reparent_notify_event_t) };
        log::info!(
            "Window reparented: {} under {} at {}x{}",
            event.window,
            event.parent,
            event.x,
            event.y
        );
    }

    fn handle_destroy_notify(&mut self, event: &ffi::xcb_generic_event_t) {
        let event = unsafe { &*(event as *const _ as *const ffi::xcb_destroy_notify_event_t) };
        log::info!("Window destroyed: {}", event.window);
        let mut data = self.instance.wm_data.lock();
        if let Some(win) = data.window(event.window) {
            win.destroyed.set(true);
            win.upgade();
            data.changed();
        }
        if let Some(parent) = data.window_to_parent.remove(&event.window) {
            data.parents.remove(&parent);
            unsafe {
                let xcb = &self.instance.backend.xcb;
                let cookie = xcb.xcb_destroy_window_checked(self.c.c, parent);
                if let Err(e) = self.c.errors.check_cookie(xcb, cookie) {
                    log::warn!("Could not destroy parent: {}", e);
                }
            }
        }
        drop(data);
        self.update_client_list();
    }

    fn handle_client_message(&mut self, event: &ffi::xcb_generic_event_t) {
        let event = unsafe { &*(event as *const _ as *const ffi::xcb_client_message_event_t) };
        if event.type_ == self.instance.atoms.net_wm_state && event.format == 32 {
            log::warn!("NET_WM_STATE client message: {:?}", event);
            self.handle_net_wm_state(event);
        } else if event.type_ == self.instance.atoms.wm_protocols && event.format == 32 {
            log::warn!("NET_WM_PROTOCOLS client message: {:?}", event);
            self.handle_net_wm_protocols(event);
        // } else if event.type_ == self.instance.atoms.net_active_window && event.format == 32 {
        //     log::warn!("NET_ACTIVE_WINDOW client message: {:?}", event);
        //     self.handle_net_active_window(event);
        } else if event.type_ == self.instance.atoms.net_wm_moveresize && event.format == 32 {
            log::warn!("NET_WM_MOVERESIZE client message: {:?}", event);
            self.handle_net_wm_moveresize(event);
        } else if event.type_ == self.instance.atoms.wm_change_state && event.format == 32 {
            log::warn!("WM_CHANGE_STATE client message: {:?}", event);
            self.handle_wm_change_state(event);
        } else {
            log::warn!("Received unexpected client message: {:?}", event);
        }
    }

    // fn handle_net_active_window(&mut self, event: &ffi::xcb_client_message_event_t) {
    //     let mut data = self.instance.wm_data.lock();
    //     let win = match data.window(event.window) {
    //         Some(w) => w,
    //         _ => return,
    //     };
    //     if win.state.get() == WindowState::Iconic {
    //         win.state.set(WindowState::Normal);
    //         win.update_wm_state();
    //         unsafe {
    //             self.instance
    //                 .backend
    //                 .xcb
    //                 .xcb_map_window(self.instance.c, win.id);
    //         }
    //     }
    //     win.upgade();
    //     data.changed();
    // }

    fn handle_net_wm_protocols(&mut self, event: &ffi::xcb_client_message_event_t) {
        let mut data = self.instance.wm_data.lock();
        let data32 = unsafe { event.data.data32 };
        if data32[0] == self.instance.atoms.net_wm_ping && event.window == self.c.screen.root {
            log::info!("Ponged {}", data32[2]);
            data.pongs.insert(data32[2]);
        }
        data.changed();
    }

    fn handle_net_wm_state(&mut self, event: &ffi::xcb_client_message_event_t) {
        let mut data = self.instance.wm_data.lock();
        let data32 = unsafe { event.data.data32 };
        let win = match data.window(event.window) {
            Some(w) => w,
            _ => return,
        };
        for property in [data32[1], data32[2]] {
            let (name, cell) = if property == self.instance.atoms.net_wm_state_above {
                ("always on top", &win.always_on_top)
            } else if property == self.instance.atoms.net_wm_state_maximized_vert {
                ("maximized vert", &win.maximized_vert)
            } else if property == self.instance.atoms.net_wm_state_maximized_horz {
                ("maximized horz", &win.maximized_horz)
            } else if property == self.instance.atoms.net_wm_state_fullscreen {
                ("fullscreen", &win.fullscreen)
            } else if property == 0 {
                continue;
            } else {
                log::warn!("Unknown _NET_WM_STATE property {}", property);
                continue;
            };
            let old = cell.get();
            match data32[0] {
                0 => cell.set(false),
                1 => cell.set(true),
                2 => cell.set(!cell.get()),
                _ => {
                    log::warn!("Unknown _NET_WM_STATE operation {}", data32[0]);
                    continue;
                }
            }
            if property == self.instance.atoms.net_wm_state_fullscreen {
                let xcb = &self.instance.backend.xcb;
                let (v1, v2) = if cell.get() {
                    if !old {
                        win.pre_fs_x.set(win.x_to_be.get());
                        win.pre_fs_y.set(win.y_to_be.get());
                        win.pre_fs_width.set(win.width_to_be.get());
                        win.pre_fs_height.set(win.height_to_be.get());
                        win.pre_fs_border.set(win.border_to_be.get());
                    }
                    let mut old_overlaps = false;
                    let mut x = self.crtcs[0].x;
                    let mut y = self.crtcs[0].y;
                    let mut width = self.crtcs[0].width;
                    let mut height = self.crtcs[0].height;
                    for crtc in &self.crtcs {
                        let overlaps = ((win.x_to_be.get() <= crtc.x
                            && win.x_to_be.get() + win.width_to_be.get() as i32 > crtc.x)
                            || (crtc.x <= win.x_to_be.get()
                                && crtc.x + crtc.width > win.x_to_be.get()))
                            && ((win.y_to_be.get() <= crtc.y
                                && win.y_to_be.get() + win.height_to_be.get() as i32 > crtc.y)
                                || (crtc.y <= win.y_to_be.get()
                                    && crtc.y + crtc.height > win.y_to_be.get()));
                        if overlaps && (!old_overlaps || (crtc.x, crtc.y) < (x, y)) {
                            x = crtc.x;
                            y = crtc.y;
                            width = crtc.width;
                            height = crtc.height;
                            old_overlaps = true;
                        }
                    }
                    ([x, y, width, height, 0], [0, 0, width, height])
                } else {
                    (
                        [
                            win.pre_fs_x.get(),
                            win.pre_fs_y.get(),
                            win.pre_fs_width.get() as i32,
                            (win.pre_fs_height.get() + TITLE_HEIGHT as u32) as i32,
                            win.pre_fs_border.get() as i32,
                        ],
                        [
                            0,
                            TITLE_HEIGHT as i32,
                            win.pre_fs_width.get() as i32,
                            win.pre_fs_height.get() as i32,
                        ],
                    )
                };
                unsafe {
                    log::info!("{:?}", v1);
                    let cookie = xcb.xcb_configure_window_checked(
                        self.c.c,
                        win.parent_id.get(),
                        (ffi::XCB_CONFIG_WINDOW_X
                            | ffi::XCB_CONFIG_WINDOW_Y
                            | ffi::XCB_CONFIG_WINDOW_WIDTH
                            | ffi::XCB_CONFIG_WINDOW_HEIGHT
                            | ffi::XCB_CONFIG_WINDOW_BORDER_WIDTH) as _,
                        v1.as_ptr() as _,
                    );
                    if let Err(e) = self.c.errors.check_cookie(xcb, cookie) {
                        log::warn!("Could not configure parent window: {}", e);
                    }
                    let cookie = xcb.xcb_configure_window_checked(
                        self.c.c,
                        win.id,
                        (ffi::XCB_CONFIG_WINDOW_X
                            | ffi::XCB_CONFIG_WINDOW_Y
                            | ffi::XCB_CONFIG_WINDOW_WIDTH
                            | ffi::XCB_CONFIG_WINDOW_HEIGHT) as _,
                        v2.as_ptr() as _,
                    );
                    if let Err(e) = self.c.errors.check_cookie(xcb, cookie) {
                        log::warn!("Could not configure window: {}", e);
                    }
                    win.x_to_be.set(v1[0] as _);
                    win.y_to_be.set(v1[1] as _);
                    win.width_to_be.set(v2[2] as _);
                    win.height_to_be.set(v2[3] as _);
                    win.border_to_be.set(v1[4] as _);
                }
            }
            log::info!("Window {} {}: {}", name, cell.get(), event.window);
        }
        win.upgade();
        data.changed();
    }
}

impl XWindow {
    fn update_wm_state(&self, c: &XConnection) {
        log::info!(
            "Updating WM_STATE of {} to {:?}",
            self.id,
            self.current_state.get()
        );
        unsafe {
            let state = match self.current_state.get() {
                WindowState::Withdrawn => 0u32,
                WindowState::Normal => 1,
                WindowState::Iconic => 3,
            };
            let instance = &self.el.data.instance.data;
            let xcb = &instance.backend.xcb;
            let cookie = xcb.xcb_change_property_checked(
                c.c,
                ffi::XCB_PROP_MODE_REPLACE as _,
                self.id,
                instance.atoms.wm_state,
                instance.atoms.wm_state,
                32,
                2,
                [state, 0].as_ptr() as _,
            );
            if let Err(e) = c.errors.check_cookie(xcb, cookie) {
                log::warn!("Could not update WM_STATE property: {}", e);
            }
        }
    }
}
