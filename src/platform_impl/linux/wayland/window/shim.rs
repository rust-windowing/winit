use std::cell::Cell;
use std::mem::ManuallyDrop;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use sctk::reexports::client::protocol::wl_compositor::WlCompositor;
use sctk::reexports::client::protocol::wl_output::WlOutput;
use sctk::reexports::client::Attached;
use sctk::reexports::protocols::staging::xdg_activation::v1::client::xdg_activation_token_v1;
use sctk::reexports::protocols::staging::xdg_activation::v1::client::xdg_activation_v1::XdgActivationV1;

use sctk::environment::Environment;
use sctk::window::{Decorations, Window};
use wayland_protocols::viewporter::client::wp_viewport::WpViewport;

use crate::dpi::{LogicalPosition, LogicalSize};

use crate::event::{Ime, WindowEvent};
use crate::platform_impl::wayland;
use crate::platform_impl::wayland::env::WinitEnv;
use crate::platform_impl::wayland::event_loop::{EventSink, WinitState};
use crate::platform_impl::wayland::protocols::wp_fractional_scale_v1::WpFractionalScaleV1;
use crate::platform_impl::wayland::seat::pointer::WinitPointer;
use crate::platform_impl::wayland::seat::text_input::TextInputHandler;
use crate::platform_impl::wayland::WindowId;
use crate::window::{CursorGrabMode, CursorIcon, ImePurpose, Theme, UserAttentionType};

use super::WinitFrame;

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

    /// Change cursor grabbing mode.
    SetCursorGrabMode(CursorGrabMode),

    /// Set cursor position.
    SetLockedCursorPosition(LogicalPosition<u32>),

    /// Drag window.
    DragWindow,

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
    ImePosition(LogicalPosition<u32>),

    /// Enable IME on the given window.
    AllowIme(bool),

    /// Set the IME purpose.
    ImePurpose(ImePurpose),

    /// Mark the window as opaque.
    Transparent(bool),

    /// Request Attention.
    ///
    /// `None` unsets the attention request.
    Attention(Option<UserAttentionType>),

    /// Passthrough mouse input to underlying windows.
    PassthroughMouseInput(bool),

    /// Redraw was requested.
    Redraw,

    /// Window should be closed.
    Close,

    /// Change window theme.
    Theme(Option<Theme>),
}

// The window update comming from the compositor.
#[derive(Default, Debug, Clone, Copy)]
pub struct WindowCompositorUpdate {
    /// New window size.
    pub size: Option<LogicalSize<u32>>,

    /// New scale factor.
    pub scale_factor: Option<f64>,

    /// Close the window.
    pub close_window: bool,
}

impl WindowCompositorUpdate {
    pub fn new() -> Self {
        Default::default()
    }
}

/// Pending update to a window requested by the user.
#[derive(Default, Debug, Clone, Copy)]
pub struct WindowUserRequest {
    /// Whether `redraw` was requested.
    pub redraw_requested: bool,

    /// Wether the frame should be refreshed.
    pub refresh_frame: bool,
}

impl WindowUserRequest {
    pub fn new() -> Self {
        Default::default()
    }
}

/// A handle to perform operations on SCTK window
/// and react to events.
pub struct WindowHandle {
    /// An actual window.
    pub window: ManuallyDrop<Window<WinitFrame>>,

    /// The state of the fractional scaling handlers for the window.
    pub fractional_scaling_state: Option<FractionalScalingState>,

    /// The scale factor of the window.
    pub scale_factor: Arc<Mutex<f64>>,

    /// The current size of the window.
    pub size: Arc<Mutex<LogicalSize<u32>>>,

    /// A pending requests to SCTK window.
    pub pending_window_requests: Arc<Mutex<Vec<WindowRequest>>>,

    /// Current cursor icon.
    pub cursor_icon: Cell<CursorIcon>,

    /// Whether the window is resizable.
    pub is_resizable: Cell<bool>,

    /// Whether the window has keyboard focus.
    pub has_focus: Arc<AtomicBool>,

    /// Allow IME events for that window.
    pub ime_allowed: Cell<bool>,

    /// IME purpose for that window.
    pub ime_purpose: Cell<ImePurpose>,

    /// Wether the window is transparent.
    pub transparent: Cell<bool>,

    /// Visible cursor or not.
    cursor_visible: Cell<bool>,

    /// Cursor confined to the surface.
    cursor_grab_mode: Cell<CursorGrabMode>,

