use std::future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::Poll;

use super::super::main_thread::MainThreadMarker;
use crate::event_loop::EventLoopProxyProvider;
use crate::platform_impl::web::event_loop::runner::WeakShared;
use crate::platform_impl::web::r#async::{AtomicWaker, Wrapper};

pub struct EventLoopProxy(Wrapper<WeakShared, Arc<State>, ()>);

struct State {
    awoken: AtomicBool,
    waker: AtomicWaker,
    closed: AtomicBool,
}

impl EventLoopProxy {
    pub fn new(main_thread: MainThreadMarker, runner: WeakShared) -> Self {
        let state = Arc::new(State {
            awoken: AtomicBool::new(false),
            waker: AtomicWaker::new(),
            closed: AtomicBool::new(false),
        });

        Self(Wrapper::new(
            main_thread,
            runner,
            |runner, _| {
                let runner = runner.borrow();
                let runner = runner.as_ref().unwrap();

                if let Some(runner) = runner.upgrade() {
                    runner.send_proxy_wake_up(true);
                }
            },
            {
                let state = Arc::clone(&state);

                move |runner| async move {
                    while future::poll_fn(|cx| {
                        if state.awoken.swap(false, Ordering::Relaxed) {
                            Poll::Ready(true)
                        } else {
                            state.waker.register(cx.waker());

                            if state.awoken.swap(false, Ordering::Relaxed) {
                                Poll::Ready(true)
                            } else {
                                if state.closed.load(Ordering::Relaxed) {
                                    return Poll::Ready(false);
                                }

                                Poll::Pending
                            }
                        }
                    })
                    .await
                    {
                        let runner = runner.borrow();
                        let runner = runner.as_ref().unwrap();

                        if let Some(runner) = runner.upgrade() {
                            runner.send_proxy_wake_up(false);
                        }
                    }
                }
            },
            state,
            |state, _| {
                state.awoken.store(true, Ordering::Relaxed);
                state.waker.wake();
            },
        ))
    }

    pub fn take(&self) -> bool {
        debug_assert!(
            MainThreadMarker::new().is_some(),
            "this should only be called from the main thread"
        );

        self.0.with_sender_data(|state| state.awoken.swap(false, Ordering::Relaxed))
    }
}

impl Drop for EventLoopProxy {
    fn drop(&mut self) {
        self.0.with_sender_data(|state| {
            state.closed.store(true, Ordering::Relaxed);
            state.waker.wake();
        });
    }
}

impl EventLoopProxyProvider for EventLoopProxy {
    fn wake_up(&self) {
        self.0.send(())
    }
}
