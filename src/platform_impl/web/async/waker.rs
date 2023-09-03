use super::AsyncSender;
use super::{channel, Wrapper};
use std::ops::Deref;

type WakerValue<T, E> = Wrapper<false, (T, fn(&T, E)), AsyncSender<E>, E>;

pub struct Waker<T: 'static, E: 'static + Send>(WakerValue<T, E>);

impl<T, E: Send> Waker<T, E> {
    pub fn new(value: T, handler: fn(&T, E)) -> Option<Self> {
        let (sender, receiver) = channel::<E>();

        Wrapper::new(
            (value, handler),
            |lock, event| {
                let lock = lock.read().unwrap();
                let (value, handler) = lock.as_ref().unwrap();
                handler(value, event)
            },
            move |lock| async move {
                while let Ok(event) = receiver.next().await {
                    let lock = lock.read().unwrap();
                    let (value, handler) = lock.as_ref().unwrap();
                    handler(value, event)
                }
            },
            sender,
            |sender, event| sender.send(event).unwrap(),
        )
        .map(Self)
    }
}

impl<T, E: Send> Clone for Waker<T, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T, E: Send> Deref for Waker<T, E> {
    type Target = WakerValue<T, E>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
