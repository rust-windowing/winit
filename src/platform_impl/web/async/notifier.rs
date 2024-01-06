use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;
use std::task::Waker;

use concurrent_queue::ConcurrentQueue;
use concurrent_queue::PushError;

#[derive(Debug)]
pub struct Notifier(Arc<Inner>);

impl Notifier {
    pub fn new() -> Self {
        Self(Arc::new(Inner {
            queue: ConcurrentQueue::unbounded(),
            ready: AtomicBool::new(false),
        }))
    }

    pub fn notify(self) {
        self.0.ready.store(true, Ordering::Relaxed);

        self.0.queue.close();

        while let Ok(waker) = self.0.queue.pop() {
            waker.wake()
        }
    }

    pub fn notified(&self) -> Notified {
        Notified(Some(Arc::clone(&self.0)))
    }
}

#[derive(Clone)]
pub struct Notified(Option<Arc<Inner>>);

impl Future for Notified {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.0.take().expect("`Receiver` polled after completion");

        if this.ready.load(Ordering::Relaxed) {
            return Poll::Ready(());
        }

        match this.queue.push(cx.waker().clone()) {
            Ok(()) => {
                if this.ready.load(Ordering::Relaxed) {
                    return Poll::Ready(());
                }

                self.0 = Some(this);
                Poll::Pending
            }
            Err(PushError::Closed(_)) => {
                debug_assert!(this.ready.load(Ordering::Relaxed));
                Poll::Ready(())
            }
            Err(PushError::Full(_)) => {
                unreachable!("found full queue despite using unbounded queue")
            }
        }
    }
}

#[derive(Debug)]
struct Inner {
    queue: ConcurrentQueue<Waker>,
    ready: AtomicBool,
}
