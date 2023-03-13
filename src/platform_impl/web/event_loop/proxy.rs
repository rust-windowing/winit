use super::runner;
use crate::event::Event;
use crate::event_loop::EventLoopClosed;
use crate::platform_impl::platform::r#async::Channel;

pub struct EventLoopProxy<T: 'static> {
    runner: Channel<runner::Shared<T>, T>,
}

impl<T: 'static> EventLoopProxy<T> {
    pub fn new(runner: runner::Shared<T>) -> Self {
        Self {
            runner: Channel::new(runner, |runner, event| {
                runner.send_event(Event::UserEvent(event))
            })
            .unwrap(),
        }
    }

    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed<T>> {
        self.runner.send(event);
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
