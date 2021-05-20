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
            if !winit_state.window_map.contains_key(&window_id) {
                return;
            }

            let scale_factor = sctk::get_surface_scale_factor(&surface) as f64;
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

            inner
                .touch_points
                .push(TouchPoint::new(surface, position, id));
        }
        TouchEvent::Up { id, .. } => {
            let touch_point = match inner.touch_points.iter().find(|p| p.id == id) {
                Some(touch_point) => touch_point,
                None => return,
            };

            let scale_factor = sctk::get_surface_scale_factor(&touch_point.surface) as f64;
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

            touch_point.position = LogicalPosition::new(x, y);

            let scale_factor = sctk::get_surface_scale_factor(&touch_point.surface) as f64;
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
                let scale_factor = sctk::get_surface_scale_factor(&touch_point.surface) as f64;
                let location = touch_point.position.to_physical(scale_factor);
                let window_id = wayland::make_wid(&touch_point.surface);

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
