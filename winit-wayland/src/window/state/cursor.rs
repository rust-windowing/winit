use std::sync::{Arc, Weak};

use sctk::compositor::SurfaceData;
use sctk::reexports::client::Proxy;
use sctk::seat::pointer::{PointerData, ThemedPointer};
use sctk::shell::WaylandSurface;
use tracing::warn;
use winit_core::cursor::{Cursor, CursorIcon, CustomCursor as CoreCustomCursor};
use winit_core::error::{NotSupportedError, RequestError};
use winit_core::window::CursorGrabMode;

use super::WindowState;
use crate::seat::{WinitPointerData, WinitPointerDataExt};
use crate::types::cursor::{CustomCursor, SelectedCursor, WaylandCustomCursor};

/// The state of the cursor grabs.
#[derive(Clone, Copy, Debug)]
pub(crate) struct GrabState {
    /// The grab mode requested by the user.
    user_grab_mode: CursorGrabMode,

    /// The current grab mode.
    current_grab_mode: CursorGrabMode,
}

impl GrabState {
    pub(crate) fn new() -> Self {
        Self { user_grab_mode: CursorGrabMode::None, current_grab_mode: CursorGrabMode::None }
    }
}

impl WindowState {
    /// Register pointer on the top-level.
    pub fn pointer_entered(&mut self, added: Weak<ThemedPointer<WinitPointerData>>) {
        self.pointers.push(added);
        self.reload_cursor_style();

        let mode = self.cursor_grab_mode.user_grab_mode;
        let _ = self.set_cursor_grab_inner(mode);
    }

    /// Pointer has left the top-level.
    pub fn pointer_left(&mut self, removed: Weak<ThemedPointer<WinitPointerData>>) {
        let mut new_pointers = Vec::new();
        for pointer in self.pointers.drain(..) {
            if let Some(pointer) = pointer.upgrade() {
                if pointer.pointer() != removed.upgrade().unwrap().pointer() {
                    new_pointers.push(Arc::downgrade(&pointer));
                }
            }
        }

        self.pointers = new_pointers;
    }

    /// Reload the cursor style on the given window.
    pub fn reload_cursor_style(&mut self) {
        if self.cursor_visible {
            match &self.selected_cursor {
                SelectedCursor::Named(icon) => self.set_cursor_icon(*icon),
                SelectedCursor::Custom(cursor) => self.apply_custom_cursor(cursor),
            }
        } else {
            self.set_cursor_visible(self.cursor_visible);
        }
    }

    /// Set the cursor icon.
    pub fn set_cursor_icon(&mut self, cursor_icon: CursorIcon) {
        self.selected_cursor = SelectedCursor::Named(cursor_icon);

        if !self.cursor_visible {
            return;
        }

        self.apply_on_pointer(|pointer, _| {
            if pointer.set_cursor(&self.handle.connection, cursor_icon).is_err() {
                warn!("Failed to set cursor to {:?}", cursor_icon);
            }
        })
    }

    /// Set the custom cursor icon.
    pub(crate) fn set_custom_cursor(&mut self, cursor: CoreCustomCursor) {
        let cursor = match cursor.cast_ref::<WaylandCustomCursor>() {
            Some(cursor) => cursor,
            None => {
                tracing::error!("unrecognized cursor passed to Wayland backend");
                return;
            },
        };

        let cursor = {
            let mut pool = self.image_pool.lock().unwrap();
            CustomCursor::new(&mut pool, cursor)
        };

        if self.cursor_visible {
            self.apply_custom_cursor(&cursor);
        }

        self.selected_cursor = SelectedCursor::Custom(cursor);
    }

    fn apply_custom_cursor(&self, cursor: &CustomCursor) {
        self.apply_on_pointer(|pointer, data| {
            let surface = pointer.surface();

            let scale = if let Some(viewport) = data.data().viewport() {
                let scale = self.scale_factor();
                let size = dpi::PhysicalSize::new(cursor.w, cursor.h).to_logical(scale);
                viewport.set_destination(size.width, size.height);
                scale
            } else if surface.version() >= 3 {
                let scale = surface.data::<SurfaceData<()>>().unwrap().scale_factor();
                surface.set_buffer_scale(scale);
                scale as f64
            } else {
                1.
            };

            surface.attach(Some(cursor.buffer.wl_buffer()), 0, 0);
            if surface.version() >= 4 {
                surface.damage_buffer(0, 0, cursor.w, cursor.h);
            } else {
                let size = dpi::PhysicalSize::new(cursor.w, cursor.h).to_logical(scale);
                surface.damage(0, 0, size.width, size.height);
            }
            surface.commit();

            let serial = pointer
                .pointer()
                .data::<PointerData<WinitPointerData>>()
                .and_then(|data| data.latest_enter_serial())
                .unwrap();

            let hotspot =
                dpi::PhysicalPosition::new(cursor.hotspot_x, cursor.hotspot_y).to_logical(scale);
            pointer.pointer().set_cursor(serial, Some(surface), hotspot.x, hotspot.y);
        });
    }

