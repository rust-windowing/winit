use cocoa;
use cocoa::appkit::{NSApp,NSApplication};
use core_foundation::base::*;
use core_foundation::runloop::*;
use context;
use std::mem;
use std::cell::{Cell,RefCell};
use std::rc::Rc;
use libc::c_void;

// Size of the coroutine's stack
const STACK_SIZE: usize = 512 * 1024;

// The `SendEvent` struct encapsulates the idea of calling [NSApp sendEvent:event].
// This is a separate struct because, in the case of resize events, dispatching an event can enter
// an internal runloop, and we don't want to get stuck there.
pub struct SendEvent {
    stack: context::stack::ProtectedFixedSizeStack,
    ctx: context::Context
}

struct Resumable(Cell<Option<context::Context>>);

impl Resumable {
    fn new(ctx: context::Context) -> Self {
        Resumable(Cell::new(Some(ctx)))
    }

    unsafe fn resume(&self, value: usize) {
        // fish out the current context
        let mut context = self.0.take().expect("Resumable should always have a context");

        // resume it, getting a new context
        let result = context.resume(value);

        // store the new context and return
        self.0.set(Some(result.context));
    }
}

// A RunLoopObserver corresponds to a CFRunLoopObserver.
struct RunLoopObserver {
    id: CFRunLoopObserverRef,
}

extern "C" fn runloop_observer_callback(observer: CFRunLoopObserverRef, activity: CFRunLoopActivity, info: *mut c_void) {
    // convert the raw pointer into an Rc
    let mut resumable: Rc<Resumable> = unsafe { Rc::from_raw(info as _) };

    // we're either about to wait or just finished waiting
    // in either case, yield back to the caller, signaling the operation is still in progress
    unsafe {
        resumable.resume(1);
    }

    // convert the Rc back into a raw pointer to retain the refcount
    Rc::into_raw(resumable);
}

extern "C" fn retain_resumable(info: *const c_void) {
    // convert the raw pointer into an Rc
    let mut resumable: Rc<Resumable> = unsafe { Rc::from_raw(info as _) };

    // clone it and conver to a raw pointer to increment the refcount
    Rc::into_raw(resumable.clone());

    // convert the Rc back into a raw pointer to retain the refcount
    Rc::into_raw(resumable);
}

extern "C" fn release_resumable(info: *const c_void) {
    // convert the raw pointer into an Rc
    let mut resumable: Rc<Resumable> = unsafe { Rc::from_raw(info as _) };

    // let it drop to decrement the refcount
}

impl RunLoopObserver {
    fn new(resumable: Rc<Resumable>) -> RunLoopObserver {
        // CFRunLoopObserverCreate copies this struct, so we can give it a pointer to this local
        let mut context: CFRunLoopObserverContext = unsafe { mem::zeroed() };
        context.info = Rc::into_raw(resumable) as _;
        context.release = release_resumable;

        // Make the runloop observer itself
        let id = unsafe {
            CFRunLoopObserverCreate(
                kCFAllocatorDefault,
                kCFRunLoopBeforeWaiting | kCFRunLoopAfterWaiting,
                1,      // repeats
                0,      // order
                runloop_observer_callback,
                &mut context as *mut CFRunLoopObserverContext,
            )
        };

        // Add to event loop
        unsafe {
            CFRunLoopAddObserver(CFRunLoopGetMain(), id, kCFRunLoopCommonModes);
        }

        // Decrement the refcount now that the platform owns it
        release_resumable(context.info);

        RunLoopObserver{
            id,
        }
    }
}

impl Drop for RunLoopObserver {
    fn drop(&mut self) {
        unsafe {
            CFRunLoopRemoveObserver(CFRunLoopGetMain(), self.id, kCFRunLoopCommonModes);
            CFRelease(self.id as _);
        }
    }
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
    fn run(self, t: context::Transfer) -> ! {
        // save our current context
        let resumable: Rc<Resumable> = Rc::new(Resumable::new(t.context));

        {
            // make a runloop observer for its side effects
            let _observer = RunLoopObserver::new(resumable.clone());

            // send the message
            unsafe {
                NSApp().sendEvent_(self.event);
            }

            // drop the runloop observer
        }

        // signal completion
        unsafe { resumable.resume(0); }

        // we should never be resumed after completion
        unreachable!();
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
