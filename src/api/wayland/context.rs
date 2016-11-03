use {Event, ElementState, MouseButton, MouseScrollDelta, TouchPhase};

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use wayland_client::{EnvHandler, default_connect, EventQueue, EventQueueHandle, Init, Proxy};
use wayland_client::protocol::{wl_compositor, wl_seat, wl_shell, wl_shm, wl_subcompositor,
                               wl_display, wl_registry, wl_output, wl_surface, wl_pointer,
                               wl_keyboard};

use super::wayland_window;
use super::wayland_kbd::MappedKeyboard;
use super::keyboard::KbdHandler;

/*
 * Registry and globals handling
 */

wayland_env!(InnerEnv,
    compositor: wl_compositor::WlCompositor,
    shell: wl_shell::WlShell,
    shm: wl_shm::WlShm,
    subcompositor: wl_subcompositor::WlSubcompositor
);

enum KbdType {
    Mapped(MappedKeyboard<KbdHandler>),
    Plain(Option<Arc<Mutex<VecDeque<Event>>>>)
}

struct WaylandEnv {
    registry: wl_registry::WlRegistry,
    inner: EnvHandler<InnerEnv>,
    monitors: Vec<OutputInfo>,
    my_id: usize,
    windows: Vec<(Arc<wl_surface::WlSurface>,Arc<Mutex<VecDeque<Event>>>)>,
    seat: Option<wl_seat::WlSeat>,
    mouse: Option<wl_pointer::WlPointer>,
    mouse_focus: Option<Arc<Mutex<VecDeque<Event>>>>,
    mouse_location: (i32, i32),
    axis_buffer: Option<(f32, f32)>,
    axis_discrete_buffer: Option<(i32, i32)>,
    axis_state: TouchPhase,
    kbd: Option<wl_keyboard::WlKeyboard>,
    kbd_handler: KbdType
}

struct OutputInfo {
    output: wl_output::WlOutput,
    id: u32,
    scale: f32,
    pix_size: (u32, u32),
    name: String
}

impl OutputInfo {
    fn new(output: wl_output::WlOutput, id: u32) -> OutputInfo {
        OutputInfo {
            output: output,
            id: id,
            scale: 1.0,
            pix_size: (0, 0),
            name: "".into()
        }
    }
}

impl WaylandEnv {
    fn new(registry: wl_registry::WlRegistry) -> WaylandEnv {
        let kbd_handler = match MappedKeyboard::new(KbdHandler::new()) {
            Ok(h) => KbdType::Mapped(h),
            Err(_) => KbdType::Plain(None)
        };
        WaylandEnv {
            registry: registry,
            inner: EnvHandler::new(),
            monitors: Vec::new(),
            my_id: 0,
            windows: Vec::new(),
            seat: None,
            mouse: None,
            mouse_focus: None,
            mouse_location: (0,0),
            axis_buffer: None,
            axis_discrete_buffer: None,
            axis_state: TouchPhase::Started,
            kbd: None,
            kbd_handler: kbd_handler
        }
    }

    fn get_seat(&self) -> Option<wl_seat::WlSeat> {
        for &(name, ref interface, version) in self.inner.globals() {
            if interface == "wl_seat" {
                // this "expect" cannot trigger (see https://github.com/vberger/wayland-client-rs/issues/69)
                let seat = self.registry.bind::<wl_seat::WlSeat>(5, name).expect("Seat cannot be destroyed");
                return Some(seat)
            }
        }
        None
    }
}

impl Init for WaylandEnv {
    fn init(&mut self, evqh: &mut EventQueueHandle, index: usize) {
        evqh.register::<_, WaylandEnv>(&self.registry, index);
        self.my_id = index
    }
}

impl wl_registry::Handler for WaylandEnv {
    fn global(&mut self,
              evqh: &mut EventQueueHandle,
              registry: &wl_registry::WlRegistry,
              name: u32,
              interface: String,
              version: u32)
    {
        if interface == "wl_output" {
            // intercept outputs
            // this "expect" cannot trigger (see https://github.com/vberger/wayland-client-rs/issues/69)
            let output = self.registry.bind::<wl_output::WlOutput>(1, name)
                             .expect("Registry cannot be dead");
            evqh.register::<_, WaylandEnv>(&output, self.my_id);
            self.monitors.push(OutputInfo::new(output, name));
        } else if interface == "wl_seat" && self.seat.is_none() {
            // Only grab the first seat
            // TODO: Handle multi-seat-setup?
            assert!(version >= 5, "Version 5 of seat interface is needed by glutin.");
            let seat = self.registry.bind::<wl_seat::WlSeat>(5, name)
                           .expect("Registry cannot be dead");
            evqh.register::<_, WaylandEnv>(&seat, self.my_id);
            self.seat = Some(seat);
        }
        self.inner.global(evqh, registry, name, interface, version);
    }

