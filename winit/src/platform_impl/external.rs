//! Fallback when no built-in backend is enabled.

use winit_core::error::{EventLoopError, NotSupportedError};
use winit_core::event_loop::EventLoopProvider;

#[derive(Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct PlatformSpecificEventLoopAttributes;

pub(crate) fn create_event_loop(
    _: &mut PlatformSpecificEventLoopAttributes,
) -> Result<Box<dyn EventLoopProvider>, EventLoopError> {
    Err(NotSupportedError::new(
        "No built-in event loop backend is available; use EventLoop::from_backend",
    )
    .into())
}
