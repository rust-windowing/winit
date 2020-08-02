use std::sync::{Arc, Mutex};

use crate::dpi::LogicalPosition;
use crate::event::{
    DeviceEvent, ElementState, ModifiersState, MouseButton, MouseScrollDelta, TouchPhase,
    WindowEvent,
};

use super::{
    event_loop::{CursorManager, EventsSink},
    make_wid,
    window::WindowStore,
    DeviceId,
};

use smithay_client_toolkit::surface;

use smithay_client_toolkit::reexports::client::protocol::{
    wl_pointer::{self, Event as PtrEvent, WlPointer},
    wl_seat,
};

use smithay_client_toolkit::reexports::protocols::unstable::relative_pointer::v1::client::{
    zwp_relative_pointer_manager_v1::ZwpRelativePointerManagerV1, zwp_relative_pointer_v1::Event,
    zwp_relative_pointer_v1::ZwpRelativePointerV1,
};

use smithay_client_toolkit::reexports::protocols::unstable::pointer_constraints::v1::client::{
    zwp_locked_pointer_v1::ZwpLockedPointerV1, zwp_pointer_constraints_v1::Lifetime,
    zwp_pointer_constraints_v1::ZwpPointerConstraintsV1,
};

use smithay_client_toolkit::reexports::client::protocol::wl_surface::WlSurface;

