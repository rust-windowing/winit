use {ControlFlow, EventsLoopClosed};
use events::Event;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, Weak};
use super::window::{self, Window};

mod nsevent;

mod runloop;
use self::runloop::Runloop;

pub struct EventsLoop {
    pub shared: Arc<Shared>,
    runloop: Runloop,
}

// State shared between the `EventsLoop` and its registered windows.
pub struct Shared {
    pub windows: Mutex<Vec<Weak<Window>>>,

    // A queue of events that are pending delivery to the library user.
    pub pending_events: Mutex<VecDeque<Event>>,
}

impl nsevent::WindowFinder for Shared {
    fn find_window_by_id(&self, id: window::Id) -> Option<Arc<Window>> {
        for window in self.windows.lock().unwrap().iter() {
            if let Some(window) = window.upgrade() {
                if window.id() == id {
                    return Some(window);
                }
            }
        }

        None
    }
}

pub struct Proxy {
    shared: Weak<Shared>,
}

impl Shared {

    pub fn new() -> Self {
        Shared {
            windows: Mutex::new(Vec::new()),
            pending_events: Mutex::new(VecDeque::new()),
        }
    }

    // Enqueues the event for prompt delivery to the application.
    pub fn enqueue_event(&self, event: Event) {
        self.pending_events.lock().unwrap().push_back(event);

        // TODO: wake the runloop
    }

    // Dequeues the first event, if any, from the queue.
    fn dequeue_event(&self) -> Option<Event> {
        self.pending_events.lock().unwrap().pop_front()
    }

    // Removes the window with the given `Id` from the `windows` list.
    //
    // This is called when a window is either `Closed` or `Drop`ped.
    pub fn find_and_remove_window(&self, id: super::window::Id) {
        if let Ok(mut windows) = self.windows.lock() {
            windows.retain(|w| match w.upgrade() {
                Some(w) => w.id() != id,
                None => true,
            });
        }
    }

}


#[derive(Debug,Clone,Copy,Eq,PartialEq)]
enum Timeout {
    Now,
    Forever,
}

impl Timeout {
    fn is_elapsed(&self) -> bool {
        match self {
            &Timeout::Now => true,
            &Timeout::Forever => false,
        }
    }
}

impl EventsLoop {

    pub fn new() -> Self {
        let shared = Arc::new(Shared::new());
        EventsLoop {
            runloop: Runloop::new(Arc::downgrade(&shared)),
            shared: shared,
        }
    }

    // Attempt to get an Event by a specified timeout.
    fn get_event(&mut self, timeout: Timeout) -> Option<Event> {
        loop {
            // Pop any queued events
            // This is immediate, so no need to consider a timeout
            if let Some(event) = self.shared.dequeue_event() {
                return Some(event);
            }

            // Attempt to get more events from the runloop
            self.runloop.work(timeout);

            // Is our time up?
            if timeout.is_elapsed() {
                // Check the queue again before returning, just in case
                return self.shared.dequeue_event();
            }

            // Loop around again
        }
    }

    pub fn poll_events<F>(&mut self, mut callback: F)
        where F: FnMut(Event),
    {
        // Return as many events as we can without blocking
        while let Some(event) = self.get_event(Timeout::Now) {
            callback(event);
        }
    }

    pub fn run_forever<F>(&mut self, mut callback: F)
        where F: FnMut(Event) -> ControlFlow
    {
        // Get events until we're told to stop
        while let Some(event) = self.get_event(Timeout::Forever) {
            // Send to the app
            let control_flow = callback(event);

            // Do what it says
            match control_flow {
                ControlFlow::Break => break,
                ControlFlow::Continue => (),
            }
        }
    }

    pub fn create_proxy(&self) -> Proxy {
        Proxy { shared: Arc::downgrade(&self.shared) }
    }
}

impl Proxy {
    pub fn wakeup(&self) -> Result<(), EventsLoopClosed> {
        if let Some(shared) = self.shared.upgrade() {
            shared.enqueue_event(Event::Awakened);
            Ok(())
        } else {
            Err(EventsLoopClosed)
        }
    }
}
