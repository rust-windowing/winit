use atomic_waker::AtomicWaker;
use once_cell::unsync::Lazy;
use std::cell::{Ref, RefCell, RefMut};
use std::future;
use std::mem::ManuallyDrop;
use std::ops::{Deref, DerefMut};
use std::rc::{Rc, Weak};
use std::sync::mpsc::{self, Receiver, RecvError, SendError, Sender, TryRecvError};
use std::sync::{Arc, Condvar, Mutex};
use std::task::Poll;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};

// Unsafe wrapper type that allows us to use `T` when it's not `Send` from other threads.
// `value` must *only* be accessed on the main thread.
pub struct MainThreadSafe<T: 'static, E: 'static> {
    value: ManuallyDrop<Weak<RefCell<T>>>,
    handler: fn(&RefCell<T>, E),
    sender: AsyncSender<E>,
}

impl<T, E> MainThreadSafe<T, E> {
    thread_local! {
        static MAIN_THREAD: Lazy<bool> = Lazy::new(|| {
            #[wasm_bindgen]
            extern "C" {
                #[derive(Clone)]
                pub(crate) type Global;

                #[wasm_bindgen(method, getter, js_name = Window)]
                fn window(this: &Global) -> JsValue;
            }

            let global: Global = js_sys::global().unchecked_into();
            !global.window().is_undefined()
        });
    }

    #[track_caller]
    pub fn new(value: T, handler: fn(&RefCell<T>, E)) -> Option<Self> {
        Self::MAIN_THREAD.with(|safe| {
            if !*safe.deref() {
                panic!("only callable from inside the `Window`")
            }
        });

        let value = Rc::new(RefCell::new(value));
        let weak = Rc::downgrade(&value);

        let (sender, receiver) = channel::<E>();

        wasm_bindgen_futures::spawn_local({
            async move {
                while let Ok(event) = receiver.next().await {
                    handler(&value, event)
                }

                // An error was returned because the channel was closed, which
                // happens when the window get dropped, so we can stop now.
                match Rc::try_unwrap(value) {
                    Ok(value) => drop(value),
                    Err(_) => panic!("couldn't enforce that value is dropped on the main thread"),
                }
            }
        });

        Some(Self {
            value: ManuallyDrop::new(weak),
            handler,
            sender,
        })
    }

    pub fn send(&self, event: E) {
        Self::MAIN_THREAD.with(|is_main_thread| {
            if *is_main_thread.deref() {
                (self.handler)(&self.value.upgrade().unwrap(), event)
            } else {
                self.sender.send(event).unwrap()
            }
        })
    }

    fn is_main_thread(&self) -> bool {
        Self::MAIN_THREAD.with(|is_main_thread| *is_main_thread.deref())
    }

    pub fn with<R>(&self, f: impl FnOnce(Ref<'_, T>) -> R) -> Option<R> {
        Self::MAIN_THREAD.with(|is_main_thread| {
            if *is_main_thread.deref() {
                Some(f(self.value.upgrade().unwrap().borrow()))
            } else {
                None
            }
        })
    }

    fn with_mut<R>(&self, f: impl FnOnce(RefMut<'_, T>) -> R) -> Option<R> {
        Self::MAIN_THREAD.with(|is_main_thread| {
            if *is_main_thread.deref() {
                Some(f(self.value.upgrade().unwrap().borrow_mut()))
            } else {
                None
            }
        })
    }
}

impl<T, E> Clone for MainThreadSafe<T, E> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            handler: self.handler,
            sender: self.sender.clone(),
        }
    }
}

unsafe impl<T, E> Send for MainThreadSafe<T, E> {}
unsafe impl<T, E> Sync for MainThreadSafe<T, E> {}

pub struct Dispatcher<T: 'static>(MainThreadSafe<T, Closure<T>>);

pub enum Closure<T> {
    Ref(Box<dyn FnOnce(&T) + Send>),
    RefMut(Box<dyn FnOnce(&mut T) + Send>),
}

