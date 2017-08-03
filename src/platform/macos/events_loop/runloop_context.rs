use std::cell::Cell;
use std::mem;
use std::sync::Weak;
use std::sync::atomic::{AtomicUsize,ATOMIC_USIZE_INIT,Ordering};
use context;
use core_foundation;
use cocoa;
use dispatch;

use super::{Shared,Timeout};
use super::nsevent;

const STACK_SIZE: usize = 512 * 1024;

// The Runloop is responsible for:
//   - receiving NSEvents from Cocoa
//   - forwarding NSEvents back to Cocoa
//   - posting Events to the queue
pub struct Runloop {
    _stack: context::stack::ProtectedFixedSizeStack,
    ctx: Option<context::Context>,
}

impl Runloop {
    // Create a runloop
    pub fn new(shared: Weak<Shared>) -> Runloop {
        // Create an inner runloop
        let mut inner: Option<InnerRunloop> = Some(InnerRunloop::new(shared));

        // Create a stack for it
        let stack = context::stack::ProtectedFixedSizeStack::new(STACK_SIZE)
            .expect("Runloop coroutine stack allocation");

        // Set up a new context
        let result = unsafe {
            // Start by calling inner_runloop_entrypoint
            let ctx = context::Context::new(&stack, inner_runloop_entrypoint);

            // Yield to the coroutine, giving it a pointer to the inner runloop, and wait for it come back
            ctx.resume(&mut inner as *mut Option<InnerRunloop> as usize)
        };

        Runloop{
            _stack: stack,
            ctx: Some(result.context),
        }
    }

    // Work the runloop, attempting to respect the timeout
    pub fn work(&mut self, timeout: Timeout) {
        unsafe {
            if !msg_send![cocoa::base::class("NSThread"), isMainThread] {
                panic!("Events can only be polled from the main thread on macOS");
            }
        }

        // Make an Option<Timeout> that contains the timeout
        // The coroutine will .take() it as soon as it returns (see InnerRunloop::yield_to_caller())
        let mut timeout: Option<Timeout> = Some(timeout);

        // Resume the coroutine, giving it a pointer to our local timeout
        let context = self.ctx.take().expect("coroutine context");
        let result = unsafe {
            context.resume(&mut timeout as *mut Option<_> as usize)
        };

        // Store the new coroutine context
        self.ctx = Some(result.context);

        // Return to caller
    }

    // Attempt to wake the Runloop. Must be thread safe.
    pub fn wake() {
        // Try to context switch back to the main thread
        if yield_to_caller() {
            // We did!
        } else {
            // Queue a block that will yield back to the caller
            dispatch::Queue::main().async(|| {
                yield_to_caller();
            });

            // Wake the runloop so it notices the new block
            unsafe {
                core_foundation::runloop::CFRunLoopWakeUp(core_foundation::runloop::CFRunLoopGetMain());
            }
        }
    }
}

thread_local!{
    // If we are inside the inner runloop, this contains the caller's context and their Timeout
    static INSIDE_INNER_RUNLOOP_CONTEXT: Cell<Option<context::Context>> = Cell::new(None);
    static INSIDE_INNER_RUNLOOP_TIMEOUT: Cell<Option<Timeout>> = Cell::new(None);
}

// This is the first function called from inside the coroutine. It must not return.
// Contract: t.data is a *mut Option<InnerRunloop>.
extern fn inner_runloop_entrypoint(t: context::Transfer) -> ! {
    let inner: *mut Option<InnerRunloop> = t.data as _;

    // Turn this into a mutable borrow, then move the inner runloop into the coroutine's stack
    let mut inner: InnerRunloop =
        unsafe { mem::transmute::<*mut Option<_>, &mut Option<_>>(inner) }
            .take()
            .unwrap();

    // Store the caller's context in the usual place
    let context = Some(t.context);
    INSIDE_INNER_RUNLOOP_CONTEXT.with(move |ctx| { ctx.set(context) });

    // Yield back to `Runloop::new()` so it can return
    // Our next execution -- and all subsequent executions -- will happen inside `Runloop::work()`.
    yield_to_caller();

    // Run the inner runloop
    inner.run();

    // Drop it
    drop(inner);

    // Yield forever
    loop {
        yield_to_caller();
    }
}


