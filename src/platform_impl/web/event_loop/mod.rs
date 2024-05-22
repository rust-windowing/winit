use std::marker::PhantomData;
use std::sync::mpsc::{self, Receiver, Sender};

use crate::application::ApplicationHandler;
use crate::error::EventLoopError;
use crate::event::Event;
use crate::event_loop::ActiveEventLoop as RootActiveEventLoop;

use super::{backend, device, window};

mod proxy;
pub(crate) mod runner;
mod state;
mod window_target;

pub(crate) use proxy::EventLoopProxy;
pub(crate) use window_target::{ActiveEventLoop, OwnedDisplayHandle};

pub struct EventLoop<T: 'static> {
    elw: RootActiveEventLoop,
    user_event_sender: Sender<T>,
    user_event_receiver: Receiver<T>,
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PlatformSpecificEventLoopAttributes {}

impl<T> EventLoop<T> {
    pub(crate) fn new(_: &PlatformSpecificEventLoopAttributes) -> Result<Self, EventLoopError> {
        let (user_event_sender, user_event_receiver) = mpsc::channel();
        let elw = RootActiveEventLoop { p: ActiveEventLoop::new(), _marker: PhantomData };
        Ok(EventLoop { elw, user_event_sender, user_event_receiver })
    }

    pub fn run_app<A: ApplicationHandler<T>>(self, app: &mut A) -> ! {
        let target = RootActiveEventLoop { p: self.elw.p.clone(), _marker: PhantomData };

        // SAFETY: Don't use `move` to make sure we leak the `event_handler` and `target`.
        let handler: Box<dyn FnMut(Event<()>)> =
            Box::new(|event| handle_event(app, &target, &self.user_event_receiver, event));

        // SAFETY: The `transmute` is necessary because `run()` requires `'static`. This is safe
        // because this function will never return and all resources not cleaned up by the point we
        // `throw` will leak, making this actually `'static`.
        let handler = unsafe { std::mem::transmute(handler) };
        self.elw.p.run(handler, false);

        // Throw an exception to break out of Rust execution and use unreachable to tell the
        // compiler this function won't return, giving it a return type of '!'
        backend::throw(
            "Using exceptions for control flow, don't mind me. This isn't actually an error!",
        );

        unreachable!();
    }

    pub fn spawn_app<A: ApplicationHandler<T> + 'static>(self, mut app: A) {
        let target = RootActiveEventLoop { p: self.elw.p.clone(), _marker: PhantomData };

        self.elw.p.run(
            Box::new(move |event| {
                handle_event(&mut app, &target, &self.user_event_receiver, event)
            }),
            true,
        );
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy::new(self.elw.p.waker(), self.user_event_sender.clone())
    }

    pub fn window_target(&self) -> &RootActiveEventLoop {
        &self.elw
    }
}

fn handle_event<T: 'static, A: ApplicationHandler<T>>(
    app: &mut A,
    target: &RootActiveEventLoop,
    user_event_receiver: &Receiver<T>,
    event: Event<()>,
) {
    match event {
        Event::NewEvents(cause) => app.new_events(target, cause),
        Event::WindowEvent { window_id, event } => app.window_event(target, window_id, event),
        Event::DeviceEvent { device_id, event } => app.device_event(target, device_id, event),
        Event::UserEvent(_) => {
            let event =
                user_event_receiver.try_recv().expect("user event signaled but not received");
            app.user_event(target, event);
        },
        Event::Suspended => app.suspended(target),
        Event::Resumed => app.resumed(target),
        Event::AboutToWait => app.about_to_wait(target),
        Event::LoopExiting => app.exiting(target),
        Event::MemoryWarning => app.memory_warning(target),
    }
}
