use std::cell::Ref;
use std::cmp::Ordering;
use std::fmt::{self, Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::{Arc, Condvar, Mutex};

use super::super::main_thread::MainThreadMarker;
use super::{channel, Receiver, Sender, Wrapper};

pub struct Dispatcher<T: 'static>(Wrapper<T, Arc<Sender<Closure<T>>>, Closure<T>>);

struct Closure<T>(Box<dyn FnOnce(&T) + Send>);

impl<T> Clone for Dispatcher<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Debug for Dispatcher<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Dispatcher").finish_non_exhaustive()
    }
}

impl<T> Eq for Dispatcher<T> {}

impl<T> Hash for Dispatcher<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<T> Ord for Dispatcher<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl<T> PartialEq for Dispatcher<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl<T> PartialOrd for Dispatcher<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Dispatcher<T> {
    pub fn new(main_thread: MainThreadMarker, value: T) -> (Self, DispatchRunner<T>) {
        let (sender, receiver) = channel::<Closure<T>>();
        let sender = Arc::new(sender);
        let receiver = Rc::new(receiver);

        let wrapper = Wrapper::new(
            main_thread,
            value,
            |value, Closure(closure)| {
                // SAFETY: The given `Closure` here isn't really `'static`, so we shouldn't do
                // anything funny with it here. See `Self::queue()`.
                closure(value.borrow().as_ref().unwrap())
            },
            {
                let receiver = Rc::clone(&receiver);
                move |value| async move {
                    while let Ok(Closure(closure)) = receiver.next().await {
                        // SAFETY: The given `Closure` here isn't really `'static`, so we shouldn't
                        // do anything funny with it here. See `Self::queue()`.
                        closure(value.borrow().as_ref().unwrap())
                    }
                }
            },
            sender,
            |sender, closure| {
                // SAFETY: The given `Closure` here isn't really `'static`, so we shouldn't do
                // anything funny with it here. See `Self::queue()`.
                sender.send(closure).unwrap()
            },
        );
        (Self(wrapper.clone()), DispatchRunner { wrapper, receiver })
    }

    pub fn value(&self, main_thread: MainThreadMarker) -> Ref<'_, T> {
        self.0.value(main_thread)
    }

    pub fn dispatch(&self, f: impl 'static + FnOnce(&T) + Send) {
        if let Some(main_thread) = MainThreadMarker::new() {
            f(&self.0.value(main_thread))
        } else {
            self.0.send(Closure(Box::new(f)))
        }
    }

    pub fn queue<R: Send>(&self, f: impl FnOnce(&T) -> R + Send) -> R {
        if let Some(main_thread) = MainThreadMarker::new() {
            f(&self.0.value(main_thread))
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
            let closure = Closure(unsafe {
                std::mem::transmute::<
                    Box<dyn FnOnce(&T) + Send>,
                    Box<dyn FnOnce(&T) + Send + 'static>,
                >(closure)
            });

            self.0.send(closure);

            let mut started = pair.0.lock().unwrap();

            while started.is_none() {
                started = pair.1.wait(started).unwrap();
            }

            started.take().unwrap()
        }
    }
}

pub struct DispatchRunner<T: 'static> {
    wrapper: Wrapper<T, Arc<Sender<Closure<T>>>, Closure<T>>,
    receiver: Rc<Receiver<Closure<T>>>,
}

impl<T> DispatchRunner<T> {
    pub fn run(&self, main_thread: MainThreadMarker) {
        while let Some(Closure(closure)) =
            self.receiver.try_recv().expect("should only be closed when `Dispatcher` is dropped")
        {
            // SAFETY: The given `Closure` here isn't really `'static`, so we shouldn't do anything
            // funny with it here. See `Self::queue()`.
            closure(&self.wrapper.value(main_thread))
        }
    }
}
