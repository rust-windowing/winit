use std::time::Duration;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{MessageChannel, MessagePort};

#[derive(Debug)]
pub struct Timeout {
    window: web_sys::Window,
    handle: i32,
    port: MessagePort,
    _message_closure: Closure<dyn FnMut()>,
    _timeout_closure: Closure<dyn FnMut()>,
}

impl Timeout {
    pub fn new<F>(window: web_sys::Window, f: F, duration: Option<Duration>) -> Timeout
    where
        F: 'static + FnMut(),
    {
        let channel = MessageChannel::new().unwrap();
        let message_closure = Closure::new(f);
        let port_1 = channel.port1();
        port_1
            .add_event_listener_with_callback("message", message_closure.as_ref().unchecked_ref())
            .expect("Failed to set message handler");
        port_1.start();

        let port_2 = channel.port2();
        let timeout_closure = Closure::new(move || {
            port_2
                .post_message(&JsValue::UNDEFINED)
                .expect("Failed to send message")
        });
        let handle = if let Some(duration) = duration {
            window.set_timeout_with_callback_and_timeout_and_arguments_0(
                timeout_closure.as_ref().unchecked_ref(),
                duration.as_millis() as i32,
            )
        } else {
            window.set_timeout_with_callback(timeout_closure.as_ref().unchecked_ref())
        }
        .expect("Failed to set timeout");

        Timeout {
            window,
            handle,
            port: port_1,
            _message_closure: message_closure,
            _timeout_closure: timeout_closure,
        }
    }
}

impl Drop for Timeout {
    fn drop(&mut self) {
        self.window.clear_timeout_with_handle(self.handle);
        self.port.close();
    }
}
