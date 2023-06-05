use atomic_waker::AtomicWaker;
use once_cell::unsync::Lazy;
use std::future;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvError, SendError, Sender, TryRecvError};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::task::Poll;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};

// Unsafe wrapper type that allows us to use `T` when it's not `Send` from other threads.
// `value` **must** only be accessed on the main thread.
pub struct MainThreadSafe<const SYNC: bool, T: 'static, E: 'static> {
    // We wrap this in an `Arc` to allow it to be safely cloned without accessing the value.
    // The `RwLock` lets us safely drop in any thread.
    // The `Option` lets us safely drop `T` only in the main thread, while letting other threads drop `None`.
    value: Arc<RwLock<Option<T>>>,
    handler: fn(&RwLock<Option<T>>, E),
    sender: AsyncSender<E>,
    // Prevent's `Send` or `Sync` to be automatically implemented.
    local: PhantomData<*const ()>,
}

impl<const SYNC: bool, T, E> MainThreadSafe<SYNC, T, E> {
    thread_local! {
        static MAIN_THREAD: Lazy<bool> = Lazy::new(|| {
            #[wasm_bindgen]
            extern "C" {
                #[derive(Clone)]
                type Global;

                #[wasm_bindgen(method, getter, js_name = Window)]
                fn window(this: &Global) -> JsValue;
            }

            let global: Global = js_sys::global().unchecked_into();
            !global.window().is_undefined()
        });
    }

    #[track_caller]
    fn new(value: T, handler: fn(&RwLock<Option<T>>, E)) -> Option<Self> {
        Self::MAIN_THREAD.with(|safe| {
            if !*safe.deref() {
                panic!("only callable from inside the `Window`")
            }
        });

        let value = Arc::new(RwLock::new(Some(value)));

        let (sender, receiver) = channel::<E>();

        wasm_bindgen_futures::spawn_local({
            let value = Arc::clone(&value);
            async move {
                while let Ok(event) = receiver.next().await {
                    handler(&value, event)
                }

                // An error was returned because the channel was closed, which
                // happens when all senders are dropped.
                value.write().unwrap().take().unwrap();
            }
        });

        Some(Self {
            value,
            handler,
            sender,
            local: PhantomData,
        })
    }

    pub fn send(&self, event: E) {
        Self::MAIN_THREAD.with(|is_main_thread| {
            if *is_main_thread.deref() {
                (self.handler)(&self.value, event)
            } else {
                self.sender.send(event).unwrap()
            }
        })
    }

    fn is_main_thread(&self) -> bool {
        Self::MAIN_THREAD.with(|is_main_thread| *is_main_thread.deref())
    }

    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> Option<R> {
        Self::MAIN_THREAD.with(|is_main_thread| {
            if *is_main_thread.deref() {
                Some(f(self.value.read().unwrap().as_ref().unwrap()))
            } else {
                None
            }
        })
    }

    fn with_mut<R>(&self, f: impl FnOnce(&mut T) -> R) -> Option<R> {
        Self::MAIN_THREAD.with(|is_main_thread| {
            if *is_main_thread.deref() {
                Some(f(self.value.write().unwrap().as_mut().unwrap()))
            } else {
                None
            }
        })
    }
}

impl<const SYNC: bool, T, E> Clone for MainThreadSafe<SYNC, T, E> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            handler: self.handler,
            sender: self.sender.clone(),
            local: PhantomData,
        }
    }
}

unsafe impl<const SYNC: bool, T, E> Send for MainThreadSafe<SYNC, T, E> {}
unsafe impl<T, E> Sync for MainThreadSafe<true, T, E> {}

fn channel<T>() -> (AsyncSender<T>, AsyncReceiver<T>) {
    let (sender, receiver) = mpsc::channel();
    let sender = Arc::new(Mutex::new(sender));
    let waker = Arc::new(AtomicWaker::new());
    let closed = Arc::new(AtomicBool::new(false));

    let sender = AsyncSender {
        sender,
        closed: closed.clone(),
        waker: Arc::clone(&waker),
    };
    let receiver = AsyncReceiver {
        receiver,
        closed,
        waker,
    };

    (sender, receiver)
}

