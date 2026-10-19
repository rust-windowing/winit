//! Touchpad pinch and rotation gestures from `zwp_pointer_gestures_v1`.
//!
//! The compositor reports these gestures only to clients that bind the protocol
//! and only while the pointer is over one of their surfaces, so without this a
//! trackpad pinch produces no input at all on Wayland. One gesture object
//! carries both the scale and the rotation of the pinch, feeding
//! [`WindowEvent::PinchGesture`] and [`WindowEvent::RotationGesture`].

use std::ops::Deref;
use std::sync::Mutex;

use sctk::globals::GlobalData;
use sctk::reexports::client::globals::{BindError, GlobalList};
use sctk::reexports::client::protocol::wl_pointer::WlPointer;
use sctk::reexports::client::{delegate_dispatch, Connection, Dispatch, Proxy, QueueHandle};
use sctk::reexports::protocols::wp::pointer_gestures::zv1::client::zwp_pointer_gesture_pinch_v1::{
    Event as PinchEvent, ZwpPointerGesturePinchV1,
};
use sctk::reexports::protocols::wp::pointer_gestures::zv1::client::zwp_pointer_gestures_v1::ZwpPointerGesturesV1;

use crate::event::{TouchPhase, WindowEvent};
use crate::platform_impl::wayland::state::WinitState;
use crate::platform_impl::wayland::{self, make_wid, WindowId};

/// The `zwp_pointer_gestures_v1` global, bound when the compositor has it.
pub struct PointerGesturesState {
    gestures: ZwpPointerGesturesV1,
}

impl PointerGesturesState {
    /// Bind the pointer gestures global.
    pub fn new(
        globals: &GlobalList,
        queue_handle: &QueueHandle<WinitState>,
    ) -> Result<Self, BindError> {
        // Version 1 already carries the pinch gesture; version 3 only adds the
        // hold gesture, so accept whatever the compositor implements.
        Ok(Self { gestures: globals.bind(queue_handle, 1..=3, GlobalData)? })
    }

    /// Report the pinch and rotation of `pointer` as window events.
    pub fn get_pinch_gesture(
        &self,
        pointer: &WlPointer,
        queue_handle: &QueueHandle<WinitState>,
    ) -> ZwpPointerGesturePinchV1 {
        self.gestures.get_pinch_gesture(pointer, queue_handle, PinchGestureData::default())
    }
}

impl Deref for PointerGesturesState {
    type Target = ZwpPointerGesturesV1;

    fn deref(&self) -> &Self::Target {
        &self.gestures
    }
}

/// The progress of the pinch gesture of one gesture object.
#[derive(Debug, Default)]
pub struct PinchGestureData {
    inner: Mutex<PinchGestureDataInner>,
}

#[derive(Debug)]
struct PinchGestureDataInner {
    /// The window the gesture started over, while a two-finger pinch runs.
    window_id: Option<WindowId>,

    /// The scale of the previous update: the protocol reports the scale
    /// relative to the beginning of the gesture.
    scale: f64,
}

impl Default for PinchGestureDataInner {
    fn default() -> Self {
        Self { window_id: None, scale: 1.0 }
    }
}

impl PinchGestureData {
    /// Start a gesture over `window_id`, unless it has fingers we don't handle.
    ///
    /// Any gesture tracked before is dropped: the compositor allows only one
    /// gesture at a time per pointer, so a new `begin` supersedes the old one.
    fn begin(&self, window_id: WindowId, fingers: u32) -> Option<(WindowId, TouchPhase)> {
        let mut inner = self.inner.lock().unwrap();
        *inner =
            PinchGestureDataInner { window_id: (fingers == 2).then_some(window_id), scale: 1.0 };

        inner.window_id.map(|window_id| (window_id, TouchPhase::Started))
    }

    /// The pinch and rotation of one update for the gesture in progress.
    ///
    /// `scale` is relative to the beginning of the gesture, so its delta is the
    /// change since the previous update. `rotation` is the clockwise change
    /// since the previous update, reported counterclockwise by winit. Updates
    /// of a gesture whose beginning we never saw are dropped.
    fn update(&self, scale: f64, rotation: f64) -> Option<(WindowId, f64, f32)> {
        let mut inner = self.inner.lock().unwrap();
        let window_id = inner.window_id?;

        // A compositor is not expected to send either, but a NaN scale would
        // poison every following delta of the gesture.
        if !scale.is_finite() {
            return None;
        }
        let pinch = scale - inner.scale;
        inner.scale = scale;
        let rotation = if rotation.is_finite() { -(rotation as f32) } else { 0.0 };

        Some((window_id, pinch, rotation))
    }

    /// End the gesture, cancelled or not.
    fn end(&self, cancelled: bool) -> Option<(WindowId, TouchPhase)> {
        let mut inner = self.inner.lock().unwrap();
        let window_id = inner.window_id.take()?;
        *inner = PinchGestureDataInner::default();

        let phase = if cancelled { TouchPhase::Cancelled } else { TouchPhase::Ended };
        Some((window_id, phase))
    }
}

impl Dispatch<ZwpPointerGesturesV1, GlobalData, WinitState> for PointerGesturesState {
    fn event(
        _state: &mut WinitState,
        _proxy: &ZwpPointerGesturesV1,
        _event: <ZwpPointerGesturesV1 as Proxy>::Event,
        _data: &GlobalData,
        _conn: &Connection,
        _qhandle: &QueueHandle<WinitState>,
    ) {
        // The global itself has no events.
    }
}

