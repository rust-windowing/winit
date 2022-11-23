//! All pointer related handling.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use sctk::reexports::client::protocol::wl_pointer::WlPointer;
use sctk::reexports::client::protocol::wl_seat::WlSeat;
use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::reexports::client::Attached;
use sctk::reexports::protocols::unstable::relative_pointer::v1::client::zwp_relative_pointer_manager_v1::ZwpRelativePointerManagerV1;
use sctk::reexports::protocols::unstable::relative_pointer::v1::client::zwp_relative_pointer_v1::ZwpRelativePointerV1;
use sctk::reexports::protocols::unstable::pointer_constraints::v1::client::zwp_pointer_constraints_v1::{ZwpPointerConstraintsV1, Lifetime};
use sctk::reexports::protocols::unstable::pointer_constraints::v1::client::zwp_confined_pointer_v1::ZwpConfinedPointerV1;
use sctk::reexports::protocols::unstable::pointer_constraints::v1::client::zwp_locked_pointer_v1::ZwpLockedPointerV1;

use sctk::seat::pointer::{ThemeManager, ThemedPointer};
use sctk::window::Window;

use crate::event::ModifiersState;
use crate::platform_impl::wayland::event_loop::WinitState;
use crate::platform_impl::wayland::window::WinitFrame;
use crate::window::CursorIcon;

mod data;
mod handlers;

use data::PointerData;

/// A proxy to Wayland pointer, which serves requests from a `WindowHandle`.
pub struct WinitPointer {
    pointer: ThemedPointer,

    /// Create confined pointers.
    pointer_constraints: Option<Attached<ZwpPointerConstraintsV1>>,

    /// Cursor to handle confine requests.
    confined_pointer: Weak<RefCell<Option<ZwpConfinedPointerV1>>>,

    /// Cursor to handle locked requests.
    locked_pointer: Weak<RefCell<Option<ZwpLockedPointerV1>>>,

    /// Latest observed serial in pointer events.
    /// used by Window::start_interactive_move()
    latest_serial: Rc<Cell<u32>>,

    /// Latest observed serial in pointer enter events.
    /// used by Window::set_cursor()
    latest_enter_serial: Rc<Cell<u32>>,

    /// Seat.
    seat: WlSeat,
}

impl PartialEq for WinitPointer {
    fn eq(&self, other: &Self) -> bool {
        *self.pointer == *other.pointer
    }
}

impl Eq for WinitPointer {}

impl WinitPointer {
    /// Set the cursor icon.
    ///
    /// Providing `None` will hide the cursor.
    pub fn set_cursor(&self, cursor_icon: Option<CursorIcon>) {
        let cursor_icon = match cursor_icon {
            Some(cursor_icon) => cursor_icon,
            None => {
                // Hide the cursor.
                // WlPointer::set_cursor() expects the serial of the last *enter*
                // event (compare to to start_interactive_move()).
                (*self.pointer).set_cursor(self.latest_enter_serial.get(), None, 0, 0);
                return;
            }
        };

        let cursors: &[&str] = match cursor_icon {
            CursorIcon::Alias => &["link"],
            CursorIcon::Arrow => &["arrow"],
            CursorIcon::Cell => &["plus"],
            CursorIcon::Copy => &["copy"],
            CursorIcon::Crosshair => &["crosshair"],
            CursorIcon::Default => &["left_ptr"],
            CursorIcon::Hand => &["hand2", "hand1"],
            CursorIcon::Help => &["question_arrow"],
            CursorIcon::Move => &["move"],
            CursorIcon::Grab => &["openhand", "grab"],
            CursorIcon::Grabbing => &["closedhand", "grabbing"],
            CursorIcon::Progress => &["progress"],
            CursorIcon::AllScroll => &["all-scroll"],
            CursorIcon::ContextMenu => &["context-menu"],

            CursorIcon::NoDrop => &["no-drop", "circle"],
            CursorIcon::NotAllowed => &["crossed_circle"],

            // Resize cursors
            CursorIcon::EResize => &["right_side"],
            CursorIcon::NResize => &["top_side"],
            CursorIcon::NeResize => &["top_right_corner"],
            CursorIcon::NwResize => &["top_left_corner"],
            CursorIcon::SResize => &["bottom_side"],
            CursorIcon::SeResize => &["bottom_right_corner"],
            CursorIcon::SwResize => &["bottom_left_corner"],
            CursorIcon::WResize => &["left_side"],
            CursorIcon::EwResize => &["h_double_arrow"],
            CursorIcon::NsResize => &["v_double_arrow"],
            CursorIcon::NwseResize => &["bd_double_arrow", "size_fdiag"],
            CursorIcon::NeswResize => &["fd_double_arrow", "size_bdiag"],
            CursorIcon::ColResize => &["split_h", "h_double_arrow"],
            CursorIcon::RowResize => &["split_v", "v_double_arrow"],
            CursorIcon::Text => &["text", "xterm"],
            CursorIcon::VerticalText => &["vertical-text"],

            CursorIcon::Wait => &["watch"],

            CursorIcon::ZoomIn => &["zoom-in"],
            CursorIcon::ZoomOut => &["zoom-out"],
        };

        let serial = Some(self.latest_enter_serial.get());
        for cursor in cursors {
            if self.pointer.set_cursor(cursor, serial).is_ok() {
                return;
            }
        }
        warn!("Failed to set cursor to {:?}", cursor_icon);
    }

