use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};

use {ControlFlow, EventsLoopClosed, PhysicalPosition, PhysicalSize};

use super::window::WindowStore;
use super::WindowId;

use sctk::output::OutputMgr;
use sctk::reexports::client::protocol::{
    wl_keyboard, wl_output, wl_pointer, wl_registry, wl_seat, wl_touch,
};
use sctk::reexports::client::{ConnectError, Display, EventQueue, GlobalEvent, Proxy};
use sctk::Environment;

use sctk::reexports::client::protocol::wl_display::RequestsTrait as DisplayRequests;
use sctk::reexports::client::protocol::wl_surface::RequestsTrait;

use ModifiersState;

pub struct EventsLoopSink {
    buffer: VecDeque<::Event>,
}

impl EventsLoopSink {
    pub fn new() -> EventsLoopSink {
        EventsLoopSink {
            buffer: VecDeque::new(),
        }
    }

    pub fn send_event(&mut self, evt: ::WindowEvent, wid: WindowId) {
        let evt = ::Event::WindowEvent {
            event: evt,
            window_id: ::WindowId(::platform::WindowId::Wayland(wid)),
        };
        self.buffer.push_back(evt);
    }

    pub fn send_raw_event(&mut self, evt: ::Event) {
        self.buffer.push_back(evt);
    }

    fn empty_with<F>(&mut self, callback: &mut F)
    where
        F: FnMut(::Event),
    {
        for evt in self.buffer.drain(..) {
            callback(evt)
        }
    }
}

pub struct EventsLoop {
    // The Event Queue
    pub evq: RefCell<EventQueue>,
    // our sink, shared with some handlers, buffering the events
    sink: Arc<Mutex<EventsLoopSink>>,
    // Whether or not there is a pending `Awakened` event to be emitted.
    pending_wakeup: Arc<AtomicBool>,
    // The window store
    pub store: Arc<Mutex<WindowStore>>,
    // the env
    pub env: Environment,
    // a cleanup switch to prune dead windows
    pub cleanup_needed: Arc<Mutex<bool>>,
    // The wayland display
    pub display: Arc<Display>,
    // The list of seats
    pub seats: Arc<Mutex<Vec<(u32, Proxy<wl_seat::WlSeat>)>>>,
}

// A handle that can be sent across threads and used to wake up the `EventsLoop`.
//
// We should only try and wake up the `EventsLoop` if it still exists, so we hold Weak ptrs.
#[derive(Clone)]
pub struct EventsLoopProxy {
    display: Weak<Display>,
    pending_wakeup: Weak<AtomicBool>,
}

impl EventsLoopProxy {
    // Causes the `EventsLoop` to stop blocking on `run_forever` and emit an `Awakened` event.
    //
    // Returns `Err` if the associated `EventsLoop` no longer exists.
    pub fn wakeup(&self) -> Result<(), EventsLoopClosed> {
        let display = self.display.upgrade();
        let wakeup = self.pending_wakeup.upgrade();
        match (display, wakeup) {
            (Some(display), Some(wakeup)) => {
                // Update the `EventsLoop`'s `pending_wakeup` flag.
                wakeup.store(true, Ordering::Relaxed);
                // Cause the `EventsLoop` to break from `dispatch` if it is currently blocked.
                let _ = display.sync(|callback| callback.implement(|_, _| {}, ()));
                display.flush().map_err(|_| EventsLoopClosed)?;
                Ok(())
            }
            _ => Err(EventsLoopClosed),
        }
    }
}

