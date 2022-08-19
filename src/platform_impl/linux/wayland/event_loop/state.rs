//! A state that we pass around in a dispatch.

use std::collections::HashMap;

use super::EventSink;
use crate::platform_impl::wayland::window::shim::{
    WindowCompositorUpdate, WindowHandle, WindowUserRequest,
};
use crate::platform_impl::wayland::WindowId;

/// Wrapper to carry winit's state.
pub struct WinitState {
    /// A sink for window and device events that is being filled during dispatching
    /// event loop and forwarded downstream afterwards.
    pub event_sink: EventSink,

    /// Window updates comming from the user requests. Those are separatelly dispatched right after
    /// `MainEventsCleared`.
    pub window_user_requests: HashMap<WindowId, WindowUserRequest>,

    /// Window updates, which are coming from SCTK or the compositor, which require
    /// calling back to the winit's downstream. They are handled right in the event loop,
    /// unlike the ones coming from buffers on the `WindowHandle`'s.
    pub window_compositor_updates: HashMap<WindowId, WindowCompositorUpdate>,

    /// Window map containing all SCTK windows. Since those windows aren't allowed
    /// to be sent to other threads, they live on the event loop's thread
    /// and requests from winit's windows are being forwarded to them either via
    /// `WindowUpdate` or buffer on the associated with it `WindowHandle`.
    pub window_map: HashMap<WindowId, WindowHandle>,
}
