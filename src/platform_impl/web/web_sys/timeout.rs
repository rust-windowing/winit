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
                &closure.as_ref().unchecked_ref(),
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
