use Event as GlutinEvent;

use std::collections::{HashMap, VecDeque, HashSet};
use std::sync::{Arc, Mutex};

use libc::c_void;

use wayland_client::{EventIterator, Proxy, ProxyId};
use wayland_client::wayland::get_display;
use wayland_client::wayland::compositor::{WlCompositor, WlSurface};
use wayland_client::wayland::output::WlOutput;
use wayland_client::wayland::seat::{WlSeat, WlPointer};
use wayland_client::wayland::shell::{WlShell, WlShellSurface};
use wayland_client::wayland::shm::WlShm;
use wayland_client::wayland::subcompositor::WlSubcompositor;

use super::wayland_kbd::MappedKeyboard;
use super::wayland_window::DecoratedSurface;

lazy_static! {
    pub static ref WAYLAND_CONTEXT: Option<WaylandContext> = {
        WaylandContext::init()
    };
}

wayland_env!(InnerEnv,
    compositor: WlCompositor,
    seat: WlSeat,
    shell: WlShell,
    shm: WlShm,
    subcompositor: WlSubcompositor
);

pub struct WaylandFocuses {
    pub pointer: Option<WlPointer>,
    pub pointer_on: Option<ProxyId>,
    pub pointer_at: Option<(f64, f64)>,
    pub keyboard: Option<MappedKeyboard>,
    pub keyboard_on: Option<ProxyId>
}

pub struct WaylandContext {
    inner: InnerEnv,
    iterator: Mutex<EventIterator>,
    monitors: Vec<(WlOutput, u32, u32, String)>,
    queues: Mutex<HashMap<ProxyId, Arc<Mutex<VecDeque<GlutinEvent>>>>>,
    known_surfaces: Mutex<HashSet<ProxyId>>,
    focuses: Mutex<WaylandFocuses>
}

impl WaylandContext {
    fn init() -> Option<WaylandContext> {
        let display = match get_display() {
            Some(display) => display,
            None => return None
        };

        let (mut inner_env, iterator) = InnerEnv::init(display);

        let outputs_events = EventIterator::new();

        let mut monitors = inner_env.globals.iter()
            .flat_map(|&(id, _, _)| inner_env.rebind_id::<WlOutput>(id))
            .map(|(mut monitor, _)| {
                monitor.set_evt_iterator(&outputs_events);
                (monitor, 0, 0, String::new())
            }).collect();

        inner_env.display.sync_roundtrip().unwrap();

        super::monitor::init_monitors(&mut monitors, outputs_events);

        Some(WaylandContext {
            inner: inner_env,
            iterator: Mutex::new(iterator),
            monitors: monitors,
            queues: Mutex::new(HashMap::new()),
            known_surfaces: Mutex::new(HashSet::new()),
            focuses: Mutex::new(WaylandFocuses {
                pointer: None,
                pointer_on: None,
                pointer_at: None,
                keyboard: None,
                keyboard_on: None
            })
        })
    }

    pub fn new_surface(&self) -> Option<(WlSurface, Arc<Mutex<VecDeque<GlutinEvent>>>)> {
        self.inner.compositor.as_ref().map(|c| {
            let s = c.0.create_surface();
            let id = s.id();
            let queue = {
                let mut q = VecDeque::new();
                q.push_back(GlutinEvent::Refresh);
                Arc::new(Mutex::new(q))
            };
            self.queues.lock().unwrap().insert(id, queue.clone());
            self.known_surfaces.lock().unwrap().insert(id);
            (s, queue)
        })
    }

    pub fn dropped_surface(&self, id: ProxyId) {
        self.queues.lock().unwrap().remove(&id);
        self.known_surfaces.lock().unwrap().remove(&id);
    }

    pub fn decorated_from(&self, surface: &WlSurface, width: i32, height: i32) -> Option<DecoratedSurface> {
        let inner = &self.inner;
        match (&inner.compositor, &inner.subcompositor, &inner.shm, &inner.shell) {
            (&Some(ref compositor), &Some(ref subcompositor), &Some(ref shm), &Some(ref shell)) => {
                DecoratedSurface::new(
                    surface, width, height,
                    &compositor.0, &subcompositor.0, &shm.0, &shell.0,
                    self.inner.rebind::<WlSeat>().map(|(seat, _)| seat)
                ).ok()
            }
            _ => None
        }
    }

    pub fn plain_from(&self, surface: &WlSurface, fullscreen: Option<ProxyId>) -> Option<WlShellSurface> {
        use wayland_client::wayland::shell::WlShellSurfaceFullscreenMethod;

        let inner = &self.inner;
        if let Some((ref shell, _)) = inner.shell {
            let shell_surface = shell.get_shell_surface(surface);
            if let Some(monitor_id) = fullscreen {
                for m in &self.monitors {
                    if m.0.id() == monitor_id {
                        shell_surface.set_fullscreen(
                            WlShellSurfaceFullscreenMethod::Default,
                            0,
                            Some(&m.0)
                        );
                        return Some(shell_surface)
                    }
                }
            }
            shell_surface.set_toplevel();
            Some(shell_surface)
        } else {
            None
        }
    }

    pub fn display_ptr(&self) -> *const c_void {
        self.inner.display.ptr() as *const _
    }

    pub fn dispatch_events(&self) {
        self.inner.display.dispatch_pending().unwrap();
        let mut iterator = self.iterator.lock().unwrap();
        let mut focuses = self.focuses.lock().unwrap();
        let known_surfaces = self.known_surfaces.lock().unwrap();
        let queues = self.queues.lock().unwrap();
        // first, keyboard events
        let kdb_evts = super::keyboard::translate_kbd_events(&mut *focuses, &known_surfaces);
        for (evt, id) in kdb_evts {
            if let Some(q) = queues.get(&id) {
                q.lock().unwrap().push_back(evt);
            }
        }
        // then, the rest
        for evt in &mut *iterator {
            if let Some((evt, id)) = super::events::translate_event(
                evt, &mut *focuses, &known_surfaces,
                self.inner.seat.as_ref().map(|s| &s.0))
            {
                if let Some(q) = queues.get(&id) {
                    q.lock().unwrap().push_back(evt);
                }
            }
        }
    }

    pub fn flush_events(&self) -> ::std::io::Result<i32> {
        self.inner.display.flush()
    }

    pub fn read_events(&self) -> ::std::io::Result<Option<i32>> {
        let guard = match self.inner.display.prepare_read() {
            Some(g) => g,
            None => return Ok(None)
        };
        return guard.read_events().map(|i| Some(i));
    }

    pub fn monitor_ids(&self) -> Vec<ProxyId> {
        self.monitors.iter().map(|o| o.0.id()).collect()
    }

    pub fn monitor_name(&self, pid: ProxyId) -> Option<String> {
        for o in &self.monitors {
            if o.0.id() == pid {
                return Some(o.3.clone())
            }
        }
        None
    }

    pub fn monitor_dimensions(&self, pid: ProxyId) -> Option<(u32, u32)> {
        for o in &self.monitors {
            if o.0.id() == pid {
                return Some((o.1, o.2))
            }
        }
        None
    }
}
