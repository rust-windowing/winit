use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

#[derive(Debug)]
pub struct Timeout {
    handle: i32,
    _closure: Closure<dyn FnMut()>,
}

impl Timeout {
    pub fn new<F>(f: F, duration: Duration) -> Timeout
    where
        F: 'static + FnMut(),
    {
        let window = web_sys::window().expect("Failed to obtain window");

        let closure = Closure::wrap(Box::new(f) as Box<dyn FnMut()>);

        let handle = window
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                closure.as_ref().unchecked_ref(),
                duration.as_millis() as i32,
            )
            .expect("Failed to set timeout");

        Timeout {
            handle,
            _closure: closure,
        }
    }
}

impl Drop for Timeout {
    fn drop(&mut self) {
        let window = web_sys::window().expect("Failed to obtain window");

        window.clear_timeout_with_handle(self.handle);
    }
}

#[derive(Debug)]
pub struct AnimationFrameRequest {
    handle: i32,
    // track callback state, because `cancelAnimationFrame` is slow
    fired: Rc<Cell<bool>>,
    _closure: Closure<dyn FnMut()>,
}

impl AnimationFrameRequest {
    pub fn new<F>(mut f: F) -> AnimationFrameRequest
    where
        F: 'static + FnMut(),
    {
        let window = web_sys::window().expect("Failed to obtain window");

        let fired = Rc::new(Cell::new(false));
        let c_fired = fired.clone();
        let closure = Closure::wrap(Box::new(move || {
            (*c_fired).set(true);
            f();
        }) as Box<dyn FnMut()>);

        let handle = window
            .request_animation_frame(closure.as_ref().unchecked_ref())
            .expect("Failed to request animation frame");

        AnimationFrameRequest {
            handle,
            fired,
            _closure: closure,
        }
    }
}

impl Drop for AnimationFrameRequest {
    fn drop(&mut self) {
        if !(*self.fired).get() {
            let window = web_sys::window().expect("Failed to obtain window");
            window
                .cancel_animation_frame(self.handle)
                .expect("Failed to cancel animation frame");
        }
    }
}
