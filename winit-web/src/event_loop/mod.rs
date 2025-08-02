use std::sync::atomic::{AtomicBool, Ordering};

use winit_core::application::ApplicationHandler;
use winit_core::error::{EventLoopError, NotSupportedError};
use winit_core::event_loop::ActiveEventLoop as RootActiveEventLoop;

use crate::{
    backend, HasMonitorPermissionFuture, MonitorPermissionFuture, PollStrategy, WaitUntilStrategy,
};

mod proxy;
pub(crate) mod runner;
mod state;
mod window_target;

pub(crate) use window_target::ActiveEventLoop;

#[derive(Debug)]
pub struct EventLoop {
    elw: ActiveEventLoop,
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct PlatformSpecificEventLoopAttributes {
    pub skip_recreation_check: bool,
}

static EVENT_LOOP_CREATED: AtomicBool = AtomicBool::new(false);

impl EventLoop {
    pub fn new(attributes: &PlatformSpecificEventLoopAttributes) -> Result<Self, EventLoopError> {
        if !attributes.skip_recreation_check && EVENT_LOOP_CREATED.swap(true, Ordering::Relaxed) {
            // For better cross-platformness.
            return Err(EventLoopError::RecreationAttempt);
        }

        let elw = ActiveEventLoop::new();

        // Only reset the recreation check if it's not being skipped by this EventLoop.
        elw.runner.event_loop_recreation(!attributes.skip_recreation_check);

        Ok(EventLoop { elw })
    }

    fn allow_event_loop_recreation() {
        EVENT_LOOP_CREATED.store(false, Ordering::Relaxed);
    }

    pub fn run_app<A: ApplicationHandler>(self, app: A) -> ! {
        let app = Box::new(app);

        // SAFETY: The `transmute` is necessary because `run()` requires `'static`. This is safe
        // because this function will never return and all resources not cleaned up by the point we
        // `throw` will leak, making this actually `'static`.
        let app = unsafe {
            std::mem::transmute::<
                Box<dyn ApplicationHandler + '_>,
                Box<dyn ApplicationHandler + 'static>,
            >(app)
        };

        // This conceptually never returns, so do not reset the recreation check.
        self.elw.runner.event_loop_recreation(false);

        self.elw.run(app);

        // Throw an exception to break out of Rust execution and use unreachable to tell the
        // compiler this function won't return, giving it a return type of '!'
        backend::throw(
            "Using exceptions for control flow, don't mind me. This isn't actually an error!",
        );

        unreachable!();
    }

    pub fn spawn_app<A: ApplicationHandler + 'static>(self, app: A) {
        self.elw.run(Box::new(app));
    }

    pub fn window_target(&self) -> &dyn RootActiveEventLoop {
        &self.elw
    }

    pub fn set_poll_strategy(&self, strategy: PollStrategy) {
        self.elw.set_poll_strategy(strategy);
    }

    pub fn poll_strategy(&self) -> PollStrategy {
        self.elw.poll_strategy()
    }

    pub fn set_wait_until_strategy(&self, strategy: WaitUntilStrategy) {
        self.elw.set_wait_until_strategy(strategy);
    }

    pub fn wait_until_strategy(&self) -> WaitUntilStrategy {
        self.elw.wait_until_strategy()
    }

    pub fn has_multiple_screens(&self) -> Result<bool, NotSupportedError> {
        self.elw.has_multiple_screens()
    }

    pub fn request_detailed_monitor_permission(&self) -> MonitorPermissionFuture {
        MonitorPermissionFuture(self.elw.request_detailed_monitor_permission())
    }

    pub fn has_detailed_monitor_permission(&self) -> HasMonitorPermissionFuture {
        HasMonitorPermissionFuture(
            self.elw.runner.monitor().has_detailed_monitor_permission_async(),
        )
    }
}