impl EventsLoop {
    pub fn new() -> Result<EventsLoop, ConnectError> {
        let (display, mut event_queue) = Display::connect_to_env()?;

        let display = Arc::new(display);
        let pending_wakeup = Arc::new(AtomicBool::new(false));
        let sink = Arc::new(Mutex::new(EventsLoopSink::new()));
        let store = Arc::new(Mutex::new(WindowStore::new()));
        let seats = Arc::new(Mutex::new(Vec::new()));

        let mut seat_manager = SeatManager {
            sink: sink.clone(),
            store: store.clone(),
            seats: seats.clone(),
            events_loop_proxy: EventsLoopProxy {
                display: Arc::downgrade(&display),
                pending_wakeup: Arc::downgrade(&pending_wakeup),
            },
        };

        let env = Environment::from_display_with_cb(
            &display,
            &mut event_queue,
            move |event, registry| {
                match event {
                    GlobalEvent::New { id, ref interface, version } => {
                        if interface == "wl_seat" {
                            seat_manager.add_seat(id, version, registry)
                        }
                    },
                    GlobalEvent::Removed { id, ref interface } => {
                        if interface == "wl_seat" {
                            seat_manager.remove_seat(id)
                        }
                    },
                }
            },
        ).unwrap();

        Ok(EventsLoop {
            display,
            evq: RefCell::new(event_queue),
            sink,
            pending_wakeup,
            store,
            env,
            cleanup_needed: Arc::new(Mutex::new(false)),
            seats,
        })
    }

    pub fn create_proxy(&self) -> EventsLoopProxy {
        EventsLoopProxy {
            display: Arc::downgrade(&self.display),
            pending_wakeup: Arc::downgrade(&self.pending_wakeup),
        }
    }

    pub fn poll_events<F>(&mut self, mut callback: F)
    where
        F: FnMut(::Event),
    {
        // send pending events to the server
        self.display.flush().expect("Wayland connection lost.");

        // dispatch any pre-buffered events
        self.sink.lock().unwrap().empty_with(&mut callback);

        // try to read pending events
        if let Some(h) = self.evq.get_mut().prepare_read() {
            h.read_events().expect("Wayland connection lost.");
        }
        // dispatch wayland events
        self.evq
            .get_mut()
            .dispatch_pending()
            .expect("Wayland connection lost.");
        self.post_dispatch_triggers();

        // dispatch buffered events to client
        self.sink.lock().unwrap().empty_with(&mut callback);
    }

    pub fn run_forever<F>(&mut self, mut callback: F)
    where
        F: FnMut(::Event) -> ControlFlow,
    {
        // send pending events to the server
        self.display.flush().expect("Wayland connection lost.");

        // Check for control flow by wrapping the callback.
        let control_flow = ::std::cell::Cell::new(ControlFlow::Continue);
        let mut callback = |event| {
            if let ControlFlow::Break = callback(event) {
                control_flow.set(ControlFlow::Break);
            }
        };

        // dispatch any pre-buffered events
        self.post_dispatch_triggers();
        self.sink.lock().unwrap().empty_with(&mut callback);

        loop {
            // dispatch events blocking if needed
            self.evq
                .get_mut()
                .dispatch()
                .expect("Wayland connection lost.");
            self.post_dispatch_triggers();

            // empty buffer of events
            self.sink.lock().unwrap().empty_with(&mut callback);

            if let ControlFlow::Break = control_flow.get() {
                break;
            }
        }
    }

    pub fn get_primary_monitor(&self) -> MonitorId {
        get_primary_monitor(&self.env.outputs)
    }

    pub fn get_available_monitors(&self) -> VecDeque<MonitorId> {
        get_available_monitors(&self.env.outputs)
    }

    pub fn get_display(&self) -> &Display {
        &*self.display
    }
}

/*
 * Private EventsLoop Internals
 */

impl EventsLoop {
    fn post_dispatch_triggers(&mut self) {
        let mut sink = self.sink.lock().unwrap();
        // process a possible pending wakeup call
        if self.pending_wakeup.load(Ordering::Relaxed) {
            sink.send_raw_event(::Event::Awakened);
            self.pending_wakeup.store(false, Ordering::Relaxed);
        }
        // prune possible dead windows
        {
            let mut cleanup_needed = self.cleanup_needed.lock().unwrap();
            if *cleanup_needed {
                let pruned = self.store.lock().unwrap().cleanup();
                *cleanup_needed = false;
                for wid in pruned {
                    sink.send_event(::WindowEvent::Destroyed, wid);
                }
            }
        }
        // process pending resize/refresh
        self.store.lock().unwrap().for_each(
            |newsize, size, new_dpi, refresh, frame_refresh, closed, wid, frame| {
                if let Some(frame) = frame {
                    if let Some((w, h)) = newsize {
                        frame.resize(w, h);
                        frame.refresh();
                        let logical_size = ::LogicalSize::new(w as f64, h as f64);
                        sink.send_event(::WindowEvent::Resized(logical_size), wid);
                        *size = (w, h);
                    } else if frame_refresh {
                        frame.refresh();
                        if !refresh {
                            frame.surface().commit()
                        }
                    }
                }
                if let Some(dpi) = new_dpi {
                    sink.send_event(::WindowEvent::HiDpiFactorChanged(dpi as f64), wid);
                }
                if refresh {
                    sink.send_event(::WindowEvent::Refresh, wid);
                }
                if closed {
                    sink.send_event(::WindowEvent::CloseRequested, wid);
                }
            },
        )
    }
}

