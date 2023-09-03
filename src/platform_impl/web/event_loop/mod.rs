use std::iter;
use std::marker::PhantomData;
use std::sync::mpsc::{self, Receiver, Sender};

use crate::error::EventLoopError;
use crate::event::Event;
use crate::event_loop::EventLoopWindowTarget as RootEventLoopWindowTarget;

use super::r#async::WakerSpawner;
use super::{backend, device, window};

mod proxy;
pub(crate) mod runner;
mod state;
mod window_target;

pub use proxy::EventLoopProxy;
pub use window_target::EventLoopWindowTarget;

pub struct EventLoop<T: 'static> {
    elw: RootEventLoopWindowTarget<T>,
    proxy_spawner: WakerSpawner<runner::Shared>,
    user_event_sender: Sender<T>,
    user_event_receiver: Receiver<T>,
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PlatformSpecificEventLoopAttributes {}

impl<T> EventLoop<T> {
    pub(crate) fn new(_: &PlatformSpecificEventLoopAttributes) -> Result<Self, EventLoopError> {
        let (user_event_sender, user_event_receiver) = mpsc::channel();
        let elw = RootEventLoopWindowTarget {
            p: EventLoopWindowTarget::new(),
            _marker: PhantomData,
        };
        let proxy_spawner = WakerSpawner::new(elw.p.runner.clone(), |runner, count| {
            runner.send_events(iter::repeat(Event::UserEvent(())).take(count))
        })
        .expect("`EventLoop` has to be created in the main thread");
        Ok(EventLoop {
            elw,
            proxy_spawner,
            user_event_sender,
            user_event_receiver,
        })
    }

    pub fn run<F>(self, mut event_handler: F) -> !
    where
        F: FnMut(Event<T>, &RootEventLoopWindowTarget<T>),
    {
        let target = RootEventLoopWindowTarget {
            p: self.elw.p.clone(),
            _marker: PhantomData,
        };

        // SAFETY: Don't use `move` to make sure we leak the `event_handler` and `target`.
        let handler: Box<dyn FnMut(Event<()>)> = Box::new(|event| {
            let event = match event.map_nonuser_event() {
                Ok(event) => event,
                Err(Event::UserEvent(())) => Event::UserEvent(
                    self.user_event_receiver
                        .try_recv()
                        .expect("handler woken up without user event"),
                ),
                Err(_) => unreachable!(),
            };
            event_handler(event, &target)
        });
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

    pub fn spawn<F>(self, mut event_handler: F)
    where
        F: 'static + FnMut(Event<T>, &RootEventLoopWindowTarget<T>),
    {
        let target = RootEventLoopWindowTarget {
            p: self.elw.p.clone(),
            _marker: PhantomData,
        };

        self.elw.p.run(
            Box::new(move |event| {
                let event = match event.map_nonuser_event() {
                    Ok(event) => event,
                    Err(Event::UserEvent(())) => Event::UserEvent(
                        self.user_event_receiver
                            .try_recv()
                            .expect("handler woken up without user event"),
                    ),
                    Err(_) => unreachable!(),
                };
                event_handler(event, &target)
            }),
            true,
        );
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy::new(self.proxy_spawner.waker(), self.user_event_sender.clone())
    }

    pub fn window_target(&self) -> &RootEventLoopWindowTarget<T> {
        &self.elw
    }
}
