use smithay_client_toolkit::{get_surface_scale_factor, reexports::client::protocol::{wl_surface::WlSurface, wl_touch::self}};
use crate::{dpi::LogicalPosition, event::TouchPhase};

struct TouchPoint {
    surface: WlSurface,
    position: LogicalPosition<f64>,
    id: i32,
}
impl std::cmp::PartialEq<i32> for TouchPoint { fn eq(&self, other: &i32) -> bool { self.id == *other } }

// Track touch points
#[derive(Default)] pub struct Touch(Vec<TouchPoint>);

impl Touch {
    pub fn handle(&mut self, mut send: impl FnMut(crate::event::WindowEvent, &WlSurface), /*windows: &[super::window::State],*/ event: wl_touch::Event) {
        let device_id = crate::event::DeviceId(super::super::DeviceId::Wayland(super::DeviceId));
        let mut send = |phase,&TouchPoint{ref surface, id, position}| {
            send(crate::event::WindowEvent::Touch(
                crate::event::Touch{device_id, phase, location: position.to_physical(get_surface_scale_factor(&surface) as f64), force: None/*TODO*/, id: id as u64}), &surface);
        };
        use wl_touch::Event::*;
        match event {
            Down {surface, id, x, y, ..} /*if windows.contains(&surface)*/ => {
                let point = TouchPoint{surface, position: LogicalPosition::new(x, y), id};
                send(TouchPhase::Started, &point);
                self.0.push(point);
            }
            Up { id, .. } => if let Some(point) = self.0.remove_item(&id) /*=>*/ {
                send(TouchPhase::Ended, &point);
            }
            Motion { id, x, y, .. } => if let Some(point) = self.0.iter_mut().find(|p| *p == &id) /*=>*/ {
                point.position = LogicalPosition::new(x, y);
                send(TouchPhase::Moved, point);
            }
            Frame => (),
            Cancel => {
                for _point in self.0.drain(..) {
                    //send(TouchPhase::Cancelled, &_point);
                    //------ borrow later used here       ^^^^^^ borrowed value does not live long enough
                }
            }
            _ => println!("Unexpected touch state"),
        }
    }
}
