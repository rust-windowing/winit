use std::sync::{Arc, Mutex};

use {ElementState, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent};
use events::ModifiersState;

use super::DeviceId;
use super::event_loop::EventsLoopSink;
use super::window::WindowStore;

use sctk::reexports::client::{NewProxy, Proxy};
use sctk::reexports::client::protocol::wl_pointer::{self, Event as PtrEvent, WlPointer};

pub fn implement_pointer(
    pointer: NewProxy<WlPointer>,
    sink: Arc<Mutex<EventsLoopSink>>,
    store: Arc<Mutex<WindowStore>>,
) -> Proxy<WlPointer> {
    let mut mouse_focus = None;
    let mut axis_buffer = None;
    let mut axis_discrete_buffer = None;
    let mut axis_state = TouchPhase::Ended;

    pointer.implement(move |evt, pointer: Proxy<_>| {
        let mut sink = sink.lock().unwrap();
        let store = store.lock().unwrap();
        match evt {
            PtrEvent::Enter {
                surface,
                surface_x,
                surface_y,
                ..
            } => {
                let wid = store.find_wid(&surface);
                if let Some(wid) = wid {
                    mouse_focus = Some(wid);
                    sink.send_event(
                        WindowEvent::CursorEntered {
                            device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                        },
                        wid,
                    );
                    sink.send_event(
                        WindowEvent::CursorMoved {
                            device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                            position: (surface_x, surface_y).into(),
                            // TODO: replace dummy value with actual modifier state
                            modifiers: ModifiersState::default(),
                        },
                        wid,
                    );
                }
            }
            PtrEvent::Leave { surface, .. } => {
                mouse_focus = None;
                let wid = store.find_wid(&surface);
                if let Some(wid) = wid {
                    sink.send_event(
                        WindowEvent::CursorLeft {
                            device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                        },
                        wid,
                    );
                }
            }
            PtrEvent::Motion {
                surface_x,
                surface_y,
                ..
            } => {
                if let Some(wid) = mouse_focus {
                    sink.send_event(
                        WindowEvent::CursorMoved {
                            device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                            position: (surface_x, surface_y).into(),
                            // TODO: replace dummy value with actual modifier state
                            modifiers: ModifiersState::default(),
                        },
                        wid,
                    );
                }
            }
            PtrEvent::Button { button, state, .. } => {
                if let Some(wid) = mouse_focus {
                    let state = match state {
                        wl_pointer::ButtonState::Pressed => ElementState::Pressed,
                        wl_pointer::ButtonState::Released => ElementState::Released,
                    };
                    let button = match button {
                        0x110 => MouseButton::Left,
                        0x111 => MouseButton::Right,
                        0x112 => MouseButton::Middle,
                        // TODO figure out the translation ?
                        _ => return,
                    };
                    sink.send_event(
                        WindowEvent::MouseInput {
                            device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                            state: state,
                            button: button,
                            // TODO: replace dummy value with actual modifier state
                            modifiers: ModifiersState::default(),
                        },
                        wid,
                    );
                }
            }
            PtrEvent::Axis { axis, value, .. } => {
                if let Some(wid) = mouse_focus {
                    if pointer.version() < 5 {
                        let (mut x, mut y) = (0.0, 0.0);
                        // old seat compatibility
                        match axis {
                            // wayland vertical sign convention is the inverse of winit
                            wl_pointer::Axis::VerticalScroll => y -= value as f32,
                            wl_pointer::Axis::HorizontalScroll => x += value as f32,
                        }
                        sink.send_event(
                            WindowEvent::MouseWheel {
                                device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                                delta: MouseScrollDelta::PixelDelta((x as f64, y as f64).into()),
                                phase: TouchPhase::Moved,
                                // TODO: replace dummy value with actual modifier state
                                modifiers: ModifiersState::default(),
                            },
                            wid,
                        );
                    } else {
                        let (mut x, mut y) = axis_buffer.unwrap_or((0.0, 0.0));
                        match axis {
                            // wayland vertical sign convention is the inverse of winit
                            wl_pointer::Axis::VerticalScroll => y -= value as f32,
                            wl_pointer::Axis::HorizontalScroll => x += value as f32,
                        }
                        axis_buffer = Some((x, y));
                        axis_state = match axis_state {
                            TouchPhase::Started | TouchPhase::Moved => TouchPhase::Moved,
                            _ => TouchPhase::Started,
                        }
                    }
                }
            }
            PtrEvent::Frame => {
                let axis_buffer = axis_buffer.take();
                let axis_discrete_buffer = axis_discrete_buffer.take();
                if let Some(wid) = mouse_focus {
                    if let Some((x, y)) = axis_discrete_buffer {
                        sink.send_event(
                            WindowEvent::MouseWheel {
                                device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                                delta: MouseScrollDelta::LineDelta(x as f32, y as f32),
                                phase: axis_state,
                                // TODO: replace dummy value with actual modifier state
                                modifiers: ModifiersState::default(),
                            },
                            wid,
                        );
                    } else if let Some((x, y)) = axis_buffer {
                        sink.send_event(
                            WindowEvent::MouseWheel {
                                device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                                delta: MouseScrollDelta::PixelDelta((x as f64, y as f64).into()),
                                phase: axis_state,
                                // TODO: replace dummy value with actual modifier state
                                modifiers: ModifiersState::default(),
                            },
                            wid,
                        );
                    }
                }
            }
            PtrEvent::AxisSource { .. } => (),
            PtrEvent::AxisStop { .. } => {
                axis_state = TouchPhase::Ended;
            }
            PtrEvent::AxisDiscrete { axis, discrete } => {
                let (mut x, mut y) = axis_discrete_buffer.unwrap_or((0, 0));
                match axis {
                    // wayland vertical sign convention is the inverse of winit
                    wl_pointer::Axis::VerticalScroll => y -= discrete,
                    wl_pointer::Axis::HorizontalScroll => x += discrete,
                }
                axis_discrete_buffer = Some((x, y));
                axis_state = match axis_state {
                    TouchPhase::Started | TouchPhase::Moved => TouchPhase::Moved,
                    _ => TouchPhase::Started,
                }
            }
        }
    })
}