// If we're inside the InnerRunloop, return the current Timeout.
fn current_timeout() -> Option<Timeout> {
    INSIDE_INNER_RUNLOOP_TIMEOUT.with(|timeout| {
        timeout.get()
    })
}

// If we're inside the InnerRunloop, context switch and return true;
// if we're outside, do nothing and return false
fn yield_to_caller() -> bool {
    // See if we we're inside the inner runloop
    // If we are in the inner runloop, take the context since we're leaving
    if let Some(context) = INSIDE_INNER_RUNLOOP_CONTEXT.with(|context_cell| { context_cell.take() }) {
        // Yield
        let t = unsafe { context.resume(0) };
        // We're returned

        // t.context is the caller's context
        let context = Some(t.context);
        // t.data is a pointer to an Option<Timeout>
        // take() it
        let timeout: *mut Option<Timeout> = t.data as *mut Option<_>;
        let timeout: Option<Timeout> =
            unsafe { mem::transmute::<*mut Option<_>, &mut Option<_>>(timeout) }
                .take();

        // Does the caller want their thread back soon?
        if timeout == Some(Timeout::Now) {
            // Try to ensure we'll yield again soon, regardless of what happens inside Cocoa
            guard_against_lengthy_operations();
        }

        // Store the new values in the thread local cells until we yield back
        INSIDE_INNER_RUNLOOP_CONTEXT.with(move |context_cell| {
            context_cell.set(context);
        });
        INSIDE_INNER_RUNLOOP_TIMEOUT.with(move |timeout_cell| {
            timeout_cell.set(timeout);
        });

        true
    } else {
        false
    }
}


fn guard_against_lengthy_operations() {
    // Schedule a block to run in the near future, just in case
    // We can get called repeatedly, and we only want the most recent call to matter, so keep track
    // using an atomic counter
    static INVOCATIONS: AtomicUsize = ATOMIC_USIZE_INIT;

    // Get the current value of the counter, and increment it
    let this_invocation = INVOCATIONS.fetch_add(1, Ordering::SeqCst);

    // Queue a block in two milliseconds
    dispatch::Queue::main().after_ms(2, move || {
        // Get the most recent invocation, which is one before the current value of the counter
        let current_counter = INVOCATIONS.load(Ordering::Acquire);
        let (most_recent_invocation, _) = current_counter.overflowing_sub(1);

        // Are we the most recent call?
        if most_recent_invocation == this_invocation {
            yield_to_caller();
        } else {
            // We have already yielded and returned
            // Do nothing
        }
    });
}

pub struct InnerRunloop {
    shared: Weak<Shared>,
    event_state: nsevent::PersistentState,
}

impl InnerRunloop {
    fn new(shared: Weak<Shared>) -> InnerRunloop {
        InnerRunloop{
            shared,
            event_state: nsevent::PersistentState::new(),
        }
    }

    fn run(&mut self) {
        loop {
            // upgrade the shared pointer
            let shared = match self.shared.upgrade() {
                None => return,
                Some(shared) => shared
            };

            // try to receive an event
            let event = match nsevent::receive_event_from_cocoa(current_timeout().unwrap_or(Timeout::Now)) {
                None => {
                    // Our timeout expired
                    // Yield
                    yield_to_caller();

                    // Retry
                    continue;
                },
                Some(event) => {
                    event
                }
            };

            // Is this a message type that doesn't need further processing?
            if nsevent::should_discard_event_early(&event) {
                continue;
            }

            // Is this a message type that we should forward back to Cocoa?
            if nsevent::should_forward_event(&event) {
                nsevent::forward_event_to_cocoa(&event);
            }

            // Can we turn it into one or more events?
            let events = nsevent::to_events(&event, &mut self.event_state, shared.as_ref());

            // Post them
            for event in events {
                shared.enqueue_event(event);
            }
        }
    }
}
