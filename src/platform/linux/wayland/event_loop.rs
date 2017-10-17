use std::collections::VecDeque;
use std::sync::{Arc, Mutex, Weak};
use std::sync::atomic::{AtomicBool, Ordering};

use {EventsLoopClosed, ControlFlow};

use super::{WaylandContext, WindowId};
use super::window::WindowStore;
use super::keyboard::init_keyboard;

use wayland_client::StateToken;
use wayland_client::protocol::{wl_seat, wl_pointer, wl_keyboard};

pub struct EventsLoopSink {
    buffer: VecDeque<::Event>
}

unsafe impl Send for EventsLoopSink { }

impl EventsLoopSink {
    pub fn new() -> EventsLoopSink{
        EventsLoopSink {
            buffer: VecDeque::new()
        }
    }

    pub fn send_event(&mut self, evt: ::WindowEvent, wid: WindowId) {
        let evt = ::Event::WindowEvent {
            event: evt,
            window_id: ::WindowId(::platform::WindowId::Wayland(wid))
        };
        self.buffer.push_back(evt);
    }

    pub fn send_raw_event(&mut self, evt: ::Event) {
        self.buffer.push_back(evt);
    }

    fn empty_with<F>(&mut self, callback: &mut F) where F: FnMut(::Event) {
        for evt in self.buffer.drain(..) {
            callback(evt)
        }
    }
}

pub struct EventsLoop {
    // the wayland context
    ctxt: Arc<WaylandContext>,
    // our sink, shared with some handlers, buffering the events
    sink: Arc<Mutex<EventsLoopSink>>,
    // Whether or not there is a pending `Awakened` event to be emitted.
    pending_wakeup: Arc<AtomicBool>,
    store: StateToken<WindowStore>
}

// A handle that can be sent across threads and used to wake up the `EventsLoop`.
//
// We should only try and wake up the `EventsLoop` if it still exists, so we hold Weak ptrs.
pub struct EventsLoopProxy {
    ctxt: Weak<WaylandContext>,
    pending_wakeup: Weak<AtomicBool>,
}

impl EventsLoopProxy {
    // Causes the `EventsLoop` to stop blocking on `run_forever` and emit an `Awakened` event.
    //
    // Returns `Err` if the associated `EventsLoop` no longer exists.
    pub fn wakeup(&self) -> Result<(), EventsLoopClosed> {
        let ctxt = self.ctxt.upgrade();
        let wakeup = self.pending_wakeup.upgrade();
        match (ctxt, wakeup) {
            (Some(ctxt), Some(wakeup)) => {
                // Update the `EventsLoop`'s `pending_wakeup` flag.
                wakeup.store(true, Ordering::Relaxed);
                // Cause the `EventsLoop` to break from `dispatch` if it is currently blocked.
                ctxt.display.sync();
                ctxt.display.flush()
                            .map_err(|_| EventsLoopClosed)?;
                Ok(())
            },
            _ => Err(EventsLoopClosed),
        }
    }
}

impl EventsLoop {
    pub fn new(mut ctxt: WaylandContext) -> EventsLoop {
        let sink = Arc::new(Mutex::new(EventsLoopSink::new()));

        let store = ctxt.evq.lock().unwrap().state().insert(WindowStore::new());

        let seat_idata = SeatIData {
            sink: sink.clone(),
            keyboard: None,
            pointer: None
        };

        ctxt.init_seat(|evqh, seat| {
            evqh.register(seat, seat_implementation(), seat_idata);
        });

        EventsLoop {
            ctxt: Arc::new(ctxt),
            sink: sink,
            pending_wakeup: Arc::new(AtomicBool::new(false)),
            store: store
        }
    }

    #[inline]
    pub fn store(&self) -> StateToken<WindowStore> {
        self.store.clone()
    }

    #[inline]
    pub fn context(&self) -> &Arc<WaylandContext> {
        &self.ctxt
    }

    pub fn create_proxy(&self) -> EventsLoopProxy {
        EventsLoopProxy {
            ctxt: Arc::downgrade(&self.ctxt),
            pending_wakeup: Arc::downgrade(&self.pending_wakeup),
        }
    }

    pub fn poll_events<F>(&mut self, mut callback: F)
        where F: FnMut(::Event)
    {
        unimplemented!()
    }

    pub fn run_forever<F>(&mut self, mut callback: F)
        where F: FnMut(::Event) -> ControlFlow,
    {
        unimplemented!()
    }
}

/*
 * Wayland protocol implementations
 */

struct SeatIData {
    sink: Arc<Mutex<EventsLoopSink>>,
    pointer: Option<wl_pointer::WlPointer>,
    keyboard: Option<wl_keyboard::WlKeyboard>
}

fn seat_implementation() -> wl_seat::Implementation<SeatIData> {
    wl_seat::Implementation {
        name: |_, _, _, _| {},
        capabilities: |evqh, idata, seat, capabilities| {
            // create pointer if applicable
            if capabilities.contains(wl_seat::Capability::Pointer) && idata.pointer.is_none() {
                let pointer = seat.get_pointer().expect("Seat is not dead");
                // FIXME: register pointer
                idata.pointer = Some(pointer);
            }
            // destroy pointer if applicable
            if !capabilities.contains(wl_seat::Capability::Pointer) {
                if let Some(pointer) = idata.pointer.take() {
                    pointer.release();
                }
            }
            // create keyboard if applicable
            if capabilities.contains(wl_seat::Capability::Keyboard) && idata.keyboard.is_none() {
                let kbd = seat.get_keyboard().expect("Seat is not dead");
                init_keyboard(evqh, &kbd, &idata.sink);
                idata.keyboard = Some(kbd);
            }
            // destroy keyboard if applicable
            if !capabilities.contains(wl_seat::Capability::Keyboard) {
                if let Some(kbd) = idata.keyboard.take() {
                    kbd.release();
                }
            }
        }
    }
}