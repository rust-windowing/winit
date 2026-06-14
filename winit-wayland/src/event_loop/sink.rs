//! An event loop's sink to deliver events from the Wayland event callbacks.

use std::time::Instant;
use std::vec::Drain;

use winit_core::event::{DeviceEvent, WindowEvent};
use winit_core::window::WindowId;

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

    /// Add a new device event to the queue, stamped with the moment of the call.
    ///
    /// Call sites that have a real compositor timestamp should prefer
    /// [`push_device_event_at`](Self::push_device_event_at).
    #[inline]
    pub fn push_device_event(&mut self, event: DeviceEvent) {
        self.push_device_event_at(event, Instant::now());
    }

    /// Add a new device event to the queue with an explicit timestamp.
    #[inline]
    pub fn push_device_event_at(&mut self, event: DeviceEvent, timestamp: Instant) {
        self.window_events.push(Event::DeviceEvent { event, timestamp });
    }

    /// Add a new window event to the queue, stamped with the moment of the call.
    ///
    /// Call sites that have a real compositor timestamp should prefer
    /// [`push_window_event_at`](Self::push_window_event_at).
    #[inline]
    pub fn push_window_event(&mut self, event: WindowEvent, window_id: WindowId) {
        self.push_window_event_at(event, window_id, Instant::now());
    }

    /// Add a new window event to the queue with an explicit timestamp.
    #[inline]
    pub fn push_window_event_at(
        &mut self,
        event: WindowEvent,
        window_id: WindowId,
        timestamp: Instant,
    ) {
        self.window_events.push(Event::WindowEvent { event, window_id, timestamp });
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
