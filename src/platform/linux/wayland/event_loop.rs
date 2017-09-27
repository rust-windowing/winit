use {WindowEvent as Event, ElementState, MouseButton, MouseScrollDelta, TouchPhase, ModifiersState,
     KeyboardInput, EventsLoopClosed, ControlFlow};

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, Weak};
use std::sync::atomic::{self, AtomicBool};

use super::{DecoratedHandler, WindowId, DeviceId, WaylandContext};


use wayland_client::{EventQueue, EventQueueHandle, Init, Proxy, Liveness};
use wayland_client::protocol::{wl_seat, wl_surface, wl_pointer, wl_keyboard};

use super::make_wid;
use super::wayland_window::DecoratedSurface;
use super::wayland_kbd::MappedKeyboard;
use super::keyboard::KbdHandler;

/// This struct is used as a holder for the callback
/// during the dispatching of events.
///
/// The proper ay to use it is:
/// - set a callback in it (and retrieve the noop one it contains)
/// - dispatch the EventQueue
/// - put back the noop callback in it
///
/// Failure to do so is unsafe™
pub struct EventsLoopSink {
    buffer: VecDeque<::Event>
}

unsafe impl Send for EventsLoopSink { }

impl EventsLoopSink {
    pub fn new() -> EventsLoopSink{EventsLoopSink {
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
    // ids of the DecoratedHandlers of the surfaces we know
    decorated_ids: Mutex<Vec<(usize, Arc<wl_surface::WlSurface>)>>,
    // our sink, shared with some handlers, buffering the events
    sink: Arc<Mutex<EventsLoopSink>>,
    // trigger cleanup of the dead surfaces
    cleanup_needed: Arc<AtomicBool>,
    // Whether or not there is a pending `Awakened` event to be emitted.
    pending_wakeup: Arc<AtomicBool>,
    hid: usize,
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
                wakeup.store(true, atomic::Ordering::Relaxed);
                // Cause the `EventsLoop` to break from `dispatch` if it is currently blocked.
                ctxt.display.sync();
                ctxt.display.flush().ok();
                Ok(())
            },
            _ => Err(EventsLoopClosed),
        }
    }
}

