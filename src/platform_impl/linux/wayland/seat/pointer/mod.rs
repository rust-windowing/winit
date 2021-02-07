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

use sctk::seat::pointer::{ThemeManager, ThemedPointer};

use crate::event::ModifiersState;
use crate::platform_impl::wayland::event_loop::WinitState;
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

    /// Latest observed serial in pointer events.
    latest_serial: Rc<Cell<u32>>,
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
                (*self.pointer).set_cursor(self.latest_serial.get(), None, 0, 0);
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
            CursorIcon::Hand => &["hand"],
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
            CursorIcon::NwseResize => &["bd_double_arrow", "size_bdiag"],
            CursorIcon::NeswResize => &["fd_double_arrow", "size_fdiag"],
            CursorIcon::ColResize => &["split_h", "h_double_arrow"],
            CursorIcon::RowResize => &["split_v", "v_double_arrow"],
            CursorIcon::Text => &["text", "xterm"],
            CursorIcon::VerticalText => &["vertical-text"],

            CursorIcon::Wait => &["watch"],

            CursorIcon::ZoomIn => &["zoom-in"],
            CursorIcon::ZoomOut => &["zoom-out"],
        };

        let serial = Some(self.latest_serial.get());
        for cursor in cursors {
            if self.pointer.set_cursor(cursor, serial).is_ok() {
                break;
            }
        }
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
            &pointer_constraints,
            &surface,
            &*self.pointer,
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
}

/// A pointer wrapper for easy releasing and managing pointers.
pub(super) struct Pointers {
    /// A pointer itself.
    pointer: ThemedPointer,

    /// A relative pointer handler.
    relative_pointer: Option<ZwpRelativePointerV1>,

    /// Confined pointer.
    confined_pointer: Rc<RefCell<Option<ZwpConfinedPointerV1>>>,
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
        let pointer_data = Rc::new(RefCell::new(PointerData::new(
            confined_pointer.clone(),
            pointer_constraints.clone(),
            modifiers_state,
        )));
        let pointer = theme_manager.theme_pointer_with_impl(
            seat,
            move |event, pointer, mut dispatch_data| {
                let winit_state = dispatch_data.get::<WinitState>().unwrap();
                handlers::handle_pointer(pointer, event, &pointer_data, winit_state);
            },
        );

        // Setup relative_pointer if it's available.
        let relative_pointer = match relative_pointer_manager.as_ref() {
            Some(relative_pointer_manager) => {
                Some(init_relative_pointer(&relative_pointer_manager, &*pointer))
            }
            None => None,
        };

        Self {
            pointer,
            relative_pointer,
            confined_pointer,
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
    let relative_pointer = relative_pointer_manager.get_relative_pointer(&*pointer);
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
        pointer_constraints.confine_pointer(surface, pointer, None, Lifetime::Persistent.to_raw());

    confined_pointer.quick_assign(move |_, _, _| {});

    confined_pointer.detach()
}
