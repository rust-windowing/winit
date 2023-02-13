//! Handlers for the pointers we're using.

use std::cell::RefCell;
use std::rc::Rc;

use sctk::reexports::client::protocol::wl_pointer::{self, Event as PointerEvent};
use sctk::reexports::client::protocol::wl_seat::WlSeat;
use sctk::reexports::protocols::unstable::relative_pointer::v1::client::zwp_relative_pointer_v1::Event as RelativePointerEvent;

use sctk::seat::pointer::ThemedPointer;

use crate::dpi::LogicalPosition;
use crate::event::{
    DeviceEvent, ElementState, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent,
};
use crate::platform_impl::wayland::event_loop::WinitState;
use crate::platform_impl::wayland::{self, DeviceId};

use super::{PointerData, WinitPointer};

// These values are comming from <linux/input-event-codes.h>.
const BTN_LEFT: u32 = 0x110;
const BTN_RIGHT: u32 = 0x111;
const BTN_MIDDLE: u32 = 0x112;

#[inline]
pub(super) fn handle_pointer(
    pointer: ThemedPointer,
    event: PointerEvent,
    pointer_data: &Rc<RefCell<PointerData>>,
    winit_state: &mut WinitState,
    seat: WlSeat,
) {
    let event_sink = &mut winit_state.event_sink;
    let mut pointer_data = pointer_data.borrow_mut();
    match event {
        PointerEvent::Enter {
            surface,
            surface_x,
            surface_y,
            serial,
            ..
        } => {
            pointer_data.latest_serial.replace(serial);
            pointer_data.latest_enter_serial.replace(serial);

            let window_id = wayland::make_wid(&surface);
            let window_handle = match winit_state.window_map.get_mut(&window_id) {
                Some(window_handle) => window_handle,
                None => return,
            };

            let scale_factor = window_handle.scale_factor();
            pointer_data.surface = Some(surface);

            // Notify window that pointer entered the surface.
            let winit_pointer = WinitPointer {
                pointer,
                confined_pointer: Rc::downgrade(&pointer_data.confined_pointer),
                locked_pointer: Rc::downgrade(&pointer_data.locked_pointer),
                pointer_constraints: pointer_data.pointer_constraints.clone(),
                latest_serial: pointer_data.latest_serial.clone(),
                latest_enter_serial: pointer_data.latest_enter_serial.clone(),
                seat,
            };
            window_handle.pointer_entered(winit_pointer);

            event_sink.push_window_event(
                WindowEvent::CursorEntered {
                    device_id: crate::event::DeviceId(crate::platform_impl::DeviceId::Wayland(
                        DeviceId,
                    )),
                },
                window_id,
            );

            let position = LogicalPosition::new(surface_x, surface_y).to_physical(scale_factor);

            event_sink.push_window_event(
                WindowEvent::CursorMoved {
                    device_id: crate::event::DeviceId(crate::platform_impl::DeviceId::Wayland(
                        DeviceId,
                    )),
                    position,
                    modifiers: *pointer_data.modifiers_state.borrow(),
                },
                window_id,
            );
        }
        PointerEvent::Leave { surface, serial } => {
            pointer_data.surface = None;
            pointer_data.latest_serial.replace(serial);

            let window_id = wayland::make_wid(&surface);

            let window_handle = match winit_state.window_map.get_mut(&window_id) {
                Some(window_handle) => window_handle,
                None => return,
            };

            // Notify a window that pointer is no longer observing it.
            let winit_pointer = WinitPointer {
                pointer,
                confined_pointer: Rc::downgrade(&pointer_data.confined_pointer),
                locked_pointer: Rc::downgrade(&pointer_data.locked_pointer),
                pointer_constraints: pointer_data.pointer_constraints.clone(),
                latest_serial: pointer_data.latest_serial.clone(),
                latest_enter_serial: pointer_data.latest_enter_serial.clone(),
                seat,
            };
            window_handle.pointer_left(winit_pointer);

            event_sink.push_window_event(
                WindowEvent::CursorLeft {
                    device_id: crate::event::DeviceId(crate::platform_impl::DeviceId::Wayland(
                        DeviceId,
                    )),
                },
                window_id,
            );
        }
        PointerEvent::Motion {
            surface_x,
            surface_y,
            ..
        } => {
            let surface = match pointer_data.surface.as_ref() {
                Some(surface) => surface,
                None => return,
            };

            let window_id = wayland::make_wid(surface);
            let window_handle = match winit_state.window_map.get(&window_id) {
                Some(w) => w,
                _ => return,
            };

            let scale_factor = window_handle.scale_factor();
            let position = LogicalPosition::new(surface_x, surface_y).to_physical(scale_factor);

            event_sink.push_window_event(
                WindowEvent::CursorMoved {
                    device_id: crate::event::DeviceId(crate::platform_impl::DeviceId::Wayland(
                        DeviceId,
                    )),
                    position,
                    modifiers: *pointer_data.modifiers_state.borrow(),
                },
                window_id,
            );
        }
        PointerEvent::Button {
            button,
            state,
            serial,
            ..
        } => {
            pointer_data.latest_serial.replace(serial);
            let window_id = match pointer_data.surface.as_ref().map(wayland::make_wid) {
                Some(window_id) => window_id,
                None => return,
            };

            let state = match state {
                wl_pointer::ButtonState::Pressed => ElementState::Pressed,
                wl_pointer::ButtonState::Released => ElementState::Released,
                _ => unreachable!(),
            };

            let button = match button {
                BTN_LEFT => MouseButton::Left,
                BTN_RIGHT => MouseButton::Right,
                BTN_MIDDLE => MouseButton::Middle,
                button => MouseButton::Other(button as u16),
            };

            event_sink.push_window_event(
                WindowEvent::MouseInput {
                    device_id: crate::event::DeviceId(crate::platform_impl::DeviceId::Wayland(
                        DeviceId,
                    )),
                    state,
                    button,
                    modifiers: *pointer_data.modifiers_state.borrow(),
                },
                window_id,
            );
        }
        PointerEvent::Axis { axis, value, .. } => {
            let surface = match pointer_data.surface.as_ref() {
                Some(surface) => surface,
                None => return,
            };

            let window_id = wayland::make_wid(surface);
            let window_handle = match winit_state.window_map.get(&window_id) {
                Some(w) => w,
                _ => return,
            };

            if pointer.as_ref().version() < 5 {
                let (mut x, mut y) = (0.0, 0.0);

                // Old seat compatibility.
                match axis {
                    // Wayland sign convention is the inverse of winit.
                    wl_pointer::Axis::VerticalScroll => y -= value as f32,
                    wl_pointer::Axis::HorizontalScroll => x -= value as f32,
                    _ => unreachable!(),
                }

                let scale_factor = window_handle.scale_factor();
                let delta = LogicalPosition::new(x as f64, y as f64).to_physical(scale_factor);

                event_sink.push_window_event(
                    WindowEvent::MouseWheel {
                        device_id: crate::event::DeviceId(crate::platform_impl::DeviceId::Wayland(
                            DeviceId,
                        )),
                        delta: MouseScrollDelta::PixelDelta(delta),
                        phase: TouchPhase::Moved,
                        modifiers: *pointer_data.modifiers_state.borrow(),
                    },
                    window_id,
                );
            } else {
                let (mut x, mut y) = pointer_data.axis_data.axis_buffer.unwrap_or((0.0, 0.0));
                match axis {
                    // Wayland sign convention is the inverse of winit.
                    wl_pointer::Axis::VerticalScroll => y -= value as f32,
                    wl_pointer::Axis::HorizontalScroll => x -= value as f32,
                    _ => unreachable!(),
                }

                pointer_data.axis_data.axis_buffer = Some((x, y));

                pointer_data.axis_data.axis_state = match pointer_data.axis_data.axis_state {
                    TouchPhase::Started | TouchPhase::Moved => TouchPhase::Moved,
                    _ => TouchPhase::Started,
                }
            }
        }
        PointerEvent::AxisDiscrete { axis, discrete } => {
            let (mut x, mut y) = pointer_data
                .axis_data
                .axis_discrete_buffer
                .unwrap_or((0., 0.));

            match axis {
                // Wayland sign convention is the inverse of winit.
                wl_pointer::Axis::VerticalScroll => y -= discrete as f32,
                wl_pointer::Axis::HorizontalScroll => x -= discrete as f32,
                _ => unreachable!(),
            }

            pointer_data.axis_data.axis_discrete_buffer = Some((x, y));

            pointer_data.axis_data.axis_state = match pointer_data.axis_data.axis_state {
                TouchPhase::Started | TouchPhase::Moved => TouchPhase::Moved,
                _ => TouchPhase::Started,
            }
        }
        PointerEvent::AxisSource { .. } => (),
        PointerEvent::AxisStop { .. } => {
            pointer_data.axis_data.axis_state = TouchPhase::Ended;
        }
        PointerEvent::Frame => {
            let axis_buffer = pointer_data.axis_data.axis_buffer.take();
            let axis_discrete_buffer = pointer_data.axis_data.axis_discrete_buffer.take();

            let surface = match pointer_data.surface.as_ref() {
                Some(surface) => surface,
                None => return,
            };
            let window_id = wayland::make_wid(surface);
            let window_handle = match winit_state.window_map.get(&window_id) {
                Some(w) => w,
                _ => return,
            };

            let window_event = if let Some((x, y)) = axis_discrete_buffer {
                WindowEvent::MouseWheel {
                    device_id: crate::event::DeviceId(crate::platform_impl::DeviceId::Wayland(
                        DeviceId,
                    )),
                    delta: MouseScrollDelta::LineDelta(x, y),
                    phase: pointer_data.axis_data.axis_state,
                    modifiers: *pointer_data.modifiers_state.borrow(),
                }
            } else if let Some((x, y)) = axis_buffer {
                let scale_factor = window_handle.scale_factor();
                let delta = LogicalPosition::new(x, y).to_physical(scale_factor);

                WindowEvent::MouseWheel {
                    device_id: crate::event::DeviceId(crate::platform_impl::DeviceId::Wayland(
                        DeviceId,
                    )),
                    delta: MouseScrollDelta::PixelDelta(delta),
                    phase: pointer_data.axis_data.axis_state,
                    modifiers: *pointer_data.modifiers_state.borrow(),
                }
            } else {
                return;
            };

            event_sink.push_window_event(window_event, window_id);
        }
        _ => (),
    }
}

#[inline]
pub(super) fn handle_relative_pointer(event: RelativePointerEvent, winit_state: &mut WinitState) {
    if let RelativePointerEvent::RelativeMotion {
        dx_unaccel,
        dy_unaccel,
        ..
    } = event
    {
        winit_state.event_sink.push_device_event(
            DeviceEvent::MouseMotion {
                delta: (dx_unaccel, dy_unaccel),
            },
            DeviceId,
        )
    }
}
