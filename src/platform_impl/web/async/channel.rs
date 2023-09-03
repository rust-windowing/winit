use atomic_waker::AtomicWaker;
use std::future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvError, SendError, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::task::Poll;

pub fn channel<T>() -> (AsyncSender<T>, AsyncReceiver<T>) {
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

pub struct AsyncReceiver<T> {
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
