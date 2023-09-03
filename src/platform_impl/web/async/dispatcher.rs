use super::{channel, AsyncSender, Wrapper};
use std::sync::{Arc, Condvar, Mutex};

pub struct Dispatcher<T: 'static>(Wrapper<true, T, AsyncSender<Closure<T>>, Closure<T>>);

struct Closure<T>(Box<dyn FnOnce(&T) + Send>);

impl<T> Dispatcher<T> {
    #[track_caller]
    pub fn new(value: T) -> Option<Self> {
        let (sender, receiver) = channel::<Closure<T>>();

        Wrapper::new(
            value,
            |value, Closure(closure)| {
                // SAFETY: The given `Closure` here isn't really `'static`, so we shouldn't do anything
                // funny with it here. See `Self::queue()`.
                closure(value.read().unwrap().as_ref().unwrap())
            },
            move |value| async move {
                while let Ok(Closure(closure)) = receiver.next().await {
                    // SAFETY: The given `Closure` here isn't really `'static`, so we shouldn't do anything
                    // funny with it here. See `Self::queue()`.
                    closure(value.read().unwrap().as_ref().unwrap())
                }
            },
            sender,
            |sender, closure| {
                // SAFETY: The given `Closure` here isn't really `'static`, so we shouldn't do anything
                // funny with it here. See `Self::queue()`.
                sender.send(closure).unwrap()
            },
        )
        .map(Self)
    }

    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> Option<R> {
        self.0.with(f)
    }

    pub fn dispatch(&self, f: impl 'static + FnOnce(&T) + Send) {
        if self.0.is_main_thread() {
            self.0.with(f).unwrap()
        } else {
            self.0.send(Closure(Box::new(f)))
        }
    }

    pub fn queue<R: Send>(&self, f: impl FnOnce(&T) -> R + Send) -> R {
        if self.0.is_main_thread() {
            self.0.with(f).unwrap()
        } else {
            let pair = Arc::new((Mutex::new(None), Condvar::new()));
            let closure = Box::new({
                let pair = pair.clone();
                move |value: &T| {
                    *pair.0.lock().unwrap() = Some(f(value));
                    pair.1.notify_one();
                }
            }) as Box<dyn FnOnce(&T) + Send>;
            // SAFETY: The `transmute` is necessary because `Closure` requires `'static`. This is
            // safe because this function won't return until `f` has finished executing. See
            // `Self::new()`.
            let closure = Closure(unsafe { std::mem::transmute(closure) });

            self.0.send(closure);

            let mut started = pair.0.lock().unwrap();

            while started.is_none() {
                started = pair.1.wait(started).unwrap();
            }

            started.take().unwrap()
        }
    }
}

impl<T> Drop for Dispatcher<T> {
    fn drop(&mut self) {
        self.0.with_sender_data(|sender| sender.close())
    }
}