    pub fn set_cursor(&mut self, cursor: Cursor) {
        match cursor {
            Cursor::Icon(icon) => self.set_cursor_icon(icon),
            Cursor::Custom(cursor) => self.set_custom_cursor(cursor),
        }
    }

    /// Set the cursor grabbing state on the top-level.
    pub fn set_cursor_grab(&mut self, mode: CursorGrabMode) -> Result<(), RequestError> {
        if self.cursor_grab_mode.user_grab_mode == mode {
            return Ok(());
        }

        self.set_cursor_grab_inner(mode)?;
        // Update user grab on success.
        self.cursor_grab_mode.user_grab_mode = mode;
        Ok(())
    }

    /// Set the grabbing state on the surface.
    fn set_cursor_grab_inner(&mut self, mode: CursorGrabMode) -> Result<(), RequestError> {
        let pointer_constraints = match self.pointer_constraints.as_ref() {
            Some(pointer_constraints) => pointer_constraints,
            None if mode == CursorGrabMode::None => return Ok(()),
            None => {
                return Err(
                    NotSupportedError::new("zwp_pointer_constraints is not available").into()
                );
            },
        };

        let mut unset_old = false;
        match self.cursor_grab_mode.current_grab_mode {
            CursorGrabMode::None => unset_old = true,
            CursorGrabMode::Confined => self.apply_on_pointer(|_, data| {
                data.data().unconfine_pointer();
                unset_old = true;
            }),
            CursorGrabMode::Locked => {
                self.apply_on_pointer(|_, data| {
                    data.data().unlock_pointer();
                    unset_old = true;
                });
            },
        }

        // In case we haven't unset the old mode, it means that we don't have a cursor above
        // the window, thus just wait for it to re-appear.
        if !unset_old {
            return Ok(());
        }

        let mut set_mode = false;
        let surface = self.window.wl_surface();
        match mode {
            CursorGrabMode::Locked => self.apply_on_pointer(|pointer, data| {
                let pointer = pointer.pointer();
                data.data().lock_pointer(pointer_constraints, surface, pointer, &self.queue_handle);
                set_mode = true;
            }),
            CursorGrabMode::Confined => self.apply_on_pointer(|pointer, data| {
                let pointer = pointer.pointer();
                data.data().confine_pointer(
                    pointer_constraints,
                    surface,
                    pointer,
                    &self.queue_handle,
                );
                set_mode = true;
            }),
            CursorGrabMode::None => {
                // Current lock/confine was already removed.
                set_mode = true;
            },
        }

        // Replace the current grab mode after we've ensure that it got updated.
        if set_mode {
            self.cursor_grab_mode.current_grab_mode = mode;
        }

        Ok(())
    }

    /// Set the position of the cursor.
    pub fn set_cursor_position(&self, position: dpi::Position) -> Result<(), RequestError> {
        let scale_factor = self.scale_factor();
        let position = position.to_logical(scale_factor);
        if self.pointer_constraints.is_none() {
            return Err(NotSupportedError::new("zwp_pointer_constraints is not available").into());
        }

        // Position can be set only for locked cursor.
        if self.cursor_grab_mode.current_grab_mode != CursorGrabMode::Locked {
            return Err(NotSupportedError::new(
                "cursor position could only be changed for locked pointer",
            )
            .into());
        }

        self.apply_on_pointer(|_, data| {
            data.data().set_locked_cursor_position(position.x, position.y);
        });

        Ok(())
    }

    /// Set the visibility state of the cursor.
    pub fn set_cursor_visible(&mut self, cursor_visible: bool) {
        self.cursor_visible = cursor_visible;

        if self.cursor_visible {
            match &self.selected_cursor {
                SelectedCursor::Named(icon) => self.set_cursor_icon(*icon),
                SelectedCursor::Custom(cursor) => self.apply_custom_cursor(cursor),
            }
        } else {
            for pointer in self.pointers.iter().filter_map(|pointer| pointer.upgrade()) {
                if let Some(latest_enter_serial) =
                    pointer.pointer().winit_data().latest_enter_serial()
                {
                    pointer.pointer().set_cursor(latest_enter_serial, None, 0, 0);
                }
            }
        }
    }
}
