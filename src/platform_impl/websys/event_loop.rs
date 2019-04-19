use event_loop::{ControlFlow, EventLoopClosed};
use event::Event;
use super::window::MonitorHandle;

use std::collections::VecDeque;

pub struct EventLoop<T: 'static> {
    pending_events: Vec<T>
}

impl<T: 'static> EventLoop<T> {
    pub fn new() -> EventLoop<T> {
        EventLoop { pending_events: Vec::new() }
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy { _marker: std::marker::PhantomData }
    }

    #[inline]
    pub fn get_available_monitors(&self) -> VecDeque<MonitorHandle> {
        vec!(MonitorHandle{}).into_iter().collect()
    }

    #[inline]
    pub fn get_primary_monitor(&self) -> MonitorHandle {
        MonitorHandle{}
    }

        /// Hijacks the calling thread and initializes the `winit` event loop with the provided
    /// closure. Since the closure is `'static`, it must be a `move` closure if it needs to
    /// access any data from the calling context.
    ///
    /// See the [`ControlFlow`] docs for information on how changes to `&mut ControlFlow` impact the
    /// event loop's behavior.
    ///
    /// Any values not passed to this function will *not* be dropped.
    ///
    /// [`ControlFlow`]: ./enum.ControlFlow.html
    #[inline]
    pub fn run<F>(self, event_handler: F) -> !
        where F: 'static + FnMut(Event<T>, &::event_loop::EventLoopWindowTarget<T>, &mut ControlFlow)
    {
        unimplemented!()
    }

    pub fn window_target(&self) -> &::event_loop::EventLoopWindowTarget<T> {
        unimplemented!()
    }

}

#[derive(Clone)]
pub struct EventLoopProxy<T: 'static> {
    _marker: std::marker::PhantomData<T>
}

impl<T: 'static> EventLoopProxy<T> {
    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed> {
        unimplemented!()
    }
}

pub struct EventLoopWindowTarget<T: 'static> {
    _marker: std::marker::PhantomData<T>
}
