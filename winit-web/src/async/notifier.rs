use alloc::boxed::Box;
use alloc::sync::Arc;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};

use once_cell::race::OnceBox;

use super::{ConcurrentQueue, PushError};

#[derive(Debug)]
pub struct Notifier<T: Clone>(Arc<Inner<T>>);

impl<T: Clone> Notifier<T> {
    pub fn new() -> Self {
        Self(Arc::new(Inner { queue: ConcurrentQueue::unbounded(), value: OnceBox::new() }))
    }

    pub fn notify(self, value: T) {
        if self.0.value.set(Box::new(value)).is_err() {
            unreachable!("value set before")
        }
    }

    pub fn notified(&self) -> Notified<T> {
        Notified(Some(Arc::clone(&self.0)))
    }
}

impl<T: Clone> Drop for Notifier<T> {
    fn drop(&mut self) {
        self.0.queue.close();

        while let Ok(waker) = self.0.queue.pop() {
            waker.wake()
        }
    }
}

#[derive(Clone, Debug)]
pub struct Notified<T: Clone>(Option<Arc<Inner<T>>>);

impl<T: Clone> Future for Notified<T> {
    type Output = Option<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.0.take().expect("`Receiver` polled after completion");

        if this.value.get().is_none() {
            match this.queue.push(cx.waker().clone()) {
                Ok(()) => {
                    if this.value.get().is_none() {
                        self.0 = Some(this);
                        return Poll::Pending;
                    }
                },
                Err(PushError::Closed(_)) => (),
                Err(PushError::Full(_)) => {
                    unreachable!("found full queue despite using unbounded queue")
                },
            }
        }

        Poll::Ready(this.value.get().cloned())
    }
}

#[derive(Debug)]
struct Inner<T> {
    queue: ConcurrentQueue<Waker>,
    value: OnceBox<T>,
}
