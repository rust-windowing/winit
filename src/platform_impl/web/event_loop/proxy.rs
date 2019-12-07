use super::runner;
use crate::event::Event;
use crate::event_loop::EventLoopClosed;

pub struct Proxy<T: 'static> {
    runner: runner::Shared<T>,
}

impl<T: 'static> Proxy<T> {
    pub fn new(runner: runner::Shared<T>) -> Self {
        Proxy { runner }
    }

    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed<T>> {
        self.runner.send_event(Event::UserEvent(event));
        Ok(())
    }
}

impl<T: 'static> Clone for Proxy<T> {
    fn clone(&self) -> Self {
        Proxy {
            runner: self.runner.clone(),
        }
    }
}
