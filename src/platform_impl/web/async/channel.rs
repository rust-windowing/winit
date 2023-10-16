use atomic_waker::AtomicWaker;
use std::future;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvError, SendError, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::task::Poll;

// NOTE: This channel doesn't wake up when all senders or receivers are
// dropped. This is acceptable as long as it's only used in `Dispatcher`, which
// has it's own `Drop` behavior.

pub fn channel<T>() -> (AsyncSender<T>, AsyncReceiver<T>) {
    let (sender, receiver) = mpsc::channel();
    let sender = Arc::new(Mutex::new(sender));
    let inner = Arc::new(Inner {
        closed: AtomicBool::new(false),
        waker: AtomicWaker::new(),
    });

    let sender = AsyncSender {
        sender,
        inner: Arc::clone(&inner),
    };
    let receiver = AsyncReceiver {
        receiver: Rc::new(receiver),
        inner,
    };

    (sender, receiver)
}

pub struct AsyncSender<T> {
    // We need to wrap it into a `Mutex` to make it `Sync`. So the sender can't
    // be accessed on the main thread, as it could block. Additionally we need
    // to wrap it in an `Arc` to make it clonable on the main thread without
    // having to block.
    sender: Arc<Mutex<Sender<T>>>,
    inner: Arc<Inner>,
}

impl<T> AsyncSender<T> {
    pub fn send(&self, event: T) -> Result<(), SendError<T>> {
        self.sender.lock().unwrap().send(event)?;
        self.inner.waker.wake();

        Ok(())
    }

    pub fn close(&self) {
        self.inner.closed.store(true, Ordering::Relaxed);
        self.inner.waker.wake()
    }
}

impl<T> Clone for AsyncSender<T> {
    fn clone(&self) -> Self {
        Self {
            sender: Arc::clone(&self.sender),
            inner: Arc::clone(&self.inner),
        }
    }
}

pub struct AsyncReceiver<T> {
    receiver: Rc<Receiver<T>>,
    inner: Arc<Inner>,
}

impl<T> AsyncReceiver<T> {
    pub async fn next(&self) -> Result<T, RecvError> {
        future::poll_fn(|cx| match self.receiver.try_recv() {
            Ok(event) => Poll::Ready(Ok(event)),
            Err(TryRecvError::Empty) => {
                self.inner.waker.register(cx.waker());

                match self.receiver.try_recv() {
                    Ok(event) => Poll::Ready(Ok(event)),
                    Err(TryRecvError::Empty) => {
                        if self.inner.closed.load(Ordering::Relaxed) {
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

    pub fn try_recv(&self) -> Result<Option<T>, RecvError> {
        match self.receiver.try_recv() {
            Ok(value) => Ok(Some(value)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(RecvError),
        }
    }
}

impl<T> Clone for AsyncReceiver<T> {
    fn clone(&self) -> Self {
        Self {
            receiver: Rc::clone(&self.receiver),
            inner: Arc::clone(&self.inner),
        }
    }
}

struct Inner {
    closed: AtomicBool,
    waker: AtomicWaker,
}
