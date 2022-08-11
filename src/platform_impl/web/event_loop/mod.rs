mod proxy;
mod runner;
mod state;
mod window_target;

pub use self::proxy::EventLoopProxy;
pub use self::window_target::EventLoopWindowTarget;

use super::{backend, device, window};
use crate::event::Event;
use crate::event_loop::ControlFlow;

use std::marker::PhantomData;
use std::rc::Rc;

pub struct EventLoop<T: 'static> {
    p: PhantomData<T>,
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PlatformSpecificEventLoopAttributes {}

impl<T> EventLoop<T> {
    pub(crate) fn new(_: &PlatformSpecificEventLoopAttributes) -> (Self, EventLoopWindowTarget<T>) {
        (EventLoop { p: PhantomData }, EventLoopWindowTarget::new())
    }

    pub fn run<F>(self, callback: F, window_target: Rc<EventLoopWindowTarget<T>>) -> !
    where
        F: 'static + FnMut(Event<'_, T>, &mut ControlFlow),
    {
        self.spawn(callback, window_target);

        // Throw an exception to break out of Rust execution and use unreachable to tell the
        // compiler this function won't return, giving it a return type of '!'
        backend::throw(
            "Using exceptions for control flow, don't mind me. This isn't actually an error!",
        );

        unreachable!();
    }

    pub fn spawn<F>(self, callback: F, window_target: Rc<EventLoopWindowTarget<T>>)
    where
        F: 'static + FnMut(Event<'_, T>, &mut ControlFlow),
    {
        window_target.run(Box::new(callback));
    }

    pub fn create_proxy(&self, window_target: Rc<EventLoopWindowTarget<T>>) -> EventLoopProxy<T> {
        window_target.proxy()
    }
}
