use futures::{ Async, Poll, Stream };
use std::cell::RefCell;
use std::rc::Weak;

pub struct WhileExists<S: Stream>(pub Weak<RefCell<S>>);

impl<S: Stream> Stream for WhileExists<S> {
    type Item = S::Item;
    type Error = S::Error;

    fn poll(&mut self) -> Poll<Option<S::Item>, S::Error> {
        self.0.upgrade()
            .map(|s| s.borrow_mut().poll())
            .unwrap_or(Ok(Async::Ready(None)))
    }
}
