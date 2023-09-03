use std::sync::mpsc::Sender;

use super::runner;
use crate::event::Event;
use crate::event_loop::EventLoopClosed;
use crate::platform_impl::platform::r#async::Channel;

pub struct EventLoopProxy<T: 'static> {
    // used to wake the event loop handler, not to actually pass data
    runner: Channel<runner::Shared, ()>,
    sender: Sender<T>,
}

impl<T: 'static> EventLoopProxy<T> {
    pub fn new(runner: runner::Shared, sender: Sender<T>) -> Self {
        Self {
            runner: Channel::new(runner, |runner, event| {
                runner.send_event(Event::UserEvent(event))
            })
            .unwrap(),
            sender,
        }
    }

    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed<T>> {
        self.sender.send(event).unwrap();
        self.runner.send(());
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
