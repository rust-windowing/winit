use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, Weak};
use std::sync::atomic::{AtomicBool, Ordering};

use {EventsLoopClosed, ControlFlow};

use super::WindowId;
use super::window::WindowStore;
use super::keyboard::init_keyboard;

use wayland_client::{EnvHandler, EnvNotify, default_connect, EventQueue, EventQueueHandle, Proxy, StateToken};
use wayland_client::protocol::{wl_compositor, wl_seat, wl_shell, wl_shm, wl_subcompositor,
                               wl_display, wl_registry, wl_output, wl_surface,
                               wl_pointer, wl_keyboard, wl_touch};

use super::wayland_window::{Frame, Shell, create_frame, FrameImplementation};
use super::wayland_protocols::unstable::xdg_shell::v6::client::zxdg_shell_v6;

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
    // The Event Queue
    pub evq: RefCell<EventQueue>,
    // our sink, shared with some handlers, buffering the events
    sink: Arc<Mutex<EventsLoopSink>>,
    // Whether or not there is a pending `Awakened` event to be emitted.
    pending_wakeup: Arc<AtomicBool>,
    // The window store
    pub store: StateToken<WindowStore>,
    // the env
    env_token: StateToken<EnvHandler<InnerEnv>>,
    // the ctxt
    pub ctxt_token: StateToken<StateContext>,
    // a cleanup switch to prune dead windows
    pub cleanup_needed: Arc<Mutex<bool>>,
    // The wayland display
    pub display: Arc<wl_display::WlDisplay>,
}

// A handle that can be sent across threads and used to wake up the `EventsLoop`.
//
// We should only try and wake up the `EventsLoop` if it still exists, so we hold Weak ptrs.
#[derive(Clone)]
pub struct EventsLoopProxy {
    display: Weak<wl_display::WlDisplay>,
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
                display.sync();
                display.flush().map_err(|_| EventsLoopClosed)?;
                Ok(())
            },
            _ => Err(EventsLoopClosed),
        }
    }
}

impl EventsLoop {
    pub fn new() -> Option<EventsLoop> {
        let (display, mut event_queue) = match default_connect() {
            Ok(ret) => ret,
            Err(_) => return None
        };

        let registry = display.get_registry();
        let ctxt_token = event_queue.state().insert(
            StateContext::new(registry.clone().unwrap())
        );
        let env_token = EnvHandler::init_with_notify(
            &mut event_queue,
            &registry,
            env_notify(),
            ctxt_token.clone()
        );

        // two round trips to fully initialize
        event_queue.sync_roundtrip().expect("Wayland connection unexpectedly lost");
        event_queue.sync_roundtrip().expect("Wayland connection unexpectedly lost");

        event_queue.state().with_value(&ctxt_token, |proxy, ctxt| {
            ctxt.ensure_shell(proxy.get_mut(&env_token))
        });

        let sink = Arc::new(Mutex::new(EventsLoopSink::new()));

        let store = event_queue.state().insert(WindowStore::new());

        let seat_idata = SeatIData {
            sink: sink.clone(),
            keyboard: None,
            pointer: None,
            touch: None,
            windows_token: store.clone()
        };

        let mut me = EventsLoop {
            display: Arc::new(display),
            evq: RefCell::new(event_queue),
            sink: sink,
            pending_wakeup: Arc::new(AtomicBool::new(false)),
            store: store,
            ctxt_token: ctxt_token,
            env_token: env_token,
            cleanup_needed: Arc::new(Mutex::new(false))
        };

        me.init_seat(|evqh, seat| {
            evqh.register(seat, seat_implementation(), seat_idata);
        });

        Some(me)
    }

    pub fn create_proxy(&self) -> EventsLoopProxy {
        EventsLoopProxy {
            display: Arc::downgrade(&self.display),
            pending_wakeup: Arc::downgrade(&self.pending_wakeup),
        }
    }

    pub fn poll_events<F>(&mut self, mut callback: F)
        where F: FnMut(::Event)
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
        self.evq.get_mut().dispatch_pending().expect("Wayland connection lost.");
        self.post_dispatch_triggers();

