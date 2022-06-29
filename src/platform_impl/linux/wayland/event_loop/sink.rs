//! An event loop's sink to deliver events from the Wayland event callbacks.

use crate::event::{DeviceEvent, DeviceId as RootDeviceId, Event, WindowEvent};
use crate::platform_impl::platform::DeviceId as PlatformDeviceId;
use crate::window::WindowId as RootWindowId;

use super::{DeviceId, WindowId};

/// An event loop's sink to deliver events from the Wayland event callbacks
/// to the winit's user.
#[derive(Default)]
pub struct EventSink {
    pub window_events: Vec<Event<'static, ()>>,
}

impl EventSink {
    pub fn new() -> Self {
        Default::default()
    }

    /// Add new device event to a queue.
    pub fn push_device_event(&mut self, event: DeviceEvent, device_id: DeviceId) {
        self.window_events.push(Event::DeviceEvent {
            event,
            device_id: RootDeviceId(PlatformDeviceId::Wayland(device_id)),
        });
    }

    /// Add new window event to a queue.
    pub fn push_window_event(&mut self, event: WindowEvent<'static>, window_id: WindowId) {
        self.window_events.push(Event::WindowEvent {
            event,
            window_id: RootWindowId(window_id),
        });
    }
}
