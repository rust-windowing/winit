//! A state that we pass around in a dispatch.

use std::collections::HashMap;

use super::EventSink;
use crate::platform_impl::wayland::window::shim::{WindowHandle, WindowUpdate};
use crate::platform_impl::wayland::WindowId;

/// Wrapper to carry winit's mutable internal state.
pub struct WinitState {
    /// A sink for various events that is being filled during dispatching
    /// event loop and forwarded as window and device events to the users
    /// of the crate afterwards.
    pub event_sink: EventSink,

    /// Window updates, which are coming from SCTK or compositor, those require
    /// calling back to the winit's user, and so handled right in event loop, unlike
    /// the ones in coming from `window_requests_sender`.
    pub window_updates: HashMap<WindowId, WindowUpdate>,

    /// Window map containing all sctk's windows, since those windows
    /// aren't allowed to be send to other threads they live on event loop's thread,
    /// and requests from winit's windows are being forwarded to them either via
    /// `WindowUpdate` or `window_requests_sender` channel.
    pub window_map: HashMap<WindowId, WindowHandle>,
}
