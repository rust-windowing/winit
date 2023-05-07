//! Handling of xdg activation, which is used for user attention requests.

use std::sync::atomic::AtomicBool;
use std::sync::Weak;

use sctk::reexports::client::delegate_dispatch;
use sctk::reexports::client::globals::BindError;
use sctk::reexports::client::globals::GlobalList;
use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::reexports::client::Dispatch;
use sctk::reexports::client::{Connection, Proxy, QueueHandle};
use sctk::reexports::protocols::xdg::activation::v1::client::xdg_activation_token_v1::{
    Event as ActivationTokenEvent, XdgActivationTokenV1,
};
use sctk::reexports::protocols::xdg::activation::v1::client::xdg_activation_v1::XdgActivationV1;

use sctk::globals::GlobalData;

use crate::platform_impl::wayland::state::WinitState;

pub struct XdgActivationState {
    xdg_activation: XdgActivationV1,
}

impl XdgActivationState {
    pub fn bind(
        globals: &GlobalList,
        queue_handle: &QueueHandle<WinitState>,
    ) -> Result<Self, BindError> {
        let xdg_activation = globals.bind(queue_handle, 1..=1, GlobalData)?;
        Ok(Self { xdg_activation })
    }

    pub fn global(&self) -> &XdgActivationV1 {
        &self.xdg_activation
    }
}

impl Dispatch<XdgActivationV1, GlobalData, WinitState> for XdgActivationState {
    fn event(
        _state: &mut WinitState,
        _proxy: &XdgActivationV1,
        _event: <XdgActivationV1 as Proxy>::Event,
        _data: &GlobalData,
        _conn: &Connection,
        _qhandle: &QueueHandle<WinitState>,
    ) {
    }
}

impl Dispatch<XdgActivationTokenV1, XdgActivationTokenData, WinitState> for XdgActivationState {
    fn event(
        state: &mut WinitState,
        proxy: &XdgActivationTokenV1,
        event: <XdgActivationTokenV1 as Proxy>::Event,
        data: &XdgActivationTokenData,
        _: &Connection,
        _: &QueueHandle<WinitState>,
    ) {
        let token = match event {
            ActivationTokenEvent::Done { token } => token,
            _ => return,
        };

        state
            .xdg_activation
            .as_ref()
            .expect("got xdg_activation event without global.")
            .global()
            .activate(token, &data.surface);

        // Mark that no request attention is in process.
        if let Some(attention_requested) = data.attention_requested.upgrade() {
            attention_requested.store(false, std::sync::atomic::Ordering::Relaxed);
        }

        proxy.destroy();
    }
}

/// The data associated with the activation request.
pub struct XdgActivationTokenData {
    /// The surface we're raising.
    surface: WlSurface,

    /// Flag to throttle attention requests.
    attention_requested: Weak<AtomicBool>,
}

impl XdgActivationTokenData {
    /// Create a new data.
    ///
    /// The `attenteion_requested` is marked as `false` on complition.
    pub fn new(surface: WlSurface, attention_requested: Weak<AtomicBool>) -> Self {
        Self {
            surface,
            attention_requested,
        }
    }
}

delegate_dispatch!(WinitState: [ XdgActivationV1: GlobalData] => XdgActivationState);
delegate_dispatch!(WinitState: [ XdgActivationTokenV1: XdgActivationTokenData] => XdgActivationState);
