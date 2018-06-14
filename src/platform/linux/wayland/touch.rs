use std::sync::{Arc, Mutex};

use {TouchPhase, WindowEvent};

use super::{DeviceId, WindowId};
use super::event_loop::EventsLoopSink;
use super::window::WindowStore;

use sctk::reexports::client::{NewProxy, Proxy};
use sctk::reexports::client::protocol::wl_touch::{Event as TouchEvent, WlTouch};

struct TouchPoint {
    wid: WindowId,
    location: (f64, f64),
    id: i32,
}

pub(crate) fn implement_touch(
    touch: NewProxy<WlTouch>,
    sink: Arc<Mutex<EventsLoopSink>>,
    store: Arc<Mutex<WindowStore>>,
) -> Proxy<WlTouch> {
    let mut pending_ids = Vec::new();
    touch.implement(move |evt, _| {
        let mut sink = sink.lock().unwrap();
        let store = store.lock().unwrap();
        match evt {
            TouchEvent::Down {
                surface, id, x, y, ..
            } => {
                let wid = store.find_wid(&surface);
                if let Some(wid) = wid {
                    sink.send_event(
                        WindowEvent::Touch(::Touch {
                            device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                            phase: TouchPhase::Started,
                            location: (x, y).into(),
                            id: id as u64,
                        }),
                        wid,
                    );
                    pending_ids.push(TouchPoint {
                        wid: wid,
                        location: (x, y),
                        id: id,
                    });
                }
            }
            TouchEvent::Up { id, .. } => {
                let idx = pending_ids.iter().position(|p| p.id == id);
                if let Some(idx) = idx {
                    let pt = pending_ids.remove(idx);
                    sink.send_event(
                        WindowEvent::Touch(::Touch {
                            device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                            phase: TouchPhase::Ended,
                            location: pt.location.into(),
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
                    sink.send_event(
                        WindowEvent::Touch(::Touch {
                            device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                            phase: TouchPhase::Moved,
                            location: (x, y).into(),
                            id: id as u64,
                        }),
                        pt.wid,
                    );
                }
            }
            TouchEvent::Frame => (),
            TouchEvent::Cancel => for pt in pending_ids.drain(..) {
                sink.send_event(
                    WindowEvent::Touch(::Touch {
                        device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                        phase: TouchPhase::Cancelled,
                        location: pt.location.into(),
                        id: pt.id as u64,
                    }),
                    pt.wid,
                );
            },
        }
    })
}