    fn global_remove(&mut self,
                     evqh: &mut EventQueueHandle,
                     registry: &wl_registry::WlRegistry,
                     name: u32)
    {
        // prune old monitors
        self.monitors.retain(|m| m.id != name);
        self.inner.global_remove(evqh, registry, name);
    }
}

declare_handler!(WaylandEnv, wl_registry::Handler, wl_registry::WlRegistry);

impl wl_output::Handler for WaylandEnv {
    fn geometry(&mut self,
                _: &mut EventQueueHandle,
                proxy: &wl_output::WlOutput,
                _x: i32, _y: i32,
                _physical_width: i32, _physical_height: i32,
                _subpixel: wl_output::Subpixel,
                make: String, model: String,
                _transform: wl_output::Transform)
    {
        for m in self.monitors.iter_mut().filter(|m| m.output.equals(proxy)) {
            m.name = format!("{} ({})", model, make);
            break;
        }
    }
    fn mode(&mut self,
            _: &mut EventQueueHandle,
            proxy: &wl_output::WlOutput,
            flags: wl_output::Mode,
            width: i32, height: i32,
            _refresh: i32)
    {
        if flags.contains(wl_output::Current) {
            for m in self.monitors.iter_mut().filter(|m| m.output.equals(proxy)) {
                m.pix_size = (width as u32, height as u32);
                break;
            }
        }
    }
    fn scale(&mut self,
             _: &mut EventQueueHandle,
             proxy: &wl_output::WlOutput,
             factor: i32)
    {
        for m in self.monitors.iter_mut().filter(|m| m.output.equals(proxy)) {
            m.scale = factor as f32;
            break;
        }
    }
}

declare_handler!(WaylandEnv, wl_output::Handler, wl_output::WlOutput);

/*
 * Main context struct
 */

pub struct WaylandContext {
    pub display: wl_display::WlDisplay,
    evq: Mutex<EventQueue>,
    env_id: usize,
}

impl WaylandContext {
    pub fn init() -> Option<WaylandContext> {
        // attempt to connect to the wayland server
        // this handles both "no libwayland" and "no compositor" cases
        let (display, mut event_queue) = match default_connect() {
            Ok(ret) => ret,
            Err(e) => return None
        };

        // this "expect" cannot trigger (see https://github.com/vberger/wayland-client-rs/issues/69)
        let registry = display.get_registry().expect("Display cannot be already destroyed.");
        let env_id = event_queue.add_handler_with_init(WaylandEnv::new(registry));
        // two syncs fully initialize
        event_queue.sync_roundtrip().expect("Wayland connection unexpectedly lost");
        event_queue.sync_roundtrip().expect("Wayland connection unexpectedly lost");

        Some(WaylandContext {
            evq: Mutex::new(event_queue),
            display: display,
            env_id: env_id
        })
    }

    pub fn dispatch_pending(&self) {
        let mut guard = self.evq.lock().unwrap();
        guard.dispatch_pending().expect("Wayland connection unexpectedly lost");
    }

    pub fn dispatch(&self) {
        let mut guard = self.evq.lock().unwrap();
        guard.dispatch().expect("Wayland connection unexpectedly lost");
    }

    pub fn flush(&self) {
        self.display.flush();
    }

    pub fn with_output<F>(&self, id: MonitorId, f: F) where F: FnOnce(&wl_output::WlOutput) {
        let mut guard = self.evq.lock().unwrap();
        let state = guard.state();
        let env = state.get_handler::<WaylandEnv>(self.env_id);
        for m in env.monitors.iter().filter(|m| m.id == id.id) {
            f(&m.output);
            break
        }
    }

