use std::future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::Poll;

use super::super::main_thread::MainThreadMarker;
use super::{AtomicWaker, Wrapper};

pub struct WakerSpawner<T: 'static>(Wrapper<Handler<T>, Sender, ()>);

pub struct Waker<T: 'static>(Wrapper<Handler<T>, Sender, ()>);

struct Handler<T> {
    value: T,
    handler: fn(&T, bool),
}

#[derive(Clone)]
struct Sender(Arc<Inner>);

impl<T> WakerSpawner<T> {
    pub fn new(main_thread: MainThreadMarker, value: T, handler: fn(&T, bool)) -> Self {
        let inner = Arc::new(Inner {
            awoken: AtomicBool::new(false),
            waker: AtomicWaker::new(),
            closed: AtomicBool::new(false),
        });

        let handler = Handler { value, handler };

        let sender = Sender(Arc::clone(&inner));

        Self(Wrapper::new(
            main_thread,
            handler,
            |handler, _| {
                let handler = handler.borrow();
                let handler = handler.as_ref().unwrap();
                (handler.handler)(&handler.value, true);
            },
            {
                let inner = Arc::clone(&inner);

                move |handler| async move {
                    while future::poll_fn(|cx| {
                        if inner.awoken.swap(false, Ordering::Relaxed) {
                            Poll::Ready(true)
                        } else {
                            inner.waker.register(cx.waker());

                            if inner.awoken.swap(false, Ordering::Relaxed) {
                                Poll::Ready(true)
                            } else {
                                if inner.closed.load(Ordering::Relaxed) {
                                    return Poll::Ready(false);
                                }

                                Poll::Pending
                            }
                        }
                    })
                    .await
                    {
                        let handler = handler.borrow();
                        let handler = handler.as_ref().unwrap();
                        (handler.handler)(&handler.value, false);
                    }
                }
            },
            sender,
            |inner, _| {
                inner.0.awoken.store(true, Ordering::Relaxed);
                inner.0.waker.wake();
            },
        ))
    }

    pub fn waker(&self) -> Waker<T> {
        Waker(self.0.clone())
    }

    pub fn take(&self) -> bool {
        debug_assert!(
            MainThreadMarker::new().is_some(),
            "this should only be called from the main thread"
        );

        self.0.with_sender_data(|inner| inner.0.awoken.swap(false, Ordering::Relaxed))
    }
}

impl<T> Drop for WakerSpawner<T> {
    fn drop(&mut self) {
        self.0.with_sender_data(|inner| {
            inner.0.closed.store(true, Ordering::Relaxed);
            inner.0.waker.wake();
        });
    }
}

impl<T> Waker<T> {
    pub fn wake(&self) {
        self.0.send(())
    }
}

impl<T> Clone for Waker<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

struct Inner {
    awoken: AtomicBool,
    waker: AtomicWaker,
    closed: AtomicBool,
}
