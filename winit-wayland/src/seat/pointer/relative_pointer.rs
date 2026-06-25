//! Relative pointer.

use std::ops::Deref;

use sctk::reexports::client::globals::{BindError, GlobalList};
use sctk::reexports::client::{Connection, QueueHandle};
use sctk::reexports::client::{Dispatch};
use sctk::reexports::protocols::wp::relative_pointer::zv1::{
    client::zwp_relative_pointer_manager_v1::ZwpRelativePointerManagerV1,
    client::zwp_relative_pointer_v1::{self, ZwpRelativePointerV1},
};

use sctk::globals::GlobalData;

use crate::state::WinitState;
use winit_core::event::DeviceEvent;

/// Wrapper around the relative pointer.
#[derive(Debug)]
pub struct RelativePointerState {
    manager: ZwpRelativePointerManagerV1,
}

impl RelativePointerState {
    /// Create new relative pointer manager.
    pub fn new(
        globals: &GlobalList,
        queue_handle: &QueueHandle<WinitState>,
    ) -> Result<Self, BindError> {
        let manager = globals.bind_singleton(queue_handle, 1..=1, GlobalData)?;
        Ok(Self { manager })
    }
}

impl Deref for RelativePointerState {
    type Target = ZwpRelativePointerManagerV1;

    fn deref(&self) -> &Self::Target {
        &self.manager
    }
}

impl Dispatch<ZwpRelativePointerManagerV1, WinitState> for GlobalData {
    fn event(
        &self,
        _state: &mut WinitState,
        _proxy: &ZwpRelativePointerManagerV1,
        _event: <ZwpRelativePointerManagerV1 as wayland_client::Proxy>::Event,
        _conn: &Connection,
        _qhandle: &QueueHandle<WinitState>,
    ) {
    }
}

impl Dispatch<ZwpRelativePointerV1, WinitState> for GlobalData {
    fn event(
        &self,
        state: &mut WinitState,
        _proxy: &ZwpRelativePointerV1,
        event: <ZwpRelativePointerV1 as wayland_client::Proxy>::Event,
        _conn: &Connection,
        _qhandle: &QueueHandle<WinitState>,
    ) {
        let (dx_unaccel, dy_unaccel) = match event {
            zwp_relative_pointer_v1::Event::RelativeMotion { dx_unaccel, dy_unaccel, .. } => {
                (dx_unaccel, dy_unaccel)
            },
            _ => return,
        };
        state
            .events_sink
            .push_device_event(DeviceEvent::PointerMotion { delta: (dx_unaccel, dy_unaccel) });
    }
}