        // dispatch buffered events to client
        self.sink.lock().unwrap().empty_with(&mut callback);
    }

    pub fn run_forever<F>(&mut self, mut callback: F)
        where F: FnMut(::Event) -> ControlFlow,
    {
        // send pending events to the server
        self.display.flush().expect("Wayland connection lost.");

        // Check for control flow by wrapping the callback.
        let control_flow = ::std::cell::Cell::new(ControlFlow::Continue);
        let mut callback = |event| if let ControlFlow::Break = callback(event) {
            control_flow.set(ControlFlow::Break);
        };

        // dispatch any pre-buffered events
        self.post_dispatch_triggers();
        self.sink.lock().unwrap().empty_with(&mut callback);

        loop {
            // dispatch events blocking if needed
            self.evq.get_mut().dispatch().expect("Wayland connection lost.");
            self.post_dispatch_triggers();

            // empty buffer of events
            self.sink.lock().unwrap().empty_with(&mut callback);

            if let ControlFlow::Break = control_flow.get() {
                break;
            }
        }
    }

    pub fn get_primary_monitor(&self) -> MonitorId {
        let mut guard = self.evq.borrow_mut();
        let state = guard.state();
        let state_ctxt = state.get(&self.ctxt_token);
        if let Some(info) = state_ctxt.monitors.iter().next() {
            MonitorId {
                info: info.clone()
            }
        } else {
            panic!("No monitor is available.")
        }
    }

    pub fn get_available_monitors(&self) -> VecDeque<MonitorId> {
        let mut guard = self.evq.borrow_mut();
        let state = guard.state();
        let state_ctxt = state.get(&self.ctxt_token);
        state_ctxt.monitors.iter()
        .map(|m| MonitorId { info: m.clone() })
        .collect()
    }
}

/*
 * Private EventsLoop Internals
 */

wayland_env!(InnerEnv,
    compositor: wl_compositor::WlCompositor,
    shm: wl_shm::WlShm,
    subcompositor: wl_subcompositor::WlSubcompositor
);

pub struct StateContext {
    registry: wl_registry::WlRegistry,
    seat: Option<wl_seat::WlSeat>,
    shell: Option<Shell>,
    monitors: Vec<Arc<Mutex<OutputInfo>>>
}

impl StateContext {
    fn new(registry: wl_registry::WlRegistry) -> StateContext {
        StateContext {
            registry: registry,
            seat: None,
            shell: None,
            monitors: Vec::new()
        }
    }

    /// Ensures a shell is available
    ///
    /// If a shell is already bound, do nothing. Otherwise,
    /// try to bind wl_shell as a fallback. If this fails,
    /// panic, as this is a bug from the compositor.
    fn ensure_shell(&mut self, env: &mut EnvHandler<InnerEnv>) {
        if self.shell.is_some() {
            return;
        }
        // xdg_shell is not available, so initialize wl_shell
        for &(name, ref interface, _) in env.globals() {
            if interface == "wl_shell" {
                self.shell = Some(Shell::Wl(self.registry.bind::<wl_shell::WlShell>(1, name)));
                return;
            }
        }
        // This is a compositor bug, it _must_ at least support wl_shell
        panic!("Compositor didi not advertize xdg_shell not wl_shell.");
    }

    pub fn monitor_id_for(&self, output: &wl_output::WlOutput) -> MonitorId {
        for info in &self.monitors {
            let guard = info.lock().unwrap();
            if guard.output.equals(output) {
                return MonitorId {
                    info: info.clone()
                };
            }
        }
        panic!("Received an inexistent wl_output?!");
    }
}

impl EventsLoop {
    pub fn init_seat<F>(&mut self, f: F)
    where F: FnOnce(&mut EventQueueHandle, &wl_seat::WlSeat)
    {
        let mut guard = self.evq.borrow_mut();
        if guard.state().get(&self.ctxt_token).seat.is_some() {
            // seat has already been init
            return;
        }

        // clone the token to make borrow checker happy
        let ctxt_token = self.ctxt_token.clone();
        let seat = guard.state().with_value(&self.env_token, |proxy, env| {
            let ctxt = proxy.get(&ctxt_token);
            for &(name, ref interface, _) in env.globals() {
                if interface == wl_seat::WlSeat::interface_name() {
                    return Some(ctxt.registry.bind::<wl_seat::WlSeat>(5, name));
                }
            }
            None
        });

        if let Some(seat) = seat {
            f(&mut *guard, &seat);
            guard.state().get_mut(&self.ctxt_token).seat = Some(seat)
        }
    }

