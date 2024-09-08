//! An event loop's sink to deliver events from the Wayland event callbacks.

use std::vec::Drain;

use crate::event::{DeviceEvent, Event, WindowEvent};
use crate::window::WindowId;

/// An event loop's sink to deliver events from the Wayland event callbacks
/// to the winit's user.
#[derive(Default)]
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
        self.window_events.push(Event::DeviceEvent { event, device_id: None });
    }

    /// Add new window event to a queue.
    #[inline]
    pub fn push_window_event(&mut self, event: WindowEvent, window_id: WindowId) {
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
