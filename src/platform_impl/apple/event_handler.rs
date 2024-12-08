use std::cell::Cell;
use std::{fmt, mem};

use crate::application::ApplicationHandler;
use crate::event_loop::ActiveEventLoop;

/// A helper type for storing a reference to `ApplicationHandler`, allowing interior mutable access
/// to it within the execution of a closure.
#[derive(Default)]
pub(crate) struct EventHandler {
    state: Cell<State>,
}

type InitClosure<'handler> =
    Box<dyn FnOnce(&dyn ActiveEventLoop) -> Box<dyn ApplicationHandler + 'handler> + 'handler>;

#[derive(Default)]
enum State {
    /// Not registered by the event loop.
    #[default]
    NotRegistered,
    /// The event is registered by the event loop.
    Registered(InitClosure<'static>),
    /// The application has been initialized, and we're ready to handle events.
    Ready(Box<dyn ApplicationHandler + 'static>),
    /// Currently executing the handler.
    CurrentlyExecuting,
    /// The application has been terminated.
    Terminated,
    // TODO: Invalid state?
}

impl fmt::Debug for EventHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = self.state.replace(State::CurrentlyExecuting);
        // NOTE: We're very careful not to panic inside the "critial" section here.
        let string = match &state {
            State::NotRegistered => "<not registered>",
            State::Registered(_) => "<registered>",
            State::Ready(_) => "<ready>",
            State::CurrentlyExecuting => "<currently executing>",
            State::Terminated => "<terminated>",
        };
        self.state.set(state);

        f.debug_struct("EventHandler").field("state", &string).finish_non_exhaustive()
    }
}

impl EventHandler {
    pub(crate) const fn new() -> Self {
        Self { state: Cell::new(State::NotRegistered) }
    }

    /// Set the event loop handler for the duration of the given closure.
    ///
    /// This is similar to using the `scoped-tls` or `scoped-tls-hkt` crates
    /// to store the handler in a thread local, such that it can be accessed
    /// from within the closure.
    pub(crate) fn set<'handler, R>(
        &self,
        init_closure: InitClosure<'handler>,
        closure: impl FnOnce() -> R,
    ) -> R {
        // SAFETY: We extend the lifetime of the handler here so that we can
        // store it in `EventHandler`'s `RefCell`.
        //
        // This is sound, since we make sure to unset the handler again at the
        // end of this function, and as such the lifetime isn't actually
        // extended beyond `'handler`.
        let handler = unsafe { mem::transmute::<InitClosure<'handler>, InitClosure<'static>>(app) };

        match self.state.try_borrow_mut().as_deref_mut() {
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
                match self.0.state.try_borrow_mut().as_deref_mut() {
                    Ok(data @ Some(_)) => {
                        *data = None;
                    },
                    Ok(None) => {
                        tracing::error!("tried to clear handler, but no handler was set");
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

    fn init(&self) {}

    fn terminate(&self) {}

    #[cfg(target_os = "macos")]
    pub(crate) fn in_use(&self) -> bool {
        self.inner.try_borrow().is_err()
    }

    pub(crate) fn ready(&self) -> bool {
        matches!(self.inner.try_borrow().as_deref(), Ok(Some(_)))
    }

    pub(crate) fn handle(&self, callback: impl FnOnce(&mut dyn ApplicationHandler)) {
        match self.inner.try_borrow_mut().as_deref_mut() {
            Ok(Some(user_app)) => {
                // It is important that we keep the reference borrowed here,
                // so that `in_use` can properly detect that the handler is
                // still in use.
                //
                // If the handler unwinds, the `RefMut` will ensure that the
                // handler is no longer borrowed.
                callback(user_app);
            },
            Ok(None) => {
                // `NSApplication`, our app state and this handler are all
                // global state and so it's not impossible that we could get
                // an event after the application has exited the `EventLoop`.
                tracing::error!("tried to run event handler, but no handler was set");
            },
            Err(_) => {
                // Prevent re-entrancy.
                panic!("tried to handle event while another event is currently being handled");
            },
        }
    }
}
