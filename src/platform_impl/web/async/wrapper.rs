use std::future::Future;
use std::marker::PhantomData;
use std::sync::{Arc, RwLock};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};

// Unsafe wrapper type that allows us to use `T` when it's not `Send` from other threads.
// `value` **must** only be accessed on the main thread.
pub struct Wrapper<const SYNC: bool, V: 'static, S: Clone + Send, E> {
    // We wrap this in an `Arc` to allow it to be safely cloned without accessing the value.
    // The `RwLock` lets us safely drop in any thread.
    // The `Option` lets us safely drop `T` only in the main thread, while letting other threads drop `None`.
    value: Arc<RwLock<Option<V>>>,
    handler: fn(&RwLock<Option<V>>, E),
    sender_data: S,
    sender_handler: fn(&S, E),
    // Prevent's `Send` or `Sync` to be automatically implemented.
    local: PhantomData<*const ()>,
}

impl<const SYNC: bool, V, S: Clone + Send, E> Wrapper<SYNC, V, S, E> {
    thread_local! {
        static MAIN_THREAD: bool = {
            #[wasm_bindgen]
            extern "C" {
                #[derive(Clone)]
                type Global;

                #[wasm_bindgen(method, getter, js_name = Window)]
                fn window(this: &Global) -> JsValue;
            }

            let global: Global = js_sys::global().unchecked_into();
            !global.window().is_undefined()
        };
    }

    #[track_caller]
    pub fn new<R: Future<Output = ()>>(
        value: V,
        handler: fn(&RwLock<Option<V>>, E),
        receiver: impl 'static + FnOnce(Arc<RwLock<Option<V>>>) -> R,
        sender_data: S,
        sender_handler: fn(&S, E),
    ) -> Option<Self> {
        Self::MAIN_THREAD.with(|safe| {
            if !safe {
                panic!("only callable from inside the `Window`")
            }
        });

        let value = Arc::new(RwLock::new(Some(value)));

        wasm_bindgen_futures::spawn_local({
            let value = Arc::clone(&value);
            async move {
                receiver(Arc::clone(&value)).await;
                value.write().unwrap().take().unwrap();
            }
        });

        Some(Self {
            value,
            handler,
            sender_data,
            sender_handler,
            local: PhantomData,
        })
    }

    pub fn send(&self, event: E) {
        Self::MAIN_THREAD.with(|is_main_thread| {
            if *is_main_thread {
                (self.handler)(&self.value, event)
            } else {
                (self.sender_handler)(&self.sender_data, event)
            }
        })
    }

    pub fn is_main_thread(&self) -> bool {
        Self::MAIN_THREAD.with(|is_main_thread| *is_main_thread)
    }

    pub fn with<T>(&self, f: impl FnOnce(&V) -> T) -> Option<T> {
        Self::MAIN_THREAD.with(|is_main_thread| {
            if *is_main_thread {
                Some(f(self.value.read().unwrap().as_ref().unwrap()))
            } else {
                None
            }
        })
    }

    pub fn with_sender_data<T>(&self, f: impl FnOnce(&S) -> T) -> T {
        f(&self.sender_data)
    }
}

impl<const SYNC: bool, V, S: Clone + Send, E> Clone for Wrapper<SYNC, V, S, E> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            handler: self.handler,
            sender_data: self.sender_data.clone(),
            sender_handler: self.sender_handler,
            local: PhantomData,
        }
    }
}

unsafe impl<const SYNC: bool, V, S: Clone + Send, E> Send for Wrapper<SYNC, V, S, E> {}
unsafe impl<V, S: Clone + Send + Sync, E> Sync for Wrapper<true, V, S, E> {}
