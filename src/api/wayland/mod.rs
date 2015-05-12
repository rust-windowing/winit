#![cfg(target_os = "linux")]
#![allow(unused_variables, dead_code)]

use self::wayland::egl::{EGLSurface, is_egl_available};
use self::wayland::core::{Display, Registry, Compositor, Shell, ShellSurface,
                          Seat, Pointer, default_display, WSurface, SurfaceId,
                          Surface, Output};

use libc;
use api::dlopen;
use api::egl::Context as EglContext;

use BuilderAttribs;
use CreationError;
use Event;
use PixelFormat;
use CursorState;
use MouseCursor;
use GlContext;

use std::collections::{VecDeque, HashMap};
use std::sync::{Arc, Mutex};
use std::ffi::CString;

extern crate wayland_client as wayland;

struct WaylandContext {
    pub display: Display,
    pub registry: Registry,
    pub compositor: Compositor,
    pub shell: Shell,
    pub seat: Seat,
    pub pointer: Option<Pointer<WSurface>>,
    windows_event_queues: Arc<Mutex<HashMap<SurfaceId, Arc<Mutex<VecDeque<Event>>>>>>,
    current_pointer_surface: Arc<Mutex<Option<SurfaceId>>>,
    outputs: Vec<Arc<Output>>
}

impl WaylandContext {
    pub fn new() -> Option<WaylandContext> {
        let display = match default_display() {
            Some(d) => d,
            None => return None,
        };
        let registry = display.get_registry();
        // let the registry get its events
        display.sync_roundtrip();
        let compositor = match registry.get_compositor() {
            Some(c) => c,
            None => return None,
        };
        let shell = match registry.get_shell() {
            Some(s) => s,
            None => return None,
        };
        let seat = match registry.get_seats().into_iter().next() {
            Some(s) => s,
            None => return None,
        };
        let outputs = registry.get_outputs().into_iter().map(Arc::new).collect::<Vec<_>>();
        // let the other globals get their events
        display.sync_roundtrip();

        let current_pointer_surface = Arc::new(Mutex::new(None));

        // rustc has trouble finding the correct type here, so we explicit it.
        let windows_event_queues = Arc::new(Mutex::new(
            HashMap::<SurfaceId, Arc<Mutex<VecDeque<Event>>>>::new()
        ));

        // handle inputs
        let mut pointer = seat.get_pointer();
        if let Some(ref mut p) = pointer {
            // set the enter/leave callbacks
            let current_surface = current_pointer_surface.clone();
            p.set_enter_action(move |_, sid, x, y| {
                *current_surface.lock().unwrap() = Some(sid);
            });
            let current_surface = current_pointer_surface.clone();
            p.set_leave_action(move |_, sid| {
                *current_surface.lock().unwrap() = None;
            });
            // set the events callbacks
            let current_surface = current_pointer_surface.clone();
            let event_queues = windows_event_queues.clone();
            p.set_motion_action(move |_, _, x, y| {
                // dispatch to the appropriate queue
                let sid = *current_surface.lock().unwrap();
                if let Some(sid) = sid {
                    let map = event_queues.lock().unwrap();
                    if let Some(queue) = map.get(&sid) {
                        queue.lock().unwrap().push_back(Event::Moved(x as i32,y as i32))
                    }
                }
            });
            let current_surface = current_pointer_surface.clone();
            let event_queues = windows_event_queues.clone();
            p.set_button_action(move |_, sid, b, s| {
                use self::wayland::core::ButtonState;
                use MouseButton;
                use ElementState;
                let button = match b {
                    0x110 => MouseButton::Left,
                    0x111 => MouseButton::Right,
                    0x112 => MouseButton::Middle,
                    _ => return
                };
                let state = match s {
                    ButtonState::WL_POINTER_BUTTON_STATE_RELEASED => ElementState::Released,
                    ButtonState::WL_POINTER_BUTTON_STATE_PRESSED => ElementState::Pressed
                };
                // dispatch to the appropriate queue
                let sid = *current_surface.lock().unwrap();
                if let Some(sid) = sid {
                    let map = event_queues.lock().unwrap();
                    if let Some(queue) = map.get(&sid) {
                        queue.lock().unwrap().push_back(Event::MouseInput(state, button))
                    }
                }
            });
        }
        Some(WaylandContext {
            display: display,
            registry: registry,
            compositor: compositor,
            shell: shell,
            seat: seat,
            pointer: pointer,
            windows_event_queues: windows_event_queues,
            current_pointer_surface: current_pointer_surface,
            outputs: outputs
        })
    }

    fn register_surface(&self, sid: SurfaceId, queue: Arc<Mutex<VecDeque<Event>>>) {
        self.windows_event_queues.lock().unwrap().insert(sid, queue);
    }

    fn deregister_surface(&self, sid: SurfaceId) {
        self.windows_event_queues.lock().unwrap().remove(&sid);
    }
}

lazy_static! {
    static ref WAYLAND_CONTEXT: Option<WaylandContext> = {
        WaylandContext::new()
    };
}

pub fn is_available() -> bool {
    WAYLAND_CONTEXT.is_some()
}

pub struct Window {
    shell_surface: ShellSurface<EGLSurface>,
    pending_events: Arc<Mutex<VecDeque<Event>>>,
    pub context: EglContext,
}

// It is okay, as the window is completely self-owned: it has its
// own wayland connexion.
unsafe impl Send for Window {}

#[derive(Clone)]
pub struct WindowProxy;

impl WindowProxy {
    pub fn wakeup_event_loop(&self) {
        if let Some(ref ctxt) = *WAYLAND_CONTEXT {
            ctxt.display.sync();
        }
    }
}

