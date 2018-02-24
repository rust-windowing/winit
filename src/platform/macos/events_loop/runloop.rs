use std::sync::Weak;
use core_foundation;
use cocoa;

use super::{Shared,Timeout};
use super::nsevent;

// The Runloop is responsible for:
//   - receiving NSEvents from Cocoa
//   - forwarding NSEvents back to Cocoa
//   - posting Events to the queue
pub struct Runloop {
    shared: Weak<Shared>,
    event_state: nsevent::PersistentState,
}

impl Runloop {
    // Create a runloop
    pub fn new(shared: Weak<Shared>) -> Runloop {
        Runloop {
            shared,
            event_state: nsevent::PersistentState::new()
        }
    }

    // Work the runloop, attempting to respect the timeout
    pub fn work(&mut self, timeout: Timeout) {
        unsafe {
            if !msg_send![cocoa::base::class("NSThread"), isMainThread] {
                panic!("Events can only be polled from the main thread on macOS");
            }
        }

        let shared = match self.shared.upgrade() {
            None => return, // event loop went away
            Some(shared) => shared
        };

        loop {
            // Return if there's already an event waiting
            if shared.has_queued_events() {
                return;
            }

            let event = match nsevent::receive_event_from_cocoa(timeout) {
                None => {
                    // Our timeout expired
                    // Bail out
                    return;
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
            let events_len = events.len();

            // Post them
            for event in events {
                shared.enqueue_event(event);
            }

            // Return if we've accomplished something or if we're out of time
            if events_len > 0 || timeout.is_elapsed() {
                break;
            }
        }
    }

    // Attempt to wake the Runloop. Must be thread safe.
    pub fn wake() {
        nsevent::post_event_to_self();

        unsafe {
            core_foundation::runloop::CFRunLoopWakeUp(core_foundation::runloop::CFRunLoopGetMain());
        }
    }
}