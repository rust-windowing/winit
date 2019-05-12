use std::convert::From;

use ::event::WindowEvent as WindowEvent;
use ::event::DeviceId as WDeviceId;
use ::event::{ElementState, MouseButton};

use ::wasm_bindgen::prelude::*;
use ::web_sys::MouseEvent;
use super::window::DeviceId;

impl From<MouseEvent> for WindowEvent {
    fn from(event: MouseEvent) -> Self {
        WindowEvent::MouseInput {
            device_id: WDeviceId(DeviceId::dummy()),
            state: ElementState::Pressed,
            button: MouseButton::Left,
            modifiers: Default::default()
        }
    }
}