use std::ops::Deref;
use std::sync::Mutex;

use dpi::{LogicalPosition, PhysicalPosition};
use sctk::compositor::SurfaceData;
use sctk::globals::GlobalData;
use sctk::reexports::client::globals::{BindError, GlobalList};
use sctk::reexports::client::{delegate_dispatch, Connection, Dispatch, Proxy, QueueHandle};
use sctk::reexports::protocols::wp::pointer_gestures::zv1::client::zwp_pointer_gesture_pinch_v1::ZwpPointerGesturePinchV1;
use wayland_protocols::wp::pointer_gestures::zv1::client::zwp_pointer_gesture_pinch_v1::Event;
use wayland_protocols::wp::pointer_gestures::zv1::client::zwp_pointer_gestures_v1::ZwpPointerGesturesV1;
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

#[derive(Debug)]
pub struct PointerGestureData {
    window_id: Mutex<Option<WindowId>>,
    previous_scale: Mutex<f64>,
}

impl Default for PointerGestureData {
    fn default() -> Self {
        Self { window_id: Default::default(), previous_scale: Mutex::new(1.0) }
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
        let (window_id, phase, pan_delta, scale_delta, rotation_delta) = match event {
            Event::Begin { time: _, serial: _, surface, fingers } => {
                if fingers != 2 {
                    // We only support two fingers for now.
                    return;
                }
                // The parent surface.
                let parent_surface = match surface.data::<SurfaceData>() {
                    Some(data) => data.parent_surface().unwrap_or(&surface),
                    None => return,
                };
                let window_id = crate::make_wid(parent_surface);

                let pan_delta = PhysicalPosition::new(0., 0.);
                *data.window_id.lock().unwrap() = Some(window_id);
                *data.previous_scale.lock().unwrap() = 1.;

                (window_id, TouchPhase::Started, pan_delta, 0., 0.)
            },
            Event::Update { time: _, dx, dy, scale, rotation } => {
                let window_id = match *data.window_id.lock().unwrap() {
                    Some(window_id) => window_id,
                    None => return,
                };
                let window = match state.windows.get_mut().get_mut(&window_id) {
                    Some(window) => window.lock().unwrap(),
                    None => return,
                };

                let scale_factor = window.scale_factor();
                let pan_delta =
                    LogicalPosition::new(dx as f32, dy as f32).to_physical(scale_factor);
                let scale_delta = scale - *data.previous_scale.lock().unwrap();
                *data.previous_scale.lock().unwrap() = scale;
                (window_id, TouchPhase::Moved, pan_delta, scale_delta, -rotation as f32)
            },
            Event::End { time: _, serial: _, cancelled } => {
                let window_id = match *data.window_id.lock().unwrap() {
                    Some(window_id) => window_id,
                    None => return,
                };
                let pan_delta = PhysicalPosition::new(0., 0.);
                *data.previous_scale.lock().unwrap() = 1.;
                let phase = if cancelled == 0 { TouchPhase::Ended } else { TouchPhase::Cancelled };
                (window_id, phase, pan_delta, 0., 0.)
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
            WindowEvent::PinchGesture { device_id: None, delta: scale_delta, phase },
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