impl Dispatch<ZwpPointerGesturePinchV1, PinchGestureData, WinitState> for PointerGesturesState {
    fn event(
        state: &mut WinitState,
        _proxy: &ZwpPointerGesturePinchV1,
        event: PinchEvent,
        data: &PinchGestureData,
        _conn: &Connection,
        _qhandle: &QueueHandle<WinitState>,
    ) {
        let device_id =
            crate::event::DeviceId(crate::platform_impl::DeviceId::Wayland(wayland::DeviceId));

        let (window_id, phase, pinch, rotation) = match event {
            PinchEvent::Begin { surface, fingers, .. } => {
                // Only a window of our own takes part in the gesture: over a
                // subsurface, a decoration or another client's surface the
                // gesture is not ours to deliver.
                let window_id = make_wid(&surface);
                if !state.windows.get_mut().contains_key(&window_id) {
                    return;
                }

                let Some((window_id, phase)) = data.begin(window_id, fingers) else {
                    return;
                };
                (window_id, phase, 0.0, 0.0)
            },
            PinchEvent::Update { scale, rotation, .. } => {
                let Some((window_id, pinch, rotation)) = data.update(scale, rotation) else {
                    return;
                };
                (window_id, TouchPhase::Moved, pinch, rotation)
            },
            PinchEvent::End { cancelled, .. } => {
                let Some((window_id, phase)) = data.end(cancelled != 0) else {
                    return;
                };
                (window_id, phase, 0.0, 0.0)
            },
            _ => return,
        };

        state.events_sink.push_window_event(
            WindowEvent::PinchGesture { device_id, delta: pinch, phase },
            window_id,
        );
        state.events_sink.push_window_event(
            WindowEvent::RotationGesture { device_id, delta: rotation, phase },
            window_id,
        );
    }
}

delegate_dispatch!(WinitState: [ZwpPointerGesturesV1: GlobalData] => PointerGesturesState);
delegate_dispatch!(WinitState: [ZwpPointerGesturePinchV1: PinchGestureData] => PointerGesturesState);

#[cfg(test)]
mod tests {
    use super::*;

    fn window() -> WindowId {
        WindowId::from(1)
    }

    #[test]
    fn the_scale_delta_is_relative_to_the_previous_update() {
        let data = PinchGestureData::default();
        assert_eq!(data.begin(window(), 2), Some((window(), TouchPhase::Started)));

        let (_, delta, _) = data.update(1.05, 0.0).unwrap();
        assert!((delta - 0.05).abs() < f64::EPSILON);

        // The protocol's scale is absolute, so the second update reports the
        // change since the first, not since the beginning.
        let (_, delta, _) = data.update(1.1, 0.0).unwrap();
        assert!((delta - 0.05).abs() < 1e-9);

        // Pinching back to the start undoes the zoom.
        let (_, delta, _) = data.update(1.0, 0.0).unwrap();
        assert!((delta + 0.1).abs() < 1e-9);
    }

    #[test]
    fn the_rotation_delta_is_clockwise_reversed() {
        let data = PinchGestureData::default();
        data.begin(window(), 2).unwrap();

        // Wayland reports clockwise degrees, winit counterclockwise ones.
        let (_, _, rotation) = data.update(1.0, 5.0).unwrap();
        assert!((rotation - -5.0).abs() < f32::EPSILON);

        // Rotation is per update, so the deltas accumulate as the gesture goes.
        let (_, _, rotation) = data.update(1.0, 2.5).unwrap();
        assert!((rotation - -2.5).abs() < f32::EPSILON);
    }

    #[test]
    fn updates_without_a_two_finger_begin_are_dropped() {
        let data = PinchGestureData::default();
        assert_eq!(data.update(1.5, 1.0), None);

        // Three fingers are not a pinch winit reports.
        assert_eq!(data.begin(window(), 3), None);
        assert_eq!(data.update(1.5, 1.0), None);

        // A begin without fingers to track also supersedes a pinch in progress.
        data.begin(window(), 2).unwrap();
        assert_eq!(data.begin(window(), 3), None);
        assert_eq!(data.update(1.5, 1.0), None);
    }

    #[test]
    fn the_end_reports_the_phase_and_stops_the_gesture() {
        let data = PinchGestureData::default();
        assert_eq!(data.end(false), None);

        data.begin(window(), 2).unwrap();
        assert_eq!(data.end(false), Some((window(), TouchPhase::Ended)));
        assert_eq!(data.update(2.0, 0.0), None);
        assert_eq!(data.end(false), None);

        data.begin(window(), 2).unwrap();
        assert_eq!(data.end(true), Some((window(), TouchPhase::Cancelled)));
    }

    #[test]
    fn a_non_finite_update_is_dropped_and_leaves_the_scale_alone() {
        let data = PinchGestureData::default();
        data.begin(window(), 2).unwrap();

        assert_eq!(data.update(f64::NAN, f64::NAN), None);

        // The next sane update still measures from the last sane scale.
        let (_, delta, _) = data.update(1.1, 0.0).unwrap();
        assert!((delta - 0.1).abs() < 1e-9);
    }
}
