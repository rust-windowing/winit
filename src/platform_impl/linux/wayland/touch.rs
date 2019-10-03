use std::sync::{Arc, Mutex};

use crate::event::{TouchPhase, WindowEvent};

use super::{event_loop::WindowEventsSink, window::WindowStore, DeviceId, WindowId};

use smithay_client_toolkit::reexports::client::protocol::{
    wl_seat,
    wl_touch::{Event as TouchEvent, WlTouch},
};

struct TouchPoint {
    wid: WindowId,
    location: (f64, f64),
    id: i32,
}

pub(crate) fn implement_touch<T: 'static>(
    seat: &wl_seat::WlSeat,
    sink: Arc<Mutex<WindowEventsSink<T>>>,
    store: Arc<Mutex<WindowStore>>,
) -> WlTouch {
    let mut pending_ids = Vec::new();
    seat.get_touch(|touch| {
        touch.implement_closure(
            move |evt, _| {
                let mut sink = sink.lock().unwrap();
                let store = store.lock().unwrap();
                match evt {
                    TouchEvent::Down {
                        surface, id, x, y, ..
                    } => {
                        let wid = store.find_wid(&surface);
                        if let Some(wid) = wid {
                            sink.send_window_event(
                                WindowEvent::Touch(crate::event::Touch {
                                    device_id: crate::event::DeviceId(
                                        crate::platform_impl::DeviceId::Wayland(DeviceId),
                                    ),
                                    phase: TouchPhase::Started,
                                    location: (x, y).into(),
                                    force: None, // TODO
                                    id: id as u64,
                                }),
                                wid,
                            );
                            pending_ids.push(TouchPoint {
                                wid,
                                location: (x, y),
                                id,
                            });
                        }
                    }
                    TouchEvent::Up { id, .. } => {
                        let idx = pending_ids.iter().position(|p| p.id == id);
                        if let Some(idx) = idx {
                            let pt = pending_ids.remove(idx);
                            sink.send_window_event(
                                WindowEvent::Touch(crate::event::Touch {
                                    device_id: crate::event::DeviceId(
                                        crate::platform_impl::DeviceId::Wayland(DeviceId),
                                    ),
                                    phase: TouchPhase::Ended,
                                    location: pt.location.into(),
                                    force: None, // TODO
                                    id: id as u64,
                                }),
                                pt.wid,
                            );
                        }
                    }
                    TouchEvent::Motion { id, x, y, .. } => {
                        let pt = pending_ids.iter_mut().find(|p| p.id == id);
                        if let Some(pt) = pt {
                            pt.location = (x, y);
                            sink.send_window_event(
                                WindowEvent::Touch(crate::event::Touch {
                                    device_id: crate::event::DeviceId(
                                        crate::platform_impl::DeviceId::Wayland(DeviceId),
                                    ),
                                    phase: TouchPhase::Moved,
                                    location: (x, y).into(),
                                    force: None, // TODO
                                    id: id as u64,
                                }),
                                pt.wid,
                            );
                        }
                    }
                    TouchEvent::Frame => (),
                    TouchEvent::Cancel => {
                        for pt in pending_ids.drain(..) {
                            sink.send_window_event(
                                WindowEvent::Touch(crate::event::Touch {
                                    device_id: crate::event::DeviceId(
                                        crate::platform_impl::DeviceId::Wayland(DeviceId),
                                    ),
                                    phase: TouchPhase::Cancelled,
                                    location: pt.location.into(),
                                    force: None, // TODO
                                    id: pt.id as u64,
                                }),
                                pt.wid,
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
