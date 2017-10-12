use std::sync::Arc;

use {EventsLoopClosed, ControlFlow};

use super::WaylandContext;

pub struct EventsLoop;

pub struct EventsLoopProxy;

impl EventsLoopProxy {
    // Causes the `EventsLoop` to stop blocking on `run_forever` and emit an `Awakened` event.
    //
    // Returns `Err` if the associated `EventsLoop` no longer exists.
    pub fn wakeup(&self) -> Result<(), EventsLoopClosed> {
        unimplemented!()
    }
}

impl EventsLoop {
    pub fn new(mut ctxt: WaylandContext) -> EventsLoop {
        unimplemented!()
    }

    #[inline]
    pub fn context(&self) -> &Arc<WaylandContext> {
        unimplemented!()
    }

    pub fn create_proxy(&self) -> EventsLoopProxy {
        EventsLoopProxy
    }

    pub fn poll_events<F>(&mut self, mut callback: F)
        where F: FnMut(::Event)
    {
        unimplemented!()
    }

    pub fn run_forever<F>(&mut self, mut callback: F)
        where F: FnMut(::Event) -> ControlFlow,
    {
        unimplemented!()
    }
}