    /// Confine the pointer to a surface.
    pub fn confine(&self, surface: &WlSurface) {
        let pointer_constraints = match &self.pointer_constraints {
            Some(pointer_constraints) => pointer_constraints,
            None => return,
        };

        let confined_pointer = match self.confined_pointer.upgrade() {
            Some(confined_pointer) => confined_pointer,
            // A pointer is gone.
            None => return,
        };

        *confined_pointer.borrow_mut() = Some(init_confined_pointer(
            pointer_constraints,
            surface,
            &self.pointer,
        ));
    }

    /// Tries to unconfine the pointer if the current pointer is confined.
    pub fn unconfine(&self) {
        let confined_pointer = match self.confined_pointer.upgrade() {
            Some(confined_pointer) => confined_pointer,
            // A pointer is gone.
            None => return,
        };

        let mut confined_pointer = confined_pointer.borrow_mut();

        if let Some(confined_pointer) = confined_pointer.take() {
            confined_pointer.destroy();
        }
    }

    pub fn lock(&self, surface: &WlSurface) {
        let pointer_constraints = match &self.pointer_constraints {
            Some(pointer_constraints) => pointer_constraints,
            None => return,
        };

        let locked_pointer = match self.locked_pointer.upgrade() {
            Some(locked_pointer) => locked_pointer,
            // A pointer is gone.
            None => return,
        };

        *locked_pointer.borrow_mut() = Some(init_locked_pointer(
            pointer_constraints,
            surface,
            &self.pointer,
        ));
    }

    pub fn unlock(&self) {
        let locked_pointer = match self.locked_pointer.upgrade() {
            Some(locked_pointer) => locked_pointer,
            // A pointer is gone.
            None => return,
        };

        let mut locked_pointer = locked_pointer.borrow_mut();

        if let Some(locked_pointer) = locked_pointer.take() {
            locked_pointer.destroy();
        }
    }

    pub fn set_cursor_position(&self, surface_x: u32, surface_y: u32) {
        let locked_pointer = match self.locked_pointer.upgrade() {
            Some(locked_pointer) => locked_pointer,
            // A pointer is gone.
            None => return,
        };

        let locked_pointer = locked_pointer.borrow_mut();
        if let Some(locked_pointer) = locked_pointer.as_ref() {
            locked_pointer.set_cursor_position_hint(surface_x.into(), surface_y.into());
        }
    }

    pub fn drag_window(&self, window: &Window<WinitFrame>) {
        // WlPointer::setart_interactive_move() expects the last serial of *any*
        // pointer event (compare to set_cursor()).
        window.start_interactive_move(&self.seat, self.latest_serial.get());
    }
}

