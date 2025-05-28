//! An event loop's sink to deliver events from the Wayland event callbacks.

use std::vec::Drain;

use winit_core::event::{DeviceEvent, WindowEvent};
use winit_core::window::SurfaceId;

use super::Event;

/// An event loop's sink to deliver events from the Wayland event callbacks
/// to the winit's user.
#[derive(Default, Debug)]
pub struct EventSink {
    pub(crate) window_events: Vec<Event>,
}

impl EventSink {
    pub fn new() -> Self {
        Default::default()
    }

    /// Return `true` if there're pending events.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.window_events.is_empty()
    }

    /// Add new device event to a queue.
    #[inline]
    pub fn push_device_event(&mut self, event: DeviceEvent) {
        self.window_events.push(Event::DeviceEvent { event });
    }

    /// Add new window event to a queue.
    #[inline]
    pub fn push_window_event(&mut self, event: WindowEvent, window_id: SurfaceId) {
        self.window_events.push(Event::WindowEvent { event, window_id });
    }

    #[inline]
    pub fn append(&mut self, other: &mut Self) {
        self.window_events.append(&mut other.window_events);
    }

    #[inline]
    pub(crate) fn drain(&mut self) -> Drain<'_, Event> {
        self.window_events.drain(..)
    }
}
