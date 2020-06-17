#![cfg(any(
    target_os = "windows",
    target_os = "macos",
    target_os = "android",
    target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "netbsd", target_os = "openbsd"
))]

use crate::{
    event::Event,
    event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget},
};

/// Additional methods on `EventLoop` that are specific to desktop platforms.
pub trait EventLoopExtDesktop {
    /// A type provided by the user that can be passed through `Event::UserEvent`.
    type UserEvent;

    /// Initializes the `winit` event loop.
    ///
    /// Unlike `run`, this function accepts non-`'static` (i.e. non-`move`) closures and returns
    /// control flow to the caller when `control_flow` is set to `ControlFlow::Exit`.
    ///
    /// # Caveats
    /// Despite its apperance at first glance, this is *not* a perfect replacement for
    /// `poll_events`. For example, this function will not return on Windows or macOS while a
    /// window is getting resized, resulting in all application logic outside of the
    /// `event_handler` closure not running until the resize operation ends. Other OS operations
    /// may also result in such freezes. This behavior is caused by fundamental limitations in the
    /// underyling OS APIs, which cannot be hidden by Winit without severe stability reprecussions.
    ///
    /// You are strongly encouraged to use `run`, unless the use of this is absolutely necessary.
    fn run_return<F>(&mut self, event_handler: F)
    where
        F: FnMut(
            Event<'_, Self::UserEvent>,
            &EventLoopWindowTarget<Self::UserEvent>,
            &mut ControlFlow,
        );
}

impl<T> EventLoopExtDesktop for EventLoop<T> {
    type UserEvent = T;

    fn run_return<F>(&mut self, event_handler: F)
    where
        F: FnMut(
            Event<'_, Self::UserEvent>,
            &EventLoopWindowTarget<Self::UserEvent>,
            &mut ControlFlow,
        ),
    {
        self.event_loop.run_return(event_handler)
    }
}
