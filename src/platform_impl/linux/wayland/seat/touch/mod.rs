//! Touch handling.

use sctk::reexports::client::protocol::wl_seat::WlSeat;
use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::reexports::client::protocol::wl_touch::WlTouch;
use sctk::reexports::client::Attached;

use crate::dpi::LogicalPosition;

use crate::platform_impl::wayland::event_loop::WinitState;

mod handlers;

/// Wrapper around touch to handle release.
pub struct Touch {
    /// Proxy to touch.
    touch: WlTouch,
}

impl Touch {
    pub fn new(seat: &Attached<WlSeat>) -> Self {
        let touch = seat.get_touch();
        let mut inner = TouchInner::new();

        touch.quick_assign(move |_, event, mut dispatch_data| {
            let winit_state = dispatch_data.get::<WinitState>().unwrap();
            handlers::handle_touch(event, &mut inner, winit_state);
        });

        Self {
            touch: touch.detach(),
        }
    }
}

impl Drop for Touch {
    fn drop(&mut self) {
        if self.touch.as_ref().version() >= 3 {
            self.touch.release();
        }
    }
}

/// The data used by touch handlers.
pub(super) struct TouchInner {
    /// Current touch points.
    touch_points: Vec<TouchPoint>,
}

impl TouchInner {
    fn new() -> Self {
        Self {
            touch_points: Vec::new(),
        }
    }
}

/// Location of touch press.
pub(super) struct TouchPoint {
    /// A surface where the touch point is located.
    surface: WlSurface,

    /// Location of the touch point.
    position: LogicalPosition<f64>,

    /// Id.
    id: i32,
}

impl TouchPoint {
    pub fn new(surface: WlSurface, position: LogicalPosition<f64>, id: i32) -> Self {
        Self {
            surface,
            position,
            id,
        }
    }
}
