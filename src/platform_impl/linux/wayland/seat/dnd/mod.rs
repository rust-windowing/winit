mod handlers;

use sctk::data_device::DataDevice;
use wayland_client::{
    protocol::{wl_data_device_manager::WlDataDeviceManager, wl_seat::WlSeat},
    Attached,
};

use crate::platform_impl::{wayland::event_loop::WinitState, WindowId};

pub(crate) struct Dnd {
    _data_device: DataDevice,
}

impl Dnd {
    pub fn new(seat: &Attached<WlSeat>, manager: &WlDataDeviceManager) -> Self {
        let mut inner = DndInner { window_id: None };
        let data_device =
            DataDevice::init_for_seat(manager, seat, move |event, mut dispatch_data| {
                let winit_state = dispatch_data.get::<WinitState>().unwrap();
                handlers::handle_dnd(event, &mut inner, winit_state);
            });
        Self {
            _data_device: data_device,
        }
    }
}

struct DndInner {
    /// Window ID of the currently hovered window.
    window_id: Option<WindowId>,
}
