use std::future;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, RecvError, SendError, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::task::{Poll, Waker};

use crate::event_loop::EventLoopClosed;

pub struct EventLoopProxy<T: 'static> {
    sender: AsyncSender<T>,
}

impl<T: 'static> EventLoopProxy<T> {
    pub fn new(sender: AsyncSender<T>) -> Self {
        Self { sender }
    }

    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed<T>> {
        match self.sender.send(event) {
            Ok(()) => Ok(()),
            Err(SendError(val)) => Err(EventLoopClosed(val)),
        }
    }
}

impl<T: 'static> Clone for EventLoopProxy<T> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

pub fn channel<T: 'static>() -> (AsyncSender<T>, AsyncReceiver<T>) {
    let (sender, receiver) = mpsc::channel();
    let waker = Arc::new(Mutex::new(None));

    let sender = AsyncSender {
        sender: Some(sender),
        waker: Arc::clone(&waker),
    };
    let receiver = AsyncReceiver {
        receiver: Rc::new(receiver),
        waker,
    };

    (sender, receiver)
}

pub struct AsyncSender<T: 'static> {
    sender: Option<Sender<T>>,
    waker: Arc<Mutex<Option<Waker>>>,
}

impl<T: 'static> AsyncSender<T> {
    pub fn send(&self, event: T) -> Result<(), SendError<T>> {
        self.sender.as_ref().unwrap().send(event)?;

        if let Some(waker) = self.waker.lock().unwrap().take() {
            waker.wake();
        }

        Ok(())
    }
}

impl<T: 'static> Clone for AsyncSender<T> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            waker: self.waker.clone(),
        }
    }
}

impl<T> Drop for AsyncSender<T> {
    fn drop(&mut self) {
        // The corresponding `Receiver` is used to spawn a future that waits
        // for messages in a loop. The future itself needs to be cleaned up
        // somehow, which is signalled by dropping the last `Sender`. But it
        // will do nothing if not woken up.

        // We have to drop the potentially last `Sender` **before** checking if
        // this is the last `Sender`. `Arc::strong_count` doesn't prevent
        // races.
        self.sender.take().unwrap();

        // This one + the one held by the future.
        if Arc::strong_count(&self.waker) == 2 {
            if let Some(waker) = self.waker.lock().unwrap().take() {
                waker.wake();
            }
        }
    }
}

pub struct AsyncReceiver<T: 'static> {
    receiver: Rc<Receiver<T>>,
    waker: Arc<Mutex<Option<Waker>>>,
}

impl<T: 'static> AsyncReceiver<T> {
    pub async fn next(&mut self) -> Result<T, RecvError> {
        future::poll_fn(|cx| match self.receiver.try_recv() {
            Ok(event) => Poll::Ready(Ok(event)),
            Err(TryRecvError::Empty) => {
                *self.waker.lock().unwrap() = Some(cx.waker().clone());

                match self.receiver.try_recv() {
                    Ok(event) => Poll::Ready(Ok(event)),
                    Err(TryRecvError::Empty) => Poll::Pending,
                    Err(TryRecvError::Disconnected) => Poll::Ready(Err(RecvError)),
                }
            }
            Err(TryRecvError::Disconnected) => Poll::Ready(Err(RecvError)),
        })
        .await
    }
}
