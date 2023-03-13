use std::future::Future;
use std::pin::Pin;
use std::sync::mpsc::{self, Receiver, RecvError, SendError, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

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
        sender,
        waker: Arc::clone(&waker),
    };
    let receiver = AsyncReceiver { receiver, waker };

    (sender, receiver)
}

pub struct AsyncSender<T: 'static> {
    sender: Sender<T>,
    waker: Arc<Mutex<Option<Waker>>>,
}

impl<T: 'static> AsyncSender<T> {
    pub fn send(&self, event: T) -> Result<(), SendError<T>> {
        self.sender.send(event)?;

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

pub struct AsyncReceiver<T: 'static> {
    receiver: Receiver<T>,
    waker: Arc<Mutex<Option<Waker>>>,
}

impl<T: 'static> Future for AsyncReceiver<T> {
    type Output = Result<T, RecvError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.receiver.try_recv() {
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
        }
    }
}
