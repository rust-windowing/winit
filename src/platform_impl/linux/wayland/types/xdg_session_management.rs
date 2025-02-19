use std::collections::HashSet;

use sctk::globals::GlobalData;
use sctk::reexports::client::globals::{BindError, GlobalList};
use sctk::reexports::client::{delegate_dispatch, Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::xdg::shell::client::xdg_toplevel::XdgToplevel;

use super::super::xdg_session_management::{
    xx_session_manager_v1::{Reason, XxSessionManagerV1},
    xx_session_v1::XxSessionV1,
    xx_toplevel_session_v1::XxToplevelSessionV1,
};

use crate::platform_impl::wayland::state::WinitState;

#[derive(Debug, Clone)]
pub struct XdgSessionManager {
    session: XxSessionV1,
    names_in_use: HashSet<String>,
}

impl XdgSessionManager {
    pub fn new(
        globals: &GlobalList,
        queue_handle: &QueueHandle<WinitState>,
        session_id: Option<String>,
    ) -> Result<Self, BindError> {
        let session_id = session_id.or_else(|| std::env::var("REMOVE_THIS").ok());

        let manager: XxSessionManagerV1 = globals.bind(queue_handle, 1..=1, GlobalData)?;
        let default = manager.get_session(Reason::Launch, session_id, queue_handle, ());

        Ok(Self { session: default, names_in_use: HashSet::new() })
    }

    pub fn add_toplevel(
        &mut self,
        queue_handle: &QueueHandle<WinitState>,
        toplevel: &XdgToplevel,
        name: impl Into<String>,
    ) {
        let name = name.into();
        if self.names_in_use.insert(name.clone()) {
            self.session.add_toplevel(toplevel, name, queue_handle, ());
        }
    }

    pub fn restore_toplevel(
        &mut self,
        queue_handle: &QueueHandle<WinitState>,
        toplevel: &XdgToplevel,
        name: impl Into<String>,
    ) {
        let name = name.into();
        if self.names_in_use.insert(name.clone()) {
            self.session.restore_toplevel(toplevel, name, queue_handle, ());
        }
    }

    pub fn remove_toplevel(
        &mut self,
        _queue_handle: &QueueHandle<WinitState>,
        _toplevel: &XdgToplevel,
        name: impl Into<String>,
    ) {
        let name = name.into();
        self.names_in_use.remove(&name);
        // self.default.remove_toplevel(toplevel, name.into(), queue_handle, ());
    }
}

impl Dispatch<XxSessionManagerV1, GlobalData, WinitState> for XdgSessionManager {
    fn event(
        _: &mut WinitState,
        _: &XxSessionManagerV1,
        _: <XxSessionManagerV1 as Proxy>::Event,
        _: &GlobalData,
        _: &Connection,
        _: &QueueHandle<WinitState>,
    ) {
    }
}

impl Dispatch<XxSessionV1, (), WinitState> for XdgSessionManager {
    fn event(
        _: &mut WinitState,
        _: &XxSessionV1,
        event: <XxSessionV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<WinitState>,
    ) {
        dbg!(event);
        // TODO: Pass session_id to the user
    }
}

impl Dispatch<XxToplevelSessionV1, (), WinitState> for XdgSessionManager {
    fn event(
        _: &mut WinitState,
        _: &XxToplevelSessionV1,
        _: <XxToplevelSessionV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<WinitState>,
    ) {
    }
}

delegate_dispatch!(WinitState: [XxSessionManagerV1: GlobalData] => XdgSessionManager);
delegate_dispatch!(WinitState: [XxSessionV1: ()] => XdgSessionManager);
delegate_dispatch!(WinitState: [XxToplevelSessionV1: ()] => XdgSessionManager);
