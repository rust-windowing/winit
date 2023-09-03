use std::iter;
use std::sync::mpsc::Sender;

use super::runner;
use crate::event::Event;
use crate::event_loop::EventLoopClosed;
use crate::platform_impl::platform::r#async::Waker;

pub struct EventLoopProxy<T: 'static> {
    runner: Waker<runner::Shared>,
    sender: Sender<T>,
}

impl<T: 'static> EventLoopProxy<T> {
    pub fn new(runner: runner::Shared, sender: Sender<T>) -> Self {
        Self {
            runner: Waker::new(runner, |runner, count| {
                runner.send_events(iter::repeat(Event::UserEvent(())).take(count))
            })
            .unwrap(),
            sender,
        }
    }

    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed<T>> {
        self.sender.send(event).unwrap();
        self.runner.wake();
        Ok(())
    }
}

impl<T: 'static> Clone for EventLoopProxy<T> {
    fn clone(&self) -> Self {
        Self {
            runner: self.runner.clone(),
            sender: self.sender.clone(),
        }
    }
}
