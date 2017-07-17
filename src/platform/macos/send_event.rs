use cocoa;
use cocoa::appkit::{NSApp,NSApplication};

pub unsafe fn try_resume(value: usize) -> bool {
    false
}

// The `SendEvent` struct encapsulates the idea of calling [NSApp sendEvent:event].
// This is a separate struct because, in the case of resize events, dispatching an event can enter
// an internal runloop, and we don't want to get stuck there.
pub struct SendEvent {
    event: cocoa::base::id,
}

impl SendEvent {
    pub fn new(event: cocoa::base::id) -> SendEvent {
        SendEvent{event: event}
    }

    // Attempt to work the send, which either a) consumes the SendEvent, indicating completion, or
    // b) returns a SendEvent, indicating there is more work yet to perform.
    pub fn work(self) -> Option<SendEvent> {
        unsafe {
            NSApp().sendEvent_(self.event);
        }
        None
    }
}