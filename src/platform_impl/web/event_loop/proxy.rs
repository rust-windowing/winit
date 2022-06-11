use super::runner;
use crate::event::Event;
use crate::event_loop::EventLoopClosed;

pub struct EventLoopProxy<T: 'static> {
    runner: runner::Shared<T>,
}

impl<T: 'static> EventLoopProxy<T> {
    pub fn new(runner: runner::Shared<T>) -> Self {
        Self { runner }
    }

    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed<T>> {
        self.runner.send_event(Event::UserEvent(event));
        Ok(())
    }
}

impl<T: 'static> Clone for EventLoopProxy<T> {
    fn clone(&self) -> Self {
        Self {
            runner: self.runner.clone(),
        }
    }
}
