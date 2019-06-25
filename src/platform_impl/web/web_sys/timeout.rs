use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct Timeout {}

impl Timeout {
    pub fn new<F>(f: F, duration: Duration) -> Timeout {
        Timeout {}
    }

    pub fn clear(self) {}
}
