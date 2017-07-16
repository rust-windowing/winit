use cocoa;
use cocoa::appkit::{NSApp,NSApplication};
use context;
use std::mem;

// Size of the coroutine's stack
const STACK_SIZE: usize = 512 * 1024;

// The `SendEvent` struct encapsulates the idea of calling [NSApp sendEvent:event].
// This is a separate struct because, in the case of resize events, dispatching an event can enter
// an internal runloop, and we don't want to get stuck there.
pub struct SendEvent {
    stack: context::stack::ProtectedFixedSizeStack,
    ctx: context::Context
}

// An instance of this struct is passed from `SendEvent::new()` to `send_event_fn()`.
// Any data that needs to flow that direction should be included here.
struct SendEventInvocation {
    event: cocoa::base::id,
}

impl SendEventInvocation {
    // `run()` is called from the SendEvent coroutine.
    //
    // It should resume t.context with 1 when there is more work to do, or 0 if it is complete.
    fn run(self, t: context::Transfer) {
        // boring
        unsafe {
            NSApp().sendEvent_(self.event);
        }

        // signal completion
        unsafe { t.context.resume(0); }
    }
}

impl SendEvent {
    pub fn new(event: cocoa::base::id) -> SendEvent {
        // Set up the invocation struct
        let invocation = SendEventInvocation {
            event: event,
        };

        // Pack the invocation into an Option<> of itself
        let mut invocation: Option<SendEventInvocation> = Some(invocation);

        // Make a callback to run from inside the coroutine
        extern fn send_event_fn(mut t: context::Transfer) -> ! {
            // t.data is a pointer to the caller's `invocation` Option
            let invocation: *mut Option<SendEventInvocation> = t.data as _;

            // Turn this into a mutable borrow, then move the invocation into the coroutine's stack
            let invocation: SendEventInvocation =
                unsafe { mem::transmute::<*mut Option<_>, &mut Option<_>>(invocation) }
                    .take()
                    .unwrap();


            // Yield back to `SendEvent::new()`
            t = unsafe { t.context.resume(1) };

            // Run the SendEvent process
            invocation.run(t);

            unreachable!();
        }

        // Set up a stack
        let stack = context::stack::ProtectedFixedSizeStack::new(STACK_SIZE)
            .expect("SendEvent stack allocation");

        // Set up a new context
        let result = unsafe {
            // Start by calling send_event_fn above
            let ctx = context::Context::new(&stack, send_event_fn);

            // Yield to the coroutine, giving it a pointer to the invocation, and wait for it come back
            ctx.resume(&mut invocation as *mut Option<SendEventInvocation> as usize)
        };

        SendEvent{
            stack: stack,
            ctx: result.context,
        }
    }

    // Attempt to work the send, which either a) consumes the SendEvent, indicating completion, or
    // b) returns a SendEvent, indicating there is more work yet to perform.
    pub fn work(self) -> Option<SendEvent> {
        // resume the coroutine
        let result = unsafe { self.ctx.resume(0) };

        if result.data == 0 {
            // done
            None
        } else {
            // more work to do
            Some(SendEvent{
                stack: self.stack,
                ctx: result.context,
            })
        }
    }
}
