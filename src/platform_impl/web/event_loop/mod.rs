mod proxy;
mod runner;
mod state;
mod window_target;

pub use self::proxy::EventLoopProxy;
pub use self::window_target::EventLoopWindowTarget;

use super::{backend, device, window};
use crate::event::Event;
use crate::event_loop::{ControlFlow, EventLoopWindowTarget as RootEventLoopWindowTarget};

use std::marker::PhantomData;

pub struct EventLoop<T: 'static> {
    elw: RootEventLoopWindowTarget<T>,
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Hash)]
pub(crate) struct PlatformSpecificEventLoopAttributes {}

impl<T> EventLoop<T> {
    pub(crate) fn new(_: &PlatformSpecificEventLoopAttributes) -> Self {
        EventLoop {
            elw: RootEventLoopWindowTarget {
                p: EventLoopWindowTarget::new(),
                _marker: PhantomData,
            },
        }
    }

    pub fn run<F>(self, mut event_handler: F) -> !
    where
        F: 'static + FnMut(Event<'_, T>, &RootEventLoopWindowTarget<T>, &mut ControlFlow),
    {
        let target = RootEventLoopWindowTarget {
            p: self.elw.p.clone(),
            _marker: PhantomData,
        };

        self.elw.p.run(Box::new(move |event, flow| {
            event_handler(event, &target, flow)
        }));

        // Throw an exception to break out of Rust exceution and use unreachable to tell the
        // compiler this function won't return, giving it a return type of '!'
        backend::throw(
            "Using exceptions for control flow, don't mind me. This isn't actually an error!",
        );

        unreachable!();
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        self.elw.p.proxy()
    }

    pub fn window_target(&self) -> &RootEventLoopWindowTarget<T> {
        &self.elw
    }
}
