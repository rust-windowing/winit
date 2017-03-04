use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicBool;

use super::{DecoratedHandler, WindowId, WaylandContext};

use wayland_client::EventQueue;
use super::wayland_window::DecoratedSurface;

pub struct EventsLoopSink {
    callback: Option<*mut FnMut(::Event)>,
    queue: VecDeque<::Event>
}

unsafe impl Send for EventsLoopSink { }

impl EventsLoopSink {
    pub fn new() -> EventsLoopSink {
        EventsLoopSink {
            callback: None,
            queue: VecDeque::new()
        }
    }

    pub fn push_event(&mut self, evt: ::WindowEvent, wid: WindowId) {
        let evt = ::Event::WindowEvent {
            event: evt,
            window_id: ::WindowId(::platform::WindowId::Wayland(wid))
        };
        if let Some(cb) = self.callback {
            unsafe { (&mut *cb)(evt) }
        } else {
            self.queue.push_back(evt)
        }
    }

    // This function is only safe of the set callback is unset before exclusive
    // access to the wayland EventQueue is finished.
    //
    // The callback also cannot be used any longer as long as it has not been
    // cleared from the Sink.
    unsafe fn set_callback(&mut self, cb: &mut FnMut(::Event)) {
        let cb: &mut FnMut(::Event) = ::std::mem::transmute(cb);
        self.callback = Some(cb as *mut _);
    }

    fn with_callback<F: FnOnce(&mut FnMut(::Event))>(&mut self, f: F) {
        if let Some(cb) = self.callback {
            f(unsafe {&mut *cb})
        }
    }

    fn clear_callback(&mut self) {
        self.callback = None
    }

    fn drain_queue<F: FnMut(::Event)>(&mut self, cb: &mut F) {
        for evt in self.queue.drain(..) {
            cb(evt)
        }
    }
}

pub struct EventsLoop {
    ctxt: Arc<WaylandContext>,
    evq: Arc<Mutex<EventQueue>>,
    decorated_ids: Mutex<Vec<(usize, WindowId)>>,
    sink: Arc<Mutex<EventsLoopSink>>,
    interrupted: AtomicBool,
}

impl EventsLoop {
    pub fn new(ctxt: Arc<WaylandContext>) -> EventsLoop {
        let evq = ctxt.display.create_event_queue();
        EventsLoop {
            ctxt: ctxt,
            evq: Arc::new(Mutex::new(evq)),
            decorated_ids: Mutex::new(Vec::new()),
            sink: Arc::new(Mutex::new(EventsLoopSink::new())),
            interrupted: AtomicBool::new(false)
        }
    }
    
    pub fn get_sink(&self) -> Arc<Mutex<EventsLoopSink>> {
        self.sink.clone()
    }

    pub fn get_event_queue(&self) -> Arc<Mutex<EventQueue>> {
        self.evq.clone()
    }
    
    pub fn register_window(&self, decorated_id: usize, wid: WindowId) {
        self.decorated_ids.lock().unwrap().push((decorated_id, wid));
    }

    fn process_resize(evq: &mut EventQueue, ids: &[(usize, WindowId)], callback: &mut FnMut(::Event))
    {
        let mut state = evq.state();
        for &(decorated_id, window_id) in ids {
            let decorated = state.get_mut_handler::<DecoratedSurface<DecoratedHandler>>(decorated_id);
            if let Some((w, h)) = decorated.handler().as_mut().and_then(|h| h.take_newsize()) {
                decorated.resize(w as i32, h as i32);
                callback(
                    ::Event::WindowEvent {
                        window_id: ::WindowId(::platform::WindowId::Wayland(window_id)),
                        event: ::WindowEvent::Resized(w,h)
                    }
                );
            }
        }
    }

    pub fn interrupt(&self) {
        self.interrupted.store(true, ::std::sync::atomic::Ordering::Relaxed);
    }

    pub fn poll_events<F>(&self, mut callback: F)
        where F: FnMut(::Event)
    {
        // send pending requests to the server...
        self.ctxt.flush();

        // first of all, get exclusive access to this event queue
        let mut evq_guard = self.evq.lock().unwrap();

        // dispatch stored events:
        self.sink.lock().unwrap().drain_queue(&mut callback);
        
        // read some events from the socket if some are waiting & queue is empty
        if let Some(guard) = evq_guard.prepare_read() {
            guard.read_events().expect("Wayland connection unexpectedly lost");
        }

        // set the callback into the sink
        unsafe { self.sink.lock().unwrap().set_callback(&mut callback) };

        // then do the actual dispatching
        self.ctxt.dispatch_pending();
        evq_guard.dispatch_pending().expect("Wayland connection unexpectedly lost");
        
        let mut sink_guard = self.sink.lock().unwrap();
        
        // events where probably dispatched, process resize
        let ids_guard = self.decorated_ids.lock().unwrap();
        sink_guard.with_callback(
            |cb| Self::process_resize(&mut evq_guard, &ids_guard, cb)
        );
        
        sink_guard.clear_callback();
        // we must keep callback alive up to this point!
        drop(callback);
        
    }

    pub fn run_forever<F>(&self, mut callback: F)
        where F: FnMut(::Event)
    {
        // send pending requests to the server...
        self.ctxt.flush();

        // first of all, get exclusive access to this event queue
        let mut evq_guard = self.evq.lock().unwrap();

        // dispatch stored events:
        self.sink.lock().unwrap().drain_queue(&mut callback);
        
        // set the callback into the sink
        unsafe { self.sink.lock().unwrap().set_callback(&mut callback) };

        while !self.interrupted.load(::std::sync::atomic::Ordering::Relaxed) {
            self.ctxt.dispatch();
            evq_guard.dispatch_pending().expect("Wayland connection unexpectedly lost");
            let ids_guard = self.decorated_ids.lock().unwrap();
            self.sink.lock().unwrap().with_callback(
                |cb| Self::process_resize(&mut evq_guard, &ids_guard, cb)
            );
            self.ctxt.flush();
        }

        self.sink.lock().unwrap().clear_callback();
        // we must keep callback alive up to this point!
        drop(callback)
    }
}
