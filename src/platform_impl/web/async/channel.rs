use std::future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvError, SendError, TryRecvError};
use std::sync::Arc;
use std::task::Poll;

use super::AtomicWaker;

pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let (sender, receiver) = mpsc::channel();
    let shared = Arc::new(Shared { closed: AtomicBool::new(false), waker: AtomicWaker::new() });

    let sender = Sender { sender, shared: Arc::clone(&shared) };
    let receiver = Receiver { receiver, shared };

    (sender, receiver)
}

pub struct Sender<T> {
    sender: mpsc::Sender<T>,
    shared: Arc<Shared>,
}

impl<T> Sender<T> {
    pub fn send(&self, event: T) -> Result<(), SendError<T>> {
        self.sender.send(event)?;
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
    receiver: mpsc::Receiver<T>,
    shared: Arc<Shared>,
}

impl<T> Receiver<T> {
    pub async fn next(&self) -> Result<T, RecvError> {
        future::poll_fn(|cx| match self.receiver.try_recv() {
            Ok(event) => Poll::Ready(Ok(event)),
            Err(TryRecvError::Empty) => {
                self.shared.waker.register(cx.waker());

                match self.receiver.try_recv() {
                    Ok(event) => Poll::Ready(Ok(event)),
                    Err(TryRecvError::Empty) => {
                        if self.shared.closed.load(Ordering::Relaxed) {
                            Poll::Ready(Err(RecvError))
                        } else {
                            Poll::Pending
                        }
                    },
                    Err(TryRecvError::Disconnected) => Poll::Ready(Err(RecvError)),
                }
            },
            Err(TryRecvError::Disconnected) => Poll::Ready(Err(RecvError)),
        })
        .await
    }

    pub fn try_recv(&self) -> Result<Option<T>, RecvError> {
        match self.receiver.try_recv() {
            Ok(value) => Ok(Some(value)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(RecvError),
        }
    }
}

struct Shared {
    closed: AtomicBool,
    waker: AtomicWaker,
}
