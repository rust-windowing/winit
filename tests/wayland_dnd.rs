//! Protocol-level regression tests using an isolated Wayland server, never the user's display.
#![cfg(all(target_os = "linux", feature = "wayland-portal"))]
use std::io::Write;
use std::sync::Arc;
use wayland_protocols::xdg::shell::server::{
    xdg_positioner, xdg_surface, xdg_toplevel, xdg_wm_base,
};
use wayland_server::protocol::{
    wl_buffer, wl_compositor, wl_data_device, wl_data_device_manager, wl_data_offer,
    wl_data_source, wl_pointer, wl_region, wl_seat, wl_shm, wl_shm_pool, wl_surface,
};
use wayland_server::{
    Client, DataInit, Dispatch, Display, DisplayHandle, GlobalDispatch, New, Resource,
};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

#[derive(Default)]
struct State {
    surface: Option<wl_surface::WlSurface>,
    device: Option<wl_data_device::WlDataDevice>,
    offered: bool,
    dropped: bool,
    bytes: Vec<u8>,
    finished: bool,
    finish_count: usize,
    receives: usize,
    case: String,
    held: Option<std::fs::File>,
}
macro_rules! global {
    ($t:ty) => {
        impl GlobalDispatch<$t, ()> for State {
            fn bind(
                _: &mut Self,
                _: &DisplayHandle,
                _: &Client,
                r: New<$t>,
                _: &(),
                i: &mut DataInit<'_, Self>,
            ) {
                i.init(r, ());
            }
        }
    };
}
global!(wl_compositor::WlCompositor);
global!(xdg_wm_base::XdgWmBase);
global!(wl_data_device_manager::WlDataDeviceManager);
impl GlobalDispatch<wl_shm::WlShm, ()> for State {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        r: New<wl_shm::WlShm>,
        _: &(),
        i: &mut DataInit<'_, Self>,
    ) {
        let r = i.init(r, ());
        r.format(wl_shm::Format::Argb8888);
        r.format(wl_shm::Format::Xrgb8888);
    }
}
impl GlobalDispatch<wl_seat::WlSeat, ()> for State {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        r: New<wl_seat::WlSeat>,
        _: &(),
        i: &mut DataInit<'_, Self>,
    ) {
        let r = i.init(r, ());
        r.capabilities(wl_seat::Capability::Pointer);
        r.name("test-seat".into());
    }
}
impl Dispatch<wl_compositor::WlCompositor, ()> for State {
    fn request(
        s: &mut Self,
        _: &Client,
        _: &wl_compositor::WlCompositor,
        r: wl_compositor::Request,
        _: &(),
        _: &DisplayHandle,
        i: &mut DataInit<'_, Self>,
    ) {
        match r {
            wl_compositor::Request::CreateSurface { id } => {
                let surface = i.init(id, ());
                if s.surface.is_none() {
                    s.surface = Some(surface);
                }
            },
            wl_compositor::Request::CreateRegion { id } => {
                i.init(id, ());
            },
            _ => {},
        }
    }
}
impl Dispatch<wl_seat::WlSeat, ()> for State {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &wl_seat::WlSeat,
        r: wl_seat::Request,
        _: &(),
        _: &DisplayHandle,
        i: &mut DataInit<'_, Self>,
    ) {
        if let wl_seat::Request::GetPointer { id } = r {
            i.init(id, ());
        }
    }
}
impl Dispatch<wl_shm::WlShm, ()> for State {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &wl_shm::WlShm,
        r: wl_shm::Request,
        _: &(),
        _: &DisplayHandle,
        i: &mut DataInit<'_, Self>,
    ) {
        if let wl_shm::Request::CreatePool { id, .. } = r {
            i.init(id, ());
        }
    }
}
impl Dispatch<wl_shm_pool::WlShmPool, ()> for State {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &wl_shm_pool::WlShmPool,
        r: wl_shm_pool::Request,
        _: &(),
        _: &DisplayHandle,
        i: &mut DataInit<'_, Self>,
    ) {
        if let wl_shm_pool::Request::CreateBuffer { id, .. } = r {
            i.init(id, ());
        }
    }
}
impl Dispatch<xdg_wm_base::XdgWmBase, ()> for State {
    fn request(
        s: &mut Self,
        _: &Client,
        _: &xdg_wm_base::XdgWmBase,
        r: xdg_wm_base::Request,
        _: &(),
        _: &DisplayHandle,
        i: &mut DataInit<'_, Self>,
    ) {
        match r {
            xdg_wm_base::Request::GetXdgSurface { id, surface } => {
                s.surface = Some(surface);
                i.init(id, ());
            },
            xdg_wm_base::Request::CreatePositioner { id } => {
                i.init(id, ());
            },
            _ => {},
        }
    }
}
impl Dispatch<xdg_surface::XdgSurface, ()> for State {
    fn request(
        s: &mut Self,
        c: &Client,
        rsc: &xdg_surface::XdgSurface,
        r: xdg_surface::Request,
        _: &(),
        d: &DisplayHandle,
        i: &mut DataInit<'_, Self>,
    ) {
        match r {
            xdg_surface::Request::GetToplevel { id } => {
                let top = i.init(id, ());
                top.configure(800, 600, vec![]);
                rsc.configure(1);
            },
            xdg_surface::Request::AckConfigure { .. } if !s.offered => {
                if let (Some(dev), Some(surface)) = (&s.device, &s.surface) {
                    let offer = c
                        .create_resource::<wl_data_offer::WlDataOffer, (), Self>(
                            d,
                            if s.case == "legacy" { 2 } else { 3 },
                            (),
                        )
                        .unwrap();
                    dev.data_offer(&offer);
                    offer.offer(
                        if s.case == "unsupported" {
                            "image/unknown"
                        } else if s.case == "portal" {
                            "application/vnd.portal.filetransfer"
                        } else {
                            "text/uri-list"
                        }
                        .into(),
                    );
                    dev.enter(2, surface, 123., 234., Some(&offer));
                    if offer.version() >= 3 {
                        offer.source_actions(wl_data_device_manager::DndAction::Copy);
                    }
                    s.offered = true;
                }
            },
            _ => {},
        }
    }
}
impl Dispatch<wl_data_device_manager::WlDataDeviceManager, ()> for State {
    fn request(
        s: &mut Self,
        _: &Client,
        _: &wl_data_device_manager::WlDataDeviceManager,
        r: wl_data_device_manager::Request,
        _: &(),
        _: &DisplayHandle,
        i: &mut DataInit<'_, Self>,
    ) {
        match r {
            wl_data_device_manager::Request::GetDataDevice { id, .. } => {
                s.device = Some(i.init(id, ()))
            },
            wl_data_device_manager::Request::CreateDataSource { id } => {
                i.init(id, ());
            },
            _ => {},
        }
    }
}
impl Dispatch<wl_data_offer::WlDataOffer, ()> for State {
    fn request(
        s: &mut Self,
        client: &Client,
        o: &wl_data_offer::WlDataOffer,
        r: wl_data_offer::Request,
        _: &(),
        display: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        match r {
            wl_data_offer::Request::Accept { mime_type: Some(_), .. }
                if s.case == "legacy" && !s.dropped =>
            {
                s.device.as_ref().unwrap().motion(50, 321., 432.);
                s.device.as_ref().unwrap().drop();
                s.device.as_ref().unwrap().leave();
                s.dropped = true;
            },
            wl_data_offer::Request::SetActions { dnd_actions, .. } if !s.dropped => {
                if s.case == "cancel" || dnd_actions.into_result().unwrap().is_empty() {
                    s.device.as_ref().unwrap().leave();
                    s.dropped = true;
                    return;
                }
                s.device.as_ref().unwrap().motion(50, 321., 432.);
                o.action(if s.case == "invalid_action" {
                    wl_data_device_manager::DndAction::Move
                } else {
                    wl_data_device_manager::DndAction::Copy
                });
                s.device.as_ref().unwrap().drop();
                s.device.as_ref().unwrap().leave();
                s.dropped = true;
            },
            wl_data_offer::Request::Receive { fd, .. } => {
                let mut f = std::fs::File::from(fd);
                s.receives += 1;
                if s.case == "overlap" && s.receives == 1 {
                    let bytes = s.bytes.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_millis(300));
                        f.write_all(&bytes).unwrap();
                    });
                    let next = client
                        .create_resource::<wl_data_offer::WlDataOffer, (), Self>(
                            display,
                            if s.case == "legacy" { 2 } else { 3 },
                            (),
                        )
                        .unwrap();
                    let device = s.device.as_ref().unwrap();
                    device.data_offer(&next);
                    next.offer("text/uri-list".into());
                    device.enter(3, s.surface.as_ref().unwrap(), 100., 200., Some(&next));
                    next.source_actions(wl_data_device_manager::DndAction::Copy);
                    s.dropped = false;
                } else if s.case == "overlap" {
                    f.write_all(b"file:///tmp/second.png\n").unwrap();
                } else if s.case == "stall" {
                    s.held = Some(f);
                } else {
                    f.write_all(&s.bytes).unwrap();
                }
            },
            wl_data_offer::Request::Finish => {
                s.finished = true;
                s.finish_count += 1;
            },
            _ => {},
        }
    }
}
macro_rules! ignore { ($($t:ty),*) => {$(impl Dispatch<$t,()> for State{fn request(_: &mut Self,_:&Client,_:&$t,_:<$t as Resource>::Request,_:&(),_:&DisplayHandle,_:&mut DataInit<'_,Self>) {}})*}; }
ignore!(
    wl_surface::WlSurface,
    wl_region::WlRegion,
    wl_buffer::WlBuffer,
    wl_pointer::WlPointer,
    wl_data_device::WlDataDevice,
    wl_data_source::WlDataSource,
    xdg_toplevel::XdgToplevel,
    xdg_positioner::XdgPositioner
);
struct App {
    window: Option<Window>,
    paths: Vec<std::path::PathBuf>,
    deadline: std::time::Instant,
    expected: usize,
    position: Option<winit::dpi::PhysicalPosition<f64>>,
}
impl ApplicationHandler for App {
    fn resumed(&mut self, e: &ActiveEventLoop) {
        self.window =
            Some(e.create_window(Window::default_attributes().with_decorations(false)).unwrap());
    }
    fn window_event(&mut self, e: &ActiveEventLoop, _: WindowId, v: WindowEvent) {
        match v {
            WindowEvent::CursorMoved { position, .. } => {
                self.position = Some(position);
                if std::env::var("DROP_CASE").as_deref() == Ok("close") {
                    self.window.take();
                }
            },
            WindowEvent::DroppedFile(p) => {
                self.paths.push(p);
                if self.paths.len() == self.expected {
                    e.exit();
                }
            },
            _ => {},
        }
    }
    fn about_to_wait(&mut self, e: &ActiveEventLoop) {
        if std::time::Instant::now() >= self.deadline {
            e.exit();
        } else {
            e.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(self.deadline));
        }
    }
}
#[test]
#[ignore = "subprocess helper; run by native_file_drop_protocol"]
fn native_client() {
    use winit::platform::wayland::EventLoopBuilderExtWayland;
    let e = EventLoop::builder().with_any_thread(true).build().unwrap();
    let case = std::env::var("DROP_CASE").unwrap_or_default();
    let expected = if case == "overlap" {
        3
    } else {
        usize::from(case == "valid" || case == "portal" || case == "legacy") * 2
    };
    let mut a = App {
        window: None,
        paths: vec![],
        deadline: std::time::Instant::now()
            + std::time::Duration::from_millis(if case == "stall" {
                11500
            } else if case == "portal" {
                7000
            } else {
                1000
            }),
        expected,
        position: None,
    };
    e.run_app(&mut a).unwrap();
    if case == "overlap" {
        assert_eq!(a.paths, vec![
            std::path::PathBuf::from("/tmp/second.png"),
            std::path::PathBuf::from("/tmp/a b.png"),
            std::path::PathBuf::from("/tmp/✓.jpg")
        ]);
        println!("overlapping drops: both transfers completed");
        return;
    }
    if expected == 0 {
        assert!(a.paths.is_empty());
        println!("native protocol rejected {case}");
        return;
    }
    if case == "portal" {
        let expected = std::env::var("DROP_HASHES").unwrap();
        let got = a
            .paths
            .iter()
            .map(|p| hash(&std::fs::read(p).unwrap_or_else(|e| panic!("portal path {p:?}: {e}"))))
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(got, expected);
        println!("portal files readable by native receiver");
        return;
    }
    assert_eq!(a.position, Some(winit::dpi::PhysicalPosition::new(321., 432.)));
    assert_eq!(a.paths, vec![
        std::path::PathBuf::from("/tmp/a b.png"),
        std::path::PathBuf::from("/tmp/✓.jpg")
    ]);
    println!("native protocol drop: 2 paths delivered");
}

fn run_case(case: &str) {
    let case = case.to_owned();
    let dir = tempfile::tempdir().unwrap();
    let socket =
        wayland_server::ListeningSocket::bind_absolute(dir.path().join("test-wayland")).unwrap();
    let mut display = Display::<State>::new().unwrap();
    let h = display.handle();
    h.create_global::<State, wl_compositor::WlCompositor, _>(4, ());
    h.create_global::<State, wl_shm::WlShm, _>(1, ());
    h.create_global::<State, wl_seat::WlSeat, _>(7, ());
    h.create_global::<State, xdg_wm_base::XdgWmBase, _>(3, ());
    h.create_global::<State, wl_data_device_manager::WlDataDeviceManager, _>(
        if case == "legacy" { 2 } else { 3 },
        (),
    );
    let portal_connection =
        if case == "portal" { Some(zbus::blocking::Connection::session().unwrap()) } else { None };
    let mut files = vec![];
    let mut hashes = vec![];
    let portal_key = portal_connection.as_ref().map(|conn| {
        use std::os::fd::AsFd;
        let p = zbus::blocking::Proxy::new(
            conn,
            "org.freedesktop.portal.Documents",
            "/org/freedesktop/portal/documents",
            "org.freedesktop.portal.FileTransfer",
        )
        .unwrap();
        let options: std::collections::HashMap<&str, zbus::zvariant::Value<'_>> =
            std::collections::HashMap::new();
        let key: String = p.call("StartTransfer", &(options,)).unwrap();
        for n in 0..2 {
            let mut file = tempfile::Builder::new().suffix(".jpg").tempfile().unwrap();
            let bytes = if let Ok(paths) = std::env::var("DROP_IMAGE_FILES") {
                std::fs::read(paths.lines().nth(n).unwrap()).unwrap()
            } else {
                format!("original test image {n}").into_bytes()
            };
            file.write_all(&bytes).unwrap();
            hashes.push(hash(&bytes));
            files.push(file);
        }
        let fds =
            files.iter().map(|f| zbus::zvariant::Fd::from(f.as_file().as_fd())).collect::<Vec<_>>();
        let options: std::collections::HashMap<&str, zbus::zvariant::Value<'_>> =
            std::collections::HashMap::new();
        p.call::<_, _, ()>("AddFiles", &(&key, fds, options)).unwrap();
        key
    });
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    let mut child = command
        .arg("--exact")
        .arg("native_client")
        .arg("--ignored")
        .arg("--nocapture")
        .env("DROP_HASHES", hashes.join(","))
        .env("DROP_CASE", &case)
        .env("WAYLAND_DISPLAY", dir.path().join("test-wayland"))
        .env_remove("DISPLAY")
        .spawn()
        .unwrap();
    let bytes = match case.as_str() {
        "portal" => portal_key.unwrap().into_bytes(),
        "remote" => b"file://remote/private.png".to_vec(),
        "nul" => b"file:///tmp/a%00.png".to_vec(),
        "oversize" => vec![b'x'; 1024 * 1024 + 1],
        "empty" => vec![],
        _ => b"file:///tmp/a%20b.png\r\nfile:///tmp/%E2%9C%93.jpg\r\n".to_vec(),
    };
    let mut state = State { bytes, case: case.clone(), ..Default::default() };
    let start = std::time::Instant::now();
    loop {
        if let Some(stream) = socket.accept().unwrap() {
            display.handle().insert_client(stream, Arc::new(ClientState)).unwrap();
        }
        display.dispatch_clients(&mut state).unwrap();
        display.flush_clients().unwrap();
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            if case == "overlap" {
                assert_eq!(state.finish_count, 2);
            }
            assert_eq!(
                state.finished,
                case == "valid" || case == "portal" || case == "overlap",
                "finish only successful transfers"
            );
            break;
        }
        if start.elapsed().as_secs() > 15 {
            let _ = child.kill();
            panic!("native protocol timed out");
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

struct ClientState;
impl wayland_server::backend::ClientData for ClientState {
    fn initialized(&self, _: wayland_server::backend::ClientId) {}
    fn disconnected(
        &self,
        _: wayland_server::backend::ClientId,
        _: wayland_server::backend::DisconnectReason,
    ) {
    }
}

fn hash(bytes: &[u8]) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut h);
    format!("{:x}", h.finish())
}

#[test]
fn native_file_drop_protocol() {
    for case in [
        "valid",
        "cancel",
        "unsupported",
        "remote",
        "nul",
        "oversize",
        "empty",
        "overlap",
        "legacy",
        "invalid_action",
        "close",
        "stall",
    ] {
        run_case(case);
    }
}

/// Uses the real desktop portal with temporary files. Requires a running user session bus.
#[test]
#[ignore = "requires org.freedesktop.portal.Documents on the user session bus"]
fn native_file_drop_portal() {
    run_case("portal");
}
