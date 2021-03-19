//! Data which is used in pointer callbacks.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::reexports::client::Attached;
use sctk::reexports::protocols::unstable::pointer_constraints::v1::client::zwp_pointer_constraints_v1::{ZwpPointerConstraintsV1};
use sctk::reexports::protocols::unstable::pointer_constraints::v1::client::zwp_confined_pointer_v1::ZwpConfinedPointerV1;

use crate::event::TouchPhase;
use crate::keyboard::ModifiersState;

/// A data being used by pointer handlers.
pub(super) struct PointerData {
    /// Winit's surface the pointer is currently over.
    pub surface: Option<WlSurface>,

    /// Current modifiers state.
    ///
    /// This refers a state of modifiers from `WlKeyboard` on
    /// the given seat.
    pub modifiers_state: Rc<RefCell<ModifiersState>>,

    /// Pointer constraints.
    pub pointer_constraints: Option<Attached<ZwpPointerConstraintsV1>>,

    pub confined_pointer: Rc<RefCell<Option<ZwpConfinedPointerV1>>>,

    /// A latest event serial.
    pub latest_serial: Rc<Cell<u32>>,

    /// The currently accumulated axis data on a pointer.
    pub axis_data: AxisData,
}

impl PointerData {
    pub fn new(
        confined_pointer: Rc<RefCell<Option<ZwpConfinedPointerV1>>>,
        pointer_constraints: Option<Attached<ZwpPointerConstraintsV1>>,
        modifiers_state: Rc<RefCell<ModifiersState>>,
    ) -> Self {
        Self {
            surface: None,
            latest_serial: Rc::new(Cell::new(0)),
            confined_pointer,
            modifiers_state,
            pointer_constraints,
            axis_data: AxisData::new(),
        }
    }
}

/// Axis data.
#[derive(Clone, Copy)]
pub(super) struct AxisData {
    /// Current state of the axis.
    pub axis_state: TouchPhase,

    /// A buffer for `PixelDelta` event.
    pub axis_buffer: Option<(f32, f32)>,

    /// A buffer for `LineDelta` event.
    pub axis_discrete_buffer: Option<(f32, f32)>,
}

impl AxisData {
    pub fn new() -> Self {
        Self {
            axis_state: TouchPhase::Ended,
            axis_buffer: None,
            axis_discrete_buffer: None,
        }
    }
}