impl EventsLoop {
    pub fn new(mut ctxt: WaylandContext) -> EventsLoop {
        let sink = Arc::new(Mutex::new(EventsLoopSink::new()));
        let inputh = InputHandler::new(&ctxt, sink.clone());
        let hid = ctxt.evq.get_mut().unwrap().add_handler_with_init(inputh);
        let ctxt = Arc::new(ctxt);

        EventsLoop {
            ctxt: ctxt,
            decorated_ids: Mutex::new(Vec::new()),
            sink: sink,
            pending_wakeup: Arc::new(AtomicBool::new(false)),
            cleanup_needed: Arc::new(AtomicBool::new(false)),
            hid: hid
        }
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

    // some internals that Window needs access to
    pub fn get_window_init(&self) -> Arc<AtomicBool> {
        self.cleanup_needed.clone()
    }

    pub fn register_window(&self, decorated_id: usize, surface: Arc<wl_surface::WlSurface>) {
        self.decorated_ids.lock().unwrap().push((decorated_id, surface.clone()));
        let mut guard = self.ctxt.evq.lock().unwrap();
        let mut state = guard.state();
        state.get_mut_handler::<InputHandler>(self.hid).windows.push(surface);
    }

    fn process_resize(evq: &mut EventQueue, ids: &[(usize, Arc<wl_surface::WlSurface>)], sink: &mut EventsLoopSink)
    {
        let mut state = evq.state();
        for &(decorated_id, ref window) in ids {
            let decorated = state.get_mut_handler::<DecoratedSurface<DecoratedHandler>>(decorated_id);
            if let Some((w, h)) = decorated.handler().as_mut().and_then(|h| h.take_newsize()) {
                decorated.resize(w as i32, h as i32);
                sink.send_event(
                     ::WindowEvent::Resized(w,h),
                     make_wid(&window)
                );
            }
            if decorated.handler().as_mut().map(|h| h.take_refresh()).unwrap_or(false) {
                sink.send_event(
                     ::WindowEvent::Refresh,
                     make_wid(&window)
                );
            }
            if decorated.handler().as_ref().map(|h| h.is_closed()).unwrap_or(false) {
                 sink.send_event(
                     ::WindowEvent::Closed,
                     make_wid(&window)
                );

            }
        }
    }

    fn prune_dead_windows(&self) {
        self.decorated_ids.lock().unwrap().retain(|&(_, ref w)| w.status() == Liveness::Alive);
        let mut evq_guard = self.ctxt.evq.lock().unwrap();
        let mut state = evq_guard.state();
        let handler = state.get_mut_handler::<InputHandler>(self.hid);
        handler.windows.retain(|w| w.status() == Liveness::Alive);
        if let Some(w) = handler.mouse_focus.take() {
            if w.status() == Liveness::Alive {
                handler.mouse_focus = Some(w)
            }
        }
    }

    fn post_dispatch_triggers(&self) {
        let mut evq_guard = self.ctxt.evq.lock().unwrap();
        let mut sink_guard = self.sink.lock().unwrap();
        let ids_guard = self.decorated_ids.lock().unwrap();
        self.emit_pending_wakeup(&mut sink_guard);
        Self::process_resize(&mut evq_guard, &ids_guard, &mut sink_guard);
    }

    pub fn poll_events<F>(&mut self, mut callback: F)
        where F: FnMut(::Event)
    {
        // send pending requests to the server...
        self.ctxt.flush();

        // dispatch any pre-buffered events
        self.sink.lock().unwrap().empty_with(&mut callback);

        // try dispatching events without blocking
        self.ctxt.read_events();
        self.ctxt.dispatch_pending();
        self.post_dispatch_triggers();

        self.sink.lock().unwrap().empty_with(&mut callback);

        if self.cleanup_needed.swap(false, atomic::Ordering::Relaxed) {
            self.prune_dead_windows()
        }
    }

    pub fn run_forever<F>(&mut self, mut callback: F)
        where F: FnMut(::Event) -> ControlFlow,
    {
        // send pending requests to the server...
        self.ctxt.flush();

        // Check for control flow by wrapping the callback.
        let control_flow = ::std::cell::Cell::new(ControlFlow::Continue);
        let mut callback = |event| if let ControlFlow::Break = callback(event) {
            control_flow.set(ControlFlow::Break);
        };

        // dispatch pre-buffered events
        self.sink.lock().unwrap().empty_with(&mut callback);

        loop {
            // dispatch events blocking if needed
            self.ctxt.dispatch();
            self.post_dispatch_triggers();

            // empty buffer of events
            self.sink.lock().unwrap().empty_with(&mut callback);

            if self.cleanup_needed.swap(false, atomic::Ordering::Relaxed) {
                self.prune_dead_windows()
            }

            if let ControlFlow::Break = control_flow.get() {
                break;
            }
        }
    }

    // If an `EventsLoopProxy` has signalled a wakeup, emit an event and reset the flag.
    fn emit_pending_wakeup(&self, sink: &mut EventsLoopSink) {
        if self.pending_wakeup.load(atomic::Ordering::Relaxed) {
            sink.send_raw_event(::Event::Awakened);
            self.pending_wakeup.store(false, atomic::Ordering::Relaxed);
        }
    }
}

enum KbdType {
    Mapped(MappedKeyboard<KbdHandler>),
    Plain(Option<WindowId>)
}

struct InputHandler {
    my_id: usize,
    windows: Vec<Arc<wl_surface::WlSurface>>,
    seat: Option<wl_seat::WlSeat>,
    mouse: Option<wl_pointer::WlPointer>,
    mouse_focus: Option<Arc<wl_surface::WlSurface>>,
    mouse_location: (f64, f64),
    axis_buffer: Option<(f32, f32)>,
    axis_discrete_buffer: Option<(i32, i32)>,
    axis_state: TouchPhase,
    kbd: Option<wl_keyboard::WlKeyboard>,
    kbd_handler: KbdType,
    callback: Arc<Mutex<EventsLoopSink>>
}

impl InputHandler {
    fn new(ctxt: &WaylandContext, sink: Arc<Mutex<EventsLoopSink>>) -> InputHandler {
        let kbd_handler = match MappedKeyboard::new(KbdHandler::new(sink.clone())) {
            Ok(h) => KbdType::Mapped(h),
            Err(_) => KbdType::Plain(None)
        };
        InputHandler {
            my_id: 0,
            windows: Vec::new(),
            seat: ctxt.get_seat(),
            mouse: None,
            mouse_focus: None,
            mouse_location: (0.0,0.0),
            axis_buffer: None,
            axis_discrete_buffer: None,
            axis_state: TouchPhase::Started,
            kbd: None,
            kbd_handler: kbd_handler,
            callback: sink
        }
    }
}

impl Init for InputHandler {
    fn init(&mut self, evqh: &mut EventQueueHandle, index: usize) {
        if let Some(ref seat) = self.seat {
            evqh.register::<_, InputHandler>(seat, index);
        }
        self.my_id = index;
    }
}

impl wl_seat::Handler for InputHandler {
    fn capabilities(&mut self,
                    evqh: &mut EventQueueHandle,
                    seat: &wl_seat::WlSeat,
                    capabilities: wl_seat::Capability)
    {
        // create pointer if applicable
        if capabilities.contains(wl_seat::Pointer) && self.mouse.is_none() {
            let pointer = seat.get_pointer().expect("Seat is not dead");
            evqh.register::<_, InputHandler>(&pointer, self.my_id);
            self.mouse = Some(pointer);
        }
        // destroy pointer if applicable
        if !capabilities.contains(wl_seat::Pointer) {
            if let Some(pointer) = self.mouse.take() {
                pointer.release();
            }
        }
        // create keyboard if applicable
        if capabilities.contains(wl_seat::Keyboard) && self.kbd.is_none() {
            let kbd = seat.get_keyboard().expect("Seat is not dead");
            evqh.register::<_, InputHandler>(&kbd, self.my_id);
            self.kbd = Some(kbd);
        }
        // destroy keyboard if applicable
        if !capabilities.contains(wl_seat::Keyboard) {
            if let Some(kbd) = self.kbd.take() {
                kbd.release();
            }
        }
    }
}

declare_handler!(InputHandler, wl_seat::Handler, wl_seat::WlSeat);

/*
 * Pointer Handling
 */

impl wl_pointer::Handler for InputHandler {
    fn enter(&mut self,
             _evqh: &mut EventQueueHandle,
             _proxy: &wl_pointer::WlPointer,
             _serial: u32,
             surface: &wl_surface::WlSurface,
             surface_x: f64,
             surface_y: f64)
    {
        self.mouse_location = (surface_x, surface_y);
        for window in &self.windows {
            if window.equals(surface) {
                self.mouse_focus = Some(window.clone());
                let (w, h) = self.mouse_location;
                let mut guard = self.callback.lock().unwrap();
                guard.send_event(Event::MouseEntered { device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)) },
                                 make_wid(window));
                guard.send_event(Event::MouseMoved { device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                                                     position: (w, h) },
                                 make_wid(window));
                break;
            }
        }
    }

    fn leave(&mut self,
             _evqh: &mut EventQueueHandle,
             _proxy: &wl_pointer::WlPointer,
             _serial: u32,
             surface: &wl_surface::WlSurface)
    {
        self.mouse_focus = None;
        for window in &self.windows {
            if window.equals(surface) {
                self.callback.lock().unwrap().send_event(Event::MouseLeft { device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)) },
                                                         make_wid(window));
            }
        }
    }

    fn motion(&mut self,
              _evqh: &mut EventQueueHandle,
              _proxy: &wl_pointer::WlPointer,
              _time: u32,
              surface_x: f64,
              surface_y: f64)
    {
        self.mouse_location = (surface_x, surface_y);
        if let Some(ref window) = self.mouse_focus {
            let (w,h) = self.mouse_location;
            self.callback.lock().unwrap().send_event(Event::MouseMoved { device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                                                                         position: (w, h) }, make_wid(window));
        }
    }

    fn button(&mut self,
              _evqh: &mut EventQueueHandle,
              _proxy: &wl_pointer::WlPointer,
              _serial: u32,
              _time: u32,
              button: u32,
              state: wl_pointer::ButtonState)
    {
        if let Some(ref window) = self.mouse_focus {
            let state = match state {
                wl_pointer::ButtonState::Pressed => ElementState::Pressed,
                wl_pointer::ButtonState::Released => ElementState::Released
            };
            let button = match button {
                0x110 => MouseButton::Left,
                0x111 => MouseButton::Right,
                0x112 => MouseButton::Middle,
                // TODO figure out the translation ?
                _ => return
            };
            self.callback.lock().unwrap().send_event(
                Event::MouseInput {
                    device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                    state: state,
                    button: button,
                },
                make_wid(window)
            );
        }
    }

    fn axis(&mut self,
            _evqh: &mut EventQueueHandle,
            _proxy: &wl_pointer::WlPointer,
            _time: u32,
            axis: wl_pointer::Axis,
            value: f64)
    {
        let (mut x, mut y) = self.axis_buffer.unwrap_or((0.0, 0.0));
        match axis {
            // wayland vertical sign convention is the inverse of winit
            wl_pointer::Axis::VerticalScroll => y -= value as f32,
            wl_pointer::Axis::HorizontalScroll => x += value as f32
        }
        self.axis_buffer = Some((x,y));
        self.axis_state = match self.axis_state {
            TouchPhase::Started | TouchPhase::Moved => TouchPhase::Moved,
            _ => TouchPhase::Started
        }
    }

    fn frame(&mut self,
             _evqh: &mut EventQueueHandle,
             _proxy: &wl_pointer::WlPointer)
    {
        let axis_buffer = self.axis_buffer.take();
        let axis_discrete_buffer = self.axis_discrete_buffer.take();
        if let Some(ref window) = self.mouse_focus {
            if let Some((x, y)) = axis_discrete_buffer {
                self.callback.lock().unwrap().send_event(
                    Event::MouseWheel {
                        device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                        delta: MouseScrollDelta::LineDelta(x as f32, y as f32),
                        phase: self.axis_state,
                    },
                    make_wid(window)
                );
            } else if let Some((x, y)) = axis_buffer {
                self.callback.lock().unwrap().send_event(
                    Event::MouseWheel {
                        device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                        delta: MouseScrollDelta::PixelDelta(x as f32, y as f32),
                        phase: self.axis_state,
                    },
                    make_wid(window)
                );
            }
        }
    }

    fn axis_source(&mut self,
                   _evqh: &mut EventQueueHandle,
                   _proxy: &wl_pointer::WlPointer,
                   _axis_source: wl_pointer::AxisSource)
    {
    }

    fn axis_stop(&mut self,
                 _evqh: &mut EventQueueHandle,
                 _proxy: &wl_pointer::WlPointer,
                 _time: u32,
                 _axis: wl_pointer::Axis)
    {
        self.axis_state = TouchPhase::Ended;
    }

    fn axis_discrete(&mut self,
                     _evqh: &mut EventQueueHandle,
                     _proxy: &wl_pointer::WlPointer,
                     axis: wl_pointer::Axis,
                     discrete: i32)
    {
        let (mut x, mut y) = self.axis_discrete_buffer.unwrap_or((0,0));
        match axis {
            // wayland vertical sign convention is the inverse of winit
            wl_pointer::Axis::VerticalScroll => y -= discrete,
            wl_pointer::Axis::HorizontalScroll => x += discrete
        }
        self.axis_discrete_buffer = Some((x,y));
                self.axis_state = match self.axis_state {
            TouchPhase::Started | TouchPhase::Moved => TouchPhase::Moved,
            _ => TouchPhase::Started
        }
    }
}

