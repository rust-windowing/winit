use std::future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::Poll;

use super::super::main_thread::MainThreadMarker;
use crate::event_loop::EventLoopProxyProvider;
use crate::platform_impl::web::event_loop::runner::WeakShared;
use crate::platform_impl::web::r#async::{AtomicWaker, Wrapper};

pub struct EventLoopProxy(Wrapper<Handler, Sender, ()>);

struct Handler {
    execution: WeakShared,
    handler: fn(&WeakShared, bool),
}

#[derive(Clone)]
struct Sender(Arc<Inner>);

impl EventLoopProxy {
    pub fn new(
        main_thread: MainThreadMarker,
        execution: WeakShared,
        handler: fn(&WeakShared, bool),
    ) -> Self {
        let inner = Arc::new(Inner {
            awoken: AtomicBool::new(false),
            waker: AtomicWaker::new(),
            closed: AtomicBool::new(false),
        });

        let handler = Handler { execution, handler };

        let sender = Sender(Arc::clone(&inner));

        Self(Wrapper::new(
            main_thread,
            handler,
            |handler, _| {
                let handler = handler.borrow();
                let handler = handler.as_ref().unwrap();
                (handler.handler)(&handler.execution, true);
            },
            {
                let inner = Arc::clone(&inner);

                move |handler| async move {
                    while future::poll_fn(|cx| {
                        if inner.awoken.swap(false, Ordering::Relaxed) {
                            Poll::Ready(true)
                        } else {
                            inner.waker.register(cx.waker());

                            if inner.awoken.swap(false, Ordering::Relaxed) {
                                Poll::Ready(true)
                            } else {
                                if inner.closed.load(Ordering::Relaxed) {
                                    return Poll::Ready(false);
                                }

                                Poll::Pending
                            }
                        }
                    })
                    .await
                    {
                        let handler = handler.borrow();
                        let handler = handler.as_ref().unwrap();
                        (handler.handler)(&handler.execution, false);
                    }
                }
            },
            sender,
            |inner, _| {
                inner.0.awoken.store(true, Ordering::Relaxed);
                inner.0.waker.wake();
            },
        ))
    }

    pub fn take(&self) -> bool {
        debug_assert!(
            MainThreadMarker::new().is_some(),
            "this should only be called from the main thread"
        );

        self.0.with_sender_data(|inner| inner.0.awoken.swap(false, Ordering::Relaxed))
    }
}

impl Drop for EventLoopProxy {
    fn drop(&mut self) {
        self.0.with_sender_data(|inner| {
            inner.0.closed.store(true, Ordering::Relaxed);
            inner.0.waker.wake();
        });
    }
}

impl EventLoopProxyProvider for EventLoopProxy {
    fn wake_up(&self) {
        self.0.send(())
    }
}

struct Inner {
    awoken: AtomicBool,
    waker: AtomicWaker,
    closed: AtomicBool,
}
