use std::cell::{Ref, RefCell};
use std::cmp;
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::sync::Arc;

use super::super::main_thread::MainThreadMarker;

// Unsafe wrapper type that allows us to use `T` when it's not `Send` from other threads.
// `value` **must** only be accessed on the main thread.
pub struct Wrapper<V: 'static, S: Clone + Send, E> {
    value: Value<V>,
    handler: fn(&RefCell<Option<V>>, E),
    sender_data: S,
    sender_handler: fn(&S, E),
}

struct Value<V> {
    // SAFETY:
    // This value must not be accessed if not on the main thread.
    //
    // - We wrap this in an `Arc` to allow it to be safely cloned without accessing the value.
    // - The `RefCell` lets us mutably access in the main thread but is safe to drop in any thread
    //   because it has no `Drop` behavior.
    // - The `Option` lets us safely drop `T` only in the main thread.
    value: Arc<RefCell<Option<V>>>,
    // Prevent's `Send` or `Sync` to be automatically implemented.
    local: PhantomData<*const ()>,
}

// SAFETY: See `Self::value`.
unsafe impl<V> Send for Value<V> {}
// SAFETY: See `Self::value`.
unsafe impl<V> Sync for Value<V> {}

impl<V, S: Clone + Send, E> Wrapper<V, S, E> {
    pub fn new<R: Future<Output = ()>>(
        _: MainThreadMarker,
        value: V,
        handler: fn(&RefCell<Option<V>>, E),
        receiver: impl 'static + FnOnce(Arc<RefCell<Option<V>>>) -> R,
        sender_data: S,
        sender_handler: fn(&S, E),
    ) -> Self {
        let value = Arc::new(RefCell::new(Some(value)));

        wasm_bindgen_futures::spawn_local({
            let value = Arc::clone(&value);
            async move {
                receiver(Arc::clone(&value)).await;
                drop(value.borrow_mut().take().unwrap());
            }
        });

        Self { value: Value { value, local: PhantomData }, handler, sender_data, sender_handler }
    }

    pub fn send(&self, event: E) {
        if MainThreadMarker::new().is_some() {
            (self.handler)(&self.value.value, event)
        } else {
            (self.sender_handler)(&self.sender_data, event)
        }
    }

    pub fn value(&self, _: MainThreadMarker) -> Ref<'_, V> {
        Ref::map(self.value.value.borrow(), |value| value.as_ref().unwrap())
    }

    pub fn with_sender_data<T>(&self, f: impl FnOnce(&S) -> T) -> T {
        f(&self.sender_data)
    }
}

impl<V, S: Clone + Send, E> Clone for Wrapper<V, S, E> {
    fn clone(&self) -> Self {
        Self {
            value: Value { value: self.value.value.clone(), local: PhantomData },
            handler: self.handler,
            sender_data: self.sender_data.clone(),
            sender_handler: self.sender_handler,
        }
    }
}

impl<V, S: Clone + Send, E> Eq for Wrapper<V, S, E> {}

impl<V, S: Clone + Send, E> Hash for Wrapper<V, S, E> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.value.value).hash(state)
    }
}

impl<V, S: Clone + Send, E> Ord for Wrapper<V, S, E> {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        Arc::as_ptr(&self.value.value).cmp(&Arc::as_ptr(&other.value.value))
    }
}

impl<V, S: Clone + Send, E> PartialOrd for Wrapper<V, S, E> {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<V, S: Clone + Send, E> PartialEq for Wrapper<V, S, E> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.value.value, &other.value.value)
    }
}
