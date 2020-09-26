use std::cell::Cell;
use std::sync::{Arc, Mutex};

use sctk::reexports::client::protocol::wl_output::WlOutput;

use sctk::window::{ConceptConfig, ConceptFrame, Decorations, Window};

use crate::dpi::{LogicalPosition, LogicalSize};

use crate::event::WindowEvent;
use crate::platform_impl::wayland::event_loop::WinitState;
use crate::platform_impl::wayland::seat::pointer::WinitPointer;
use crate::platform_impl::wayland::seat::text_input::TextInputHandler;
use crate::platform_impl::wayland::WindowId;
use crate::window::CursorIcon;

/// A request to SCTK window from Winit window.
#[derive(Debug, Clone)]
pub enum WindowRequest {
    /// Set fullscreen.
    ///
    /// Passing `None` will set it on the current monitor.
    Fullscreen(Option<WlOutput>),

    /// Unset fullscreen.
    UnsetFullscreen,

    /// Show cursor for the certain window or not.
    ShowCursor(bool),

    /// Change the cursor icon.
    NewCursorIcon(CursorIcon),

    /// Grab cursor.
    GrabCursor(bool),

    /// Maximize the window.
    Maximize(bool),

    /// Minimize the window.
    Minimize,

    /// Request decorations change.
    Decorate(bool),

    /// Make the window resizeable.
    Resizeable(bool),

    /// Set the title for window.
    Title(String),

    /// Min size.
    MinSize(Option<LogicalSize<u32>>),

    /// Max size.
    MaxSize(Option<LogicalSize<u32>>),

    /// New frame size.
    FrameSize(LogicalSize<u32>),

    /// Set IME window position.
    IMEPosition(LogicalPosition<u32>),

    /// Redraw was requested.
    Redraw,

    /// A new theme for a concept frame was requested.
    Theme(ConceptConfig),

    /// Window should be closed.
    Close,
}

/// Pending update to a window from SCTK window.
#[derive(Debug, Clone, Copy)]
pub struct WindowUpdate {
    /// New window size.
    pub size: Option<LogicalSize<u32>>,

    /// New scale factor.
    pub scale_factor: Option<i32>,

    /// Whether `redraw` was requested.
    pub redraw_requested: bool,

    /// Wether the frame should be refreshed.
    pub refresh_frame: bool,

    /// Close the window.
    pub close_window: bool,
}

impl WindowUpdate {
    pub fn new() -> Self {
        Self {
            size: None,
            scale_factor: None,
            redraw_requested: false,
            refresh_frame: false,
            close_window: false,
        }
    }

    pub fn take(&mut self) -> Self {
        let size = self.size.take();
        let scale_factor = self.scale_factor.take();

        let redraw_requested = self.redraw_requested;
        self.redraw_requested = false;

        let refresh_frame = self.refresh_frame;
        self.refresh_frame = false;

        let close_window = self.close_window;
        self.close_window = false;

        Self {
            size,
            scale_factor,
            redraw_requested,
            refresh_frame,
            close_window,
        }
    }
}

/// A handle to perform operations on SCTK window
/// and react to events.
pub struct WindowHandle {
    /// An actual window.
    pub window: Window<ConceptFrame>,

    /// The current size of the window.
    pub size: Arc<Mutex<LogicalSize<u32>>>,

    /// A pending requests to SCTK window.
    pub pending_window_requests: Arc<Mutex<Vec<WindowRequest>>>,

    /// Current cursor icon.
    pub cursor_icon: Cell<CursorIcon>,

    /// Visible cursor or not.
    cursor_visible: Cell<bool>,

    /// Cursor confined to the surface.
    confined: Cell<bool>,

    /// Pointers over the current surface.
    pointers: Vec<WinitPointer>,

    /// Text inputs on the current surface.
    text_inputs: Vec<TextInputHandler>,
}

impl WindowHandle {
    pub fn new(
        window: Window<ConceptFrame>,
        size: Arc<Mutex<LogicalSize<u32>>>,
        pending_window_requests: Arc<Mutex<Vec<WindowRequest>>>,
    ) -> Self {
        Self {
            window,
            size,
            pending_window_requests,
            cursor_icon: Cell::new(CursorIcon::Default),
            confined: Cell::new(false),
            cursor_visible: Cell::new(true),
            pointers: Vec::new(),
            text_inputs: Vec::new(),
        }
    }

    pub fn set_cursor_grab(&self, grab: bool) {
        // The new requested state matches the current confine status, return.
        if self.confined.get() == grab {
            return;
        }

        self.confined.replace(grab);

        for pointer in self.pointers.iter() {
            if self.confined.get() {
                let surface = self.window.surface();
                pointer.confine(&surface);
            } else {
                pointer.unconfine();
            }
        }
    }

    /// Pointer appeared over the window.
    pub fn pointer_entered(&mut self, pointer: WinitPointer) {
        let position = self.pointers.iter().position(|p| *p == pointer);

        if position.is_none() {
            if self.confined.get() {
                let surface = self.window.surface();
                pointer.confine(&surface);
            }
            self.pointers.push(pointer);
        }

        // Apply the current cursor style.
        self.set_cursor_visible(self.cursor_visible.get());
    }

    /// Pointer left the window.
    pub fn pointer_left(&mut self, pointer: WinitPointer) {
        let position = self.pointers.iter().position(|p| *p == pointer);

        if let Some(position) = position {
            let pointer = self.pointers.remove(position);

            // Drop the confined pointer.
            if self.confined.get() {
                pointer.unconfine();
            }
        }
    }

