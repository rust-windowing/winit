use std::sync::mpsc::{SendError, Sender};

use super::runner;
use crate::event_loop::EventLoopClosed;
use crate::platform_impl::platform::r#async::Waker;

pub struct EventLoopProxy<T: 'static> {
    runner: Waker<runner::Shared>,
    sender: Sender<T>,
}

impl<T: 'static> EventLoopProxy<T> {
    pub fn new(runner: Waker<runner::Shared>, sender: Sender<T>) -> Self {
        Self { runner, sender }
    }

    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed<T>> {
        self.sender
            .send(event)
            .map_err(|SendError(event)| EventLoopClosed(event))?;
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
