use super::Wrapper;
use atomic_waker::AtomicWaker;
use std::future;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::Poll;

pub struct Waker<T: 'static> {
    wrapper: Wrapper<false, Handler<T>, Sender, usize>,
    counter: Arc<AtomicUsize>,
    waker: Arc<AtomicWaker>,
}

struct Handler<T> {
    value: T,
    handler: fn(&T, usize),
}

#[derive(Clone)]
struct Sender {
    counter: Arc<AtomicUsize>,
    waker: Arc<AtomicWaker>,
    closed: Arc<AtomicBool>,
}

impl Drop for Sender {
    fn drop(&mut self) {
        if Arc::strong_count(&self.closed) == 1 {
            self.closed.store(true, Ordering::Relaxed);
            self.waker.wake();
        }
    }
}

impl<T> Waker<T> {
    #[track_caller]
    pub fn new(value: T, handler: fn(&T, usize)) -> Option<Self> {
        let counter = Arc::new(AtomicUsize::new(0));
        let waker = Arc::new(AtomicWaker::new());
        let closed = Arc::new(AtomicBool::new(false));

        let handler = Handler { value, handler };

        let sender = Sender {
            counter: Arc::clone(&counter),
            waker: Arc::clone(&waker),
            closed: Arc::clone(&closed),
        };

        let wrapper = Wrapper::new(
            handler,
            |handler, count| {
                let handler = handler.read().unwrap();
                let handler = handler.as_ref().unwrap();
                (handler.handler)(&handler.value, count);
            },
            {
                let counter = Arc::clone(&counter);
                let waker = Arc::clone(&waker);

                move |handler| async move {
                    while let Some(count) = future::poll_fn(|cx| {
                        let count = counter.swap(0, Ordering::Relaxed);

                        if count > 0 {
                            Poll::Ready(Some(count))
                        } else {
                            if closed.load(Ordering::Relaxed) {
                                return Poll::Ready(None);
                            }

                            waker.register(cx.waker());

                            let count = counter.swap(0, Ordering::Relaxed);

                            if count > 0 {
                                Poll::Ready(Some(count))
                            } else {
                                if closed.load(Ordering::Relaxed) {
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
                inner.counter.fetch_add(1, Ordering::Relaxed);
                inner.waker.wake();
            },
        )?;

        Some(Self {
            wrapper,
            counter,
            waker,
        })
    }

    pub fn wake(&self) {
        self.wrapper.send(1)
    }
}

impl<T> Clone for Waker<T> {
    fn clone(&self) -> Self {
        Self {
            wrapper: self.wrapper.clone(),
            counter: Arc::clone(&self.counter),
            waker: Arc::clone(&self.waker),
        }
    }
}
