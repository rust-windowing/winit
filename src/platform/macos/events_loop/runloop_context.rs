//! This is the coroutine-based `Runloop` implementation. See `runloop.rs` for the simple blocking
//! `Runloop` implementation.
//!
//! ## Structure
//!
//! The basic `Runloop` does everything in `Runloop`. The `context`-enabled version moves those
//! functions to `InnerRunloop`, and it adds a `Runloop` with the same public interface whose
//! purpose is to start an `InnerRunloop` coroutine and context switch into it as needed.
//!
//! ## Entering the coroutine
//!
//! After initialization in `Runloop::new()`, `Runloop::work()` is the only place where the main
//! thread context switches into the `InnerRunloop` coroutine.
//!
//! `Runloop::work()` is called only by `EventLoop::get_event()`. Whatever invariants about the main
//! thread are true at that point remain true for the entire duration of the inner runloop. For
//! example, we know that the main thread is not holding locks inside `Shared`, so the coroutine can
//! acquire and release locks on `Shared` without deadlocking on the main thread.
//!
//! `Runloop::work()` checks the `NSThread`'s identity to ensure that the coroutine can only be
//! resumed from the main thread.
//!
//! ## Moving data into the coroutine
//!
//! The initial call into the coroutine entrypoint needs to bring an `InnerRunloop`, and all
//! subsequent calls into the coroutine bring a `Timeout`. The strategy here is to accomplish this
//! by combining three properties:
//!
//! * `context` can carry a `data: usize` along during a context switch.
//! * When execution is transferred into a coroutine, the caller stops running until execution
//!   transfers back.
//! * `Option::take()` moves a value out of the `Option`.
//!
//! Put together, this means the caller can declare a local `Option`, pass a `&mut` of it into the
//! coroutine, and the coroutine can safely `.take()` its value. Again, there's no concurrency at
//! work -- everything executes sequentially -- so we can guarantee that there's only one mutable
//! borrow to the caller's `Option`.
//!
//! Actually doing this via a `usize` uses `&mut` as `*mut` as `usize` on the way down, and then
//! `usize` as `*mut` transmute `&mut` on the way up. One could transmute straight to and from
//! `usize`, but casting all the way through preserves symmetry.
//!
//! Calls into the coroutine look like:
//!
//! ```ignore
//! // caller
//! let mut input: Option<Foo> = Some(Foo);
//! context.resume(&mut input as *mut Option<Foo> as usize);
//!
//!         // coroutine's resume() returns, holding the &mut Option
//!         let t: context::Transfer = context.resume( /* … */ );
//!         let input: *mut Option<Foo> = t.data as *mut Option<Foo>;
//!         let input: &mut Option<Foo> = mem::transmute(input);
//!         let input: Foo = input.take().unwrap();
//!         // input is now moved to the coroutine
//!         // coroutine eventually returns to the caller
//!         t.context.resume( /* … */ );
//!
//! // caller's context.resume() returns
//! // input = None, since the value was taken by the coroutine
//! ```
//!
//! ## Inside the coroutine
//!
//! `yield_to_caller()` is the place where the coroutine context switches back to `Runloop::work()`,
//! and it is therefore also the place where the coroutine resumes.
//!
//! `yield_to_caller()` sets a thread local cell containing the caller's context when execution
//! switched into the coroutine, and it moves the context out of that cell before it switches back.
//! If that cell is full, then we are currently inside the coroutine; if that cell is empty, then we
//! are not.
//!
//! `yield_to_caller()` also sets a thread local cell containing the caller's `Timeout`, which can
//! be retrieved by `fn current_timeout() -> Option<Timeout>`. The inner runloop uses this when
//! asking Cocoa to receive an event.
//!
//! The coroutine's `InnerRunloop` looks very much like the normal blocking `Runloop`. It tries to
//! receive an event from Cocoa, forwards it back to Cocoa, translates it into zero or more
//! `Event`s, and posts them to the queue.
//!
//! ## Exiting the coroutine
//!
//! `Shared::enqueue_event()` enqueues the event and then tries to wake the runloop, and the
//! coroutine version of `Runloop:::wake()` calls `yield_to_caller()`. This means that if we enqueue
//! an event from inside the coroutine -- for example, from the normal inner runloop or because a
//! Cocoa callback posted an event -- then execution immediately returns to
//! `EventLoop::get_event()`, which checks `Shared`'s event queue, finds an event, and returns it to
//! its caller.
//!
//! If `Runloop::wake()` finds that its caller is _not_ inside the coroutine -- for example, because
//! it's on a different thread calling `Proxy::wakeup()` -- it uses `libdispatch` to enqueues a
//! block on the main thread that calls `yield_to_caller()`, then uses `CFRunLoopWakeUp()` to wake
//! the thread in case it was sleeping. The system runloop will then check its dispatch queue, run
//! the block, and thus yield control of the main thread, even if we're stuck inside someone else's
//! runloop.
//!
//! Additionally, if the coroutine is invoked with `Timeout::Now`, it calls
//! `guard_against_lengthy_operations()` which enqueues a block for execution in the very near
//! future, i.e. a couple milliseconds. This puts an upper bound on how long the coroutine will run
//! after a caller has specified `Timeout::Now`.


use std::cell::Cell;
use std::mem;
use std::sync::Weak;
use std::sync::atomic::{AtomicBool,ATOMIC_BOOL_INIT,Ordering};
use std::time::{Instant,Duration};
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
    static INSIDE_INNER_RUNLOOP_ENTERED_AT: Cell<Instant> = Cell::new(Instant::now());
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

        // Store the new values in the thread local cells until we yield back
        INSIDE_INNER_RUNLOOP_CONTEXT.with(move |context_cell| {
            context_cell.set(context);
        });
        INSIDE_INNER_RUNLOOP_TIMEOUT.with(move |timeout_cell| {
            timeout_cell.set(timeout);
        });
        INSIDE_INNER_RUNLOOP_ENTERED_AT.with(move |entered_at_cell| {
            entered_at_cell.set(Instant::now());
        });

        // Does the caller want their thread back soon?
        if timeout == Some(Timeout::Now) {
            // Try to ensure we'll yield again soon, regardless of what happens inside Cocoa
            guard_against_lengthy_operations();
        }

        true
    } else {
        false
    }
}

fn guard_against_lengthy_operations() {
    // We can get called repeatedly, and we only want a single block in the runloop's execution
    // queue, so keep track of if there is currently one queued
    static HAS_BLOCK_QUEUED: AtomicBool = ATOMIC_BOOL_INIT;

    // Is there currently a block queued?
    if HAS_BLOCK_QUEUED.load(Ordering::Acquire) {
        // Do nothing
        return;
    }

    // Queue a block in two milliseconds
    dispatch::Queue::main().after_ms(2, move || {
        // Indicate that there is not currently a block queued
        HAS_BLOCK_QUEUED.store(false, Ordering::Release);

        // Are we in an invocation that's supposed to yield promptly?
        if current_timeout() == Some(Timeout::Now) {
            // Figure out when we entered the runloop as compared to now
            let runloop_entered_at = INSIDE_INNER_RUNLOOP_ENTERED_AT.with(move |entered_at_cell| {
                entered_at_cell.get()
            });
            let duration_since_runloop_entry = runloop_entered_at.elapsed();

            // Did we enter more than one millisecond ago?
            if duration_since_runloop_entry > Duration::from_millis(1) {
                // Return, even if this is a bit early
                yield_to_caller();
            } else {
                // We haven't been in the runloop very long
                // Instead of returning, queue another block for the near future
                guard_against_lengthy_operations();
            }
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