    /// Pointers over the current surface.
    pointers: Vec<WinitPointer>,

    /// Text inputs on the current surface.
    text_inputs: Vec<TextInputHandler>,

    /// XdgActivation object.
    xdg_activation: Option<Attached<XdgActivationV1>>,

    /// Indicator whether user attention is requested.
    attention_requested: Cell<bool>,

    /// Compositor
    compositor: Attached<WlCompositor>,
}

impl WindowHandle {
    pub fn new(
        env: &Environment<WinitEnv>,
        window: Window<WinitFrame>,
        size: Arc<Mutex<LogicalSize<u32>>>,
        has_focus: Arc<AtomicBool>,
        fractional_scaling_state: Option<FractionalScalingState>,
        scale_factor: f64,
        pending_window_requests: Arc<Mutex<Vec<WindowRequest>>>,
    ) -> Self {
        let xdg_activation = env.get_global::<XdgActivationV1>();
        // Unwrap is safe, since we can't create window without compositor anyway and won't be
        // here.
        let compositor = env.get_global::<WlCompositor>().unwrap();

        Self {
            window: ManuallyDrop::new(window),
            fractional_scaling_state,
            scale_factor: Arc::new(Mutex::new(scale_factor)),
            size,
            pending_window_requests,
            cursor_icon: Cell::new(CursorIcon::Default),
            is_resizable: Cell::new(true),
            transparent: Cell::new(false),
            cursor_grab_mode: Cell::new(CursorGrabMode::None),
            cursor_visible: Cell::new(true),
            pointers: Vec::new(),
            text_inputs: Vec::new(),
            xdg_activation,
            attention_requested: Cell::new(false),
            compositor,
            ime_allowed: Cell::new(false),
            ime_purpose: Cell::new(ImePurpose::default()),
            has_focus,
        }
    }

    pub fn scale_factor(&self) -> f64 {
        *self.scale_factor.lock().unwrap()
    }

    pub fn set_cursor_grab(&self, mode: CursorGrabMode) {
        // The new requested state matches the current confine status, return.
        let old_mode = self.cursor_grab_mode.replace(mode);
        if old_mode == mode {
            return;
        }

        // Clear old pointer data.
        match old_mode {
            CursorGrabMode::None => (),
            CursorGrabMode::Confined => self.pointers.iter().for_each(|p| p.unconfine()),
            CursorGrabMode::Locked => self.pointers.iter().for_each(|p| p.unlock()),
        }

        let surface = self.window.surface();
        match mode {
            CursorGrabMode::Locked => self.pointers.iter().for_each(|p| p.lock(surface)),
            CursorGrabMode::Confined => self.pointers.iter().for_each(|p| p.confine(surface)),
            CursorGrabMode::None => {
                // Current lock/confine was already removed.
            }
        }
    }

    pub fn set_locked_cursor_position(&self, position: LogicalPosition<u32>) {
        // XXX the cursor locking is ensured inside `Window`.
        self.pointers
            .iter()
            .for_each(|p| p.set_cursor_position(position.x, position.y));
    }

    pub fn set_user_attention(&self, request_type: Option<UserAttentionType>) {
        let xdg_activation = match self.xdg_activation.as_ref() {
            None => return,
            Some(xdg_activation) => xdg_activation,
        };

        //  Urgency is only removed by the compositor and there's no need to raise urgency when it
        //  was already raised.
        if request_type.is_none() || self.attention_requested.get() {
            return;
        }

        let xdg_activation_token = xdg_activation.get_activation_token();
        let surface = self.window.surface();
        let window_id = wayland::make_wid(surface);
        let xdg_activation = xdg_activation.clone();

        xdg_activation_token.quick_assign(move |xdg_token, event, mut dispatch_data| {
            let token = match event {
                xdg_activation_token_v1::Event::Done { token } => token,
                _ => return,
            };

            let winit_state = dispatch_data.get::<WinitState>().unwrap();
            let window_handle = match winit_state.window_map.get_mut(&window_id) {
                Some(window_handle) => window_handle,
                None => return,
            };

            let surface = window_handle.window.surface();
            xdg_activation.activate(token, surface);

            // Mark that attention request was done and drop the token.
            window_handle.attention_requested.replace(false);
            xdg_token.destroy();
        });

        xdg_activation_token.set_surface(surface);
        xdg_activation_token.commit();
        self.attention_requested.replace(true);
    }

