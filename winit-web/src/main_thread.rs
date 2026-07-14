use alloc::boxed::Box;
use core::fmt::{self, Debug, Formatter};
use core::marker::PhantomData;
use core::mem;

use once_cell::race::OnceBox;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};

use super::r#async::{self, Sender};

fn is_main_thread() -> bool {
    #[wasm_bindgen]
    extern "C" {
        #[derive(Clone)]
        type Global;

        #[wasm_bindgen(method, getter, js_name = Window)]
        fn window(this: &Global) -> JsValue;
    }

    !js_sys::global().unchecked_into::<Global>().window().is_undefined()
}

#[derive(Clone, Copy, Debug)]
pub struct MainThreadMarker(PhantomData<*const ()>);

impl MainThreadMarker {
    pub fn new() -> Option<Self> {
        is_main_thread().then_some(MainThreadMarker(PhantomData))
    }
}

pub struct MainThreadSafe<T: 'static>(Option<T>);

impl<T> MainThreadSafe<T> {
    pub fn new(_: MainThreadMarker, value: T) -> Self {
        DROP_HANDLER.get_or_init(|| {
            let (sender, receiver) = r#async::channel();
            wasm_bindgen_futures::spawn_local(
                async move { while receiver.next().await.is_ok() {} },
            );

            Box::new(sender)
        });

        Self(Some(value))
    }

    pub fn into_inner(mut self, _: MainThreadMarker) -> T {
        self.0.take().expect("already taken or dropped")
    }

    pub fn get(&self, _: MainThreadMarker) -> &T {
        self.0.as_ref().expect("already taken or dropped")
    }
}

impl<T: Debug> Debug for MainThreadSafe<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if MainThreadMarker::new().is_some() {
            f.debug_tuple("MainThreadSafe").field(&self.0).finish()
        } else {
            f.debug_struct("MainThreadSafe").finish_non_exhaustive()
        }
    }
}

impl<T> Drop for MainThreadSafe<T> {
    fn drop(&mut self) {
        if let Some(value) = self.0.take() {
            if mem::needs_drop::<T>() && MainThreadMarker::new().is_none() {
                DROP_HANDLER
                    .get()
                    .expect("drop handler not initialized when setting canvas")
                    .send(DropBox(Box::new(value)))
                    .expect("sender dropped in main thread")
            }
        }
    }
}

unsafe impl<T> Send for MainThreadSafe<T> {}
unsafe impl<T> Sync for MainThreadSafe<T> {}

static DROP_HANDLER: OnceBox<Sender<DropBox>> = OnceBox::new();

struct DropBox(#[allow(dead_code)] Box<dyn Any>);

unsafe impl Send for DropBox {}
unsafe impl Sync for DropBox {}

trait Any {}
impl<T> Any for T {}
