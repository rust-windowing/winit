use once_cell::unsync::OnceCell;
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;

#[derive(Debug)]
pub struct Timeout {
    window: web_sys::Window,
    handle: i32,
    _closure: Closure<dyn FnMut()>,
}

impl Timeout {
    pub fn new<F>(window: web_sys::Window, f: F, duration: Duration) -> Timeout
    where
        F: 'static + FnMut(),
    {
        let closure = Closure::wrap(Box::new(f) as Box<dyn FnMut()>);

        let handle = window
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                closure.as_ref().unchecked_ref(),
                duration.as_millis() as i32,
            )
            .expect("Failed to set timeout");

        Timeout {
            window,
            handle,
            _closure: closure,
        }
    }
}

impl Drop for Timeout {
    fn drop(&mut self) {
        self.window.clear_timeout_with_handle(self.handle);
    }
}

#[derive(Debug)]
pub struct IdleCallback {
    window: web_sys::Window,
    handle: Handle,
    fired: Rc<Cell<bool>>,
    _closure: Closure<dyn FnMut()>,
}

#[derive(Clone, Copy, Debug)]
enum Handle {
    IdleCallback(u32),
    Timeout(i32),
}

impl IdleCallback {
    pub fn new<F>(window: web_sys::Window, mut f: F) -> IdleCallback
    where
        F: 'static + FnMut(),
    {
        let fired = Rc::new(Cell::new(false));
        let c_fired = fired.clone();
        let closure = Closure::wrap(Box::new(move || {
            (*c_fired).set(true);
            f();
        }) as Box<dyn FnMut()>);

        let handle = if has_idle_callback_support(&window) {
            Handle::IdleCallback(
                window
                    .request_idle_callback(closure.as_ref().unchecked_ref())
                    .expect("Failed to request idle callback"),
            )
        } else {
            Handle::Timeout(
                window
                    .set_timeout_with_callback(closure.as_ref().unchecked_ref())
                    .expect("Failed to set timeout"),
            )
        };

        IdleCallback {
            window,
            handle,
            fired,
            _closure: closure,
        }
    }
}

impl Drop for IdleCallback {
    fn drop(&mut self) {
        if !(*self.fired).get() {
            match self.handle {
                Handle::IdleCallback(handle) => self.window.cancel_idle_callback(handle),
                Handle::Timeout(handle) => self.window.clear_timeout_with_handle(handle),
            }
        }
    }
}

fn has_idle_callback_support(window: &web_sys::Window) -> bool {
    thread_local! {
        static IDLE_CALLBACK_SUPPORT: OnceCell<bool> = OnceCell::new();
    }

    IDLE_CALLBACK_SUPPORT.with(|support| {
        *support.get_or_init(|| {
            #[wasm_bindgen]
            extern "C" {
                type IdleCallbackSupport;

                #[wasm_bindgen(method, getter, js_name = requestIdleCallback)]
                fn has_request_idle_callback(this: &IdleCallbackSupport) -> JsValue;
            }

            let support: &IdleCallbackSupport = window.unchecked_ref();
            !support.has_request_idle_callback().is_undefined()
        })
    })
}
