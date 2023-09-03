use atomic_waker::AtomicWaker;
use std::future::{self, Future};
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
pub struct MainThreadSafe<const SYNC: bool, T: 'static, S: Clone + Send, E> {
    // We wrap this in an `Arc` to allow it to be safely cloned without accessing the value.
    // The `RwLock` lets us safely drop in any thread.
    // The `Option` lets us safely drop `T` only in the main thread, while letting other threads drop `None`.
    value: Arc<RwLock<Option<T>>>,
    handler: fn(&RwLock<Option<T>>, E),
    sender_data: S,
    sender_handler: fn(&S, E),
    // Prevent's `Send` or `Sync` to be automatically implemented.
    local: PhantomData<*const ()>,
}

impl<const SYNC: bool, T, S: Clone + Send, E> MainThreadSafe<SYNC, T, S, E> {
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
        value: T,
        handler: fn(&RwLock<Option<T>>, E),
        receiver: impl 'static + FnOnce(Arc<RwLock<Option<T>>>) -> R,
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

                // An error was returned because the channel was closed, which
                // happens when all senders are dropped.
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

    fn is_main_thread(&self) -> bool {
        Self::MAIN_THREAD.with(|is_main_thread| *is_main_thread)
    }

    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> Option<R> {
        Self::MAIN_THREAD.with(|is_main_thread| {
            if *is_main_thread {
                Some(f(self.value.read().unwrap().as_ref().unwrap()))
            } else {
                None
            }
        })
    }
}

impl<const SYNC: bool, T, S: Clone + Send, E> Clone for MainThreadSafe<SYNC, T, S, E> {
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

unsafe impl<const SYNC: bool, T, S: Clone + Send, E> Send for MainThreadSafe<SYNC, T, S, E> {}
unsafe impl<T, S: Clone + Send + Sync, E> Sync for MainThreadSafe<true, T, S, E> {}

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

pub struct AsyncSender<T> {
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

pub struct Dispatcher<T: 'static>(MainThreadSafe<true, T, AsyncSender<Closure<T>>, Closure<T>>);

pub struct Closure<T>(Box<dyn FnOnce(&T) + Send>);

impl<T> Dispatcher<T> {
    #[track_caller]
    pub fn new(value: T) -> Option<Self> {
        let (sender, receiver) = channel::<Closure<T>>();

        MainThreadSafe::new(
            value,
            |value, Closure(closure)| {
                // SAFETY: The given `Closure` here isn't really `'static`, so we shouldn't do anything
                // funny with it here. See `Self::queue()`.
                closure(value.read().unwrap().as_ref().unwrap())
            },
            move |value| async move {
                while let Ok(Closure(closure)) = receiver.next().await {
                    // SAFETY: The given `Closure` here isn't really `'static`, so we shouldn't do anything
                    // funny with it here. See `Self::queue()`.
                    closure(value.read().unwrap().as_ref().unwrap())
                }
            },
            sender,
            |sender, closure| {
                // SAFETY: The given `Closure` here isn't really `'static`, so we shouldn't do anything
                // funny with it here. See `Self::queue()`.
                sender.send(closure).unwrap()
            },
        )
        .map(Self)
    }

    pub fn dispatch(&self, f: impl 'static + FnOnce(&T) + Send) {
        if self.is_main_thread() {
            self.0.with(f).unwrap()
        } else {
            self.0.send(Closure(Box::new(f)))
        }
    }

    pub fn queue<R: Send>(&self, f: impl FnOnce(&T) -> R + Send) -> R {
        if self.is_main_thread() {
            self.0.with(f).unwrap()
        } else {
            let pair = Arc::new((Mutex::new(None), Condvar::new()));
            let closure = Box::new({
                let pair = pair.clone();
                move |value: &T| {
                    *pair.0.lock().unwrap() = Some(f(value));
                    pair.1.notify_one();
                }
            }) as Box<dyn FnOnce(&T) + Send>;
            // SAFETY: The `transmute` is necessary because `Closure` requires `'static`. This is
            // safe because this function won't return until `f` has finished executing. See
            // `Self::new()`.
            let closure = Closure(unsafe { std::mem::transmute(closure) });

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
    type Target = MainThreadSafe<true, T, AsyncSender<Closure<T>>, Closure<T>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

type ChannelValue<T, E> = MainThreadSafe<false, (T, fn(&T, E)), AsyncSender<E>, E>;

pub struct Channel<T: 'static, E: 'static + Send>(ChannelValue<T, E>);

impl<T, E: Send> Channel<T, E> {
    pub fn new(value: T, handler: fn(&T, E)) -> Option<Self> {
        let (sender, receiver) = channel::<E>();

        MainThreadSafe::new(
            (value, handler),
            |lock, event| {
                let lock = lock.read().unwrap();
                let (value, handler) = lock.as_ref().unwrap();
                handler(value, event)
            },
            move |lock| async move {
                while let Ok(event) = receiver.next().await {
                    let lock = lock.read().unwrap();
                    let (value, handler) = lock.as_ref().unwrap();
                    handler(value, event)
                }
            },
            sender,
            |sender, event| sender.send(event).unwrap(),
        )
        .map(Self)
    }
}

impl<T, E: Send> Clone for Channel<T, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T, E: Send> Deref for Channel<T, E> {
    type Target = ChannelValue<T, E>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
