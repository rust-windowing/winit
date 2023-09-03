use super::Wrapper;
use atomic_waker::AtomicWaker;
use std::future;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::Poll;

pub struct Waker<T: 'static> {
    wrapper: Wrapper<false, Handler<T>, Sender, usize>,
    inner: Arc<Inner>,
}

struct Handler<T> {
    value: T,
    handler: fn(&T, usize),
}

#[derive(Clone)]
struct Sender(Arc<Inner>);

impl Drop for Sender {
    fn drop(&mut self) {
        if Arc::strong_count(&self.0) == 1 {
            self.0.closed.store(true, Ordering::Relaxed);
            self.0.waker.wake();
        }
    }
}

impl<T> Waker<T> {
    #[track_caller]
    pub fn new(value: T, handler: fn(&T, usize)) -> Option<Self> {
        let inner = Arc::new(Inner {
            counter: AtomicUsize::new(0),
            waker: AtomicWaker::new(),
            closed: AtomicBool::new(false),
        });

        let handler = Handler { value, handler };

        let sender = Sender(Arc::clone(&inner));

        let wrapper = Wrapper::new(
            handler,
            |handler, count| {
                let handler = handler.read().unwrap();
                let handler = handler.as_ref().unwrap();
                (handler.handler)(&handler.value, count);
            },
            {
                let inner = Arc::clone(&inner);

                move |handler| async move {
                    while let Some(count) = future::poll_fn(|cx| {
                        let count = inner.counter.swap(0, Ordering::Relaxed);

                        if count > 0 {
                            Poll::Ready(Some(count))
                        } else {
                            if inner.closed.load(Ordering::Relaxed) {
                                return Poll::Ready(None);
                            }

                            inner.waker.register(cx.waker());

                            let count = inner.counter.swap(0, Ordering::Relaxed);

                            if count > 0 {
                                Poll::Ready(Some(count))
                            } else {
                                if inner.closed.load(Ordering::Relaxed) {
                                    return Poll::Ready(None);
                                }

                                Poll::Pending
                            }
                        }
                    })
                    .await
                    {
                        let handler = handler.read().unwrap();
                        let handler = handler.as_ref().unwrap();
                        (handler.handler)(&handler.value, count);
                    }
                }
            },
            sender,
            |inner, _| {
                inner.0.counter.fetch_add(1, Ordering::Relaxed);
                inner.0.waker.wake();
            },
        )?;

        Some(Self { wrapper, inner })
    }

    pub fn wake(&self) {
        self.wrapper.send(1)
    }
}

impl<T> Clone for Waker<T> {
    fn clone(&self) -> Self {
        Self {
            wrapper: self.wrapper.clone(),
            inner: Arc::clone(&self.inner),
        }
    }
}

struct Inner {
    counter: AtomicUsize,
    waker: AtomicWaker,
    closed: AtomicBool,
}
