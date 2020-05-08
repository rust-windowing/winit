use std::sync::{Arc, Mutex};

use crate::dpi::LogicalPosition;
use crate::event::{TouchPhase, WindowEvent};

use super::{event_loop::EventsSink, make_wid, window::WindowStore, DeviceId};

use smithay_client_toolkit::surface;

use smithay_client_toolkit::reexports::client::protocol::{
    wl_seat,
    wl_surface::WlSurface,
    wl_touch::{Event as TouchEvent, WlTouch},
};

// location is in logical coordinates.
struct TouchPoint {
    surface: WlSurface,
    position: LogicalPosition<f64>,
    id: i32,
}

pub(crate) fn implement_touch(
    seat: &wl_seat::WlSeat,
    sink: EventsSink,
    store: Arc<Mutex<WindowStore>>,
) -> WlTouch {
    let mut pending_ids = Vec::new();
    seat.get_touch(|touch| {
        touch.implement_closure(
            move |evt, _| {
                let store = store.lock().unwrap();
                match evt {
                    TouchEvent::Down {
                        surface, id, x, y, ..
                    } => {
                        let wid = store.find_wid(&surface);
                        if let Some(wid) = wid {
                            let scale_factor = surface::get_dpi_factor(&surface) as f64;
                            let position = LogicalPosition::new(x, y);

                            sink.send_window_event(
                                WindowEvent::Touch(crate::event::Touch {
                                    device_id: crate::event::DeviceId(
                                        crate::platform_impl::DeviceId::Wayland(DeviceId),
                                    ),
                                    phase: TouchPhase::Started,
                                    location: position.to_physical(scale_factor),
                                    force: None, // TODO
                                    id: id as u64,
                                }),
                                wid,
                            );
                            pending_ids.push(TouchPoint {
                                surface,
                                position,
                                id,
                            });
                        }
                    }
                    TouchEvent::Up { id, .. } => {
                        let idx = pending_ids.iter().position(|p| p.id == id);
                        if let Some(idx) = idx {
                            let pt = pending_ids.remove(idx);

                            let scale_factor = surface::get_dpi_factor(&pt.surface) as f64;
                            let location = pt.position.to_physical(scale_factor);

                            sink.send_window_event(
                                WindowEvent::Touch(crate::event::Touch {
                                    device_id: crate::event::DeviceId(
                                        crate::platform_impl::DeviceId::Wayland(DeviceId),
                                    ),
                                    phase: TouchPhase::Ended,
                                    location,
                                    force: None, // TODO
                                    id: id as u64,
                                }),
                                make_wid(&pt.surface),
                            );
                        }
                    }
                    TouchEvent::Motion { id, x, y, .. } => {
                        let pt = pending_ids.iter_mut().find(|p| p.id == id);
                        if let Some(pt) = pt {
                            pt.position = LogicalPosition::new(x, y);

                            let scale_factor = surface::get_dpi_factor(&pt.surface) as f64;
                            let location = pt.position.to_physical(scale_factor);

                            sink.send_window_event(
                                WindowEvent::Touch(crate::event::Touch {
                                    device_id: crate::event::DeviceId(
                                        crate::platform_impl::DeviceId::Wayland(DeviceId),
                                    ),
                                    phase: TouchPhase::Moved,
                                    location,
                                    force: None, // TODO
                                    id: id as u64,
                                }),
                                make_wid(&pt.surface),
                            );
                        }
                    }
                    TouchEvent::Frame => (),
                    TouchEvent::Cancel => {
                        for pt in pending_ids.drain(..) {
                            let scale_factor = surface::get_dpi_factor(&pt.surface) as f64;
                            let location = pt.position.to_physical(scale_factor);

                            sink.send_window_event(
                                WindowEvent::Touch(crate::event::Touch {
                                    device_id: crate::event::DeviceId(
                                        crate::platform_impl::DeviceId::Wayland(DeviceId),
                                    ),
                                    phase: TouchPhase::Cancelled,
                                    location,
                                    force: None, // TODO
                                    id: pt.id as u64,
                                }),
                                make_wid(&pt.surface),
                            );
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
