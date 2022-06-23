#![cfg(any(
    target_os = "windows",
    target_os = "macos",
    target_os = "android",
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]

use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    process,
};

use crate::{
    event::Event,
    event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget},
};

/// Additional methods on [`EventLoop`] to return control flow to the caller.
pub trait EventLoopExtRunReturn {
    /// A type provided by the user that can be passed through [`Event::UserEvent`].
    type UserEvent;

    /// Initializes the `winit` event loop.
    ///
    /// Unlike [`EventLoop::run`], this function accepts non-`'static` (i.e. non-`move`) closures
    /// and returns control flow to the caller when `control_flow` is set to [`ControlFlow::Exit`].
    ///
    /// # Caveats
    ///
    /// Despite its appearance at first glance, this is *not* a perfect replacement for
    /// `poll_events`. For example, this function will not return on Windows or macOS while a
    /// window is getting resized, resulting in all application logic outside of the
    /// `event_handler` closure not running until the resize operation ends. Other OS operations
    /// may also result in such freezes. This behavior is caused by fundamental limitations in the
    /// underlying OS APIs, which cannot be hidden by `winit` without severe stability repercussions.
    ///
    /// You are strongly encouraged to use `run`, unless the use of this is absolutely necessary.
    ///
    /// ## Platform-specific
    ///
    /// - **Unix-alikes** (**X11** or **Wayland**): This function returns `1` upon disconnection from
    ///   the display server.
    fn run_return<'a, F>(&'a mut self, event_handler: F) -> i32
    where
        F: FnMut(
            Event<'_, Self::UserEvent>,
            &'a EventLoopWindowTarget<Self::UserEvent>,
            &mut ControlFlow,
        );
}

impl<T> EventLoopExtRunReturn for EventLoop<T> {
    type UserEvent = T;

    fn run_return<'a, F>(&'a mut self, event_handler: F) -> i32
    where
        F: FnMut(
            Event<'_, Self::UserEvent>,
            &'a EventLoopWindowTarget<Self::UserEvent>,
            &mut ControlFlow,
        ),
    {
        self.event_loop.run_return(event_handler)
    }
}

impl<T> crate::platform_impl::EventLoop<T> {
    pub fn run<F>(mut self, callback: F) -> !
    where
        F: 'static + FnMut(Event<'_, T>, &'static EventLoopWindowTarget<T>, &mut ControlFlow),
    {
        // SAFETY: `process::exit` will terminate the entire program before `self` is
        // dropped, and `catch_unwind` prevents control from from exiting this function
        // by panicking, therefore it will live for the rest of the program ('static).
        //
        // I believe this pointer casting is the correct way to do it because that's how
        // `Box::leak` is implemented (https://doc.rust-lang.org/1.60.0/src/alloc/boxed.rs.html#1147-1152)
        let this: &'static mut Self = unsafe { &mut *(&mut self as *mut Self) };
        // Note: we don't touch `callback` again if this unwinds, so it doesn't matter
        // if it's unwind safe.
        let exit_code = catch_unwind(AssertUnwindSafe(|| this.run_return(callback)))
            // 101 seems to be the status code Rust uses for panics.
            // Note: the panic message gets printed before unwinding, so we don't have to print it
            // ourselves.
            .unwrap_or(101);
        process::exit(exit_code);
    }
}