pub fn implement_pointer(
    seat: &wl_seat::WlSeat,
    sink: EventsSink,
    store: Arc<Mutex<WindowStore>>,
    modifiers_tracker: Arc<Mutex<ModifiersState>>,
    cursor_manager: Arc<Mutex<CursorManager>>,
) -> WlPointer {
    seat.get_pointer(|pointer| {
        // Currently focused winit surface
        let mut mouse_focus = None;
        let mut axis_buffer = None;
        let mut axis_discrete_buffer = None;
        let mut axis_state = TouchPhase::Ended;

        pointer.implement_closure(
            move |evt, pointer| {
                let store = store.lock().unwrap();
                let mut cursor_manager = cursor_manager.lock().unwrap();
                match evt {
                    PtrEvent::Enter {
                        surface,
                        surface_x,
                        surface_y,
                        ..
                    } => {
                        let wid = store.find_wid(&surface);

                        if let Some(wid) = wid {
                            let scale_factor = surface::get_dpi_factor(&surface) as f64;
                            mouse_focus = Some(surface);

                            // Reload cursor style only when we enter winit's surface. Calling
                            // this function every time on `PtrEvent::Enter` could interfere with
                            // SCTK CSD handling, since it changes cursor icons when you hover
                            // cursor over the window borders.
                            cursor_manager.reload_cursor_style();

                            sink.send_window_event(
                                WindowEvent::CursorEntered {
                                    device_id: crate::event::DeviceId(
                                        crate::platform_impl::DeviceId::Wayland(DeviceId),
                                    ),
                                },
                                wid,
                            );

                            let position = LogicalPosition::new(surface_x, surface_y)
                                .to_physical(scale_factor);

                            sink.send_window_event(
                                WindowEvent::CursorMoved {
                                    device_id: crate::event::DeviceId(
                                        crate::platform_impl::DeviceId::Wayland(DeviceId),
                                    ),
                                    position,
                                    modifiers: modifiers_tracker.lock().unwrap().clone(),
                                },
                                wid,
                            );
                        }
                    }
                    PtrEvent::Leave { surface, .. } => {
                        mouse_focus = None;
                        let wid = store.find_wid(&surface);
                        if let Some(wid) = wid {
                            sink.send_window_event(
                                WindowEvent::CursorLeft {
                                    device_id: crate::event::DeviceId(
                                        crate::platform_impl::DeviceId::Wayland(DeviceId),
                                    ),
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
                        if let Some(surface) = mouse_focus.as_ref() {
                            let wid = make_wid(surface);

                            let scale_factor = surface::get_dpi_factor(&surface) as f64;
                            let position = LogicalPosition::new(surface_x, surface_y)
                                .to_physical(scale_factor);

                            sink.send_window_event(
                                WindowEvent::CursorMoved {
                                    device_id: crate::event::DeviceId(
                                        crate::platform_impl::DeviceId::Wayland(DeviceId),
                                    ),
                                    position,
                                    modifiers: modifiers_tracker.lock().unwrap().clone(),
                                },
                                wid,
                            );
                        }
                    }
                    PtrEvent::Button { button, state, .. } => {
                        if let Some(surface) = mouse_focus.as_ref() {
                            let state = match state {
                                wl_pointer::ButtonState::Pressed => ElementState::Pressed,
                                wl_pointer::ButtonState::Released => ElementState::Released,
                                _ => unreachable!(),
                            };
                            let button = match button {
                                0x110 => MouseButton::Left,
                                0x111 => MouseButton::Right,
                                0x112 => MouseButton::Middle,
                                // TODO figure out the translation ?
                                _ => return,
                            };
                            sink.send_window_event(
                                WindowEvent::MouseInput {
                                    device_id: crate::event::DeviceId(
                                        crate::platform_impl::DeviceId::Wayland(DeviceId),
                                    ),
                                    state,
                                    button,
                                    modifiers: modifiers_tracker.lock().unwrap().clone(),
                                },
                                make_wid(surface),
                            );
                        }
                    }
                    PtrEvent::Axis { axis, value, .. } => {
                        if let Some(surface) = mouse_focus.as_ref() {
                            let wid = make_wid(surface);
                            if pointer.as_ref().version() < 5 {
                                let (mut x, mut y) = (0.0, 0.0);
                                // old seat compatibility
                                match axis {
                                    // wayland vertical sign convention is the inverse of winit
                                    wl_pointer::Axis::VerticalScroll => y -= value as f32,
                                    wl_pointer::Axis::HorizontalScroll => x += value as f32,
                                    _ => unreachable!(),
                                }
                                let scale_factor = surface::get_dpi_factor(&surface) as f64;
                                let delta = LogicalPosition::new(x as f64, y as f64)
                                    .to_physical(scale_factor);
                                sink.send_window_event(
                                    WindowEvent::MouseWheel {
                                        device_id: crate::event::DeviceId(
                                            crate::platform_impl::DeviceId::Wayland(DeviceId),
                                        ),
                                        delta: MouseScrollDelta::PixelDelta(delta),
                                        phase: TouchPhase::Moved,
                                        modifiers: modifiers_tracker.lock().unwrap().clone(),
                                    },
                                    wid,
                                );
                            } else {
                                let (mut x, mut y) = axis_buffer.unwrap_or((0.0, 0.0));
                                match axis {
                                    // wayland vertical sign convention is the inverse of winit
                                    wl_pointer::Axis::VerticalScroll => y -= value as f32,
                                    wl_pointer::Axis::HorizontalScroll => x += value as f32,
                                    _ => unreachable!(),
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
                        if let Some(surface) = mouse_focus.as_ref() {
                            let wid = make_wid(surface);
                            if let Some((x, y)) = axis_discrete_buffer {
                                sink.send_window_event(
                                    WindowEvent::MouseWheel {
                                        device_id: crate::event::DeviceId(
                                            crate::platform_impl::DeviceId::Wayland(DeviceId),
                                        ),
                                        delta: MouseScrollDelta::LineDelta(x, y),
                                        phase: axis_state,
                                        modifiers: modifiers_tracker.lock().unwrap().clone(),
                                    },
                                    wid,
                                );
                            } else if let Some((x, y)) = axis_buffer {
                                let scale_factor = surface::get_dpi_factor(&surface) as f64;
                                let delta = LogicalPosition::new(x, y).to_physical(scale_factor);
                                sink.send_window_event(
                                    WindowEvent::MouseWheel {
                                        device_id: crate::event::DeviceId(
                                            crate::platform_impl::DeviceId::Wayland(DeviceId),
                                        ),
                                        delta: MouseScrollDelta::PixelDelta(delta),
                                        phase: axis_state,
                                        modifiers: modifiers_tracker.lock().unwrap().clone(),
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
                        let (mut x, mut y) = axis_discrete_buffer.unwrap_or((0.0, 0.0));
                        match axis {
                            // wayland vertical sign convention is the inverse of winit
                            wl_pointer::Axis::VerticalScroll => y -= discrete as f32,
                            wl_pointer::Axis::HorizontalScroll => x += discrete as f32,
                            _ => unreachable!(),
                        }
                        axis_discrete_buffer = Some((x, y));
                        axis_state = match axis_state {
                            TouchPhase::Started | TouchPhase::Moved => TouchPhase::Moved,
                            _ => TouchPhase::Started,
                        }
                    }
                    _ => unreachable!(),
                }
            },
            (),
        )
    })
    .unwrap()
}

pub fn implement_relative_pointer(
    sink: EventsSink,
    pointer: &WlPointer,
    manager: &ZwpRelativePointerManagerV1,
) -> Result<ZwpRelativePointerV1, ()> {
    manager.get_relative_pointer(pointer, |rel_pointer| {
        rel_pointer.implement_closure(
            move |evt, _rel_pointer| match evt {
                Event::RelativeMotion { dx, dy, .. } => {
                    sink.send_device_event(DeviceEvent::MouseMotion { delta: (dx, dy) }, DeviceId)
                }
                _ => unreachable!(),
            },
            (),
        )
    })
}

pub fn implement_locked_pointer(
    surface: &WlSurface,
    pointer: &WlPointer,
    constraints: &ZwpPointerConstraintsV1,
) -> Result<ZwpLockedPointerV1, ()> {
    constraints.lock_pointer(surface, pointer, None, Lifetime::Persistent.to_raw(), |c| {
        c.implement_closure(|_, _| (), ())
    })
}