/// A pointer wrapper for easy releasing and managing pointers.
pub(super) struct Pointers {
    /// A pointer itself.
    pointer: ThemedPointer,

    /// A relative pointer handler.
    relative_pointer: Option<ZwpRelativePointerV1>,

    /// Confined pointer.
    confined_pointer: Rc<RefCell<Option<ZwpConfinedPointerV1>>>,

    /// Locked pointer.
    locked_pointer: Rc<RefCell<Option<ZwpLockedPointerV1>>>,
}

impl Pointers {
    pub(super) fn new(
        seat: &Attached<WlSeat>,
        theme_manager: &ThemeManager,
        relative_pointer_manager: &Option<Attached<ZwpRelativePointerManagerV1>>,
        pointer_constraints: &Option<Attached<ZwpPointerConstraintsV1>>,
        modifiers_state: Rc<RefCell<ModifiersState>>,
    ) -> Self {
        let confined_pointer = Rc::new(RefCell::new(None));
        let locked_pointer = Rc::new(RefCell::new(None));

        let pointer_data = Rc::new(RefCell::new(PointerData::new(
            confined_pointer.clone(),
            locked_pointer.clone(),
            pointer_constraints.clone(),
            modifiers_state,
        )));

        let pointer_seat = seat.detach();
        let pointer = theme_manager.theme_pointer_with_impl(
            seat,
            move |event, pointer, mut dispatch_data| {
                let winit_state = dispatch_data.get::<WinitState>().unwrap();
                handlers::handle_pointer(
                    pointer,
                    event,
                    &pointer_data,
                    winit_state,
                    pointer_seat.clone(),
                );
            },
        );

        // Setup relative_pointer if it's available.
        let relative_pointer = relative_pointer_manager
            .as_ref()
            .map(|relative_pointer_manager| {
                init_relative_pointer(relative_pointer_manager, &pointer)
            });

        Self {
            pointer,
            relative_pointer,
            confined_pointer,
            locked_pointer,
        }
    }
}

impl Drop for Pointers {
    fn drop(&mut self) {
        // Drop relative pointer.
        if let Some(relative_pointer) = self.relative_pointer.take() {
            relative_pointer.destroy();
        }

        // Drop confined pointer.
        if let Some(confined_pointer) = self.confined_pointer.borrow_mut().take() {
            confined_pointer.destroy();
        }

        // Drop lock ponter.
        if let Some(locked_pointer) = self.locked_pointer.borrow_mut().take() {
            locked_pointer.destroy();
        }

        // Drop the pointer itself in case it's possible.
        if self.pointer.as_ref().version() >= 3 {
            self.pointer.release();
        }
    }
}

pub(super) fn init_relative_pointer(
    relative_pointer_manager: &ZwpRelativePointerManagerV1,
    pointer: &WlPointer,
) -> ZwpRelativePointerV1 {
    let relative_pointer = relative_pointer_manager.get_relative_pointer(pointer);
    relative_pointer.quick_assign(move |_, event, mut dispatch_data| {
        let winit_state = dispatch_data.get::<WinitState>().unwrap();
        handlers::handle_relative_pointer(event, winit_state);
    });

    relative_pointer.detach()
}

pub(super) fn init_confined_pointer(
    pointer_constraints: &Attached<ZwpPointerConstraintsV1>,
    surface: &WlSurface,
    pointer: &WlPointer,
) -> ZwpConfinedPointerV1 {
    let confined_pointer =
        pointer_constraints.confine_pointer(surface, pointer, None, Lifetime::Persistent);

    confined_pointer.quick_assign(move |_, _, _| {});

    confined_pointer.detach()
}

pub(super) fn init_locked_pointer(
    pointer_constraints: &Attached<ZwpPointerConstraintsV1>,
    surface: &WlSurface,
    pointer: &WlPointer,
) -> ZwpLockedPointerV1 {
    let locked_pointer =
        pointer_constraints.lock_pointer(surface, pointer, None, Lifetime::Persistent);

    locked_pointer.quick_assign(move |_, _, _| {});

    locked_pointer.detach()
}
