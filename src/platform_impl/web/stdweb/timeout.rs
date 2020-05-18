use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;
use stdweb::web::{window, IWindowOrWorker, RequestAnimationFrameHandle, TimeoutHandle};

#[derive(Debug)]
pub struct Timeout {
    handle: Option<TimeoutHandle>,
}

impl Timeout {
    pub fn new<F>(f: F, duration: Duration) -> Timeout
    where
        F: 'static + FnMut(),
    {
        Timeout {
            handle: Some(window().set_clearable_timeout(f, duration.as_millis() as u32)),
        }
    }
}

impl Drop for Timeout {
    fn drop(&mut self) {
        let handle = self.handle.take().unwrap();
        handle.clear();
    }
}

#[derive(Debug)]
pub struct AnimationFrameRequest {
    handle: Option<RequestAnimationFrameHandle>,
    // track callback state, because `cancelAnimationFrame` is slow
    fired: Rc<Cell<bool>>,
}

impl AnimationFrameRequest {
    pub fn new<F>(mut f: F) -> AnimationFrameRequest
    where
        F: 'static + FnMut(),
    {
        let fired = Rc::new(Cell::new(false));
        let c_fired = fired.clone();
        let handle = window().request_animation_frame(move |_| {
            (*c_fired).set(true);
            f();
        });

        AnimationFrameRequest {
            handle: Some(handle),
            fired,
        }
    }
}

impl Drop for AnimationFrameRequest {
    fn drop(&mut self) {
        if !(*self.fired).get() {
            if let Some(handle) = self.handle.take() {
                handle.cancel();
            }
        }
    }
}