pub struct MonitorID {
    output: Arc<Output>
}

pub fn get_available_monitors() -> VecDeque<MonitorID> {
    WAYLAND_CONTEXT.as_ref().unwrap().outputs.iter().map(|o| MonitorID::new(o.clone())).collect()
}
pub fn get_primary_monitor() -> MonitorID {
    match WAYLAND_CONTEXT.as_ref().unwrap().outputs.iter().next() {
        Some(o) => MonitorID::new(o.clone()),
        None => panic!("No monitor is available.")
    }
}

impl MonitorID {
    fn new(output: Arc<Output>) -> MonitorID {
        MonitorID {
            output: output
        }
    }

    pub fn get_name(&self) -> Option<String> {
        Some(format!("{} - {}", self.output.manufacturer(), self.output.model()))
    }

    pub fn get_native_identifier(&self) -> ::native_monitor::NativeMonitorId {
        ::native_monitor::NativeMonitorId::Unavailable
    }

    pub fn get_dimensions(&self) -> (u32, u32) {
        let (w, h) = self.output.dimensions();
        (w as u32, h as u32)
    }
}


pub struct PollEventsIterator<'a> {
    window: &'a Window,
}

impl<'a> Iterator for PollEventsIterator<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        if let Some(ref ctxt) = *WAYLAND_CONTEXT {
            ctxt.display.dispatch_pending();
        }
        self.window.pending_events.lock().unwrap().pop_front()
    }
}

pub struct WaitEventsIterator<'a> {
    window: &'a Window,
}

impl<'a> Iterator for WaitEventsIterator<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        if let Some(ref ctxt) = *WAYLAND_CONTEXT {
            ctxt.display.dispatch();
        }
        self.window.pending_events.lock().unwrap().pop_front()
    }
}

impl Window {
    pub fn new(builder: BuilderAttribs) -> Result<Window, CreationError> {
        use self::wayland::internals::FFI;

        let wayland_context = match *WAYLAND_CONTEXT {
            Some(ref c) => c,
            None => return Err(CreationError::NotSupported),
        };

        if !is_egl_available() { return Err(CreationError::NotSupported) }

        let (w, h) = builder.dimensions.unwrap_or((800, 600));

        let surface = EGLSurface::new(
            wayland_context.compositor.create_surface(),
            w as i32,
            h as i32
        );

        let context = {
            let libegl = unsafe { dlopen::dlopen(b"libEGL.so\0".as_ptr() as *const _, dlopen::RTLD_NOW) };
            if libegl.is_null() {
                return Err(CreationError::NotSupported);
            }
            let egl = ::api::egl::ffi::egl::Egl::load_with(|sym| {
                let sym = CString::new(sym).unwrap();
                unsafe { dlopen::dlsym(libegl, sym.as_ptr()) }
            });
            try!(EglContext::new(
                egl,
                builder,
                Some(wayland_context.display.ptr() as *const _),
                surface.ptr() as *const _
            ))
        };

        let shell_surface = wayland_context.shell.get_shell_surface(surface);
        shell_surface.set_toplevel();
        let events = Arc::new(Mutex::new(VecDeque::new()));

        wayland_context.register_surface(shell_surface.get_wsurface().get_id(), events.clone());

        wayland_context.display.flush().unwrap();

        Ok(Window {
            shell_surface: shell_surface,
            pending_events: events,
            context: context
        })
    }

    pub fn is_closed(&self) -> bool {
        false
    }

    pub fn set_title(&self, title: &str) {
    }

    pub fn show(&self) {
    }

    pub fn hide(&self) {
    }

    pub fn get_position(&self) -> Option<(i32, i32)> {
        // not available with wayland
        None
    }

    pub fn set_position(&self, _x: i32, _y: i32) {
        // not available with wayland
    }

    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        unimplemented!()
    }

    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        unimplemented!()
    }

    pub fn set_inner_size(&self, _x: u32, _y: u32) {
        unimplemented!()
    }

    pub fn create_window_proxy(&self) -> WindowProxy {
        WindowProxy
    }

    pub fn poll_events(&self) -> PollEventsIterator {
        PollEventsIterator {
            window: self
        }
    }

    pub fn wait_events(&self) -> WaitEventsIterator {
        WaitEventsIterator {
            window: self
        }
    }

    pub fn set_window_resize_callback(&mut self, _: Option<fn(u32, u32)>) {
    }

    pub fn set_cursor(&self, cursor: MouseCursor) {
    }

    pub fn set_cursor_state(&self, state: CursorState) -> Result<(), String> {
        Ok(())
    }

    pub fn hidpi_factor(&self) -> f32 {
        1.0
    }

    pub fn set_cursor_position(&self, x: i32, y: i32) -> Result<(), ()> {
        Ok(())
    }

    pub fn platform_display(&self) -> *mut libc::c_void {
        unimplemented!()
    }

    pub fn platform_window(&self) -> *mut libc::c_void {
        unimplemented!()
    }
}

impl GlContext for Window {

    unsafe fn make_current(&self) {
        self.context.make_current()
    }

    fn is_current(&self) -> bool {
        self.context.is_current()
    }

    fn get_proc_address(&self, addr: &str) -> *const libc::c_void {
        self.context.get_proc_address(addr)
    }

    fn swap_buffers(&self) {
        self.context.swap_buffers()
    }

    fn get_api(&self) -> ::Api {
        self.context.get_api()
    }

    fn get_pixel_format(&self) -> PixelFormat {
        self.context.get_pixel_format().clone()
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        if let Some(ref ctxt) = *WAYLAND_CONTEXT {
            ctxt.deregister_surface(self.shell_surface.get_wsurface().get_id())
        }
    }
}
