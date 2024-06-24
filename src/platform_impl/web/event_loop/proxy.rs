use std::rc::Weak;

use super::runner::Execution;
use crate::platform_impl::platform::r#async::Waker;

#[derive(Clone)]
pub struct EventLoopProxy {
    runner: Waker<Weak<Execution>>,
}

impl EventLoopProxy {
    pub fn new(runner: Waker<Weak<Execution>>) -> Self {
        Self { runner }
    }

    pub fn wake_up(&self) {
        self.runner.wake();
    }
}