struct AsyncSender<T> {
    // We need to wrap it into a `Mutex` to make it `Sync`. So the sender can't
    // be accessed on the main thread, as it could block. Additionally we need
    // to wrap it in an `Arc` to make it clonable on the main thread without
    // having to block.
    sender: Arc<Mutex<Sender<T>>>,
    closed: Arc<AtomicBool>,
    waker: Arc<AtomicWaker>,
}

impl<T> AsyncSender<T> {
    pub fn send(&self, event: T) -> Result<(), SendError<T>> {
        self.sender.lock().unwrap().send(event)?;
        self.waker.wake();

        Ok(())
    }
}

impl<T> Clone for AsyncSender<T> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            waker: self.waker.clone(),
            closed: self.closed.clone(),
        }
    }
}

impl<T> Drop for AsyncSender<T> {
    fn drop(&mut self) {
        // If it's the last + the one held by the receiver make sure to wake it
        // up and tell it that all receiver have dropped.
        if Arc::strong_count(&self.closed) == 2 {
            self.closed.store(true, Ordering::Relaxed);
            self.waker.wake()
        }
    }
}

struct AsyncReceiver<T> {
    receiver: Receiver<T>,
    closed: Arc<AtomicBool>,
    waker: Arc<AtomicWaker>,
}

impl<T> AsyncReceiver<T> {
    pub async fn next(&self) -> Result<T, RecvError> {
        future::poll_fn(|cx| match self.receiver.try_recv() {
            Ok(event) => Poll::Ready(Ok(event)),
            Err(TryRecvError::Empty) => {
                if self.closed.load(Ordering::Relaxed) {
                    return Poll::Ready(Err(RecvError));
                }

                self.waker.register(cx.waker());

                match self.receiver.try_recv() {
                    Ok(event) => Poll::Ready(Ok(event)),
                    Err(TryRecvError::Empty) => {
                        if self.closed.load(Ordering::Relaxed) {
                            Poll::Ready(Err(RecvError))
                        } else {
                            Poll::Pending
                        }
                    }
                    Err(TryRecvError::Disconnected) => Poll::Ready(Err(RecvError)),
                }
            }
            Err(TryRecvError::Disconnected) => Poll::Ready(Err(RecvError)),
        })
        .await
    }
}

pub struct Dispatcher<T: 'static>(MainThreadSafe<true, T, Closure<T>>);

pub enum Closure<T> {
    Ref(Box<dyn FnOnce(&T) + Send>),
    RefMut(Box<dyn FnOnce(&mut T) + Send>),
}

impl<T> Dispatcher<T> {
    #[track_caller]
    pub fn new(value: T) -> Option<Self> {
        MainThreadSafe::new(value, |value, closure| match closure {
            Closure::Ref(f) => f(value.read().unwrap().as_ref().unwrap()),
            Closure::RefMut(f) => f(value.write().unwrap().as_mut().unwrap()),
        })
        .map(Self)
    }

    pub fn dispatch(&self, f: impl 'static + FnOnce(&T) + Send) {
        if self.is_main_thread() {
            self.0.with(f).unwrap()
        } else {
            self.0.send(Closure::Ref(Box::new(f)))
        }
    }

    pub fn dispatch_mut(&self, f: impl 'static + FnOnce(&mut T) + Send) {
        if self.is_main_thread() {
            self.0.with_mut(f).unwrap()
        } else {
            self.0.send(Closure::RefMut(Box::new(f)))
        }
    }

    pub fn queue<R: 'static + Send>(&self, f: impl 'static + FnOnce(&T) -> R + Send) -> R {
        if self.is_main_thread() {
            self.0.with(f).unwrap()
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
    type Target = MainThreadSafe<true, T, Closure<T>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

type ChannelValue<T, E> = MainThreadSafe<false, (T, fn(&T, E)), E>;

pub struct Channel<T: 'static, E: 'static>(ChannelValue<T, E>);

impl<T, E> Channel<T, E> {
    pub fn new(value: T, handler: fn(&T, E)) -> Option<Self> {
        MainThreadSafe::new((value, handler), |runner, event| {
            let lock = runner.read().unwrap();
            let (value, handler) = lock.as_ref().unwrap();
            handler(value, event);
        })
        .map(Self)
    }
}

impl<T, E> Clone for Channel<T, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T, E> Deref for Channel<T, E> {
    type Target = ChannelValue<T, E>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
