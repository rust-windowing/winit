use std::cell::{Ref, RefCell};
use std::future::Future;
use std::marker::PhantomData;
use std::sync::Arc;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};

// Unsafe wrapper type that allows us to use `T` when it's not `Send` from other threads.
// `value` **must** only be accessed on the main thread.
pub struct Wrapper<const SYNC: bool, V: 'static, S: Clone + Send, E> {
    value: Value<SYNC, V>,
    handler: fn(&RefCell<Option<V>>, E),
    sender_data: S,
    sender_handler: fn(&S, E),
}

struct Value<const SYNC: bool, V> {
    // SAFETY:
    // This value must not be accessed if not on the main thread.
    //
    // - We wrap this in an `Arc` to allow it to be safely cloned without
    //   accessing the value.
    // - The `RefCell` lets us mutably access in the main thread but is safe to
    //   drop in any thread because it has no `Drop` behavior.
    // - The `Option` lets us safely drop `T` only in the main thread.
    value: Arc<RefCell<Option<V>>>,
    // Prevent's `Send` or `Sync` to be automatically implemented.
    local: PhantomData<*const ()>,
}

// SAFETY: See `Self::value`.
unsafe impl<const SYNC: bool, V> Send for Value<SYNC, V> {}
// SAFETY: See `Self::value`.
unsafe impl<V> Sync for Value<true, V> {}

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
        handler: fn(&RefCell<Option<V>>, E),
        receiver: impl 'static + FnOnce(Arc<RefCell<Option<V>>>) -> R,
        sender_data: S,
        sender_handler: fn(&S, E),
    ) -> Option<Self> {
        Self::MAIN_THREAD.with(|safe| {
            if !safe {
                panic!("only callable from inside the `Window`")
            }
        });

        let value = Arc::new(RefCell::new(Some(value)));

        wasm_bindgen_futures::spawn_local({
            let value = Arc::clone(&value);
            async move {
                receiver(Arc::clone(&value)).await;
                drop(value.borrow_mut().take().unwrap());
            }
        });

        Some(Self {
            value: Value {
                value,
                local: PhantomData,
            },
            handler,
            sender_data,
            sender_handler,
        })
    }

    pub fn send(&self, event: E) {
        Self::MAIN_THREAD.with(|is_main_thread| {
            if *is_main_thread {
                (self.handler)(&self.value.value, event)
            } else {
                (self.sender_handler)(&self.sender_data, event)
            }
        })
    }

    pub fn is_main_thread(&self) -> bool {
        Self::MAIN_THREAD.with(|is_main_thread| *is_main_thread)
    }

    pub fn value(&self) -> Option<Ref<'_, V>> {
        Self::MAIN_THREAD.with(|is_main_thread| {
            if *is_main_thread {
                Some(Ref::map(self.value.value.borrow(), |value| {
                    value.as_ref().unwrap()
                }))
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
            value: Value {
                value: self.value.value.clone(),
                local: PhantomData,
            },
            handler: self.handler,
            sender_data: self.sender_data.clone(),
            sender_handler: self.sender_handler,
        }
    }
}
