use js_sys::{Function, Object, Promise, Reflect};
use once_cell::unsync::{Lazy, OnceCell};
use std::time::Duration;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{AbortController, AbortSignal, MessageChannel, MessagePort};

#[derive(Debug)]
pub struct Schedule(Inner);

#[derive(Debug)]
enum Inner {
    Scheduler {
        controller: AbortController,
        _closure: Closure<dyn FnMut()>,
    },
    Timeout {
        window: web_sys::Window,
        handle: i32,
        port: MessagePort,
        _message_closure: Closure<dyn FnMut()>,
        _timeout_closure: Closure<dyn FnMut()>,
    },
}

impl Schedule {
    pub fn new<F>(window: web_sys::Window, f: F, duration: Option<Duration>) -> Schedule
    where
        F: 'static + FnMut(),
    {
        if has_scheduler_support(&window) {
            Self::new_scheduler(window, f, duration)
        } else {
            Self::new_timeout(window, f, duration)
        }
    }

    fn new_scheduler<F>(window: web_sys::Window, f: F, duration: Option<Duration>) -> Schedule
    where
        F: 'static + FnMut(),
    {
        let window: WindowSupportExt = window.unchecked_into();
        let scheduler = window.scheduler();

        let closure = Closure::new(f);
        let mut options = SchedulerPostTaskOptions::new();
        let controller = AbortController::new().expect("Failed to create `AbortController`");
        options.signal(&controller.signal());

        if let Some(duration) = duration {
            options.delay(duration.as_millis() as f64);
        }

        thread_local! {
            static REJECT_HANDLER: Lazy<Closure<dyn FnMut(JsValue)>> = Lazy::new(|| Closure::new(|_| ()));
        }
        REJECT_HANDLER.with(|handler| {
            let _ = scheduler
                .post_task_with_options(closure.as_ref().unchecked_ref(), &options)
                .catch(handler);
        });

        Schedule(Inner::Scheduler {
            controller,
            _closure: closure,
        })
    }

    fn new_timeout<F>(window: web_sys::Window, f: F, duration: Option<Duration>) -> Schedule
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

        Schedule(Inner::Timeout {
            window,
            handle,
            port: port_1,
            _message_closure: message_closure,
            _timeout_closure: timeout_closure,
        })
    }
}

impl Drop for Schedule {
    fn drop(&mut self) {
        match &self.0 {
            Inner::Scheduler { controller, .. } => controller.abort(),
            Inner::Timeout {
                window,
                handle,
                port,
                ..
            } => {
                window.clear_timeout_with_handle(*handle);
                port.close();
            }
        }
    }
}

fn has_scheduler_support(window: &web_sys::Window) -> bool {
    thread_local! {
        static SCHEDULER_SUPPORT: OnceCell<bool> = OnceCell::new();
    }

    SCHEDULER_SUPPORT.with(|support| {
        *support.get_or_init(|| {
            #[wasm_bindgen]
            extern "C" {
                type SchedulerSupport;

                #[wasm_bindgen(method, getter, js_name = scheduler)]
                fn has_scheduler(this: &SchedulerSupport) -> JsValue;
            }

            let support: &SchedulerSupport = window.unchecked_ref();

            !support.has_scheduler().is_undefined()
        })
    })
}

#[wasm_bindgen]
extern "C" {
    type WindowSupportExt;

    #[wasm_bindgen(method, getter)]
    fn scheduler(this: &WindowSupportExt) -> Scheduler;

    type Scheduler;

    #[wasm_bindgen(method, js_name = postTask)]
    fn post_task_with_options(
        this: &Scheduler,
        callback: &Function,
        options: &SchedulerPostTaskOptions,
    ) -> Promise;

    type SchedulerPostTaskOptions;
}

impl SchedulerPostTaskOptions {
    fn new() -> Self {
        Object::new().unchecked_into()
    }

    fn delay(&mut self, val: f64) -> &mut Self {
        let r = Reflect::set(self, &JsValue::from("delay"), &val.into());
        debug_assert!(r.is_ok(), "Failed to set `delay` property");
        self
    }

    fn signal(&mut self, val: &AbortSignal) -> &mut Self {
        let r = Reflect::set(self, &JsValue::from("signal"), &val.into());
        debug_assert!(r.is_ok(), "Failed to set `signal` property");
        self
    }
}