/*
 * Wayland protocol implementations
 */

struct SeatManager {
    sink: Arc<Mutex<EventsLoopSink>>,
    store: Arc<Mutex<WindowStore>>,
    seats: Arc<Mutex<Vec<(u32, Proxy<wl_seat::WlSeat>)>>>,
    events_loop_proxy: EventsLoopProxy,
}

impl SeatManager {
    fn add_seat(&mut self, id: u32, version: u32, registry: Proxy<wl_registry::WlRegistry>) {
        use self::wl_registry::RequestsTrait as RegistryRequests;
        use std::cmp::min;

        let mut seat_data = SeatData {
            sink: self.sink.clone(),
            store: self.store.clone(),
            pointer: None,
            keyboard: None,
            touch: None,
            events_loop_proxy: self.events_loop_proxy.clone(),
            modifiers_tracker: Arc::new(Mutex::new(ModifiersState::default())),
        };
        let seat = registry
            .bind(min(version, 5), id, move |seat| {
                seat.implement(move |event, seat| {
                    seat_data.receive(event, seat)
                }, ())
            })
            .unwrap();
        self.store.lock().unwrap().new_seat(&seat);
        self.seats.lock().unwrap().push((id, seat));
    }

    fn remove_seat(&mut self, id: u32) {
        use self::wl_seat::RequestsTrait as SeatRequests;
        let mut seats = self.seats.lock().unwrap();
        if let Some(idx) = seats.iter().position(|&(i, _)| i == id) {
            let (_, seat) = seats.swap_remove(idx);
            if seat.version() >= 5 {
                seat.release();
            }
        }
    }
}

struct SeatData {
    sink: Arc<Mutex<EventsLoopSink>>,
    store: Arc<Mutex<WindowStore>>,
    pointer: Option<Proxy<wl_pointer::WlPointer>>,
    keyboard: Option<Proxy<wl_keyboard::WlKeyboard>>,
    touch: Option<Proxy<wl_touch::WlTouch>>,
    events_loop_proxy: EventsLoopProxy,
    modifiers_tracker: Arc<Mutex<ModifiersState>>,
}

impl SeatData {
    fn receive(&mut self, evt: wl_seat::Event, seat: Proxy<wl_seat::WlSeat>) {
        match evt {
            wl_seat::Event::Name { .. } => (),
            wl_seat::Event::Capabilities { capabilities } => {
                // create pointer if applicable
                if capabilities.contains(wl_seat::Capability::Pointer) && self.pointer.is_none() {
                    self.pointer = Some(super::pointer::implement_pointer(
                        &seat,
                        self.sink.clone(),
                        self.store.clone(),
                        self.modifiers_tracker.clone(),
                    ))
                }
                // destroy pointer if applicable
                if !capabilities.contains(wl_seat::Capability::Pointer) {
                    if let Some(pointer) = self.pointer.take() {
                        if pointer.version() >= 3 {
                            use self::wl_pointer::RequestsTrait;
                            pointer.release();
                        }
                    }
                }
                // create keyboard if applicable
                if capabilities.contains(wl_seat::Capability::Keyboard) && self.keyboard.is_none() {
                    self.keyboard = Some(super::keyboard::init_keyboard(
                        &seat,
                        self.sink.clone(),
                        self.events_loop_proxy.clone(),
                        self.modifiers_tracker.clone(),
                    ))
                }
                // destroy keyboard if applicable
                if !capabilities.contains(wl_seat::Capability::Keyboard) {
                    if let Some(kbd) = self.keyboard.take() {
                        if kbd.version() >= 3 {
                            use self::wl_keyboard::RequestsTrait;
                            kbd.release();
                        }
                    }
                }
                // create touch if applicable
                if capabilities.contains(wl_seat::Capability::Touch) && self.touch.is_none() {
                    self.touch = Some(super::touch::implement_touch(
                        &seat,
                        self.sink.clone(),
                        self.store.clone(),
                    ))
                }
                // destroy touch if applicable
                if !capabilities.contains(wl_seat::Capability::Touch) {
                    if let Some(touch) = self.touch.take() {
                        if touch.version() >= 3 {
                            use self::wl_touch::RequestsTrait;
                            touch.release();
                        }
                    }
                }
            }
        }
    }
}

