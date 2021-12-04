use super::XInstanceData;
use crate::backends::x11::XConnection;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::Arc;
use tokio::io::unix::AsyncFd;
use tokio::io::Interest;
use tokio::sync::mpsc::UnboundedReceiver;
use xcb_dl::ffi;
use xcb_dl_util::error::{XcbError, XcbErrorType};

#[derive(Debug, PartialEq)]
pub(super) enum DndMsg {
    Move(u32, u32),
    Drop,
    Cancel,
    Stop,
}

pub(super) fn run(
    instance: Arc<XInstanceData>,
    rx: UnboundedReceiver<DndMsg>,
    path: &Path,
) -> impl Future<Output = ()> {
    unsafe {
        let c = XConnection::new(&instance.backend, instance.display);
        let xcb = &instance.backend.xcb;
        let window_id = xcb.xcb_generate_id(c.c);
        let cookie = xcb.xcb_create_window_checked(
            c.c,
            0,
            window_id,
            c.screen.root,
            0,
            0,
            1,
            1,
            0,
            ffi::XCB_WINDOW_CLASS_INPUT_OUTPUT as _,
            0,
            0,
            ptr::null(),
        );
        if let Err(e) = c.errors.check_cookie(xcb, cookie) {
            panic!("Could not create dummy window: {}", e);
        }
        let cookie =
            xcb.xcb_set_selection_owner_checked(c.c, window_id, instance.atoms.x_dnd_selection, 0);
        if let Err(e) = c.errors.check_cookie(xcb, cookie) {
            panic!("Could not take ownership of XdndSelection: {}", e);
        }
        let cookie = xcb.xcb_change_property_checked(
            c.c,
            ffi::XCB_PROP_MODE_REPLACE as _,
            window_id,
            instance.atoms.x_dnd_type_list,
            ffi::XCB_ATOM_ATOM,
            32,
            1,
            &instance.atoms.uri_list as *const _ as _,
        );
        if let Err(e) = c.errors.check_cookie(xcb, cookie) {
            panic!("Could set XdndTypeList on {}: {}", window_id, e);
        }

        let dnd = Dnd {
            c,
            rx,
            instance,
            window_id,
            target: None,
            drop: false,
            dropped: false,
            accept: None,
            path: path.to_owned(),
            stop: false,
        };

        dnd.run()
    }
}

struct Dnd {
    c: XConnection,
    rx: UnboundedReceiver<DndMsg>,
    instance: Arc<XInstanceData>,
    window_id: ffi::xcb_window_t,
    target: Option<ffi::xcb_window_t>,
    drop: bool,
    dropped: bool,
    accept: Option<bool>,
    path: PathBuf,
    stop: bool,
}

const TIME: u32 = 99;

impl Dnd {
    async fn run(mut self) {
        let fd = AsyncFd::with_interest(self.c.fd, Interest::READABLE).unwrap();
        while !self.stop {
            self.handle_events();
            tokio::select! {
                guard = fd.readable() => {
                    guard.unwrap().clear_ready();
                }
                msg = self.rx.recv() => {
                    match msg {
                        Some(msg) => self.handle_msg(msg),
                        _ => return,
                    }
                }
            }
        }
    }

    fn handle_msg(&mut self, msg: DndMsg) {
        if self.drop && msg != DndMsg::Stop {
            panic!("Drop already set but received message");
        }
        match msg {
            DndMsg::Move(x, y) => self.handle_move(x, y),
            DndMsg::Drop => self.handle_drop(),
            DndMsg::Cancel => self.handle_cancel(),
            DndMsg::Stop => self.stop = true,
        }
    }

    fn handle_cancel(&mut self) {
        let window = match self.target {
            Some(w) => w,
            _ => return,
        };
        let e = self.send_msg(
            window,
            self.instance.atoms.x_dnd_leave,
            [self.window_id, 0, 0, 0, 0],
        );
        if let Err(e) = e {
            log::warn!("Could not send XdndLeave to {}: {}", window, e);
        }
        self.target = None;
    }

    fn handle_drop(&mut self) {
        self.drop = true;
        let window = match self.target {
            Some(w) => w,
            _ => {
                log::warn!("Dropping without a target window");
                return;
            }
        };
        if let Some(accept) = self.accept {
            let (ty, v2) = if accept {
                (self.instance.atoms.x_dnd_drop, TIME)
            } else {
                (self.instance.atoms.x_dnd_leave, 0)
            };
            let e = self.send_msg(window, ty, [self.window_id, 0, v2, 0, 0]);
            if let Err(e) = e {
                log::warn!(
                    "Could not send {} to {}: {}",
                    if accept { "XdndDrop" } else { "XdndLeave" },
                    window,
                    e
                );
                return;
            }
            self.dropped = true;
        }
    }

    fn send_msg(
        &self,
        window: ffi::xcb_window_t,
        type_: ffi::xcb_atom_t,
        data32: [u32; 5],
    ) -> Result<(), XcbError> {
        let xcb = &self.instance.backend.xcb;
        let msg = ffi::xcb_client_message_event_t {
            response_type: ffi::XCB_CLIENT_MESSAGE,
            format: 32,
            window,
            type_,
            data: ffi::xcb_client_message_data_t { data32 },
            ..Default::default()
        };
        unsafe {
            let cookie = xcb.xcb_send_event_checked(self.c.c, 0, window, 0, &msg as *const _ as _);
            self.c.errors.check_cookie(xcb, cookie)
        }
    }

