use super::{backend, HasMonitorPermissionFuture, MonitorPermissionFuture};
use crate::application::ApplicationHandler;
use crate::error::{EventLoopError, NotSupportedError};
use crate::event_loop::ActiveEventLoop as RootActiveEventLoop;
use crate::platform::web::{PollStrategy, WaitUntilStrategy};

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
pub(crate) struct PlatformSpecificEventLoopAttributes {}

impl EventLoop {
    pub(crate) fn new(_: &PlatformSpecificEventLoopAttributes) -> Result<Self, EventLoopError> {
        Ok(EventLoop { elw: ActiveEventLoop::new() })
    }

    pub fn run_app<A: ApplicationHandler + 'static>(self, app: A) -> Result<(), EventLoopError> {
        self.elw.run(Box::new(app));
        Ok(())
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

    pub(crate) fn request_detailed_monitor_permission(&self) -> MonitorPermissionFuture {
        self.elw.request_detailed_monitor_permission()
    }

    pub fn has_detailed_monitor_permission(&self) -> HasMonitorPermissionFuture {
        self.elw.runner.monitor().has_detailed_monitor_permission_async()
    }
}