    pub fn create_window<H: wayland_window::Handler>(&self)
        -> (Arc<wl_surface::WlSurface>, Arc<Mutex<VecDeque<Event>>>, wayland_window::DecoratedSurface<H>)
    {
        let mut guard = self.evq.lock().unwrap();
        let mut state = guard.state();
        let env = state.get_mut_handler::<WaylandEnv>(self.env_id);
        // this "expect" cannot trigger (see https://github.com/vberger/wayland-client-rs/issues/69)
        let surface = Arc::new(env.inner.compositor.create_surface().expect("Compositor cannot be dead"));
        let eventiter = Arc::new(Mutex::new(VecDeque::new()));
        env.windows.push((surface.clone(), eventiter.clone()));
        let decorated = wayland_window::DecoratedSurface::new(
            &*surface, 800, 600,
            &env.inner.compositor,
            &env.inner.subcompositor,
            &env.inner.shm,
            &env.inner.shell,
            env.get_seat(),
            false
        ).expect("Failed to create a tmpfile buffer.");
        (surface, eventiter, decorated)
    }

    pub fn prune_dead_windows(&self) {
        let mut guard = self.evq.lock().unwrap();
        let mut state = guard.state();
        let env = state.get_mut_handler::<WaylandEnv>(self.env_id);
        env.windows.retain(|w| w.0.is_alive());
    }
}

/*
 * Monitors API
 */

pub fn get_primary_monitor(ctxt: &Arc<WaylandContext>) -> MonitorId {
    let mut guard = ctxt.evq.lock().unwrap();
    let state = guard.state();
    let env = state.get_handler::<WaylandEnv>(ctxt.env_id);
    if let Some(ref monitor) = env.monitors.iter().next() {
        MonitorId {
            id: monitor.id,
            ctxt: ctxt.clone()
        }
    } else {
        panic!("No monitor is available.")
    }
}

pub fn get_available_monitors(ctxt: &Arc<WaylandContext>) -> VecDeque<MonitorId> {
    let mut guard = ctxt.evq.lock().unwrap();
    let state = guard.state();
    let env = state.get_handler::<WaylandEnv>(ctxt.env_id);
    env.monitors.iter()
       .map(|m| MonitorId { id: m.id, ctxt: ctxt.clone() })
       .collect()
}

#[derive(Clone)]
pub struct MonitorId {
    id: u32,
    ctxt: Arc<WaylandContext>
}

impl MonitorId {
    pub fn get_name(&self) -> Option<String> {
        let mut guard = self.ctxt.evq.lock().unwrap();
        let state = guard.state();
        let env = state.get_handler::<WaylandEnv>(self.ctxt.env_id);
        for m in env.monitors.iter().filter(|m| m.id == self.id) {
            return Some(m.name.clone())
        }
        // if we reach here, this monitor does not exist any more
        None
    }

    #[inline]
    pub fn get_native_identifier(&self) -> ::native_monitor::NativeMonitorId {
        ::native_monitor::NativeMonitorId::Unavailable
    }

    pub fn get_dimensions(&self) -> (u32, u32) {
        let mut guard = self.ctxt.evq.lock().unwrap();
        let state = guard.state();
        let env = state.get_handler::<WaylandEnv>(self.ctxt.env_id);
        for m in env.monitors.iter().filter(|m| m.id == self.id) {
            return m.pix_size
        }
        // if we reach here, this monitor does not exist any more
        (0,0)
    }
}

/*
 * Input Handling
 */