    fn post_dispatch_triggers(&mut self) {
        let mut sink = self.sink.lock().unwrap();
        let evq = self.evq.get_mut();
        // process a possible pending wakeup call
        if self.pending_wakeup.load(Ordering::Relaxed) {
            sink.send_raw_event(::Event::Awakened);
            self.pending_wakeup.store(false, Ordering::Relaxed);
        }
        // prune possible dead windows
        {
            let mut cleanup_needed = self.cleanup_needed.lock().unwrap();
            if *cleanup_needed {
                evq.state().get_mut(&self.store).cleanup();
                *cleanup_needed = false;
            }
        }
        // process pending resize/refresh
        evq.state().get_mut(&self.store).for_each(
            |newsize, refresh, frame_refresh, closed, wid, frame| {
                if let Some(frame) = frame {
                    if let Some((w, h)) = newsize {
                        frame.resize(w as i32, h as i32);
                        frame.refresh();
                        sink.send_event(::WindowEvent::Resized(w as u32, h as u32), wid);
                    } else if frame_refresh {
                        frame.refresh();
                    }
                }
                if refresh {
                    sink.send_event(::WindowEvent::Refresh, wid);
                }
                if closed {
                    sink.send_event(::WindowEvent::Closed, wid);
                }
            }
        )
    }

    /// Create a new window with given dimensions
    ///
    /// Grabs a lock on the event queue in the process
    pub fn create_window<ID: 'static, F>(&self, width: u32, height: u32, implem: FrameImplementation<ID>, idata: F)
        -> (wl_surface::WlSurface, Frame)
    where F: FnOnce(&wl_surface::WlSurface) -> ID
    {
        let (surface, frame) = {
            let mut guard = self.evq.borrow_mut();
            let env = guard.state().get(&self.env_token).clone_inner().unwrap();
            let shell = match guard.state().get(&self.ctxt_token).shell {
                Some(Shell::Wl(ref wl_shell)) => Shell::Wl(wl_shell.clone().unwrap()),
                Some(Shell::Xdg(ref xdg_shell)) => Shell::Xdg(xdg_shell.clone().unwrap()),
                None => unreachable!()
            };
            let seat = guard.state().get(&self.ctxt_token).seat.as_ref().and_then(|s| s.clone());
            let surface = env.compositor.create_surface();
            let frame = create_frame(
                &mut guard,
                implem,
                idata(&surface),
                &surface, width as i32, height as i32,
                &env.compositor,
                &env.subcompositor,
                &env.shm,
                &shell,
                seat
            ).expect("Failed to create a tmpfile buffer.");
            (surface, frame)
        };

        (surface, frame)
    }
}

/*
 * Wayland protocol implementations
 */

fn env_notify() -> EnvNotify<StateToken<StateContext>> {
    EnvNotify {
        new_global: |evqh, token, registry, id, interface, version| {
            use std::cmp::min;
            if interface == wl_output::WlOutput::interface_name() {
                // a new output is available
                let output = registry.bind::<wl_output::WlOutput>(min(version, 3), id);
                evqh.register(&output, output_impl(), token.clone());
                evqh.state().get_mut(&token).monitors.push(
                    Arc::new(Mutex::new(OutputInfo::new(output, id)))
                );
            } else if interface == zxdg_shell_v6::ZxdgShellV6::interface_name() {
                // We have an xdg_shell, bind it
                let xdg_shell = registry.bind::<zxdg_shell_v6::ZxdgShellV6>(1, id);
                evqh.register(&xdg_shell, xdg_ping_implementation(), ());
                evqh.state().get_mut(&token).shell = Some(Shell::Xdg(xdg_shell));
            }
        },
        del_global: |evqh, token, _, id| {
            // maybe this was a monitor, cleanup
            evqh.state().get_mut(&token).monitors.retain(
                |m| m.lock().unwrap().id != id
            );
        },
        ready: |_, _, _| {}
    }
}

fn xdg_ping_implementation() -> zxdg_shell_v6::Implementation<()> {
    zxdg_shell_v6::Implementation {
        ping: |_, _, shell, serial| {
            shell.pong(serial);
        }
    }
}