impl Drop for SeatData {
    fn drop(&mut self) {
        if let Some(pointer) = self.pointer.take() {
            if pointer.version() >= 3 {
                use self::wl_pointer::RequestsTrait;
                pointer.release();
            }
        }
        if let Some(kbd) = self.keyboard.take() {
            if kbd.version() >= 3 {
                use self::wl_keyboard::RequestsTrait;
                kbd.release();
            }
        }
        if let Some(touch) = self.touch.take() {
            if touch.version() >= 3 {
                use self::wl_touch::RequestsTrait;
                touch.release();
            }
        }
    }
}

/*
 * Monitor stuff
 */

pub struct MonitorId {
    pub(crate) proxy: Proxy<wl_output::WlOutput>,
    pub(crate) mgr: OutputMgr,
}

impl Clone for MonitorId {
    fn clone(&self) -> MonitorId {
        MonitorId {
            proxy: self.proxy.clone(),
            mgr: self.mgr.clone(),
        }
    }
}

impl fmt::Debug for MonitorId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        #[derive(Debug)]
        struct MonitorId {
            name: Option<String>,
            native_identifier: u32,
            dimensions: PhysicalSize,
            position: PhysicalPosition,
            hidpi_factor: i32,
        }

        let monitor_id_proxy = MonitorId {
            name: self.get_name(),
            native_identifier: self.get_native_identifier(),
            dimensions: self.get_dimensions(),
            position: self.get_position(),
            hidpi_factor: self.get_hidpi_factor(),
        };

        monitor_id_proxy.fmt(f)
    }
}

impl MonitorId {
    pub fn get_name(&self) -> Option<String> {
        self.mgr.with_info(&self.proxy, |_, info| {
            format!("{} ({})", info.model, info.make)
        })
    }

    #[inline]
    pub fn get_native_identifier(&self) -> u32 {
        self.mgr.with_info(&self.proxy, |id, _| id).unwrap_or(0)
    }

    pub fn get_dimensions(&self) -> PhysicalSize {
        match self.mgr.with_info(&self.proxy, |_, info| {
            info.modes
                .iter()
                .find(|m| m.is_current)
                .map(|m| m.dimensions)
        }) {
            Some(Some((w, h))) => (w as u32, h as u32),
            _ => (0, 0),
        }.into()
    }

    pub fn get_position(&self) -> PhysicalPosition {
        self.mgr
            .with_info(&self.proxy, |_, info| info.location)
            .unwrap_or((0, 0))
            .into()
    }

    #[inline]
    pub fn get_hidpi_factor(&self) -> i32 {
        self.mgr
            .with_info(&self.proxy, |_, info| info.scale_factor)
            .unwrap_or(1)
    }
}

pub fn get_primary_monitor(outputs: &OutputMgr) -> MonitorId {
    outputs.with_all(|list| {
        if let Some(&(_, ref proxy, _)) = list.first() {
            MonitorId {
                proxy: proxy.clone(),
                mgr: outputs.clone(),
            }
        } else {
            panic!("No monitor is available.")
        }
    })
}

pub fn get_available_monitors(outputs: &OutputMgr) -> VecDeque<MonitorId> {
    outputs.with_all(|list| {
        list.iter()
            .map(|&(_, ref proxy, _)| MonitorId {
                proxy: proxy.clone(),
                mgr: outputs.clone(),
            })
            .collect()
    })
}
