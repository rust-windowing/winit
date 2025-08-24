use libc::dev_t;
use sctk::globals::GlobalData;
use wayland_client::globals::{BindError, GlobalList};
use wayland_client::{delegate_dispatch, Connection, Dispatch, QueueHandle};
use wayland_protocols::wp::linux_dmabuf::zv1::client::zwp_linux_dmabuf_feedback_v1::ZwpLinuxDmabufFeedbackV1;
use wayland_protocols::wp::linux_dmabuf::zv1::client::zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1;
use wayland_protocols::wp::linux_dmabuf::zv1::client::{
    zwp_linux_dmabuf_feedback_v1, zwp_linux_dmabuf_v1,
};

use crate::state::{ExtensionEvents, WinitState};

#[derive(Debug)]
pub struct LinuxDmabufManager {
    _manager: ZwpLinuxDmabufV1,
    feedback: LinuxDmabufFeedback,
}

#[derive(Debug)]
pub struct LinuxDmabufFeedback {
    _feedback: ZwpLinuxDmabufFeedbackV1,
    pending_device: Option<dev_t>,
}

impl LinuxDmabufManager {
    pub fn new(
        globals: &GlobalList,
        queue_handle: &QueueHandle<WinitState>,
    ) -> Result<Self, BindError> {
        let manager: ZwpLinuxDmabufV1 = globals.bind(queue_handle, 4..=5, GlobalData)?;
        let feedback = manager.get_default_feedback(queue_handle, GlobalData);
        let feedback = LinuxDmabufFeedback { _feedback: feedback, pending_device: None };
        Ok(Self { _manager: manager, feedback })
    }
}

impl Dispatch<ZwpLinuxDmabufV1, GlobalData, WinitState> for LinuxDmabufManager {
    fn event(
        _state: &mut WinitState,
        _proxy: &ZwpLinuxDmabufV1,
        _event: zwp_linux_dmabuf_v1::Event,
        _data: &GlobalData,
        _conn: &Connection,
        _qhandle: &QueueHandle<WinitState>,
    ) {
        // nothing
    }
}

impl Dispatch<ZwpLinuxDmabufFeedbackV1, GlobalData, WinitState> for LinuxDmabufManager {
    fn event(
        state: &mut WinitState,
        _proxy: &ZwpLinuxDmabufFeedbackV1,
        event: zwp_linux_dmabuf_feedback_v1::Event,
        _data: &GlobalData,
        _conn: &Connection,
        _qhandle: &QueueHandle<WinitState>,
    ) {
        use zwp_linux_dmabuf_feedback_v1::Event;
        match event {
            Event::Done => {
                let manager = state.linux_dmabuf_manager.as_mut().unwrap();
                if let Some(device) = manager.feedback.pending_device.take() {
                    state.extension_events.push(ExtensionEvents::LinuxMainDevice(device));
                }
            },
            Event::MainDevice { device } => {
                let dev = dev_t::from_ne_bytes(device.try_into().unwrap());
                state.linux_dmabuf_manager.as_mut().unwrap().feedback.pending_device = Some(dev);
            },
            _ => {},
        }
    }
}

delegate_dispatch!(WinitState: [ZwpLinuxDmabufV1: GlobalData] => LinuxDmabufManager);
delegate_dispatch!(WinitState: [ZwpLinuxDmabufFeedbackV1: GlobalData] => LinuxDmabufManager);
