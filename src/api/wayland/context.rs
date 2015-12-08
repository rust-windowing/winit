use wayland_client::EventIterator;
use wayland_client::wayland::get_display;
use wayland_client::wayland::compositor::WlCompositor;
use wayland_client::wayland::seat::WlSeat;
use wayland_client::wayland::shell::WlShell;
use wayland_client::wayland::shm::WlShm;
use wayland_client::wayland::subcompositor::WlSubcompositor;

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

pub struct WaylandContext {
    inner: InnerEnv,
    iterator: EventIterator
}

impl WaylandContext {
    fn init() -> Option<WaylandContext> {
        let display = match get_display() {
            Some(display) => display,
            None => return None
        };

        let (inner_env, iterator) = InnerEnv::init(display);

        Some(WaylandContext {
            inner: inner_env,
            iterator: iterator
        })
    }
}