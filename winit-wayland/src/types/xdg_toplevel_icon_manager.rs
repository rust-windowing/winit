//! Handling of xdg toplevel icon manager, which is used for icon setting requests.

use std::fmt;
use std::fmt::Formatter;

use sctk::globals::GlobalData;
use sctk::shm::slot::{Buffer, SlotPool};
use wayland_client::globals::{BindError, GlobalList};
use wayland_client::protocol::wl_shm::Format;
use wayland_client::{delegate_dispatch, Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::xdg::toplevel_icon::v1::client::xdg_toplevel_icon_manager_v1::XdgToplevelIconManagerV1;
use wayland_protocols::xdg::toplevel_icon::v1::client::xdg_toplevel_icon_v1::XdgToplevelIconV1;
use winit_core::icon::{Icon, RgbaIcon};

use crate::image_to_buffer;
use crate::state::WinitState;

#[derive(Debug)]
pub struct XdgToplevelIconManagerState {
    xdg_toplevel_icon_manager: XdgToplevelIconManagerV1,
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum ToplevelIconError {
    /// The icon's unsupported
    Unsupported,
}

#[derive(Debug)]
pub struct ToplevelIcon {
    buffer: Buffer,
}

impl ToplevelIcon {
    pub fn new(icon: Icon, pool: &mut SlotPool) -> Result<Self, ToplevelIconError> {
        let icon = match icon.cast_ref::<RgbaIcon>() {
            Some(icon) => icon,
            None => return Err(ToplevelIconError::Unsupported),
        };

        let buffer = image_to_buffer(
            icon.width() as i32,
            icon.height() as i32,
            icon.buffer(),
            Format::Argb8888,
            pool,
        )
        .unwrap();

        Ok(Self { buffer })
    }

    pub fn add_buffer(&self, xdg_toplevel_icon: &XdgToplevelIconV1) {
        xdg_toplevel_icon.add_buffer(self.buffer.wl_buffer(), 1);
    }
}

impl fmt::Display for ToplevelIconError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ToplevelIconError::Unsupported => write!(f, "this icon is unsupported on Wayland"),
        }
    }
}

impl XdgToplevelIconManagerState {
    pub fn bind(
        globals: &GlobalList,
        queue_handle: &QueueHandle<WinitState>,
    ) -> Result<Self, BindError> {
        let xdg_toplevel_icon_manager = globals.bind(queue_handle, 1..=1, GlobalData)?;
        Ok(Self { xdg_toplevel_icon_manager })
    }

    pub fn global(&self) -> &XdgToplevelIconManagerV1 {
        &self.xdg_toplevel_icon_manager
    }
}

impl Dispatch<XdgToplevelIconManagerV1, GlobalData, WinitState> for XdgToplevelIconManagerState {
    fn event(
        _state: &mut WinitState,
        _proxy: &XdgToplevelIconManagerV1,
        _event: <XdgToplevelIconManagerV1 as Proxy>::Event,
        _data: &GlobalData,
        _conn: &Connection,
        _qhandle: &QueueHandle<WinitState>,
    ) {
        // No events.
    }
}

impl Dispatch<XdgToplevelIconV1, GlobalData, WinitState> for XdgToplevelIconManagerState {
    fn event(
        _state: &mut WinitState,
        _proxy: &XdgToplevelIconV1,
        _event: <XdgToplevelIconV1 as Proxy>::Event,
        _data: &GlobalData,
        _conn: &Connection,
        _qhandle: &QueueHandle<WinitState>,
    ) {
        // No events.
    }
}

delegate_dispatch!(WinitState: [XdgToplevelIconManagerV1: GlobalData] => XdgToplevelIconManagerState);
delegate_dispatch!(WinitState: [XdgToplevelIconV1: GlobalData] => XdgToplevelIconManagerState);
