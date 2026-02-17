use std::cell::RefCell;
use std::{fmt, mem};

use winit_core::application::ApplicationHandler;

/// A helper type for storing a reference to `ApplicationHandler`, allowing interior mutable access
/// to it within the execution of a closure.
#[derive(Default)]
pub struct EventHandler {
    /// This can be in the following states:
    /// - Not registered by the event loop, or terminated (None).
    /// - Present (Some(handler)).
    /// - Currently executing the handler / in use (RefCell borrowed).
    inner: RefCell<Option<Box<dyn ApplicationHandler + 'static>>>,
}

impl fmt::Debug for EventHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = match self.inner.try_borrow().as_deref() {
            Ok(Some(_)) => "<available>",
            Ok(None) => "<not set>",
            Err(_) => "<in use>",
        };
        f.debug_struct("EventHandler").field("state", &state).finish_non_exhaustive()
    }
}

impl EventHandler {
    pub fn new() -> Self {
        Self { inner: RefCell::new(None) }
    }

    /// Set the event loop handler for the duration of the given closure.
    ///
    /// This is similar to using the `scoped-tls` or `scoped-tls-hkt` crates
    /// to store the handler in a thread local, such that it can be accessed
    /// from within the closure.
    pub fn set<'handler, R>(
        &self,
        app: Box<dyn ApplicationHandler + 'handler>,
        closure: impl FnOnce() -> R,
    ) -> R {
        // SAFETY: We extend the lifetime of the handler here so that we can
        // store it in `EventHandler`'s `RefCell`.
        //
        // This is sound, since we make sure to unset the handler again at the
        // end of this function, and as such the lifetime isn't actually
        // extended beyond `'handler`.
        let handler = unsafe {
            mem::transmute::<
                Box<dyn ApplicationHandler + 'handler>,
                Box<dyn ApplicationHandler + 'static>,
            >(app)
        };

        match self.inner.try_borrow_mut().as_deref_mut() {
            Ok(Some(_)) => {
                unreachable!("tried to set handler while another was already set");
            },
            Ok(data @ None) => {
                *data = Some(handler);
            },
            Err(_) => {
                unreachable!("tried to set handler that is currently in use");
            },
        }

        struct ClearOnDrop<'a>(&'a EventHandler);

        impl Drop for ClearOnDrop<'_> {
            fn drop(&mut self) {
                match self.0.inner.try_borrow_mut().as_deref_mut() {
                    Ok(data @ Some(_)) => {
                        let handler = data.take();
                        // Explicitly `Drop` the application handler.
                        drop(handler);
                    },
                    Ok(None) => {
                        // Allowed, happens if the handler was cleared manually
                        // elsewhere (such as in `applicationWillTerminate:`).
                    },
                    Err(_) => {
                        // Note: This is not expected to ever happen, this
                        // module generally controls the `RefCell`, and
                        // prevents it from ever being borrowed outside of it.
                        //
                        // But if it _does_ happen, it is a serious error, and
                        // we must abort the process, it'd be unsound if we
                        // weren't able to unset the handler.
                        eprintln!("tried to clear handler that is currently in use");
                        std::process::abort();
                    },
                }
            }
        }

        let _clear_on_drop = ClearOnDrop(self);

        // Note: The RefCell should not be borrowed while executing the
        // closure, that'd defeat the whole point.
        closure()

        // `_clear_on_drop` will be dropped here, or when unwinding, ensuring
        // soundness.
    }

    pub fn in_use(&self) -> bool {
        self.inner.try_borrow().is_err()
    }

    pub fn ready(&self) -> bool {
        matches!(self.inner.try_borrow().as_deref(), Ok(Some(_)))
    }

    /// Try to call the handler and return a value.
    ///
    /// Returns `None` if the handler is not set or is currently in use (re-entrant call).
    ///
    /// It is important that we keep the `RefMut` borrowed during the callback, so that `in_use`
    /// can properly detect that the handler is still in use. If the handler unwinds, the `RefMut`
    /// will ensure that the handler is no longer borrowed.
    pub fn handle_with_result<R>(
        &self,
        callback: impl FnOnce(&mut (dyn ApplicationHandler + '_)) -> R,
    ) -> Option<R> {
        match self.inner.try_borrow_mut().as_deref_mut() {
            Ok(Some(user_app)) => Some(callback(&mut **user_app)),
            Ok(None) => {
                // `NSApplication`, our app state and this handler are all global state and so
                // it's not impossible that we could get an event after the application has
                // exited the `EventLoop`.
                tracing::error!("tried to run event handler, but no handler was set");
                None
            },
            Err(_) => {
                // Handler is currently in use, return None instead of panicking.
                None
            },
        }
    }

    pub fn handle(&self, callback: impl FnOnce(&mut (dyn ApplicationHandler + '_))) {
        match self.handle_with_result(callback) {
            Some(()) => {},
            // Handler not set — already logged by handle_with_result.
            None if !self.in_use() => {},
            None => {
                // Prevent re-entrancy.
                panic!("tried to handle event while another event is currently being handled");
            },
        }
    }

    pub fn terminate(&self) {
        match self.inner.try_borrow_mut().as_deref_mut() {
            Ok(data @ Some(_)) => {
                let handler = data.take();
                // Explicitly `Drop` the application handler.
                drop(handler);
            },
            Ok(None) => {
                // When terminating, we expect the application handler to still be registered.
                tracing::error!("tried to clear handler, but no handler was set");
            },
            Err(_) => {
                panic!("tried to clear handler while an event is currently being handled");
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use winit_core::application::ApplicationHandler;
    use winit_core::event::WindowEvent;
    use winit_core::event_loop::ActiveEventLoop;
    use winit_core::window::WindowId;

    use super::EventHandler;

    struct DummyApp;

    impl ApplicationHandler for DummyApp {
        fn can_create_surfaces(&mut self, _event_loop: &dyn ActiveEventLoop) {}

        fn window_event(
            &mut self,
            _event_loop: &dyn ActiveEventLoop,
            _window_id: WindowId,
            _event: WindowEvent,
        ) {
        }
    }

    #[test]
    fn handle_with_result_returns_value() {
        let handler = EventHandler::new();
        handler.set(Box::new(DummyApp), || {
            let result = handler.handle_with_result(|_app| 42);
            assert_eq!(result, Some(42));
        });
    }

    #[test]
    fn handle_with_result_returns_none_when_not_set() {
        let handler = EventHandler::new();
        let result = handler.handle_with_result(|_app| 42);
        assert_eq!(result, None);
    }

    #[test]
    fn handle_with_result_returns_none_when_in_use() {
        let handler = EventHandler::new();
        handler.set(Box::new(DummyApp), || {
            // Borrow the handler via `handle`, then try `handle_with_result`
            // from within — simulating re-entrancy.
            handler.handle(|_app| {
                let result = handler.handle_with_result(|_app| 42);
                assert_eq!(result, None);
            });
        });
    }

    #[test]
    fn handle_with_result_returns_none_when_reentrant_through_self() {
        let handler = EventHandler::new();
        handler.set(Box::new(DummyApp), || {
            let result = handler.handle_with_result(|_app| {
                // Re-entrant call through handle_with_result itself.
                handler.handle_with_result(|_app| 42)
            });
            assert_eq!(result, Some(None));
        });
    }

    #[test]
    #[should_panic(
        expected = "tried to handle event while another event is currently being handled"
    )]
    fn handle_panics_on_reentrant_call() {
        let handler = EventHandler::new();
        handler.set(Box::new(DummyApp), || {
            handler.handle(|_app| {
                // Re-entrant handle must still panic after the refactoring.
                handler.handle(|_app| {});
            });
        });
    }
}