    pub fn text_input_entered(&mut self, text_input: TextInputHandler) {
        if self
            .text_inputs
            .iter()
            .find(|t| *t == &text_input)
            .is_none()
        {
            self.text_inputs.push(text_input);
        }
    }

    pub fn text_input_left(&mut self, text_input: TextInputHandler) {
        if let Some(position) = self.text_inputs.iter().position(|t| *t == text_input) {
            self.text_inputs.remove(position);
        }
    }

    pub fn set_ime_position(&self, position: LogicalPosition<u32>) {
        // XXX This won't fly unless user will have a way to request IME window per seat, since
        // the ime windows will be overlapping, but winit doesn't expose API to specify for
        // which seat we're setting IME position.
        let (x, y) = (position.x as i32, position.y as i32);
        for text_input in self.text_inputs.iter() {
            text_input.set_ime_position(x, y);
        }
    }

    pub fn set_cursor_visible(&self, visible: bool) {
        self.cursor_visible.replace(visible);
        let cursor_icon = match visible {
            true => Some(self.cursor_icon.get()),
            false => None,
        };

        for pointer in self.pointers.iter() {
            pointer.set_cursor(cursor_icon)
        }
    }

    pub fn set_cursor_icon(&self, cursor_icon: CursorIcon) {
        self.cursor_icon.replace(cursor_icon);

        if !self.cursor_visible.get() {
            return;
        }

        for pointer in self.pointers.iter() {
            pointer.set_cursor(Some(cursor_icon));
        }
    }
}

#[inline]
pub fn handle_window_requests(winit_state: &mut WinitState) {
    let window_map = &mut winit_state.window_map;
    let window_updates = &mut winit_state.window_updates;
    let mut windows_to_close: Vec<WindowId> = Vec::new();

    // Process the rest of the events.
    for (window_id, window_handle) in window_map.iter_mut() {
        let mut requests = window_handle.pending_window_requests.lock().unwrap();
        for request in requests.drain(..) {
            match request {
                WindowRequest::Fullscreen(fullscreen) => {
                    window_handle.window.set_fullscreen(fullscreen.as_ref());
                }
                WindowRequest::UnsetFullscreen => {
                    window_handle.window.unset_fullscreen();
                }
                WindowRequest::ShowCursor(show_cursor) => {
                    window_handle.set_cursor_visible(show_cursor);
                }
                WindowRequest::NewCursorIcon(cursor_icon) => {
                    window_handle.set_cursor_icon(cursor_icon);
                }
                WindowRequest::IMEPosition(position) => {
                    window_handle.set_ime_position(position);
                }
                WindowRequest::GrabCursor(grab) => {
                    window_handle.set_cursor_grab(grab);
                }
                WindowRequest::Maximize(maximize) => {
                    if maximize {
                        window_handle.window.set_maximized();
                    } else {
                        window_handle.window.unset_maximized();
                    }
                }
                WindowRequest::Minimize => {
                    window_handle.window.set_minimized();
                }
                WindowRequest::Decorate(decorate) => {
                    let decorations = match decorate {
                        true => Decorations::FollowServer,
                        false => Decorations::None,
                    };

                    window_handle.window.set_decorate(decorations);

                    // We should refresh the frame to apply decorations change.
                    let window_update = window_updates.get_mut(&window_id).unwrap();
                    window_update.refresh_frame = true;
                }
                WindowRequest::Resizeable(resizeable) => {
                    window_handle.window.set_resizable(resizeable);

                    // We should refresh the frame to update button state.
                    let window_update = window_updates.get_mut(&window_id).unwrap();
                    window_update.refresh_frame = true;
                }
                WindowRequest::Title(title) => {
                    window_handle.window.set_title(title);

                    // We should refresh the frame to draw new title.
                    let window_update = window_updates.get_mut(&window_id).unwrap();
                    window_update.refresh_frame = true;
                }
                WindowRequest::MinSize(size) => {
                    let size = size.map(|size| (size.width, size.height));
                    window_handle.window.set_min_size(size);

                    let window_update = window_updates.get_mut(&window_id).unwrap();
                    window_update.redraw_requested = true;
                }
                WindowRequest::MaxSize(size) => {
                    let size = size.map(|size| (size.width, size.height));
                    window_handle.window.set_max_size(size);

                    let window_update = window_updates.get_mut(&window_id).unwrap();
                    window_update.redraw_requested = true;
                }
                WindowRequest::FrameSize(size) => {
                    // Set new size.
                    window_handle.window.resize(size.width, size.height);

                    // We should refresh the frame after resize.
                    let window_update = window_updates.get_mut(&window_id).unwrap();
                    window_update.refresh_frame = true;
                }
                WindowRequest::Redraw => {
                    let window_update = window_updates.get_mut(&window_id).unwrap();
                    window_update.redraw_requested = true;
                }
                WindowRequest::Theme(concept_config) => {
                    window_handle.window.set_frame_config(concept_config);

                    // We should refresh the frame to apply new theme.
                    let window_update = window_updates.get_mut(&window_id).unwrap();
                    window_update.refresh_frame = true;
                }
                WindowRequest::Close => {
                    // The window was requested to be closed.
                    windows_to_close.push(*window_id);

                    // Send event that the window was destroyed.
                    let event_sink = &mut winit_state.event_sink;
                    event_sink.push_window_event(WindowEvent::Destroyed, *window_id);
                }
            };
        }
    }

    // Close the windows.
    for window in windows_to_close {
        let _ = window_map.remove(&window);
        let _ = window_updates.remove(&window);
    }
}
