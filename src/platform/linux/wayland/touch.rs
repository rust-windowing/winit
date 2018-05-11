use std::sync::{Arc, Mutex};

use {TouchPhase, WindowEvent};

use super::{DeviceId, make_wid};
use super::event_loop::EventsLoopSink;
use super::window::WindowStore;

use sctk::reexports::client::{NewProxy, Proxy};
use sctk::reexports::client::protocol::wl_surface;
use sctk::reexports::client::protocol::wl_touch::{Event as TouchEvent, WlTouch};

struct TouchPoint {
    surface: Proxy<wl_surface::WlSurface>,
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
                let dpi = store.get_dpi(&surface) as f64;
                let location = (dpi*x, dpi*y);
                sink.send_event(
                    WindowEvent::Touch(::Touch {
                        device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                        phase: TouchPhase::Started,
                        location,
                        id: id as u64,
                    }),
                    make_wid(&surface),
                );
                pending_ids.push(TouchPoint {
                    surface: surface,
                    location,
                    id: id,
                });
            }
            TouchEvent::Up { id, .. } => {
                let idx = pending_ids.iter().position(|p| p.id == id);
                if let Some(idx) = idx {
                    let pt = pending_ids.remove(idx);
                    sink.send_event(
                        WindowEvent::Touch(::Touch {
                            device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                            phase: TouchPhase::Ended,
                            location: pt.location,
                            id: id as u64,
                        }),
                        make_wid(&pt.surface),
                    );
                }
            }
            TouchEvent::Motion { id, x, y, .. } => {
                let pt = pending_ids.iter_mut().find(|p| p.id == id);
                if let Some(pt) = pt {
                    let dpi = store.get_dpi(&pt.surface) as f64;
                    pt.location = (dpi*x, dpi*y);
                    sink.send_event(
                        WindowEvent::Touch(::Touch {
                            device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                            phase: TouchPhase::Moved,
                            location: pt.location,
                            id: id as u64,
                        }),
                        make_wid(&pt.surface),
                    );
                }
            }
            TouchEvent::Frame => (),
            TouchEvent::Cancel => for pt in pending_ids.drain(..) {
                sink.send_event(
                    WindowEvent::Touch(::Touch {
                        device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                        phase: TouchPhase::Cancelled,
                        location: pt.location,
                        id: pt.id as u64,
                    }),
                    make_wid(&pt.surface),
                );
            },
        }
    })
}