declare_handler!(InputHandler, wl_pointer::Handler, wl_pointer::WlPointer);

/*
 * Keyboard Handling
 */

impl wl_keyboard::Handler for InputHandler {
    // mostly pass-through
    fn keymap(&mut self,
              evqh: &mut EventQueueHandle,
              proxy: &wl_keyboard::WlKeyboard,
              format: wl_keyboard::KeymapFormat,
              fd: ::std::os::unix::io::RawFd,
              size: u32)
    {
        match self.kbd_handler {
            KbdType::Mapped(ref mut h) => h.keymap(evqh, proxy, format, fd, size),
            _ => ()
        }
    }

    fn enter(&mut self,
             evqh: &mut EventQueueHandle,
             proxy: &wl_keyboard::WlKeyboard,
             serial: u32,
             surface: &wl_surface::WlSurface,
             keys: Vec<u8>)
    {
        for window in &self.windows {
            if window.equals(surface) {
                self.callback.lock().unwrap().send_event(Event::Focused(true), make_wid(window));
                match self.kbd_handler {
                    KbdType::Mapped(ref mut h) => {
                        h.handler().target = Some(make_wid(window));
                        h.enter(evqh, proxy, serial, surface, keys);
                    },
                    KbdType::Plain(ref mut target) => {
                        *target = Some(make_wid(window))
                    }
                }
                break;
            }
        }
    }

