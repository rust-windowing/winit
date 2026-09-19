use std::time::Duration;

use sctk::compositor::FrameCallbackData;
use sctk::reexports::client::Proxy;
use sctk::reexports::client::protocol::wl_seat::WlSeat;
use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::reexports::csd_frame::{DecorationsFrame, FrameAction, FrameClick, ResizeEdge};
use sctk::reexports::protocols::xdg::shell::client::xdg_toplevel::ResizeEdge as XdgResizeEdge;
use sctk::shell::WaylandSurface;
use sctk::shell::xdg::window::DecorationMode;
use winit_core::cursor::CursorIcon;
use winit_core::window::WindowId;

use super::{WindowState, WindowType};
use crate::state::{WindowCompositorUpdate, WinitState};

/// The state of the frame callback.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameCallbackState {
    /// No frame callback was requested.
    #[default]
    None,
    /// The frame callback was requested, but not yet arrived, the redraw events are throttled.
    Requested,
    /// The callback was marked as done, and user could receive redraw requested
    Received,
}

impl WindowState {
    /// Get the current state of the frame callback.
    pub fn frame_callback_state(&self) -> FrameCallbackState {
        self.frame_callback_state
    }

    /// The frame callback was received, but not yet sent to the user.
    pub fn frame_callback_received(&mut self) {
        self.frame_callback_state = FrameCallbackState::Received;
    }

    /// Reset the frame callbacks state.
    pub fn frame_callback_reset(&mut self) {
        self.frame_callback_state = FrameCallbackState::None;
    }

    /// Request a frame callback if we don't have one for this window in flight.
    pub fn request_frame_callback(&mut self) {
        let surface = self.window.wl_surface();
        match self.frame_callback_state {
            FrameCallbackState::None | FrameCallbackState::Received => {
                self.frame_callback_state = FrameCallbackState::Requested;
                surface.frame(&self.queue_handle, FrameCallbackData(surface.clone()));
            },
            FrameCallbackState::Requested => (),
        }
    }

    /// Tells whether the window should be closed.
    #[allow(clippy::too_many_arguments)]
    pub fn frame_click(
        &mut self,
        click: FrameClick,
        pressed: bool,
        seat: &WlSeat,
        serial: u32,
        timestamp: Duration,
        window_id: WindowId,
        updates: &mut Vec<WindowCompositorUpdate>,
    ) -> Option<bool> {
        let xdg_toplevel = self.window.xdg_toplevel()?;

        match self.frame.as_mut()?.on_click(timestamp, click, pressed)? {
            FrameAction::Minimize => xdg_toplevel.set_minimized(),
            FrameAction::Maximize => xdg_toplevel.set_maximized(),
            FrameAction::UnMaximize => xdg_toplevel.unset_maximized(),
            FrameAction::Close => WinitState::queue_close(updates, window_id),
            FrameAction::Move => self.has_pending_move = Some(serial),
            FrameAction::Resize(edge) => {
                let edge = match edge {
                    ResizeEdge::None => XdgResizeEdge::None,
                    ResizeEdge::Top => XdgResizeEdge::Top,
                    ResizeEdge::Bottom => XdgResizeEdge::Bottom,
                    ResizeEdge::Left => XdgResizeEdge::Left,
                    ResizeEdge::TopLeft => XdgResizeEdge::TopLeft,
                    ResizeEdge::BottomLeft => XdgResizeEdge::BottomLeft,
                    ResizeEdge::Right => XdgResizeEdge::Right,
                    ResizeEdge::TopRight => XdgResizeEdge::TopRight,
                    ResizeEdge::BottomRight => XdgResizeEdge::BottomRight,
                    _ => return None,
                };
                xdg_toplevel.resize(seat, serial, edge);
            },
            FrameAction::ShowMenu(x, y) => xdg_toplevel.show_window_menu(seat, serial, x, y),
            _ => (),
        };

        Some(false)
    }

    pub fn frame_point_left(&mut self) {
        if let Some(frame) = self.frame.as_mut() {
            frame.click_point_left();
        }
    }

    // Move the point over decorations.
    pub fn frame_point_moved(
        &mut self,
        seat: &WlSeat,
        surface: &WlSurface,
        timestamp: Duration,
        x: f64,
        y: f64,
    ) -> Option<CursorIcon> {
        // Take the serial if we had any, so it doesn't stick around.
        let serial = self.has_pending_move.take();

        let frame = self.frame.as_mut()?;
        let cursor = frame.click_point_moved(timestamp, &surface.id(), x, y);
        // If we have a cursor change, that means that cursor is over the decorations,
        // so try to apply move.
        if let Some(serial) = cursor.is_some().then_some(serial).flatten() {
            self.xdg_toplevel()?._move(seat, serial);
            None
        } else {
            cursor
        }
    }

    #[inline]
    pub fn is_decorated(&mut self) -> bool {
        match &mut self.window {
            WindowType::Window { last_configure, .. } => {
                let csd = last_configure
                    .as_ref()
                    .map(|configure| configure.decoration_mode == DecorationMode::Client)
                    .unwrap_or(false);
                if let Some(frame) = csd.then_some(self.frame.as_ref()).flatten() {
                    !frame.is_hidden()
                } else {
                    // Server side decorations.
                    true
                }
            },
            WindowType::Popup { .. } => false, // Popup window does not have any decoration
        }
    }

    /// Get the origin of the content surface by considering the client side decoration if available
    /// This is required for example when creating a popup, because as parent a xdg_surface must be
    /// passed but the frame is only a wl_surface
    pub fn content_surface_origin(&self) -> dpi::LogicalPosition<i32> {
        self.frame.as_ref().map(|frame| frame.location().into()).unwrap_or_else(|| (0, 0).into())
    }

    /// Refresh the decorations frame if it's present returning whether the client should redraw.
    pub fn refresh_frame(&mut self) -> bool {
        if let Some(frame) = self.frame.as_mut() {
            if !frame.is_hidden() && frame.is_dirty() {
                return frame.draw();
            }
        }

        false
    }

    fn request_decoration_mode(&self, mode: Option<DecorationMode>) {
        match &self.window {
            WindowType::Window { window, .. } => window.request_decoration_mode(mode),
            WindowType::Popup { .. } => {},
        }
    }

    /// Whether show or hide client side decorations.
    #[inline]
    pub fn set_decorate(&mut self, decorate: bool) {
        if decorate == self.decorate && !self.prefer_csd {
            return;
        }

        self.decorate = decorate;

        let last_configure = match &self.window {
            WindowType::Window { last_configure, .. } => last_configure,
            WindowType::Popup { .. } => return, // Popup does not have any decoration
        };

        match last_configure.as_ref().map(|configure| configure.decoration_mode) {
            Some(DecorationMode::Server) if !self.decorate => {
                // To disable decorations we should request client and hide the frame.
                self.request_decoration_mode(Some(DecorationMode::Client))
            },
            _ if self.decorate && self.prefer_csd => {
                self.request_decoration_mode(Some(DecorationMode::Client))
            },
            _ if self.decorate => self.request_decoration_mode(Some(DecorationMode::Server)),
            _ => (),
        }

        if let Some(frame) = self.frame.as_mut() {
            frame.set_hidden(!decorate);
            // Force the resize.
            self.resize(self.size);
        }
    }
}
