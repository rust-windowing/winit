use wasm_bindgen::{prelude::Closure, JsCast};
use web_sys::{AddEventListenerOptions, EventTarget};

pub(super) struct EventListenerHandle<T: ?Sized> {
    target: EventTarget,
    event_type: &'static str,
    listener: Closure<T>,
}

impl<T: ?Sized> EventListenerHandle<T> {
    pub fn new<U>(target: &U, event_type: &'static str, listener: Closure<T>) -> Self
    where
        U: Clone + Into<EventTarget>,
    {
        let target = target.clone().into();
        target
            .add_event_listener_with_callback(event_type, listener.as_ref().unchecked_ref())
            .expect("Failed to add event listener");
        EventListenerHandle {
            target,
            event_type,
            listener,
        }
    }

    pub fn with_options<U>(
        target: &U,
        event_type: &'static str,
        listener: Closure<T>,
        options: &AddEventListenerOptions,
    ) -> Self
    where
        U: Clone + Into<EventTarget>,
    {
        let target = target.clone().into();
        target
            .add_event_listener_with_callback_and_add_event_listener_options(
                event_type,
                listener.as_ref().unchecked_ref(),
                options,
            )
            .expect("Failed to add event listener");
        EventListenerHandle {
            target,
            event_type,
            listener,
        }
    }
}

impl<T: ?Sized> Drop for EventListenerHandle<T> {
    fn drop(&mut self) {
        self.target
            .remove_event_listener_with_callback(
                self.event_type,
                self.listener.as_ref().unchecked_ref(),
            )
            .unwrap_or_else(|e| {
                web_sys::console::error_2(
                    &format!("Error removing event listener {}", self.event_type).into(),
                    &e,
                )
            });
    }
}
