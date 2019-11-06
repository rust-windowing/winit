use std::time::Duration;
use stdweb::web::{window, IWindowOrWorker, TimeoutHandle};

#[derive(Debug)]
pub struct Timeout {
    handle: TimeoutHandle,
}

impl Timeout {
    pub fn new<F>(f: F, duration: Duration) -> Timeout
    where
        F: 'static + FnMut(),
    {
        Timeout {
            handle: window().set_clearable_timeout(f, duration.as_millis() as u32),
        }
    }
}

impl Drop for Timeout {
    fn drop(&mut self) {
        let handle = std::mem::replace(&mut self.handle, unsafe { std::mem::uninitialized() });
        handle.clear();
    }
}
