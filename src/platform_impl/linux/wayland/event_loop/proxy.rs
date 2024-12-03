//! An event loop proxy.

use std::sync::Arc;

use sctk::reexports::calloop::ping::Ping;

use crate::event_loop::{EventLoopProxy as CoreEventLoopProxy, EventLoopProxyProvider};

/// A handle that can be sent across the threads and used to wake up the `EventLoop`.
pub struct EventLoopProxy {
    ping: Ping,
}

impl EventLoopProxyProvider for EventLoopProxy {
    fn wake_up(&self) {
        self.ping.ping();
    }
}

impl EventLoopProxy {
    pub fn new(ping: Ping) -> Self {
        Self { ping }
    }
}

impl From<EventLoopProxy> for CoreEventLoopProxy {
    fn from(value: EventLoopProxy) -> Self {
        CoreEventLoopProxy::new(Arc::new(value))
    }
}
