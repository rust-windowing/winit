//! Default provider for configurations without a built-in backend.
//!
//! This type cannot be constructed. Applications must supply their own provider.

use winit_core::application::ApplicationHandler;
use winit_core::cursor::{CustomCursor, CustomCursorSource};
use winit_core::error::{CustomCursorError, EventLoopError, NotSupportedError};
use winit_core::event_loop::{
    ControlFlow, DeviceEvents, EventLoopProvider, EventLoopProxy, OwnedDisplayHandle,
};

#[derive(Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct PlatformSpecificEventLoopAttributes;

#[derive(Debug)]
pub enum EventLoop {}

impl EventLoop {
    pub(crate) fn new(_: &PlatformSpecificEventLoopAttributes) -> Result<Self, EventLoopError> {
        Err(NotSupportedError::new(
            "No built-in event loop backend is available; use EventLoop::from_backend",
        )
        .into())
    }
}

impl EventLoop {
    pub fn run_app<A: ApplicationHandler + 'static>(self, _: A) -> Result<(), EventLoopError> {
        match self {}
    }
}

impl EventLoopProvider for EventLoop {
    fn run_app(self: Box<Self>, app: Box<dyn ApplicationHandler>) -> Result<(), EventLoopError> {
        (*self).run_app(app)
    }

    fn into_register(
        self: Box<Self>,
    ) -> Result<Box<dyn winit_core::event_loop::register::RegisterApp>, Box<dyn EventLoopProvider>>
    {
        Err(self)
    }

    fn into_never_return(
        self: Box<Self>,
    ) -> Result<
        Box<dyn winit_core::event_loop::never_return::RunAppNeverReturn>,
        Box<dyn EventLoopProvider>,
    > {
        Err(self)
    }

    fn create_proxy(&self) -> EventLoopProxy {
        match *self {}
    }

    fn owned_display_handle(&self) -> OwnedDisplayHandle {
        match *self {}
    }

    fn listen_device_events(&self, _: DeviceEvents) {
        match *self {}
    }

    fn set_control_flow(&self, _: ControlFlow) {
        match *self {}
    }

    fn create_custom_cursor(
        &self,
        _: CustomCursorSource,
    ) -> Result<CustomCursor, CustomCursorError> {
        match *self {}
    }
}
