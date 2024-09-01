use super::{backend, event, window, HasMonitorPermissionFuture, MonitorPermissionFuture};
use crate::application::ApplicationHandler;
use crate::error::{EventLoopError, NotSupportedError};
use crate::event::Event;
use crate::event_loop::ActiveEventLoop as RootActiveEventLoop;
use crate::platform::web::{PollStrategy, WaitUntilStrategy};

mod proxy;
pub(crate) mod runner;
mod state;
mod window_target;

pub(crate) use proxy::EventLoopProxy;
pub(crate) use window_target::{ActiveEventLoop, OwnedDisplayHandle};

pub struct EventLoop {
    elw: ActiveEventLoop,
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PlatformSpecificEventLoopAttributes {}

impl EventLoop {
    pub(crate) fn new(_: &PlatformSpecificEventLoopAttributes) -> Result<Self, EventLoopError> {
        Ok(EventLoop { elw: ActiveEventLoop::new() })
    }

    pub fn run_app<A: ApplicationHandler>(self, mut app: A) -> ! {
        let event_loop = self.elw.clone();

        // SAFETY: Don't use `move` to make sure we leak the `event_handler` and `target`.
        let handler: Box<dyn FnMut(Event)> =
            Box::new(|event| handle_event(&mut app, &event_loop, event));

        // SAFETY: The `transmute` is necessary because `run()` requires `'static`. This is safe
        // because this function will never return and all resources not cleaned up by the point we
        // `throw` will leak, making this actually `'static`.
        let handler = unsafe {
            std::mem::transmute::<Box<dyn FnMut(Event)>, Box<dyn FnMut(Event) + 'static>>(handler)
        };
        self.elw.run(handler, false);

        // Throw an exception to break out of Rust execution and use unreachable to tell the
        // compiler this function won't return, giving it a return type of '!'
        backend::throw(
            "Using exceptions for control flow, don't mind me. This isn't actually an error!",
        );

        unreachable!();
    }

    pub fn spawn_app<A: ApplicationHandler + 'static>(self, mut app: A) {
        let event_loop = self.elw.clone();
        self.elw.run(Box::new(move |event| handle_event(&mut app, &event_loop, event)), true);
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

fn handle_event<A: ApplicationHandler>(app: &mut A, target: &ActiveEventLoop, event: Event) {
    match event {
        Event::NewEvents(cause) => app.new_events(target, cause),
        Event::WindowEvent { window_id, event } => app.window_event(target, window_id, event),
        Event::DeviceEvent { device_id, event } => app.device_event(target, device_id, event),
        Event::UserWakeUp => app.proxy_wake_up(target),
        Event::Suspended => app.suspended(target),
        Event::Resumed => app.resumed(target),
        Event::AboutToWait => app.about_to_wait(target),
        Event::MemoryWarning => app.memory_warning(target),
    }
}
