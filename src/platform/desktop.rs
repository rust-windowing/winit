#![cfg(any(
    target_os = "windows",
    target_os = "macos",
    target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "netbsd", target_os = "openbsd"
))]

use event::Event;
use event_loop::{EventLoop, ControlFlow};

/// Additional methods on `EventLoop` that are specific to desktop platforms.
pub trait EventLoopExtDesktop {
    type UserEvent;
    /// Initializes the `winit` event loop.
    ///
    /// Unlikes `run`, this function *does* return control flow to the caller when `control_flow`
    /// is set to `ControlFlow::Exit`.
    fn run_return<F>(&mut self, event_handler: F)
        where F: FnMut(Event<Self::UserEvent>, &EventLoop<Self::UserEvent>, &mut ControlFlow);
}

impl<T> EventLoopExtDesktop for EventLoop<T> {
    type UserEvent = T;

    fn run_return<F>(&mut self, event_handler: F)
        where F: FnMut(Event<T>, &EventLoop<T>, &mut ControlFlow)
    {
        self.events_loop.run_return(event_handler)
    }
}
