use std::sync::{Arc, Mutex};

use {WindowEvent as Event, TouchPhase};

use super::{WindowId, DeviceId};
use super::event_loop::EventsLoopSink;
use super::window::WindowStore;

use wayland_client::StateToken;
use wayland_client::protocol::wl_touch;

pub struct TouchIData {
    sink: Arc<Mutex<EventsLoopSink>>,
    windows_token: StateToken<WindowStore>,
    pending_ids: Vec<TouchPoint>,
}

struct TouchPoint {
    wid: WindowId,
    location: (f64, f64),
    id: i32
}

impl TouchIData {
    pub fn new(sink: &Arc<Mutex<EventsLoopSink>>, token: StateToken<WindowStore>)
        -> TouchIData
    {
        TouchIData {
            sink: sink.clone(),
            windows_token: token,
            pending_ids: Vec::new(),
        }
    }
}

pub fn touch_implementation() -> wl_touch::Implementation<TouchIData> {
    wl_touch::Implementation {
        down: |evqh, idata, _, _serial, _time, surface, touch_id, x, y| {
            let wid = evqh.state().get(&idata.windows_token).find_wid(surface);
            if let Some(wid) = wid {
                let mut guard = idata.sink.lock().unwrap();
                guard.send_event(
                    Event::Touch(::Touch {
                        device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                        phase: TouchPhase::Started,
                        location: (x, y),
                        id: touch_id as u64
                    }),
                    wid,
                );
                idata.pending_ids.push(TouchPoint {
                    wid: wid,
                    location: (x, y),
                    id: touch_id
                });
            }
        },
        up: |_, idata, _, _serial, _time, touch_id| {
            let idx = idata.pending_ids.iter().position(|p| p.id == touch_id);
            if let Some(idx) = idx {
                let pt = idata.pending_ids.remove(idx);
                let mut guard = idata.sink.lock().unwrap();
                guard.send_event(
                    Event::Touch(::Touch {
                        device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                        phase: TouchPhase::Ended,
                        location: pt.location,
                        id: touch_id as u64
                    }),
                    pt.wid,
                );
            }
        },
        motion: |_, idata, _, _time, touch_id, x, y| {
            let pt = idata.pending_ids.iter_mut().find(|p| p.id == touch_id);
            if let Some(pt) = pt {
                let mut guard = idata.sink.lock().unwrap();
                pt.location = (x, y);
                guard.send_event(
                    Event::Touch(::Touch {
                        device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                        phase: TouchPhase::Moved,
                        location: (x, y),
                        id: touch_id as u64
                    }),
                    pt.wid,
                );
            }
        },
        frame: |_, _, _| {},
        cancel: |_, idata, _| {
            let mut guard = idata.sink.lock().unwrap();
            for pt in idata.pending_ids.drain(..) {
                guard.send_event(
                    Event::Touch(::Touch {
                        device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                        phase: TouchPhase::Cancelled,
                        location: pt.location,
                        id: pt.id as u64
                    }),
                    pt.wid,
                );
            }
        }
    }
}