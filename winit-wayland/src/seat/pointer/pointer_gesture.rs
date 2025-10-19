use std::ops::Deref;
use std::sync::Mutex;

use dpi::{LogicalPosition, PhysicalPosition};
use sctk::compositor::SurfaceData;
use sctk::globals::GlobalData;
use sctk::reexports::client::globals::{BindError, GlobalList};
use sctk::reexports::client::{Connection, Dispatch, Proxy, QueueHandle, delegate_dispatch};
use sctk::reexports::protocols::wp::pointer_gestures::zv1::client::zwp_pointer_gesture_pinch_v1::{
    Event, ZwpPointerGesturePinchV1,
};
use sctk::reexports::protocols::wp::pointer_gestures::zv1::client::zwp_pointer_gestures_v1::ZwpPointerGesturesV1;
use winit_core::event::{TouchPhase, WindowEvent};
use winit_core::window::WindowId;

use crate::state::WinitState;

/// Wrapper around the pointer gesture.
#[derive(Debug)]
pub struct PointerGesturesState {
    pointer_gestures: ZwpPointerGesturesV1,
}

impl PointerGesturesState {
    /// Create a new pointer gesture
    pub fn new(
        globals: &GlobalList,
        queue_handle: &QueueHandle<WinitState>,
    ) -> Result<Self, BindError> {
        let pointer_gestures = globals.bind(queue_handle, 1..=1, GlobalData)?;
        Ok(Self { pointer_gestures })
    }
}

#[derive(Debug, Default)]
pub struct PointerGestureData {
    inner: Mutex<PointerGestureDataInner>,
}

#[derive(Debug)]
pub struct PointerGestureDataInner {
    window_id: Option<WindowId>,
    previous_pinch: f64,
}

impl Default for PointerGestureDataInner {
    fn default() -> Self {
        Self { window_id: Default::default(), previous_pinch: 1.0 }
    }
}

impl Deref for PointerGesturesState {
    type Target = ZwpPointerGesturesV1;

    fn deref(&self) -> &Self::Target {
        &self.pointer_gestures
    }
}

impl Dispatch<ZwpPointerGesturesV1, GlobalData, WinitState> for PointerGesturesState {
    fn event(
        _state: &mut WinitState,
        _proxy: &ZwpPointerGesturesV1,
        _event: <ZwpPointerGesturesV1 as wayland_client::Proxy>::Event,
        _data: &GlobalData,
        _conn: &Connection,
        _qhandle: &QueueHandle<WinitState>,
    ) {
        unreachable!("zwp_pointer_gestures_v1 has no events")
    }
}

impl Dispatch<ZwpPointerGesturePinchV1, PointerGestureData, WinitState> for PointerGesturesState {
    fn event(
        state: &mut WinitState,
        _proxy: &ZwpPointerGesturePinchV1,
        event: <ZwpPointerGesturePinchV1 as Proxy>::Event,
        data: &PointerGestureData,
        _conn: &Connection,
        _qhandle: &QueueHandle<WinitState>,
    ) {
        let mut pointer_gesture_data = data.inner.lock().unwrap();
        let (window_id, phase, pan_delta, pinch_delta, rotation_delta) = match event {
            Event::Begin { surface, fingers, .. } => {
                // We only support two fingers for now.
                if fingers != 2 {
                    return;
                }

                // Don't handle events from a subsurface.
                if surface.data::<SurfaceData>().is_none_or(|data| data.parent_surface().is_some())
                {
                    return;
                }

                let window_id = crate::make_wid(&surface);

                pointer_gesture_data.window_id = Some(window_id);
                pointer_gesture_data.previous_pinch = 1.;

                (window_id, TouchPhase::Started, PhysicalPosition::new(0., 0.), 0., 0.)
            },
            Event::Update { dx, dy, scale: pinch, rotation, .. } => {
                let window_id = match pointer_gesture_data.window_id {
                    Some(window_id) => window_id,
                    _ => return,
                };

                let scale_factor = match state.windows.get_mut().get_mut(&window_id) {
                    Some(window) => window.lock().unwrap().scale_factor(),
                    None => return,
                };

                let pan_delta =
                    LogicalPosition::new(dx as f32, dy as f32).to_physical(scale_factor);

                let pinch_delta = pinch - pointer_gesture_data.previous_pinch;
                pointer_gesture_data.previous_pinch = pinch;

                // Wayland provides rotation in degrees cw, opposite of winit's degrees ccw.
                let rotation_delta = -rotation as f32;
                (window_id, TouchPhase::Moved, pan_delta, pinch_delta, rotation_delta)
            },
            Event::End { cancelled, .. } => {
                let window_id = match pointer_gesture_data.window_id {
                    Some(window_id) => window_id,
                    _ => return,
                };

                // Reset the state.
                *pointer_gesture_data = Default::default();

                let phase = if cancelled == 0 { TouchPhase::Ended } else { TouchPhase::Cancelled };
                (window_id, phase, PhysicalPosition::new(0., 0.), 0., 0.)
            },
            _ => unreachable!("Unknown event {event:?}"),
        };

        // The chance of only one of these events being necessary is extremely small,
        // so it is easier to just send all three
        state.events_sink.push_window_event(
            WindowEvent::PanGesture { device_id: None, delta: pan_delta, phase },
            window_id,
        );
        state.events_sink.push_window_event(
            WindowEvent::PinchGesture { device_id: None, delta: pinch_delta, phase },
            window_id,
        );
        state.events_sink.push_window_event(
            WindowEvent::RotationGesture { device_id: None, delta: rotation_delta, phase },
            window_id,
        );
    }
}

delegate_dispatch!(WinitState: [ZwpPointerGesturesV1: GlobalData] => PointerGesturesState);
delegate_dispatch!(WinitState: [ZwpPointerGesturePinchV1: PointerGestureData] => PointerGesturesState);
