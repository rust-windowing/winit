use super::runner::WeakShared;
use crate::platform_impl::platform::r#async::Waker;

#[derive(Clone)]
pub struct EventLoopProxy {
    runner: Waker<WeakShared>,
}

impl EventLoopProxy {
    pub fn new(runner: Waker<WeakShared>) -> Self {
        Self { runner }
    }

    pub fn wake_up(&self) {
        self.runner.wake();
    }
}
