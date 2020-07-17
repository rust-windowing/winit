//! Seat handling and managing.

use std::cell::RefCell;
use std::rc::Rc;

use sctk::reexports::protocols::unstable::relative_pointer::v1::client::zwp_relative_pointer_manager_v1::ZwpRelativePointerManagerV1;
use sctk::reexports::protocols::unstable::pointer_constraints::v1::client::zwp_pointer_constraints_v1::ZwpPointerConstraintsV1;

use sctk::reexports::client::protocol::wl_seat::WlSeat;
use sctk::reexports::client::Attached;

use sctk::environment::Environment;
use sctk::reexports::calloop::LoopHandle;
use sctk::seat::pointer::ThemeManager;
use sctk::seat::{SeatData, SeatListener};

use super::env::WinitEnv;
use super::event_loop::WinitState;
use crate::event::ModifiersState;

mod keyboard;
pub mod pointer;
mod touch;

use keyboard::Keyboard;
use pointer::Pointers;
use touch::Touch;

pub struct SeatManager {
    /// Listener for seats.
    _seat_listener: SeatListener,
}

impl SeatManager {
    pub fn new(
        env: &Environment<WinitEnv>,
        loop_handle: LoopHandle<WinitState>,
        theme_manager: ThemeManager,
    ) -> Self {
        let relative_pointer_manager = env.get_global::<ZwpRelativePointerManagerV1>();
        let pointer_constraints = env.get_global::<ZwpPointerConstraintsV1>();

        let mut inner = SeatManagerInner::new(
            theme_manager,
            relative_pointer_manager,
            pointer_constraints,
            loop_handle,
        );

        // Handle existing seats.
        for seat in env.get_all_seats() {
            let seat_data = match sctk::seat::clone_seat_data(&seat) {
                Some(seat_data) => seat_data,
                None => continue,
            };

            inner.process_seat_update(&seat, &seat_data);
        }

        let seat_listener = env.listen_for_seats(move |seat, seat_data, _| {
            inner.process_seat_update(&seat, &seat_data);
        });

        Self {
            _seat_listener: seat_listener,
        }
    }
}

/// Inner state of the seat manager.
struct SeatManagerInner {
    /// Currently observed seats.
    seats: Vec<SeatInfo>,

    /// Loop handle.
    loop_handle: LoopHandle<WinitState>,

    /// Relative pointer manager.
    relative_pointer_manager: Option<Attached<ZwpRelativePointerManagerV1>>,

    /// Pointer constraints.
    pointer_constraints: Option<Attached<ZwpPointerConstraintsV1>>,

    /// A theme manager.
    theme_manager: ThemeManager,
}

impl SeatManagerInner {
    fn new(
        theme_manager: ThemeManager,
        relative_pointer_manager: Option<Attached<ZwpRelativePointerManagerV1>>,
        pointer_constraints: Option<Attached<ZwpPointerConstraintsV1>>,
        loop_handle: LoopHandle<WinitState>,
    ) -> Self {
        Self {
            seats: Vec::new(),
            theme_manager,
            relative_pointer_manager,
            pointer_constraints,
            loop_handle,
        }
    }

    /// Handle seats update from the `SeatListener`.
    pub fn process_seat_update(&mut self, seat: &Attached<WlSeat>, seat_data: &SeatData) {
        let detached_seat = seat.detach();

        let position = self.seats.iter().position(|si| si.seat == detached_seat);
        let index = position.unwrap_or_else(|| {
            self.seats
                .push(SeatInfo::new(detached_seat, None, None, None));
            self.seats.len() - 1
        });

        let seat_info = &mut self.seats[index];

        // Pointer handling.
        if seat_data.has_pointer && !seat_data.defunct {
            if seat_info.pointer.is_none() {
                seat_info.pointer = Some(Pointers::new(
                    &seat,
                    &self.theme_manager,
                    &self.relative_pointer_manager,
                    &self.pointer_constraints,
                    seat_info.modifiers_state.clone(),
                ));
            }
        } else {
            seat_info.pointer = None;
        }

        // Handle keyboard.
        if seat_data.has_keyboard && !seat_data.defunct {
            if seat_info.keyboard.is_none() {
                seat_info.keyboard = Keyboard::new(
                    &seat,
                    self.loop_handle.clone(),
                    seat_info.modifiers_state.clone(),
                );
            }
        } else {
            seat_info.keyboard = None;
        }

        // Handle touch.
        if seat_data.has_touch && !seat_data.defunct {
            if seat_info.touch.is_none() {
                seat_info.touch = Some(Touch::new(&seat));
            }
        } else {
            seat_info.touch = None;
        }
    }
}

/// Resources associtated with a given seat.
struct SeatInfo {
    /// Seat to which this `SeatInfo` belongs.
    seat: WlSeat,

    /// A keyboard handle with its repeat rate handling.
    keyboard: Option<Keyboard>,

    /// All pointers we're using on a seat.
    pointer: Option<Pointers>,

    /// Touch handling.
    touch: Option<Touch>,

    /// The current state of modifiers observed in keyboard handler.
    ///
    /// We keep modifiers state on a seat, since it's being used by pointer events as well.
    modifiers_state: Rc<RefCell<ModifiersState>>,
}

impl SeatInfo {
    pub fn new(
        seat: WlSeat,
        keyboard: Option<Keyboard>,
        pointer: Option<Pointers>,
        touch: Option<Touch>,
    ) -> Self {
        Self {
            seat,
            keyboard,
            pointer,
            touch,
            modifiers_state: Rc::new(RefCell::new(ModifiersState::default())),
        }
    }
}