    /// Pointer appeared over the window.
    pub fn pointer_entered(&mut self, pointer: WinitPointer) {
        let position = self.pointers.iter().position(|p| *p == pointer);

        if position.is_none() {
            let surface = self.window.surface();
            match self.cursor_grab_mode.get() {
                CursorGrabMode::None => (),
                CursorGrabMode::Locked => pointer.lock(surface),
                CursorGrabMode::Confined => pointer.confine(surface),
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

            // Drop the grabbing mode.
            match self.cursor_grab_mode.get() {
                CursorGrabMode::None => (),
                CursorGrabMode::Locked => pointer.unlock(),
                CursorGrabMode::Confined => pointer.unconfine(),
            }
        }
    }

    pub fn text_input_entered(&mut self, text_input: TextInputHandler) {
        if !self.text_inputs.iter().any(|t| *t == text_input) {
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

    pub fn passthrough_mouse_input(&self, passthrough_mouse_input: bool) {
        if passthrough_mouse_input {
            let region = self.compositor.create_region();
            region.add(0, 0, 0, 0);
            self.window
                .surface()
                .set_input_region(Some(&region.detach()));
            region.destroy();
        } else {
            // Using `None` results in the entire window being clickable.
            self.window.surface().set_input_region(None);
        }
    }

    pub fn set_transparent(&self, transparent: bool) {
        self.transparent.set(transparent);
        let surface = self.window.surface();
        if transparent {
            surface.set_opaque_region(None);
        } else {
            let region = self.compositor.create_region();
            region.add(0, 0, i32::MAX, i32::MAX);
            surface.set_opaque_region(Some(&region.detach()));
            region.destroy();
        }
    }

    pub fn set_ime_allowed(&self, allowed: bool, event_sink: &mut EventSink) {
        if self.ime_allowed.get() == allowed {
            return;
        }

        self.ime_allowed.replace(allowed);
        let window_id = wayland::make_wid(self.window.surface());

        let purpose = allowed.then(|| self.ime_purpose.get());
        for text_input in self.text_inputs.iter() {
            text_input.set_input_allowed(purpose);
        }

        let event = if allowed {
            WindowEvent::Ime(Ime::Enabled)
        } else {
            WindowEvent::Ime(Ime::Disabled)
        };

        event_sink.push_window_event(event, window_id);
    }

    pub fn set_ime_purpose(&self, purpose: ImePurpose) {
        if self.ime_purpose.get() == purpose {
            return;
        }

        self.ime_purpose.replace(purpose);

        if self.ime_allowed.get() {
            for text_input in self.text_inputs.iter() {
                text_input.set_content_type_by_purpose(purpose);
            }
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

    pub fn drag_window(&self) {
        for pointer in self.pointers.iter() {
            pointer.drag_window(&self.window);
        }
    }
}

#[inline]
pub fn handle_window_requests(winit_state: &mut WinitState) {
    let window_map = &mut winit_state.window_map;
    let window_user_requests = &mut winit_state.window_user_requests;
    let window_compositor_updates = &mut winit_state.window_compositor_updates;
    let mut windows_to_close: Vec<WindowId> = Vec::new();

    // Process the rest of the events.
    for (window_id, window_handle) in window_map.iter_mut() {
        let mut requests = window_handle.pending_window_requests.lock().unwrap();
        let requests = requests.drain(..);
        for request in requests {
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
                WindowRequest::ImePosition(position) => {
                    window_handle.set_ime_position(position);
                }
                WindowRequest::AllowIme(allow) => {
                    let event_sink = &mut winit_state.event_sink;
                    window_handle.set_ime_allowed(allow, event_sink);
                }
                WindowRequest::ImePurpose(purpose) => {
                    window_handle.set_ime_purpose(purpose);
                }
                WindowRequest::SetCursorGrabMode(mode) => {
                    window_handle.set_cursor_grab(mode);
                }
                WindowRequest::SetLockedCursorPosition(position) => {
                    window_handle.set_locked_cursor_position(position);
                }
                WindowRequest::DragWindow => {
                    window_handle.drag_window();
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
                WindowRequest::Transparent(transparent) => {
                    window_handle.set_transparent(transparent);

                    // This requires surface commit.
                    let window_request = window_user_requests.get_mut(window_id).unwrap();
                    window_request.redraw_requested = true;
                }
                WindowRequest::Decorate(decorate) => {
                    let decorations = match decorate {
                        true => Decorations::FollowServer,
                        false => Decorations::None,
                    };

                    window_handle.window.set_decorate(decorations);

                    // We should refresh the frame to apply decorations change.
                    let window_request = window_user_requests.get_mut(window_id).unwrap();
                    window_request.refresh_frame = true;
                }
                WindowRequest::Resizeable(resizeable) => {
                    window_handle.window.set_resizable(resizeable);

                    // We should refresh the frame to update button state.
                    let window_request = window_user_requests.get_mut(window_id).unwrap();
                    window_request.refresh_frame = true;
                }
                WindowRequest::Title(title) => {
                    window_handle.window.set_title(title);

                    // We should refresh the frame to draw new title.
                    let window_request = window_user_requests.get_mut(window_id).unwrap();
                    window_request.refresh_frame = true;
                }
                WindowRequest::MinSize(size) => {
                    let size = size.map(|size| (size.width, size.height));
                    window_handle.window.set_min_size(size);

                    let window_request = window_user_requests.get_mut(window_id).unwrap();
                    window_request.refresh_frame = true;
                }
                WindowRequest::MaxSize(size) => {
                    let size = size.map(|size| (size.width, size.height));
                    window_handle.window.set_max_size(size);

                    let window_request = window_user_requests.get_mut(window_id).unwrap();
                    window_request.refresh_frame = true;
                }
                WindowRequest::FrameSize(size) => {
                    if !window_handle.is_resizable.get() {
                        // On Wayland non-resizable window is achieved by setting both min and max
                        // size of the window to the same value.
                        let size = Some((size.width, size.height));
                        window_handle.window.set_max_size(size);
                        window_handle.window.set_min_size(size);
                    }

                    window_handle.window.resize(size.width, size.height);

                    // We should refresh the frame after resize.
                    let window_request = window_user_requests.get_mut(window_id).unwrap();
                    window_request.refresh_frame = true;
                }
                WindowRequest::PassthroughMouseInput(passthrough) => {
                    window_handle.passthrough_mouse_input(passthrough);

                    let window_request = window_user_requests.get_mut(window_id).unwrap();
                    window_request.refresh_frame = true;
                }
                WindowRequest::Attention(request_type) => {
                    window_handle.set_user_attention(request_type);
                }
                WindowRequest::Redraw => {
                    let window_request = window_user_requests.get_mut(window_id).unwrap();
                    window_request.redraw_requested = true;
                }
                WindowRequest::Close => {
                    // The window was requested to be closed.
                    windows_to_close.push(*window_id);

                    // Send event that the window was destroyed.
                    let event_sink = &mut winit_state.event_sink;
                    event_sink.push_window_event(WindowEvent::Destroyed, *window_id);
                }
                WindowRequest::Theme(_theme) => {
                    #[cfg(feature = "sctk-adwaita")]
                    {
                        window_handle.window.set_frame_config(match _theme {
                            Some(theme) => theme.into(),
                            None => sctk_adwaita::FrameConfig::auto(),
                        });

                        let window_requst = window_user_requests.get_mut(window_id).unwrap();
                        window_requst.refresh_frame = true;
                    }
                }
            };
        }
    }

    // Close the windows.
    for window in windows_to_close {
        let _ = window_map.remove(&window);
        let _ = window_user_requests.remove(&window);
        let _ = window_compositor_updates.remove(&window);
    }
}

impl Drop for WindowHandle {
    fn drop(&mut self) {
        // Drop the fractional scaling before the surface.
        let _ = self.fractional_scaling_state.take();

        unsafe {
            let surface = self.window.surface().clone();
            // The window must be destroyed before wl_surface.
            ManuallyDrop::drop(&mut self.window);
            surface.destroy();
        }
    }
}

/// Fractional scaling objects.
pub struct FractionalScalingState {
    /// The wp-viewport of the window.
    pub viewport: WpViewport,

    /// The wp-fractional-scale of the window surface.
    pub fractional_scale: WpFractionalScaleV1,
}

impl FractionalScalingState {
    pub fn new(viewport: WpViewport, fractional_scale: WpFractionalScaleV1) -> Self {
        Self {
            viewport,
            fractional_scale,
        }
    }
}

impl Drop for FractionalScalingState {
    fn drop(&mut self) {
        self.viewport.destroy();
        self.fractional_scale.destroy();
    }
}
