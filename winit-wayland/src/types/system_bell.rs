use sctk::globals::GlobalData;
use sctk::reexports::client::globals::{BindError, GlobalList};
use sctk::reexports::client::{Connection, Dispatch, Proxy, QueueHandle, delegate_dispatch};
use sctk::reexports::protocols::xdg::system_bell::v1::client::xdg_system_bell_v1::XdgSystemBellV1;

use crate::state::WinitState;

#[derive(Debug)]
pub struct SystemBellState {
    system_bell: XdgSystemBellV1,
}

impl SystemBellState {
    pub fn bind(
        globals: &GlobalList,
        queue_handle: &QueueHandle<WinitState>,
    ) -> Result<Self, BindError> {
        let system_bell = globals.bind(queue_handle, 1..=1, GlobalData)?;
        Ok(Self { system_bell })
    }

    pub fn global(&self) -> &XdgSystemBellV1 {
        &self.system_bell
    }
}

impl Dispatch<XdgSystemBellV1, GlobalData, WinitState> for SystemBellState {
    fn event(
        _: &mut WinitState,
        _: &XdgSystemBellV1,
        _: <XdgSystemBellV1 as Proxy>::Event,
        _: &GlobalData,
        _: &Connection,
        _: &QueueHandle<WinitState>,
    ) {
    }
}

delegate_dispatch!(WinitState: [XdgSystemBellV1: GlobalData] => SystemBellState);
