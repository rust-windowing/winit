use async_channel::{Sender, TrySendError};

use crate::event_loop::EventLoopClosed;

pub struct EventLoopProxy<T: 'static> {
    pub sender: Sender<T>,
}

impl<T: 'static> EventLoopProxy<T> {
    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed<T>> {
        match self.sender.try_send(event) {
            Ok(()) => Ok(()),
            Err(TrySendError::Closed(val)) => Err(EventLoopClosed(val)),
            // Note: `async-channel` has no way to block on sending something,
            // so this is our only option for making this synchronous.
            Err(TrySendError::Full(_)) => unreachable!("`EventLoopProxy` channels are unbounded"),
        }
    }
}

impl<T: 'static> Clone for EventLoopProxy<T> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}
