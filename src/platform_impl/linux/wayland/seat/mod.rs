//! Seat handling.

use std::sync::Arc;

use fnv::FnvHashMap;

use sctk::reexports::client::protocol::wl_seat::WlSeat;
use sctk::reexports::client::protocol::wl_touch::WlTouch;
use sctk::reexports::client::{Connection, Proxy, QueueHandle};
use sctk::reexports::protocols::wp::relative_pointer::zv1::client::zwp_relative_pointer_v1::ZwpRelativePointerV1;
use sctk::reexports::protocols::wp::text_input::zv3::client::zwp_text_input_v3::ZwpTextInputV3;

use sctk::seat::pointer::{ThemeSpec, ThemedPointer};
use sctk::seat::{Capability as SeatCapability, SeatHandler, SeatState};

use crate::keyboard::ModifiersState;
use crate::platform_impl::wayland::state::WinitState;

mod keyboard;
mod pointer;
mod text_input;
mod touch;

pub use pointer::relative_pointer::RelativePointerState;
pub use pointer::{PointerConstraintsState, WinitPointerData, WinitPointerDataExt};
pub use text_input::{TextInputState, ZwpTextInputV3Ext};

use keyboard::{KeyboardData, KeyboardState};
use text_input::TextInputData;
use touch::TouchPoint;

#[derive(Debug)]
pub struct WinitSeatState {
    /// The pointer bound on the seat.
    pointer: Option<Arc<ThemedPointer<WinitPointerData>>>,

    /// The touch bound on the seat.
    touch: Option<WlTouch>,

    /// The mapping from touched points to the surfaces they're present.
    touch_map: FnvHashMap<i32, TouchPoint>,

    /// The text input bound on the seat.
    text_input: Option<Arc<ZwpTextInputV3>>,

    /// The relative pointer bound on the seat.
    relative_pointer: Option<ZwpRelativePointerV1>,

    /// The keyboard bound on the seat.
    keyboard_state: Option<KeyboardState>,

    /// The current modifiers state on the seat.
    modifiers: ModifiersState,

    /// Wether we have pending modifiers.
    modifiers_pending: bool,
}

impl WinitSeatState {
    pub fn new() -> Self {
        Self {
            pointer: None,
            touch: None,
            relative_pointer: None,
            text_input: None,
            touch_map: Default::default(),
            keyboard_state: None,
            modifiers: ModifiersState::empty(),
            modifiers_pending: false,
        }
    }
}

impl SeatHandler for WinitState {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_capability(
        &mut self,
        _: &Connection,
        queue_handle: &QueueHandle<Self>,
        seat: WlSeat,
        capability: SeatCapability,
    ) {
        let seat_state = self.seats.get_mut(&seat.id()).unwrap();

        match capability {
            SeatCapability::Touch if seat_state.touch.is_none() => {
                seat_state.touch = self.seat_state.get_touch(queue_handle, &seat).ok();
            }
            SeatCapability::Keyboard if seat_state.keyboard_state.is_none() => {
                let keyboard = seat.get_keyboard(queue_handle, KeyboardData::new(seat.clone()));
                seat_state.keyboard_state =
                    Some(KeyboardState::new(keyboard, self.loop_handle.clone()));
            }
            SeatCapability::Pointer if seat_state.pointer.is_none() => {
                let surface = self.compositor_state.create_surface(queue_handle);
                let surface_id = surface.id();
                let pointer_data = WinitPointerData::new(seat.clone(), surface);
                let themed_pointer = self
                    .seat_state
                    .get_pointer_with_theme_and_data(
                        queue_handle,
                        &seat,
                        ThemeSpec::System,
                        pointer_data,
                    )
                    .expect("failed to create pointer with present capability.");

                seat_state.relative_pointer = self.relative_pointer.as_ref().map(|manager| {
                    manager.get_relative_pointer(
                        themed_pointer.pointer(),
                        queue_handle,
                        sctk::globals::GlobalData,
                    )
                });

                let themed_pointer = Arc::new(themed_pointer);

                // Register cursor surface.
                self.pointer_surfaces
                    .insert(surface_id, themed_pointer.clone());

                seat_state.pointer = Some(themed_pointer);
            }
            _ => (),
        }

        if let Some(text_input_state) = seat_state
            .text_input
            .is_none()
            .then_some(self.text_input_state.as_ref())
            .flatten()
        {
            seat_state.text_input = Some(Arc::new(text_input_state.get_text_input(
                &seat,
                queue_handle,
                TextInputData::default(),
            )));
        }
    }

    fn remove_capability(
        &mut self,
        _: &Connection,
        _queue_handle: &QueueHandle<Self>,
        seat: WlSeat,
        capability: SeatCapability,
    ) {
        let seat_state = self.seats.get_mut(&seat.id()).unwrap();

        match capability {
            SeatCapability::Touch => {
                if let Some(touch) = seat_state.touch.take() {
                    if touch.version() >= 3 {
                        touch.release();
                    }
                }
            }
            SeatCapability::Pointer => {
                if let Some(relative_pointer) = seat_state.relative_pointer.take() {
                    relative_pointer.destroy();
                }

                if let Some(pointer) = seat_state.pointer.take() {
                    let pointer_data = pointer.pointer().winit_data();

                    // Remove the cursor from the mapping.
                    let surface_id = pointer_data.cursor_surface().id();
                    let _ = self.pointer_surfaces.remove(&surface_id);

                    // Remove the inner locks/confines before dropping the pointer.
                    pointer_data.unlock_pointer();
                    pointer_data.unconfine_pointer();

                    if pointer.pointer().version() >= 3 {
                        pointer.pointer().release();
                    }
                }
            }
            SeatCapability::Keyboard => {
                seat_state.keyboard_state = None;
            }
            _ => (),
        }

        if let Some(text_input) = seat_state.text_input.take() {
            text_input.destroy();
        }
    }

    fn new_seat(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        seat: WlSeat,
    ) {
        self.seats.insert(seat.id(), WinitSeatState::new());
    }

    fn remove_seat(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        seat: WlSeat,
    ) {
        let _ = self.seats.remove(&seat.id());
    }
}

sctk::delegate_seat!(WinitState);