struct SeatIData {
    sink: Arc<Mutex<EventsLoopSink>>,
    pointer: Option<wl_pointer::WlPointer>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    touch: Option<wl_touch::WlTouch>,
    windows_token: StateToken<WindowStore>
}

fn seat_implementation() -> wl_seat::Implementation<SeatIData> {
    wl_seat::Implementation {
        name: |_, _, _, _| {},
        capabilities: |evqh, idata, seat, capabilities| {
            // create pointer if applicable
            if capabilities.contains(wl_seat::Capability::Pointer) && idata.pointer.is_none() {
                let pointer = seat.get_pointer().expect("Seat is not dead");
                let p_idata = super::pointer::PointerIData::new(
                    &idata.sink,
                    idata.windows_token.clone()
                );
                evqh.register(&pointer, super::pointer::pointer_implementation(), p_idata);
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
            // create touch if applicable
            if capabilities.contains(wl_seat::Capability::Touch) && idata.touch.is_none() {
                let touch = seat.get_touch().expect("Seat is not dead");
                let t_idata = super::touch::TouchIData::new(
                    &idata.sink,
                    idata.windows_token.clone()
                );
                evqh.register(&touch, super::touch::touch_implementation(), t_idata);
                idata.touch = Some(touch);
            }
            // destroy touch if applicable
            if !capabilities.contains(wl_seat::Capability::Touch) {
                if let Some(touch) = idata.touch.take() {
                    touch.release();
                }
            }
        }
    }
}

/*
 * Monitor stuff
 */

fn output_impl() -> wl_output::Implementation<StateToken<StateContext>> {
    wl_output::Implementation {
        geometry: |evqh, token, output, x, y, physical_width, physical_height, _, make, model, _| {
            let ctxt = evqh.state().get_mut(token);
            for info in &ctxt.monitors {
                let mut guard = info.lock().unwrap();
                if guard.output.equals(output) {
                    guard.pix_pos = (x, y);
                    guard.name = format!("{} - {}", make, model);
                    guard.extents_mm = (physical_width as u64, physical_height as u64);
                    return;
                }
            }
        },
        mode: |evqh, token, output, flags, w, h, _refresh| {
            if flags.contains(wl_output::Mode::Current) {
                let ctxt = evqh.state().get_mut(token);
                for info in &ctxt.monitors {
                    let mut guard = info.lock().unwrap();
                    if guard.output.equals(output) {
                        guard.pix_size = (w as u32, h as u32);
                        return;
                    }
                }
            }
        },
        done: |_, _, _| {},
        scale: |evqh, token, output, scale| {
            let ctxt = evqh.state().get_mut(token);
            for info in &ctxt.monitors {
                let mut guard = info.lock().unwrap();
                if guard.output.equals(output) {
                    guard.scale = scale as f32;
                    return;
                }
            }
        }
    }
}

pub struct OutputInfo {
    pub output: wl_output::WlOutput,
    pub id: u32,
    pub scale: f32,
    pub pix_size: (u32, u32),
    pub pix_pos: (i32, i32),
    pub name: String,
    pub extents_mm: (u64, u64),
}

impl OutputInfo {
    fn new(output: wl_output::WlOutput, id: u32) -> OutputInfo {
        OutputInfo {
            output: output,
            id: id,
            scale: 1.0,
            pix_size: (0, 0),
            pix_pos: (0, 0),
            name: "".into(),
            extents_mm: (0, 0),
        }
    }
}

#[derive(Clone)]
pub struct MonitorId {
    pub info: Arc<Mutex<OutputInfo>>
}

impl MonitorId {
    #[inline]
    pub fn get_name(&self) -> Option<String> {
        Some(self.info.lock().unwrap().name.clone())
    }

    #[inline]
    pub fn get_native_identifier(&self) -> u32 {
        self.info.lock().unwrap().id
    }

    #[inline]
    pub fn get_dimensions(&self) -> (u32, u32) {
        self.info.lock().unwrap().pix_size
    }

    #[inline]
    pub fn get_position(&self) -> (i32, i32) {
        self.info.lock().unwrap().pix_pos
    }

    #[inline]
    pub fn get_hidpi_factor(&self) -> f32 {
        self.info.lock().unwrap().scale
    }

    #[inline]
    pub fn get_physical_extents(&self) -> (u64, u64) {
        self.info.lock().unwrap().extents_mm
    }
}
