use alloc::sync::Arc;
use core::error::Error;
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::Poll;
use core::{fmt, future};

use super::{AtomicWaker, ConcurrentQueue, PopError, PushError};

pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let queue = ConcurrentQueue::unbounded();
    let shared =
        Arc::new(Shared { closed: AtomicBool::new(false), waker: AtomicWaker::new(), queue });

    let sender = Sender { shared: Arc::clone(&shared) };
    let receiver = Receiver { shared };

    (sender, receiver)
}

pub struct Sender<T> {
    shared: Arc<Shared<T>>,
}

impl<T> Sender<T> {
    pub fn send(&self, event: T) -> Result<(), SendError<T>> {
        if let Err(PushError::Closed(event) | PushError::Full(event)) =
            self.shared.queue.push(event)
        {
            return Err(SendError(event));
        }

        self.shared.waker.wake();

        Ok(())
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        self.shared.closed.store(true, Ordering::Relaxed);
        self.shared.waker.wake();
    }
}

pub struct Receiver<T> {
    shared: Arc<Shared<T>>,
}

impl<T> Receiver<T> {
    pub async fn next(&self) -> Result<T, RecvError> {
        future::poll_fn(|cx| match self.shared.queue.pop() {
            Ok(event) => Poll::Ready(Ok(event)),
            Err(PopError::Empty) => {
                self.shared.waker.register(cx.waker());

                match self.shared.queue.pop() {
                    Ok(event) => Poll::Ready(Ok(event)),
                    Err(PopError::Empty) => {
                        if self.shared.closed.load(Ordering::Relaxed) {
                            Poll::Ready(Err(RecvError))
                        } else {
                            Poll::Pending
                        }
                    },
                    Err(PopError::Closed) => Poll::Ready(Err(RecvError)),
                }
            },
            Err(PopError::Closed) => Poll::Ready(Err(RecvError)),
        })
        .await
    }

    pub fn try_recv(&self) -> Result<Option<T>, RecvError> {
        match self.shared.queue.pop() {
            Ok(value) => Ok(Some(value)),
            Err(PopError::Empty) => Ok(None),
            Err(PopError::Closed) => Err(RecvError),
        }
    }
}

struct Shared<T> {
    closed: AtomicBool,
    waker: AtomicWaker,
    queue: ConcurrentQueue<T>,
}

/// An error returned from the [`Sender::send`] function on **channel**s.
#[derive(PartialEq, Eq, Clone, Copy)]
pub struct SendError<T>(pub T);

impl<T> fmt::Debug for SendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SendError").finish_non_exhaustive()
    }
}

impl<T> fmt::Display for SendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "sending on a closed channel".fmt(f)
    }
}

impl<T> Error for SendError<T> {}

/// An error returned from the [`try_recv`] function on a [`Receiver`].
///
/// [`try_recv`]: Receiver::try_recv
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct RecvError;

impl fmt::Display for RecvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "receiving on a closed channel".fmt(f)
    }
}

impl Error for RecvError {}