impl<T> Dispatcher<T> {
    #[track_caller]
    pub fn new(value: T) -> Option<Self> {
        MainThreadSafe::new(value, |value, closure| {
            match closure {
                Closure::Ref(f) => f(value.borrow().deref()),
                Closure::RefMut(f) => f(value.borrow_mut().deref_mut()),
            }

            // An error was returned because the channel was closed, which
            // happens when the window get dropped, so we can stop now.
        })
        .map(Self)
    }

    pub fn dispatch(&self, f: impl 'static + FnOnce(&T) + Send) {
        if self.is_main_thread() {
            self.0.with(|value| f(value.deref())).unwrap()
        } else {
            self.0.send(Closure::Ref(Box::new(f)))
        }
    }

    pub fn dispatch_mut(&self, f: impl 'static + FnOnce(&mut T) + Send) {
        if self.is_main_thread() {
            self.0.with_mut(|mut value| f(value.deref_mut())).unwrap()
        } else {
            self.0.send(Closure::RefMut(Box::new(f)))
        }
    }

    pub fn queue<R: 'static + Send>(&self, f: impl 'static + FnOnce(&T) -> R + Send) -> R {
        if self.is_main_thread() {
            self.0.with(|value| f(value.deref())).unwrap()
        } else {
            let pair = Arc::new((Mutex::new(None), Condvar::new()));
            let closure = Closure::Ref(Box::new({
                let pair = pair.clone();
                move |value| {
                    *pair.0.lock().unwrap() = Some(f(value));
                    pair.1.notify_one();
                }
            }));

            self.0.send(closure);

            let mut started = pair.0.lock().unwrap();

            while started.is_none() {
                started = pair.1.wait(started).unwrap();
            }

            started.take().unwrap()
        }
    }
}

impl<T> Deref for Dispatcher<T> {
    type Target = MainThreadSafe<T, Closure<T>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

fn channel<T>() -> (AsyncSender<T>, AsyncReceiver<T>) {
    let (sender, receiver) = mpsc::channel();
    let sender = Mutex::new(Some(sender));
    let waker = Arc::new(AtomicWaker::new());

    let sender = AsyncSender {
        sender,
        waker: Arc::clone(&waker),
    };
    let receiver = AsyncReceiver { receiver, waker };

    (sender, receiver)
}

struct AsyncSender<T> {
    sender: Mutex<Option<Sender<T>>>,
    waker: Arc<AtomicWaker>,
}

impl<T> AsyncSender<T> {
    pub fn send(&self, event: T) -> Result<(), SendError<T>> {
        self.sender.lock().unwrap().as_ref().unwrap().send(event)?;
        self.waker.wake();

        Ok(())
    }
}

impl<T> Clone for AsyncSender<T> {
    fn clone(&self) -> Self {
        Self {
            sender: Mutex::new(self.sender.lock().unwrap().clone()),
            waker: self.waker.clone(),
        }
    }
}

impl<T> Drop for AsyncSender<T> {
    fn drop(&mut self) {
        self.sender.lock().unwrap().take().unwrap();

        // If it's the last + the one held by the receiver make sure to wake it
        // up and tell it to drop the value. It will only drop the value if the
        // receiver reports that the last sender was dropped, this is why we
        // drop the sender before this check.
        if Arc::strong_count(&self.waker) == 2 {
            self.waker.wake()
        }
    }
}

struct AsyncReceiver<T> {
    receiver: Receiver<T>,
    waker: Arc<AtomicWaker>,
}

impl<T> AsyncReceiver<T> {
    pub async fn next(&self) -> Result<T, RecvError> {
        future::poll_fn(|cx| match self.receiver.try_recv() {
            Ok(event) => Poll::Ready(Ok(event)),
            Err(TryRecvError::Empty) => {
                self.waker.register(cx.waker());

                match self.receiver.try_recv() {
                    Ok(event) => Poll::Ready(Ok(event)),
                    Err(TryRecvError::Empty) => Poll::Pending,
                    Err(TryRecvError::Disconnected) => Poll::Ready(Err(RecvError)),
                }
            }
            Err(TryRecvError::Disconnected) => Poll::Ready(Err(RecvError)),
        })
        .await
    }
}
