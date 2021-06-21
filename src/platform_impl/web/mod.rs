// Brief introduction to the internals of the web backend:
// The web backend used to support both wasm-bindgen and stdweb as methods of binding to the
// environment. Because they are both supporting the same underlying APIs, the actual web bindings
// are cordoned off into backend abstractions, which present the thinnest unifying layer possible.
//
// When adding support for new events or interactions with the browser, first consult trusted
// documentation (such as MDN) to ensure it is well-standardised and supported across many browsers.
// Once you have decided on the relevant web APIs, add support to both backends.
//
// The backend is used by the rest of the module to implement Winit's business logic, which forms
// the rest of the code. 'device', 'error', 'monitor', and 'window' define web-specific structures
// for winit's cross-platform structures. They are all relatively simple translations.
//
// The event_loop module handles listening for and processing events. 'Proxy' implements
// EventLoopProxy and 'WindowTarget' implements EventLoopWindowTarget. WindowTarget also handles
// registering the event handlers. The 'Execution' struct in the 'runner' module handles taking
// incoming events (from the registered handlers) and ensuring they are passed to the user in a
// compliant way.

mod device;
mod error;
mod event_loop;
mod monitor;
mod window;
mod keyboard;

#[path = "web_sys/mod.rs"]
mod backend;

pub use self::device::Id as DeviceId;
pub use self::error::OsError;
pub use self::event_loop::{
    EventLoop, Proxy as EventLoopProxy, WindowTarget as EventLoopWindowTarget,
};
pub use self::monitor::{Handle as MonitorHandle, Mode as VideoMode};
pub use self::window::{
    Id as WindowId, PlatformSpecificBuilderAttributes as PlatformSpecificWindowBuilderAttributes,
    Window,
};

pub(crate) use self::keyboard::KeyEventExtra;
pub(crate) use crate::icon::NoIcon as PlatformIcon;

#[derive(Clone, Copy)]
pub(crate) struct ScaleChangeArgs {
    old_scale: f64,
    new_scale: f64,
}
