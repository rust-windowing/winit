//! Various handlers for touch events.

use sctk::reexports::client::protocol::wl_touch::Event as TouchEvent;

use crate::dpi::LogicalPosition;
use crate::event::{TouchPhase, WindowEvent};

use crate::platform_impl::wayland::event_loop::WinitState;
use crate::platform_impl::wayland::{self, DeviceId};

use super::{TouchInner, TouchPoint};

/// Handle WlTouch events.
#[inline]
pub(super) fn handle_touch(
    event: TouchEvent,
    inner: &mut TouchInner,
    winit_state: &mut WinitState,
) {
    let event_sink = &mut winit_state.event_sink;

    match event {
        TouchEvent::Down {
            surface, id, x, y, ..
        } => {
            let window_id = wayland::make_wid(&surface);
            let window_handle = match winit_state.window_map.get(&window_id) {
                Some(w) => w,
                _ => return,
            };

            let scale_factor = window_handle.scale_factor();
            let position = LogicalPosition::new(x, y);

            event_sink.push_window_event(
                WindowEvent::Touch(crate::event::Touch {
                    device_id: crate::event::DeviceId(crate::platform_impl::DeviceId::Wayland(
                        DeviceId,
                    )),
                    phase: TouchPhase::Started,
                    location: position.to_physical(scale_factor),
                    force: None, // TODO
                    id: id as u64,
                }),
                window_id,
            );

            // For `TouchEvent::Up` we don't receive a position, so we're tracking active
            // touch points. Update either a known touch id or register a new one.
            if let Some(i) = inner.touch_points.iter().position(|p| p.id == id) {
                inner.touch_points[i].position = position;
            } else {
                inner
                    .touch_points
                    .push(TouchPoint::new(surface, position, id));
            }
        }
        TouchEvent::Up { id, .. } => {
            let touch_point = match inner.touch_points.iter().find(|p| p.id == id) {
                Some(touch_point) => touch_point,
                None => return,
            };

            let window_id = wayland::make_wid(&touch_point.surface);
            let window_handle = match winit_state.window_map.get(&window_id) {
                Some(w) => w,
                _ => return,
            };
            let scale_factor = window_handle.scale_factor();
            let location = touch_point.position.to_physical(scale_factor);
            let window_id = wayland::make_wid(&touch_point.surface);

            event_sink.push_window_event(
                WindowEvent::Touch(crate::event::Touch {
                    device_id: crate::event::DeviceId(crate::platform_impl::DeviceId::Wayland(
                        DeviceId,
                    )),
                    phase: TouchPhase::Ended,
                    location,
                    force: None, // TODO
                    id: id as u64,
                }),
                window_id,
            );
        }
        TouchEvent::Motion { id, x, y, .. } => {
            let touch_point = match inner.touch_points.iter_mut().find(|p| p.id == id) {
                Some(touch_point) => touch_point,
                None => return,
            };
            let window_id = wayland::make_wid(&touch_point.surface);
            let window_handle = match winit_state.window_map.get(&window_id) {
                Some(w) => w,
                _ => return,
            };

            touch_point.position = LogicalPosition::new(x, y);

            let scale_factor = window_handle.scale_factor();
            let location = touch_point.position.to_physical(scale_factor);
            let window_id = wayland::make_wid(&touch_point.surface);

            event_sink.push_window_event(
                WindowEvent::Touch(crate::event::Touch {
                    device_id: crate::event::DeviceId(crate::platform_impl::DeviceId::Wayland(
                        DeviceId,
                    )),
                    phase: TouchPhase::Moved,
                    location,
                    force: None, // TODO
                    id: id as u64,
                }),
                window_id,
            );
        }
        TouchEvent::Frame => (),
        TouchEvent::Cancel => {
            for touch_point in inner.touch_points.drain(..) {
                let window_id = wayland::make_wid(&touch_point.surface);
                let window_handle = match winit_state.window_map.get(&window_id) {
                    Some(w) => w,
                    _ => return,
                };

                let scale_factor = window_handle.scale_factor();
                let location = touch_point.position.to_physical(scale_factor);

                event_sink.push_window_event(
                    WindowEvent::Touch(crate::event::Touch {
                        device_id: crate::event::DeviceId(crate::platform_impl::DeviceId::Wayland(
                            DeviceId,
                        )),
                        phase: TouchPhase::Cancelled,
                        location,
                        force: None, // TODO
                        id: touch_point.id as u64,
                    }),
                    window_id,
                );
            }
        }
        _ => (),
    }
}
