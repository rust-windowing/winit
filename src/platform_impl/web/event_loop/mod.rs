use std::marker::PhantomData;

use crate::event::Event;
use crate::event_loop::{
    ControlFlow, EventLoopCreationError, EventLoopWindowTarget as RootEventLoopWindowTarget,
};

use super::{backend, device, window};

mod proxy;
pub(crate) mod runner;
mod state;
mod window_target;

pub use proxy::EventLoopProxy;
pub use window_target::EventLoopWindowTarget;

pub struct EventLoop<T: 'static> {
    elw: RootEventLoopWindowTarget<T>,
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PlatformSpecificEventLoopAttributes {}

impl<T> EventLoop<T> {
    pub(crate) fn new(
        _: &PlatformSpecificEventLoopAttributes,
    ) -> Result<Self, EventLoopCreationError> {
        Ok(EventLoop {
            elw: RootEventLoopWindowTarget {
                p: EventLoopWindowTarget::new(),
                _marker: PhantomData,
            },
        })
    }

    pub fn run<F>(self, mut event_handler: F) -> !
    where
        F: FnMut(Event<T>, &RootEventLoopWindowTarget<T>, &mut ControlFlow),
    {
        let target = RootEventLoopWindowTarget {
            p: self.elw.p.clone(),
            _marker: PhantomData,
        };

        // SAFETY: Don't use `move` to make sure we leak the `event_handler` and `target`.
        let handler: Box<dyn FnMut(_, _)> =
            Box::new(|event, flow| event_handler(event, &target, flow));
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
        F: 'static + FnMut(Event<T>, &RootEventLoopWindowTarget<T>, &mut ControlFlow),
    {
        let target = RootEventLoopWindowTarget {
            p: self.elw.p.clone(),
            _marker: PhantomData,
        };

        self.elw.p.run(
            Box::new(move |event, flow| event_handler(event, &target, flow)),
            true,
        );
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        self.elw.p.proxy()
    }

    pub fn window_target(&self) -> &RootEventLoopWindowTarget<T> {
        &self.elw
    }
}