    fn leave(&mut self,
             evqh: &mut EventQueueHandle,
             proxy: &wl_keyboard::WlKeyboard,
             serial: u32,
             surface: &wl_surface::WlSurface)
    {
        for window in &self.windows {
            if window.equals(surface) {
                self.callback.lock().unwrap().send_event(Event::Focused(false), make_wid(window));
                match self.kbd_handler {
                    KbdType::Mapped(ref mut h) => {
                        h.handler().target = None;
                        h.leave(evqh, proxy, serial, surface);
                    },
                    KbdType::Plain(ref mut target) => {
                        *target = None
                    }
                }
                break;
            }
        }
    }

    fn key(&mut self,
           evqh: &mut EventQueueHandle,
           proxy: &wl_keyboard::WlKeyboard,
           serial: u32,
           time: u32,
           key: u32,
           state: wl_keyboard::KeyState)
    {
        match self.kbd_handler {
            KbdType::Mapped(ref mut h) => h.key(evqh, proxy, serial, time, key, state),
            KbdType::Plain(Some(wid)) => {
                let state = match state {
                    wl_keyboard::KeyState::Pressed => ElementState::Pressed,
                    wl_keyboard::KeyState::Released => ElementState::Released,
                };
                // This is fallback impl if libxkbcommon was not available
                // This case should probably never happen, as most wayland
                // compositors _need_ libxkbcommon anyway...
                //
                // In this case, we don't have the modifiers state information
                // anyway, as we need libxkbcommon to interpret it (it is
                // supposed to be serialized by the compositor using libxkbcommon)
                self.callback.lock().unwrap().send_event(
                    Event::KeyboardInput {
                        device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                        input: KeyboardInput {
                            state: state,
                            scancode: key,
                            virtual_keycode: None,
                            modifiers: ModifiersState::default(),
                        },
                    },
                    wid
                );
            },
            KbdType::Plain(None) => ()
        }
    }

    fn modifiers(&mut self,
                 evqh: &mut EventQueueHandle,
                 proxy: &wl_keyboard::WlKeyboard,
                 serial: u32,
                 mods_depressed: u32,
                 mods_latched: u32,
                 mods_locked: u32,
                 group: u32)
    {
        match self.kbd_handler {
            KbdType::Mapped(ref mut h) => h.modifiers(evqh, proxy, serial, mods_depressed,
                                                      mods_latched, mods_locked, group),
            _ => ()
        }
    }

    fn repeat_info(&mut self,
                   evqh: &mut EventQueueHandle,
                   proxy: &wl_keyboard::WlKeyboard,
                   rate: i32,
                   delay: i32)
    {
        match self.kbd_handler {
            KbdType::Mapped(ref mut h) => h.repeat_info(evqh, proxy, rate, delay),
            _ => ()
        }
    }
}

declare_handler!(InputHandler, wl_keyboard::Handler, wl_keyboard::WlKeyboard);
