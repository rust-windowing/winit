// Brief introduction to the internals of the Web backend:
// The Web backend used to support both wasm-bindgen and stdweb as methods of binding to the
// environment. Because they are both supporting the same underlying APIs, the actual Web bindings
// are cordoned off into backend abstractions, which present the thinnest unifying layer possible.
//
// When adding support for new events or interactions with the browser, first consult trusted
// documentation (such as MDN) to ensure it is well-standardised and supported across many browsers.
// Once you have decided on the relevant Web APIs, add support to both backends.
//
// The backend is used by the rest of the module to implement Winit's business logic, which forms
// the rest of the code. 'device', 'error', 'monitor', and 'window' define Web-specific structures
// for winit's cross-platform structures. They are all relatively simple translations.
//
// The event_loop module handles listening for and processing events. 'Proxy' implements
// EventLoopProxy and 'WindowTarget' implements ActiveEventLoop. WindowTarget also handles
// registering the event handlers. The 'Execution' struct in the 'runner' module handles taking
// incoming events (from the registered handlers) and ensuring they are passed to the user in a
// compliant way.

// TODO: FP, remove when <https://github.com/rust-lang/rust-clippy/issues/12377> is fixed.
#![allow(clippy::empty_docs)]

mod r#async;
mod cursor;
mod error;
mod event;
mod event_loop;
mod keyboard;
mod lock;
mod main_thread;
mod monitor;
mod web_sys;
mod window;

pub(crate) use cursor::{
    CustomCursor as PlatformCustomCursor, CustomCursorFuture,
    CustomCursorSource as PlatformCustomCursorSource,
};

pub(crate) use self::event_loop::{
    ActiveEventLoop, EventLoop, EventLoopProxy, OwnedDisplayHandle,
    PlatformSpecificEventLoopAttributes,
};
pub(crate) use self::keyboard::KeyEventExtra;
pub(crate) use self::monitor::{
    HasMonitorPermissionFuture, MonitorHandle, MonitorPermissionFuture, OrientationLockFuture,
    VideoModeHandle,
};
use self::web_sys as backend;
pub use self::window::{PlatformSpecificWindowAttributes, Window};
pub(crate) use crate::icon::NoIcon as PlatformIcon;
