use std::sync::{Arc, Mutex};

use {WindowEvent as Event, ElementState, MouseButton, MouseScrollDelta, TouchPhase};

use super::{WindowId, DeviceId};
use super::event_loop::EventsLoopSink;
use super::window::WindowStore;

use wayland_client::{Proxy, StateToken};
use wayland_client::protocol::wl_pointer;

pub struct PointerIData {
    sink: Arc<Mutex<EventsLoopSink>>,
    windows_token: StateToken<WindowStore>,
    mouse_focus: Option<WindowId>,
    axis_buffer: Option<(f32, f32)>,
    axis_discrete_buffer: Option<(i32, i32)>,
    axis_state: TouchPhase,
}

impl PointerIData {
    pub fn new(sink: &Arc<Mutex<EventsLoopSink>>, token: StateToken<WindowStore>)
        -> PointerIData
    {
        PointerIData {
            sink: sink.clone(),
            windows_token: token,
            mouse_focus: None,
            axis_buffer: None,
            axis_discrete_buffer: None,
            axis_state: TouchPhase::Cancelled
        }
    }
}

pub fn pointer_implementation() -> wl_pointer::Implementation<PointerIData> {
    wl_pointer::Implementation {
        enter: |evqh, idata, _, _, surface, x, y| {
            let wid = evqh.state().get(&idata.windows_token).find_wid(surface);
            if let Some(wid) = wid {
                idata.mouse_focus = Some(wid);
                let mut guard = idata.sink.lock().unwrap();
                guard.send_event(
                    Event::CursorEntered {
                        device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                    },
                    wid,
                );
                guard.send_event(
                    Event::CursorMoved {
                        device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                        position: (x, y),
                    },
                    wid,
                );
            }
        },
        leave: |evqh, idata, _, _, surface| {
            idata.mouse_focus = None;
            let wid = evqh.state().get(&idata.windows_token).find_wid(surface);
            if let Some(wid) = wid {
                let mut guard = idata.sink.lock().unwrap();
                guard.send_event(
                    Event::CursorLeft {
                        device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                    },
                    wid,
                );
            }
        },
        motion: |_, idata, _, _, x, y| {
            if let Some(wid) = idata.mouse_focus {
                idata.sink.lock().unwrap().send_event(
                    Event::CursorMoved {
                        device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                        position: (x, y)
                    },
                    wid
                );
            }
        },
        button: |_, idata, _, _, _, button, state| {
            if let Some(wid) = idata.mouse_focus {
                let state = match state {
                    wl_pointer::ButtonState::Pressed => ElementState::Pressed,
                    wl_pointer::ButtonState::Released => ElementState::Released
                };
                let button = match button {
                    0x110 => MouseButton::Left,
                    0x111 => MouseButton::Right,
                    0x112 => MouseButton::Middle,
                    // TODO figure out the translation ?
                    _ => return
                };
                idata.sink.lock().unwrap().send_event(
                    Event::MouseInput {
                        device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                        state: state,
                        button: button,
                    },
                    wid
                );
            }
        },
        axis: |_, idata, pointer, _, axis, value| {
            if let Some(wid) = idata.mouse_focus {
                if pointer.version() < 5 {
                    let (mut x, mut y) = (0.0, 0.0);
                    // old seat compatibility
                    match axis {
                        // wayland vertical sign convention is the inverse of winit
                        wl_pointer::Axis::VerticalScroll => y -= value as f32,
                        wl_pointer::Axis::HorizontalScroll => x += value as f32
                    }
                    idata.sink.lock().unwrap().send_event(
                        Event::MouseWheel {
                            device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                            delta: MouseScrollDelta::PixelDelta(x as f32, y as f32),
                            phase: TouchPhase::Moved,
                        },
                        wid
                    );
                } else {
                    let (mut x, mut y) = idata.axis_buffer.unwrap_or((0.0, 0.0));
                    match axis {
                        // wayland vertical sign convention is the inverse of winit
                        wl_pointer::Axis::VerticalScroll => y -= value as f32,
                        wl_pointer::Axis::HorizontalScroll => x += value as f32
                    }
                    idata.axis_buffer = Some((x,y));
                    idata.axis_state = match idata.axis_state {
                        TouchPhase::Started | TouchPhase::Moved => TouchPhase::Moved,
                        _ => TouchPhase::Started
                    }
                }
            }
        },
        frame: |_, idata, _| {
            let axis_buffer = idata.axis_buffer.take();
            let axis_discrete_buffer = idata.axis_discrete_buffer.take();
            if let Some(wid) = idata.mouse_focus {
                if let Some((x, y)) = axis_discrete_buffer {
                    idata.sink.lock().unwrap().send_event(
                        Event::MouseWheel {
                            device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                            delta: MouseScrollDelta::LineDelta(x as f32, y as f32),
                            phase: idata.axis_state,
                        },
                        wid
                    );
                } else if let Some((x, y)) = axis_buffer {
                    idata.sink.lock().unwrap().send_event(
                        Event::MouseWheel {
                            device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                            delta: MouseScrollDelta::PixelDelta(x as f32, y as f32),
                            phase: idata.axis_state,
                        },
                        wid
                    );
                }
            }
        },
        axis_source: |_, _, _, _| {},
        axis_stop: |_, idata, _, _, _| {
            idata.axis_state = TouchPhase::Ended;
        },
        axis_discrete: |_, idata, _, axis, discrete| {
            let (mut x, mut y) = idata.axis_discrete_buffer.unwrap_or((0,0));
            match axis {
                // wayland vertical sign convention is the inverse of winit
                wl_pointer::Axis::VerticalScroll => y -= discrete,
                wl_pointer::Axis::HorizontalScroll => x += discrete
            }
            idata.axis_discrete_buffer = Some((x,y));
            idata.axis_state = match idata.axis_state {
                TouchPhase::Started | TouchPhase::Moved => TouchPhase::Moved,
                _ => TouchPhase::Started
            }
        },
    }
}
