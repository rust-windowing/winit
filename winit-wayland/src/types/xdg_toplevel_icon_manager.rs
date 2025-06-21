//! Handling of xdg toplevel icon manager, which is used for icon setting requests.

use crate::state::WinitState;
use sctk::globals::GlobalData;
use sctk::shm::slot::{Buffer, SlotPool};
use tracing::warn;
use wayland_client::globals::{BindError, GlobalList};
use wayland_client::protocol::wl_shm::Format;
use wayland_client::{delegate_dispatch, Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::xdg::toplevel_icon::v1::client::xdg_toplevel_icon_manager_v1::XdgToplevelIconManagerV1;
use wayland_protocols::xdg::toplevel_icon::v1::client::xdg_toplevel_icon_v1::XdgToplevelIconV1;
use winit_core::icon::{Icon, RgbaIcon};

#[derive(Debug)]
pub struct XdgToplevelIconManagerState {
    xdg_toplevel_icon_manager: XdgToplevelIconManagerV1,
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
    }
}

#[derive(Debug)]
pub enum ToplevelIconError {
    InvalidBuffer,
    Immutable,
    NoBuffer,
}

#[derive(Debug)]
pub struct ToplevelIcon {
    buffer: Buffer,
}

impl ToplevelIcon {
    pub fn new(icon: Icon, pool: &mut SlotPool) -> Result<Self, ToplevelIconError> {
        let buffer: Buffer;
        let canvas: &mut [u8];
        if let Some(icon) = icon.cast_ref::<RgbaIcon>() {
            (buffer, canvas) = pool
                .create_buffer(
                    icon.width() as i32,
                    icon.height() as i32,
                    4 * (icon.width() as i32),
                    Format::Argb8888,
                )
                .unwrap();

            for (canvas_chunk, rgba) in
                canvas.chunks_exact_mut(4).zip(icon.buffer().chunks_exact(4))
            {
                // Alpha in buffer is premultiplied.
                let alpha = rgba[3] as f32 / 255.;
                let r = (rgba[0] as f32 * alpha) as u32;
                let g = (rgba[1] as f32 * alpha) as u32;
                let b = (rgba[2] as f32 * alpha) as u32;
                let color = ((rgba[3] as u32) << 24) + (r << 16) + (g << 8) + b;
                let array: &mut [u8; 4] = canvas_chunk.try_into().unwrap();
                *array = color.to_le_bytes();
            }
        } else {
            warn!("invalid icon");
            return Err(ToplevelIconError::InvalidBuffer);
        }

        Ok(Self { buffer })
    }

    pub fn add_buffer(&self, xdg_toplevel_icon: &XdgToplevelIconV1) {
        xdg_toplevel_icon.add_buffer(self.buffer.wl_buffer(), 1);
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
        // No events
    }
}

delegate_dispatch!(WinitState: [XdgToplevelIconManagerV1: GlobalData] => XdgToplevelIconManagerState);
delegate_dispatch!(WinitState: [XdgToplevelIconV1: GlobalData] => XdgToplevelIconManagerState);