    fn handle_move(&mut self, x: u32, y: u32) {
        unsafe {
            let xcb = &self.instance.backend.xcb;
            let mut window = self.c.screen.root;
            loop {
                let mut err = ptr::null_mut();
                let reply = xcb.xcb_translate_coordinates_reply(
                    self.c.c,
                    xcb.xcb_translate_coordinates(
                        self.c.c,
                        self.c.screen.root,
                        window,
                        x as i16,
                        y as i16,
                    ),
                    &mut err,
                );
                let reply = match self.c.errors.check(xcb, reply, err) {
                    Ok(r) => r,
                    Err(e) => {
                        log::warn!("Could not translate coordinates: {}", e);
                        return;
                    }
                };
                if reply.child == 0 {
                    break;
                }
                window = reply.child;
            }
            log::info!("Window at {}x{} is {}", x, y, window);
            if Some(window) != self.target {
                if let Some(prev) = self.target.take() {
                    let e = self.send_msg(
                        prev,
                        self.instance.atoms.x_dnd_leave,
                        [self.window_id, 0, 0, 0, 0],
                    );
                    if let Err(e) = e {
                        log::warn!("Could not send XdndLeave message to {}: {}", prev, e);
                    }
                }
                let version = xcb_dl_util::property::get_property::<u32>(
                    xcb,
                    &self.c.errors,
                    window,
                    self.instance.atoms.x_dnd_aware,
                    ffi::XCB_ATOM_ATOM,
                    false,
                    10000,
                );
                let version = match version {
                    Ok(v) => v,
                    Err(e) => {
                        log::warn!(
                            "Could not get value of XdndAware property of window {}: {}",
                            window,
                            e
                        );
                        return;
                    }
                };
                if version.len() != 1 {
                    log::warn!(
                        "XdndAware property of window {} has unexpected length: {}",
                        window,
                        version.len()
                    );
                    return;
                }
                let version = version[0];
                if version != 5 {
                    log::warn!(
                        "XdndAware property of window {} reports unexpected version: {}",
                        window,
                        version
                    );
                    return;
                }
                log::info!("Sending XdndEnter to {}", window);
                let e = self.send_msg(
                    window,
                    self.instance.atoms.x_dnd_enter,
                    [self.window_id, (5 << 24) | 1, 0, 0, 0],
                );
                if let Err(e) = e {
                    log::warn!("Could not send XdndEnter message to {}: {}", window, e);
                    return;
                }
                self.target = Some(window);
                self.accept = None;
            }
            log::info!("Sending XdndPosition {}x{} to {}", x, y, window);
            let e = self.send_msg(
                window,
                self.instance.atoms.x_dnd_position,
                [
                    self.window_id,
                    0,
                    (x << 16) | y,
                    TIME,
                    self.instance.atoms.x_dnd_action_copy,
                ],
            );
            if let Err(e) = e {
                log::warn!("Could not send XdndPosition message to {}: {}", window, e);
                return;
            }
            self.accept = None;
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
            ffi::XCB_SELECTION_REQUEST => self.handle_selection_request(event),
            ffi::XCB_CLIENT_MESSAGE => self.handle_client_message(event),
            _ => {
                log::warn!("Received unexpected event: {:?}", event);
            }
        }
    }

    fn handle_selection_request(&mut self, event: &ffi::xcb_generic_event_t) {
        let event = unsafe { &*(event as *const _ as *const ffi::xcb_selection_request_event_t) };
        log::info!("Got selection request: {:?}", event);
        if event.owner != self.window_id
            || event.selection != self.instance.atoms.x_dnd_selection
            || event.target != self.instance.atoms.uri_list
        {
            log::warn!("Received unexpected selection request: {:?}", event);
            return;
        }
        if event.time != TIME {
            log::warn!("Selection request has unexpected time: {}", event.time);
            return;
        }
        let property = if event.property == 0 {
            event.selection
        } else {
            event.property
        };
        unsafe {
            let path = format!("file://{}", self.path.display());
            let xcb = &self.instance.backend.xcb;
            let cookie = xcb.xcb_change_property_checked(
                self.c.c,
                ffi::XCB_PROP_MODE_REPLACE as _,
                event.requestor,
                property,
                self.instance.atoms.uri_list,
                8,
                path.len() as _,
                path.as_ptr() as _,
            );
            if let Err(e) = self.c.errors.check_cookie(xcb, cookie) {
                log::warn!("Could not set property on {}: {}", event.requestor, e);
                return;
            }
            let msg = ffi::xcb_selection_notify_event_t {
                response_type: ffi::XCB_SELECTION_NOTIFY,
                requestor: event.requestor,
                selection: event.selection,
                target: event.target,
                time: event.time,
                property,
                ..Default::default()
            };
            let cookie =
                xcb.xcb_send_event_checked(self.c.c, 0, event.requestor, 0, &msg as *const _ as _);
            if let Err(e) = self.c.errors.check_cookie(xcb, cookie) {
                log::warn!(
                    "Could not send selection notify to {}: {}",
                    event.requestor,
                    e
                );
            }
        }
    }

    fn handle_client_message(&mut self, event: &ffi::xcb_generic_event_t) {
        let event = unsafe { &*(event as *const _ as *const ffi::xcb_client_message_event_t) };
        if event.type_ != self.instance.atoms.x_dnd_status {
            log::warn!("Received unexpected client message: {:?}", event);
            return;
        }
        if self.dropped {
            log::warn!("Received client message after file was already dropped");
            return;
        }
        let data = unsafe { event.data.data32 };
        if Some(data[0]) != self.target {
            log::warn!(
                "Received client message from window other than the current target: {:?}",
                event
            );
            return;
        }
        let accept_was_none = self.accept.is_none();
        self.accept = Some(data[1] & 1 == 1);
        if data[4] != self.instance.atoms.x_dnd_action_copy
            && data[4] != self.instance.atoms.x_dnd_action_private
        {
            log::warn!("Unexpected dnd action: {}", data[4]);
        }
        if accept_was_none && self.drop {
            self.handle_drop();
        }
    }
}
