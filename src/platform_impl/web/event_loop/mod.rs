mod proxy;
mod runner;
mod state;
mod window_target;

pub use self::proxy::Proxy;
pub use self::window_target::WindowTarget;

use super::{backend, device, monitor, window};
use crate::event::Event;
use crate::event_loop as root;

use std::collections::{vec_deque::IntoIter as VecDequeIter, VecDeque};
use std::marker::PhantomData;

pub struct EventLoop<T: 'static> {
    elw: root::EventLoopWindowTarget<T>,
}

impl<T> EventLoop<T> {
    pub fn new() -> Self {
        EventLoop {
            elw: root::EventLoopWindowTarget {
                p: WindowTarget::new(),
                _marker: PhantomData,
            },
        }
    }

    pub fn available_monitors(&self) -> VecDequeIter<monitor::Handle> {
        VecDeque::new().into_iter()
    }

    pub fn primary_monitor(&self) -> monitor::Handle {
        monitor::Handle
    }

    pub fn run<F>(self, mut event_handler: F) -> !
    where
        F: 'static
            + FnMut(Event<'static, T>, &root::EventLoopWindowTarget<T>, &mut root::ControlFlow),
    {
        let target = root::EventLoopWindowTarget {
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

    pub fn create_proxy(&self) -> Proxy<T> {
        self.elw.p.proxy()
    }

    pub fn window_target(&self) -> &root::EventLoopWindowTarget<T> {
        &self.elw
    }
}