impl wl_seat::Handler for WaylandEnv {
    fn capabilities(&mut self,
                    evqh: &mut EventQueueHandle,
                    seat: &wl_seat::WlSeat,
                    capabilities: wl_seat::Capability)
    {
        // create pointer if applicable
        if capabilities.contains(wl_seat::Pointer) && self.mouse.is_none() {
            let pointer = seat.get_pointer().expect("Seat is not dead");
            evqh.register::<_, WaylandEnv>(&pointer, self.my_id);
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
            evqh.register::<_, WaylandEnv>(&kbd, self.my_id);
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

declare_handler!(WaylandEnv, wl_seat::Handler, wl_seat::WlSeat);

/*
 * Pointer Handling
 */

impl wl_pointer::Handler for WaylandEnv {
    fn enter(&mut self,
             _evqh: &mut EventQueueHandle,
             _proxy: &wl_pointer::WlPointer,
             _serial: u32,
             surface: &wl_surface::WlSurface,
             surface_x: f64,
             surface_y: f64)
    {
        self.mouse_location = (surface_x as i32, surface_y as i32);
        for &(ref window, ref eviter) in &self.windows {
            if window.equals(surface) {
                self.mouse_focus = Some(eviter.clone());
                let (w, h) = self.mouse_location;
                let mut event_queue = eviter.lock().unwrap();
                event_queue.push_back(Event::MouseEntered);
                event_queue.push_back(Event::MouseMoved(w, h));
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
        for &(ref window, ref eviter) in &self.windows {
            if window.equals(surface) {
                let mut event_queue = eviter.lock().unwrap();
                event_queue.push_back(Event::MouseLeft);
                break;
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
        self.mouse_location = (surface_x as i32, surface_y as i32);
        if let Some(ref eviter) = self.mouse_focus {
            let (w,h) = self.mouse_location;
            eviter.lock().unwrap().push_back(
                Event::MouseMoved(w, h)
            );
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
        if let Some(ref eviter) = self.mouse_focus {
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
            eviter.lock().unwrap().push_back(
                Event::MouseInput(state, button)
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
            wl_pointer::Axis::VerticalScroll => y += value as f32,
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
        if let Some(ref eviter) = self.mouse_focus {
            if let Some((x, y)) = axis_discrete_buffer {
                eviter.lock().unwrap().push_back(
                    Event::MouseWheel(
                        MouseScrollDelta::LineDelta(x as f32, y as f32),
                        self.axis_state
                    )
                );
            } else if let Some((x, y)) = axis_buffer {
                eviter.lock().unwrap().push_back(
                    Event::MouseWheel(
                        MouseScrollDelta::PixelDelta(x as f32, y as f32),
                        self.axis_state
                    )
                );
            }
        }
    }

    fn axis_source(&mut self,
                   _evqh: &mut EventQueueHandle,
                   _proxy: &wl_pointer::WlPointer,
                   axis_source: wl_pointer::AxisSource)
    {
    }

    fn axis_stop(&mut self,
                 _evqh: &mut EventQueueHandle,
                 _proxy: &wl_pointer::WlPointer,
                 _time: u32,
                 axis: wl_pointer::Axis)
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
            wl_pointer::Axis::VerticalScroll => y += discrete,
            wl_pointer::Axis::HorizontalScroll => x += discrete
        }
        self.axis_discrete_buffer = Some((x,y));
                self.axis_state = match self.axis_state {
            TouchPhase::Started | TouchPhase::Moved => TouchPhase::Moved,
            _ => TouchPhase::Started
        }
    }
}

declare_handler!(WaylandEnv, wl_pointer::Handler, wl_pointer::WlPointer);

/*
 * Keyboard Handling
 */

impl wl_keyboard::Handler for WaylandEnv {
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
        let mut opt_eviter = None;
        for &(ref window, ref eviter) in &self.windows {
            if window.equals(surface) {
                opt_eviter = Some(eviter.clone());
                break;
            }
        }
        if let Some(ref eviter) = opt_eviter {
            // send focused event
            let mut guard = eviter.lock().unwrap();
            guard.push_back(Event::Focused(true));
        }
        match self.kbd_handler {
            KbdType::Mapped(ref mut h) => {
                h.handler().target = opt_eviter;
                h.enter(evqh, proxy, serial, surface, keys);
            },
            KbdType::Plain(ref mut opt) => { *opt = opt_eviter; }
        }
    }

    fn leave(&mut self,
             evqh: &mut EventQueueHandle,
             proxy: &wl_keyboard::WlKeyboard,
             serial: u32,
             surface: &wl_surface::WlSurface)
    {
        let opt_eviter = match self.kbd_handler {
            KbdType::Mapped(ref mut h) => {
                let eviter = h.handler().target.take();
                h.leave(evqh, proxy, serial, surface);
                eviter
            },
            KbdType::Plain(ref mut opt) => opt.take()
        };
        if let Some(eviter) = opt_eviter {
            let mut guard = eviter.lock().unwrap();
            guard.push_back(Event::Focused(false));
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
            KbdType::Plain(Some(ref eviter)) => {
                let state = match state {
                    wl_keyboard::KeyState::Pressed => ElementState::Pressed,
                    wl_keyboard::KeyState::Released => ElementState::Released,
                };
                let mut guard = eviter.lock().unwrap();
                guard.push_back(Event::KeyboardInput(
                    state,
                    key as u8,
                    None
                ));
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

declare_handler!(WaylandEnv, wl_keyboard::Handler, wl_keyboard::WlKeyboard);
