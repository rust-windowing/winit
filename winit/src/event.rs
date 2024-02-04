//! The [`Event`] enum and assorted supporting types.
//!
//! These are sent to the closure given to [`EventLoop::run(...)`], where they get
//! processed and used to modify the program state. For more details, see the root-level documentation.
//!
//! Some of these events represent different "parts" of a traditional event-handling loop. You could
//! approximate the basic ordering loop of [`EventLoop::run(...)`] like this:
//!
//! ```rust,ignore
//! let mut start_cause = StartCause::Init;
//!
//! while !elwt.exiting() {
//!     event_handler(NewEvents(start_cause), elwt);
//!
//!     for e in (window events, user events, device events) {
//!         event_handler(e, elwt);
//!     }
//!
//!     for w in (redraw windows) {
//!         event_handler(RedrawRequested(w), elwt);
//!     }
//!
//!     event_handler(AboutToWait, elwt);
//!     start_cause = wait_if_necessary();
//! }
//!
//! event_handler(LoopExiting, elwt);
//! ```
//!
//! This leaves out timing details like [`ControlFlow::WaitUntil`] but hopefully
//! describes what happens in what order.
//!
//! [`EventLoop::run(...)`]: crate::event_loop::EventLoop::run
//! [`ControlFlow::WaitUntil`]: crate::event_loop::ControlFlow::WaitUntil

use crate::platform_impl;

#[doc(inline)]
pub use winit_core::event::{
    DeviceEvent, DeviceId, ElementState, Force, Ime, InnerSizeIgnored, InnerSizeWriter, Modifiers,
    MouseButton, MouseScrollDelta, RawKeyEvent, StartCause, Touch, TouchPhase,
};

pub type Event<T> = winit_core::event::Event<T, KeyExtra>;
pub type WindowEvent = winit_core::event::WindowEvent<KeyExtra>;
pub type KeyEvent = winit_core::event::KeyEvent<KeyExtra>;

/// Extra keyboard information.
#[derive(Debug, Clone)]
pub struct KeyExtra {
    #[allow(dead_code)]
    pub(crate) extra: platform_impl::KeyEventExtra,
}
