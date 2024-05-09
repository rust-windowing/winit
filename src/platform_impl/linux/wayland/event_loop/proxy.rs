//! An event loop proxy.

use sctk::reexports::calloop::ping::Ping;

/// A handle that can be sent across the threads and used to wake up the `EventLoop`.
#[derive(Clone)]
pub struct EventLoopProxy {
    ping: Ping,
}

impl EventLoopProxy {
    pub fn new(ping: Ping) -> Self {
        Self { ping }
    }

    pub fn wake_up(&self) {
        self.ping.ping();
    }
}
