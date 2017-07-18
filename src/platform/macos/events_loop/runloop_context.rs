use std::cell::Cell;
use std::mem;
use std::sync::Weak;
use context;
use core_foundation;
use cocoa::{self, foundation};
use cocoa::appkit::{self, NSApplication, NSApp};

use super::{Shared,Timeout};
use super::nsevent;
use events::Event;

const STACK_SIZE: usize = 512 * 1024;

// The Runloop is responsible for:
//   - receiving NSEvents from Cocoa
//   - forwarding NSEvents back to Cocoa
//   - posting Events to the queue
pub struct Runloop {
    stack: context::stack::ProtectedFixedSizeStack,
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

        // Make a callback to run from inside the coroutine which delegates to the
        extern fn inner_runloop_entrypoint(t: context::Transfer) -> ! {
            // t.data is a pointer to the constructor's `inner` variable
            let inner: *mut Option<InnerRunloop> = t.data as _;

            // Turn this into a mutable borrow, then move the inner runloop into the coroutine's stack
            let mut inner: InnerRunloop =
                unsafe { mem::transmute::<*mut Option<_>, &mut Option<_>>(inner) }
                    .take()
                    .unwrap();

            // Store the caller's context
            inner.caller = Some(t.context);

            // Yield back to `Runloop::new()` so it can return
            inner.yield_to_caller();

            // Run the inner runloop
            inner.run_coroutine();
        }

        // Set up a new context
        let result = unsafe {
            // Start by calling inner_runloop_entrypoint
            let ctx = context::Context::new(&stack, inner_runloop_entrypoint);

            // Yield to the coroutine, giving it a pointer to the inner runloop, and wait for it come back
            ctx.resume(&mut inner as *mut Option<InnerRunloop> as usize)
        };

        Runloop{
            stack,
            ctx: Some(result.context)
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

        assert_eq!(result.data, 1, "expected coroutine runloop to be active");

        // Return to caller
    }

    // Attempt to wake the Runloop. Must be thread safe.
    pub fn wake() {
        // Try to context switch back to the main thread
        if yield_to_caller() {
            // We did!
        } else {
            unsafe {
                core_foundation::runloop::CFRunLoopWakeUp(core_foundation::runloop::CFRunLoopGetMain());
            }
        }
    }
}

thread_local!{
    // A pointer to the InnerRunloop, if we are presently inside the InnerRunloop coroutine
    static INSIDE_INNER_RUNLOOP: Cell<Option<*mut InnerRunloop>> = Cell::new(None);
}

// If we're inside the InnerRunloop, call InnerRunloop::yield_to_caller() and return true;
// if we're outside, do nothing and return false
fn yield_to_caller() -> bool {
    INSIDE_INNER_RUNLOOP.with(|runloop| {
        if let Some(runloop) = runloop.get() {
            let runloop: &mut InnerRunloop = unsafe { mem::transmute(runloop) };
            runloop.yield_to_caller();
            true
        } else {
            false
        }
    })
}

pub struct InnerRunloop {
    shared: Weak<Shared>,
    event_state: nsevent::PersistentState,
    timeout: Timeout,
    shutdown: bool, // should the runloop shut down?
    caller: Option<context::Context>,
}

impl InnerRunloop {
    fn new(shared: Weak<Shared>) -> InnerRunloop {
        InnerRunloop{
            shared,
            event_state: nsevent::PersistentState::new(),
            timeout: Timeout::Now,
            shutdown: false,
            caller: None,
        }
    }

    fn yield_to_caller(&mut self) {
        if let Some(ctx) = self.caller.take() {
            // clear INSIDE_INNER_RUNLOOP, since we're leaving
            INSIDE_INNER_RUNLOOP.with(|runloop| {
                runloop.set(None);
            });

            // yield
            let t = unsafe { ctx.resume(1) };

            // t.context is the caller's context
            self.caller = Some(t.context);

            // t.data is a pointer to an Option<Timeout>
            // take it
            let timeout = t.data as *mut Option<Timeout>;
            let timeout =
                unsafe { mem::transmute::<*mut Option<_>, &mut Option<_>>(timeout) }
                    .take()
                    .unwrap();

            // store the new timeout
            self.timeout = timeout;

            // set INSIDE_INNER_RUNLOOP, since we're entering
            INSIDE_INNER_RUNLOOP.with(|runloop| {
                runloop.set(Some(self as *mut InnerRunloop));
            });
        }
    }

    fn run_coroutine(mut self) -> ! {
        // run the normal process
        self.run();

        // extract the context
        let mut ctx = self.caller.take().expect("run_coroutine() context");

        // drop the rest
        drop(self);

        // keep yielding until they give up
        loop {
            let t = unsafe { ctx.resume(0) };
            println!("coroutine runloop is terminated but is still getting called");
            ctx = t.context;
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
            let event = match self.receive_event_from_cocoa() {
                None => {
                    // Our timeout expired
                    // Yield
                    self.yield_to_caller();

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
                self.forward_event_to_cocoa(&event);
            }

            // Can we turn it into one or more events?
            let events = nsevent::to_events(&event, &mut self.event_state, shared.as_ref());

            // Post them
            for event in events {
                shared.enqueue_event(event);
            }
        }
    }

    fn receive_event_from_cocoa(&mut self) -> Option<nsevent::RetainedEvent> {
        unsafe {
            let pool = foundation::NSAutoreleasePool::new(cocoa::base::nil);

            // Pick a timeout
            let timeout = match self.timeout {
                Timeout::Now => foundation::NSDate::distantPast(cocoa::base::nil),
                Timeout::Forever => foundation::NSDate::distantFuture(cocoa::base::nil),
            };

            // Poll for the next event
            let ns_event = appkit::NSApp().nextEventMatchingMask_untilDate_inMode_dequeue_(
                appkit::NSAnyEventMask.bits() | appkit::NSEventMaskPressure.bits(),
                timeout,
                foundation::NSDefaultRunLoopMode,
                cocoa::base::YES);

            // Wrap the event, if any, in a RetainedEvent
            let event = if ns_event == cocoa::base::nil {
                None
            } else {
                Some(nsevent::RetainedEvent::new(ns_event))
            };

            let _: () = msg_send![pool, release];

            return event
        }
    }

    fn forward_event_to_cocoa(&mut self, event: &nsevent::RetainedEvent) {
        unsafe {
            NSApp().sendEvent_(event.id());
        }
    }
}