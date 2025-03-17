use std::sync::atomic::{AtomicBool, Ordering};

use winit_core::application::ApplicationHandler;
use winit_core::error::{EventLoopError, NotSupportedError};
use winit_core::event_loop::ActiveEventLoop as RootActiveEventLoop;

use crate::{
    HasMonitorPermissionFuture, MonitorPermissionFuture, PollStrategy, WaitUntilStrategy, backend,
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
pub struct PlatformSpecificEventLoopAttributes {}

static EVENT_LOOP_CREATED: AtomicBool = AtomicBool::new(false);

impl EventLoop {
    pub fn new(_: &PlatformSpecificEventLoopAttributes) -> Result<Self, EventLoopError> {
        if EVENT_LOOP_CREATED.swap(true, Ordering::Relaxed) {
            // For better cross-platformness.
            return Err(EventLoopError::RecreationAttempt);
        }

        Ok(EventLoop { elw: ActiveEventLoop::new() })
    }

    fn allow_event_loop_recreation() {
        EVENT_LOOP_CREATED.store(false, Ordering::Relaxed);
    }

    pub fn register_app<A: ApplicationHandler + 'static>(self, app: A) {
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
