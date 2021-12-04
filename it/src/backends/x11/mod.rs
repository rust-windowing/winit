use crate::backend::{
    Backend, BackendDeviceId, BackendFlags, BackendIcon, Button, Device, DndProcess, EventLoop,
    Finger, Instance, Keyboard, Mouse, PressedButton, PressedKey, Seat, Touchscreen, Window,
    WindowProperties,
};
use crate::backends::x11::dnd::DndMsg;
use crate::backends::x11::layout::{layouts, set_names, Layouts};
use crate::backends::x11::wm::TITLE_HEIGHT;
use crate::backends::x11::MessageType::{
    MT_BUTTON_PRESS, MT_BUTTON_RELEASE, MT_CREATE_MOUSE, MT_CREATE_MOUSE_REPLY, MT_CREATE_TOUCH,
    MT_CREATE_TOUCH_REPLY, MT_ENABLE_SECOND_MONITOR, MT_ENABLE_SECOND_MONITOR_REPLY,
    MT_GET_VIDEO_INFO, MT_GET_VIDEO_INFO_REPLY, MT_MOUSE_MOVE, MT_MOUSE_SCROLL, MT_REMOVE_DEVICE,
    MT_TOUCH_DOWN, MT_TOUCH_DOWN_REPLY, MT_TOUCH_MOVE, MT_TOUCH_UP,
};
use crate::env::set_env;
use crate::event::{map_event, DeviceEvent, DeviceEventExt, Event, UserEvent};
use crate::eventstream::EventStream;
use crate::keyboard::{Key, Layout};
use crate::test::with_test_data;
use parking_lot::Mutex;
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Display;
use std::fs::File;
use std::future::Future;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Command;
use std::sync::{Arc, Weak};
use std::task::{Context, Poll, Waker};
use std::time::Duration;
use std::{mem, ptr};
use tokio::io::unix::AsyncFd;
use tokio::io::Interest;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;
use uapi::c::{AF_UNIX, O_CLOEXEC, SOCK_CLOEXEC, SOCK_SEQPACKET};
use uapi::{pipe2, socketpair, IntoUstr, OwnedFd, Pod, UapiReadExt, UstrPtr};
use winit::event::{DeviceId, ElementState, RawKeyEvent};
use winit::event_loop::{ControlFlow, EventLoop as WEventLoop};
use winit::keyboard::KeyCode;
use winit::platform::run_return::EventLoopExtRunReturn;
use winit::platform::unix::{
    DeviceIdExtUnix, EventLoopExtUnix, EventLoopWindowTargetExtUnix, WindowExtUnix,
};
use winit::window::{Window as WWindow, WindowBuilder};
use xcb_dl::{ffi, Xcb, XcbRandr, XcbRender, XcbXfixes, XcbXinput, XcbXkb};
use xcb_dl_util::error::XcbErrorParser;
use MessageType::{MT_CREATE_KEYBOARD, MT_CREATE_KEYBOARD_REPLY, MT_KEY_PRESS, MT_KEY_RELEASE};

mod dnd;
mod evdev;
mod keysyms;
mod layout;
mod wm;

const DEFAULT_X_PATH: &str = "/usr/lib/Xorg";
// const DEFAULT_X_PATH: &str = "/home/julian/c/xserver/install/bin/X";

pub fn backend() -> Box<dyn Backend> {
    let x_path = match std::env::var("X_PATH") {
        Ok(p) => p,
        _ => DEFAULT_X_PATH.to_string(),
    };
    let default_module_path = Command::new(&x_path)
        .arg("-showDefaultModulePath")
        .output()
        .unwrap()
        .stderr;
    unsafe {
        Box::new(Arc::new(XBackend {
            x_path,
            default_module_path: String::from_utf8(default_module_path)
                .unwrap()
                .trim()
                .to_string(),
            xcb: Xcb::load_loose().unwrap(),
            xinput: XcbXinput::load_loose().unwrap(),
            xrandr: XcbRandr::load_loose().unwrap(),
            xfixes: XcbXfixes::load_loose().unwrap(),
            render: XcbRender::load_loose().unwrap(),
            xkb: XcbXkb::load_loose().unwrap(),
            layouts: layouts(),
        }))
    }
}

struct XBackend {
    x_path: String,
    default_module_path: String,
    xcb: Xcb,
    xinput: XcbXinput,
    xrandr: XcbRandr,
    xfixes: XcbXfixes,
    render: XcbRender,
    xkb: XcbXkb,
    layouts: Layouts,
}

impl Backend for Arc<XBackend> {
    fn instantiate(&self) -> Box<dyn Instance> {
        let (psock, chsock) = socketpair(AF_UNIX, SOCK_SEQPACKET | SOCK_CLOEXEC, 0).unwrap();
        let (mut ppipe, chpipe) = pipe2(O_CLOEXEC).unwrap();
        let tmpdir = crate::test::with_test_data(|td| td.test_dir.join("x11_data"));
        std::fs::create_dir_all(&tmpdir).unwrap();
        let config_file = tmpdir.join("config.conf");
        let log_file = tmpdir.join("log");
        let stderr_file = tmpdir.join("stderr").into_ustr();
        let config_dir = tmpdir.join("conf");
        let module_path = format!(
            "{},{}/x11-module/install",
            self.default_module_path,
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::write(&config_file, CONFIG).unwrap();
        let env = {
            let mut env = UstrPtr::new();
            for name in ["HOME", "PATH"] {
                env.push(format!("{}={}", name, std::env::var(name).unwrap()));
            }
            env.push(format!("WINIT_IT_SOCKET={}", chsock.raw()));
            env
        };
        let args = {
            let mut args = UstrPtr::new();
            args.push(&*self.x_path);
            args.push("-config");
            args.push(&*config_file);
            args.push("-configdir");
            args.push(&*config_dir);
            args.push("-modulepath");
            args.push(&*module_path);
            args.push("-seat");
            args.push("winit-seat");
            args.push("-logfile");
            args.push(&*log_file);
            args.push("-noreset");
            args.push("-displayfd");
            args.push(chpipe.to_string().into_ustr().to_owned());
            args
        };
        log::trace!("args: {:?}", args);
        log::trace!("env: {:?}", env);
        let chpid = unsafe { uapi::fork().unwrap() };
        if chpid == 0 {
            let null = uapi::open("/dev/null\0", libc::O_RDWR, 0).unwrap();
            let stderr = uapi::open(&*stderr_file, libc::O_CREAT | libc::O_WRONLY, 0o666).unwrap();
            uapi::dup2(null.raw(), 0).unwrap();
            uapi::dup2(null.raw(), 1).unwrap();
            uapi::dup2(stderr.raw(), 2).unwrap();
            uapi::fcntl_setfd(chsock.raw(), 0).unwrap();
            uapi::fcntl_setfd(chpipe.raw(), 0).unwrap();
            drop(null);
            drop(stderr);
            unsafe {
                uapi::map_err!(libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL)).unwrap();
            }
            uapi::execvpe(&*self.x_path, &args, &env).unwrap();
        }
        drop(chpipe);
        let display = ppipe
            .read_to_new_ustring()
            .unwrap()
            .into_string()
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        log::trace!("display: {}", display);

        let (second_crtc, second_output, first_output, large_mode_id, small_mode_id);
        unsafe {
            let mut msg = Message {
                ty: MT_GET_VIDEO_INFO as _,
            };
            uapi::write(psock.raw(), &msg).unwrap();
            uapi::read(psock.raw(), &mut msg).unwrap();
            assert_eq!(msg.ty, MT_GET_VIDEO_INFO_REPLY as _);
            second_crtc = msg.get_video_info_reply.second_crtc;
            second_output = msg.get_video_info_reply.second_output;
            first_output = msg.get_video_info_reply.first_output;
            large_mode_id = msg.get_video_info_reply.large_mode_id;
            small_mode_id = msg.get_video_info_reply.small_mode_id;
        }

        let mut instance = XInstanceData {
            backend: self.clone(),
            xserver_pid: chpid,
            sock: psock,
            display,
            wm_data: Mutex::new(WmData {
                wakers: vec![],
                windows: Default::default(),
                parents: Default::default(),
                window_to_parent: Default::default(),
                pongs: Default::default(),
            }),
            atoms: Default::default(),
            second_crtc,
            second_output,
            first_output,
            _large_mode_id: large_mode_id,
            small_mode_id,
        };

        let c = XConnection::new(self, display);

        unsafe {
            let cookie =
                self.xrandr
                    .xcb_randr_set_output_primary_checked(c.c, c.screen.root, first_output);
            c.errors.check_cookie(&self.xcb, cookie).unwrap();
        }

        instance.atoms.net_wm_state = c.atom("_NET_WM_STATE");
        instance.atoms.wm_change_state = c.atom("WM_CHANGE_STATE");
        instance.atoms.wm_state = c.atom("WM_STATE");
        instance.atoms.net_wm_name = c.atom("_NET_WM_NAME");
        instance.atoms.net_wm_icon = c.atom("_NET_WM_ICON");
        instance.atoms.wm_delete_window = c.atom("WM_DELETE_WINDOW");
        instance.atoms.net_wm_ping = c.atom("_NET_WM_PING");
        instance.atoms.utf8_string = c.atom("UTF8_STRING");
        instance.atoms.net_wm_state_above = c.atom("_NET_WM_STATE_ABOVE");
        instance.atoms.net_wm_state_fullscreen = c.atom("_NET_WM_STATE_FULLSCREEN");
        instance.atoms.net_frame_extents = c.atom("_NET_FRAME_EXTENTS");
        instance.atoms.net_wm_state_maximized_horz = c.atom("_NET_WM_STATE_MAXIMIZED_HORZ");
        instance.atoms.net_wm_state_maximized_vert = c.atom("_NET_WM_STATE_MAXIMIZED_VERT");
        instance.atoms.motif_wm_hints = c.atom("_MOTIF_WM_HINTS");
        instance.atoms.wm_name = c.atom("WM_NAME");
        instance.atoms.wm_normal_hints = c.atom("WM_NORMAL_HINTS");
        instance.atoms.wm_hints = c.atom("WM_HINTS");
        instance.atoms.wm_class = c.atom("WM_CLASS");
        instance.atoms.wm_protocols = c.atom("WM_PROTOCOLS");
        instance.atoms.net_active_window = c.atom("_NET_ACTIVE_WINDOW");
        instance.atoms.net_supported = c.atom("_NET_SUPPORTED");
        instance.atoms.net_client_list = c.atom("_NET_CLIENT_LIST");
        instance.atoms.net_client_list_stacking = c.atom("_NET_CLIENT_LIST_STACKING");
        instance.atoms.net_frame_extents = c.atom("_NET_FRAME_EXTENTS");
        instance.atoms.net_supporting_wm_check = c.atom("_NET_SUPPORTING_WM_CHECK");
        instance.atoms.net_wm_moveresize = c.atom("_NET_WM_MOVERESIZE");
        instance.atoms.x_dnd_aware = c.atom("XdndAware");
        instance.atoms.x_dnd_selection = c.atom("XdndSelection");
        instance.atoms.x_dnd_enter = c.atom("XdndEnter");
        instance.atoms.x_dnd_type_list = c.atom("XdndTypeList");
        instance.atoms.x_dnd_position = c.atom("XdndPosition");
        instance.atoms.x_dnd_action_copy = c.atom("XdndActionCopy");
        instance.atoms.x_dnd_action_private = c.atom("XdndActionPrivate");
        instance.atoms.x_dnd_status = c.atom("XdndStatus");
        instance.atoms.x_dnd_leave = c.atom("XdndLeave");
        instance.atoms.x_dnd_drop = c.atom("XdndDrop");
        instance.atoms.uri_list = c.atom("text/uri-list");

        let instance = Arc::new(instance);

        let wm = Some(tokio::task::spawn_local(wm::run(instance.clone())));

        let (core_p, core_kb) = unsafe {
            let mut err = ptr::null_mut();
            let reply = self.xkb.xcb_xkb_use_extension_reply(
                c.c,
                self.xkb.xcb_xkb_use_extension(c.c, 1, 0),
                &mut err,
            );
            c.errors.check(&self.xcb, reply, err).unwrap();
            let reply = self.xrandr.xcb_randr_query_version_reply(
                c.c,
                self.xrandr.xcb_randr_query_version(c.c, 1, 3),
                &mut err,
            );
            c.errors.check(&self.xcb, reply, err).unwrap();
            let reply = self.xfixes.xcb_xfixes_query_version_reply(
                c.c,
                self.xfixes.xcb_xfixes_query_version(c.c, 1, 0),
                &mut err,
            );
            c.errors.check(&self.xcb, reply, err).unwrap();
            let cookie = self.xinput.xcb_input_xi_query_version(c.c, 2, 0);
            let reply = self
                .xinput
                .xcb_input_xi_query_version_reply(c.c, cookie, &mut err);
            let _reply = c.errors.check(&self.xcb, reply, err).unwrap();
            let cookie = self
                .xinput
                .xcb_input_xi_query_device(c.c, ffi::XCB_INPUT_DEVICE_ALL_MASTER as _);
            let reply = self
                .xinput
                .xcb_input_xi_query_device_reply(c.c, cookie, &mut err);
            let reply = c.errors.check(&self.xcb, reply, err).unwrap();
            let mut iter = self
                .xinput
                .xcb_input_xi_query_device_infos_iterator(&*reply);
            let mut core = None;
            while iter.rem > 0 {
                let info = &*iter.data;
                if info.type_ == ffi::XCB_INPUT_DEVICE_TYPE_MASTER_POINTER as _ {
                    assert!(core.is_none());
                    core = Some((info.deviceid, info.attachment));
                }
                self.xinput.xcb_input_xi_device_info_next(&mut iter);
            }
            core.unwrap()
        };

        Box::new(Arc::new(XInstance {
            c,
            data: instance.clone(),
            event_loops: Default::default(),
            wm,
            core_p,
            core_kb,
            core_layout: Arc::new(Cell::new(Layout::Qwerty)),
            next_seat_id: Cell::new(1),
        }))
    }

    fn name(&self) -> &str {
        "x11"
    }

    fn flags(&self) -> BackendFlags {
        BackendFlags::MT_SAFE
            | BackendFlags::WINIT_SET_ALWAYS_ON_TOP
            | BackendFlags::WINIT_SET_DECORATIONS
            | BackendFlags::WINIT_SET_INNER_SIZE
            | BackendFlags::WINIT_SET_OUTER_POSITION
            | BackendFlags::WINIT_SET_TITLE
            | BackendFlags::WINIT_SET_VISIBLE
            | BackendFlags::WINIT_SET_MAXIMIZED
            | BackendFlags::WINIT_SET_MINIMIZED
            | BackendFlags::WINIT_SET_SIZE_BOUNDS
            | BackendFlags::WINIT_SET_ATTENTION
            | BackendFlags::WINIT_SET_RESIZABLE
            | BackendFlags::WINIT_SET_ICON
            // | BackendFlags::WINIT_TRANSPARENCY
            | BackendFlags::X11
            | BackendFlags::SET_OUTER_POSITION
            | BackendFlags::SET_INNER_SIZE
            | BackendFlags::DEVICE_ADDED
            | BackendFlags::DEVICE_REMOVED
            | BackendFlags::CREATE_SEAT
            | BackendFlags::SECOND_MONITOR
            | BackendFlags::MONITOR_NAMES
            | BackendFlags::WINIT_SET_CURSOR_POSITION
    }
}

struct XConnection {
    backend: Arc<XBackend>,
    c: *mut ffi::xcb_connection_t,
    fd: libc::c_int,
    errors: XcbErrorParser,
    screen: ffi::xcb_screen_t,
}

impl XConnection {
    fn new(backend: &Arc<XBackend>, display: u32) -> Self {
        unsafe {
            let display_str = uapi::format_ustr!(":{}", display);
            let c = backend
                .xcb
                .xcb_connect(display_str.as_ptr(), ptr::null_mut());
            let parser = XcbErrorParser::new(&backend.xcb, c);
            parser.check_connection(&backend.xcb).unwrap();
            let screen = *backend
                .xcb
                .xcb_setup_roots_iterator(backend.xcb.xcb_get_setup(c))
                .data;
            Self {
                backend: backend.clone(),
                c,
                fd: backend.xcb.xcb_get_file_descriptor(c),
                errors: parser,
                screen,
            }
        }
    }

    fn atom(&self, name: &str) -> ffi::xcb_atom_t {
        unsafe {
            let mut err = ptr::null_mut();
            let reply = self.backend.xcb.xcb_intern_atom_reply(
                self.c,
                self.backend
                    .xcb
                    .xcb_intern_atom(self.c, 0, name.len() as _, name.as_ptr() as _),
                &mut err,
            );
            self.errors
                .check(&self.backend.xcb, reply, err)
                .unwrap()
                .atom
        }
    }
}

impl Drop for XConnection {
    fn drop(&mut self) {
        unsafe {
            self.backend.xcb.xcb_disconnect(self.c);
        }
    }
}

struct XInstanceData {
    backend: Arc<XBackend>,
    xserver_pid: libc::pid_t,
    sock: OwnedFd,
    display: u32,
    wm_data: Mutex<WmData>,
    atoms: Atoms,
    second_crtc: u32,
    second_output: u32,
    first_output: u32,
    _large_mode_id: u32,
    small_mode_id: u32,
}

struct XInstance {
    c: XConnection,
    data: Arc<XInstanceData>,
    event_loops: Mutex<Vec<Weak<XEventLoopData>>>,
    wm: Option<JoinHandle<()>>,
    core_p: ffi::xcb_input_device_id_t,
    core_kb: ffi::xcb_input_device_id_t,
    core_layout: Arc<Cell<Layout>>,
    next_seat_id: Cell<usize>,
}

unsafe impl Send for XInstance {}
unsafe impl Sync for XInstance {}

impl XInstance {
    fn cursor_grab_status(&self) -> bool {
        let grabbed;
        unsafe {
            let xcb = &self.data.backend.xcb;
            let mut err = ptr::null_mut();
            let reply = xcb.xcb_grab_pointer_reply(
                self.c.c,
                xcb.xcb_grab_pointer(self.c.c, 1, self.c.screen.root, 0, 0, 0, 0, 0, 0),
                &mut err,
            );
            let reply = match self.c.errors.check(xcb, reply, err) {
                Ok(r) => r,
                Err(e) => panic!("Could not grab pointer: {}", e),
            };
            grabbed = match reply.status as u32 {
                0 => false,
                ffi::XCB_GRAB_STATUS_ALREADY_GRABBED => true,
                _ => panic!("Unexpected grab status"),
            };
            if !grabbed {
                let cookie = xcb.xcb_ungrab_pointer_checked(self.c.c, 0);
                if let Err(e) = self.c.errors.check_cookie(xcb, cookie) {
                    panic!("Could not ungrab pointer: {}", e);
                }
            }
        }
        grabbed
    }

    fn add_dev(&self, req: MessageType, rep: MessageType) -> ffi::xcb_input_device_id_t {
        let mut msg = Message { ty: req as _ };
        uapi::write(self.data.sock.raw(), &msg).unwrap();
        uapi::read(self.data.sock.raw(), &mut msg).unwrap();
        unsafe {
            assert_eq!(msg.ty, rep as _);
            msg.create_keyboard_reply.id as _
        }
    }

    fn add_keyboard(&self) -> ffi::xcb_input_device_id_t {
        self.add_dev(MT_CREATE_KEYBOARD, MT_CREATE_KEYBOARD_REPLY)
    }

    fn add_mouse(&self) -> ffi::xcb_input_device_id_t {
        self.add_dev(MT_CREATE_MOUSE, MT_CREATE_MOUSE_REPLY)
    }

    fn add_touchscreen(&self) -> ffi::xcb_input_device_id_t {
        self.add_dev(MT_CREATE_TOUCH, MT_CREATE_TOUCH_REPLY)
    }

    fn assign_slave(&self, slave: ffi::xcb_input_device_id_t, master: ffi::xcb_input_device_id_t) {
        unsafe {
            let xcb = &self.data.backend.xcb;
            let xinput = &self.data.backend.xinput;
            #[repr(C)]
            struct Change {
                hc: ffi::xcb_input_hierarchy_change_t,
                data: ffi::xcb_input_hierarchy_change_data_t__attach_slave,
            }
            let change = Change {
                hc: ffi::xcb_input_hierarchy_change_t {
                    type_: ffi::XCB_INPUT_HIERARCHY_CHANGE_TYPE_ATTACH_SLAVE as _,
                    len: (mem::size_of::<Change>() / 4) as _,
                },
                data: ffi::xcb_input_hierarchy_change_data_t__attach_slave {
                    deviceid: slave,
                    master,
                },
            };
            let cookie = xinput.xcb_input_xi_change_hierarchy_checked(self.c.c, 1, &change.hc);
            if let Err(e) = self.c.errors.check_cookie(xcb, cookie) {
                panic!("Could not assign slave to master: {}", e);
            }
        }
    }

    fn set_layout(
        &self,
        slave: ffi::xcb_input_device_id_t,
        layout: Layout,
        prev_layout: Option<Layout>,
    ) {
        if Some(layout) == prev_layout {
            return;
        }
        let change_map = match (layout, prev_layout) {
            (_, None) => true,
            (Layout::QwertySwapped, _) => true,
            (_, Some(Layout::QwertySwapped)) => true,
            _ => false,
        };
        let backend = &self.data.backend;
        let (group, msg) = match layout {
            Layout::Qwerty => (0, &backend.layouts.msg1),
            Layout::Azerty => (1, &backend.layouts.msg1),
            Layout::QwertySwapped => (0, &backend.layouts.msg2),
        };
        unsafe {
            let xcb = &self.data.backend.xcb;
            let xkb = &self.data.backend.xkb;
            if change_map {
                let mut header = msg.header;
                header.device_spec = slave;
                let mut iovecs = [
                    libc::iovec {
                        iov_base: ptr::null_mut(),
                        iov_len: 0,
                    },
                    libc::iovec {
                        iov_base: ptr::null_mut(),
                        iov_len: 0,
                    },
                    libc::iovec {
                        iov_base: &mut header as *mut _ as _,
                        iov_len: mem::size_of_val(&header),
                    },
                    libc::iovec {
                        iov_base: msg.body.as_ptr() as _,
                        iov_len: msg.body.len(),
                    },
                ];
                let request = ffi::xcb_protocol_request_t {
                    count: 2,
                    ext: xkb.xcb_xkb_id(),
                    opcode: ffi::XCB_XKB_SET_MAP,
                    isvoid: 1,
                };
                let sequence = xcb.xcb_send_request(
                    self.c.c,
                    ffi::XCB_REQUEST_CHECKED,
                    &mut iovecs[2],
                    &request,
                );
                let cookie = ffi::xcb_void_cookie_t { sequence };
                if let Err(e) = self.c.errors.check_cookie(xcb, cookie) {
                    panic!("Could not set keymap: {}", e);
                }
                let cookie = set_names(xkb, &self.c, slave);
                if let Err(e) = self.c.errors.check_cookie(xcb, cookie) {
                    panic!("Could not set level names: {}", e);
                }
            }
            let cookie =
                xkb.xcb_xkb_latch_lock_state_checked(self.c.c, slave, 0, 0, 1, group, 0, 0, 0);
            if let Err(e) = self.c.errors.check_cookie(xcb, cookie) {
                panic!("Could not set keymap group: {}", e);
            }
        }
    }

    fn create_seat2(&self) -> (ffi::xcb_input_device_id_t, ffi::xcb_input_device_id_t) {
        unsafe {
            let xinput = &self.data.backend.xinput;
            let xcb = &self.data.backend.xcb;
            let name = format!("seat{}", self.next_seat_id.get());
            let kb_name = format!("{} keyboard", name);
            self.next_seat_id.set(self.next_seat_id.get() + 1);
            #[repr(C)]
            #[derive(Debug)]
            struct Change {
                hc: ffi::xcb_input_add_master_t,
                name: [u8; 16],
            }
            let mut change = Change {
                hc: ffi::xcb_input_add_master_t {
                    type_: ffi::XCB_INPUT_HIERARCHY_CHANGE_TYPE_ADD_MASTER as _,
                    len: (mem::size_of::<Change>() / 4) as _,
                    name_len: 16,
                    send_core: 0,
                    enable: 1,
                },
                name: [0; 16],
            };
            write!(&mut change.name[..], "{}", name).unwrap();
            let cookie =
                xinput.xcb_input_xi_change_hierarchy_checked(self.c.c, 1, &change as *const _ as _);
            if let Err(e) = self.c.errors.check_cookie(xcb, cookie) {
                panic!("Could not add master: {}", e);
            }
            let mut err = ptr::null_mut();
            let reply = xinput.xcb_input_xi_query_device_reply(
                self.c.c,
                xinput.xcb_input_xi_query_device(self.c.c, ffi::XCB_INPUT_DEVICE_ALL_MASTER as _),
                &mut err,
            );
            let reply = match self.c.errors.check(xcb, reply, err) {
                Ok(r) => r,
                Err(e) => panic!("Could not query input devices: {}", e),
            };
            let mut infos = xinput.xcb_input_xi_query_device_infos_iterator(&*reply);
            let (kb_id, pointer_id) = loop {
                if infos.rem == 0 {
                    panic!("Newly created seat does not exist");
                }
                let info = &*infos.data;
                if info.type_ == ffi::XCB_INPUT_DEVICE_TYPE_MASTER_KEYBOARD as _ {
                    let name = std::slice::from_raw_parts(
                        xinput.xcb_input_xi_device_info_name(infos.data) as *const u8,
                        info.name_len as _,
                    );
                    if name == kb_name.as_bytes() {
                        break (info.deviceid, info.attachment);
                    }
                }
                xinput.xcb_input_xi_device_info_next(&mut infos);
            };
            self.set_layout(kb_id, Layout::Qwerty, None);
            (pointer_id, kb_id)
        }
    }

    fn get_cursor(&self) -> (Vec<u32>, i32, i32, i32, i32) {
        unsafe {
            let xcb = &self.data.backend.xcb;
            let xfixes = &self.data.backend.xfixes;
            let mut err = ptr::null_mut();
            let reply = xfixes.xcb_xfixes_get_cursor_image_reply(
                self.c.c,
                xfixes.xcb_xfixes_get_cursor_image(self.c.c),
                &mut err,
            );
            let reply = match self.c.errors.check(xcb, reply, err) {
                Ok(r) => r,
                Err(e) => panic!("Could not get cursor image: {}", e),
            };
            let image = std::slice::from_raw_parts(
                xfixes.xcb_xfixes_get_cursor_image_cursor_image(&*reply),
                (reply.width * reply.height) as usize,
            );
            (
                image.to_vec(),
                reply.width as _,
                reply.height as _,
                reply.x as i32 - reply.xhot as i32,
                reply.y as i32 - reply.yhot as i32,
            )
        }
    }
}

fn create_seat(instance: &Arc<XInstance>) -> Arc<XSeat> {
    let (pointer_id, kb_id) = instance.create_seat2();
    Arc::new(XSeat {
        instance: instance.clone(),
        pointer: pointer_id,
        keyboard: kb_id,
        layout: Arc::new(Cell::new(Layout::Qwerty)),
    })
}

impl Instance for Arc<XInstance> {
    fn backend(&self) -> &dyn Backend {
        &self.data.backend
    }

    fn default_seat(&self) -> Box<dyn Seat> {
        Box::new(Arc::new(XSeat {
            instance: self.clone(),
            pointer: self.core_p,
            keyboard: self.core_kb,
            layout: self.core_layout.clone(),
        }))
    }

    fn create_seat(&self) -> Box<dyn Seat> {
        Box::new(create_seat(self))
    }

    fn create_event_loop(&self) -> Box<dyn EventLoop> {
        let barrier_seat = create_seat(self);
        barrier_seat.un_focus();
        let barrier_kb = add_keyboard(&barrier_seat);
        let el = {
            let _var = set_env("DISPLAY", &format!(":{}", self.data.display));
            WEventLoop::new_x11_any_thread().unwrap()
        };
        let el_c = el.xcb_connection().unwrap();
        let el_fd = unsafe { self.data.backend.xcb.xcb_get_file_descriptor(el_c as _) };
        let el = Arc::new(XEventLoopData {
            instance: self.clone(),
            el: Mutex::new(el),
            waiters: Default::default(),
            events: Default::default(),
            version: Cell::new(1),
            cached_num_monitors: Cell::new(usize::MAX),
            barrier_kb,
        });
        let el2 = el.clone();
        let jh = tokio::task::spawn_local(async move {
            let afd = AsyncFd::with_interest(el_fd, Interest::READABLE).unwrap();
            loop {
                el2.run();
                afd.readable().await.unwrap().clear_ready();
            }
        });
        self.event_loops.lock().push(Arc::downgrade(&el));
        Box::new(Arc::new(XEventLoop {
            data: el,
            jh: Some(jh),
        }))
    }

    fn take_screenshot(&self) {
        unsafe {
            let mut err = ptr::null_mut();
            let reply = self.data.backend.xcb.xcb_get_geometry_reply(
                self.c.c,
                self.data
                    .backend
                    .xcb
                    .xcb_get_geometry(self.c.c, self.c.screen.root),
                &mut err,
            );
            let attr = self
                .c
                .errors
                .check(&self.data.backend.xcb, reply, err)
                .unwrap();
            let reply = self.data.backend.xcb.xcb_get_image_reply(
                self.c.c,
                self.data.backend.xcb.xcb_get_image(
                    self.c.c,
                    ffi::XCB_IMAGE_FORMAT_Z_PIXMAP as u8,
                    self.c.screen.root,
                    attr.x,
                    attr.y,
                    attr.width,
                    attr.height,
                    !0,
                ),
                &mut err,
            );
            let mut image = self
                .c
                .errors
                .check(&self.data.backend.xcb, reply, err)
                .unwrap();
            let data = std::slice::from_raw_parts_mut(
                self.data.backend.xcb.xcb_get_image_data(&mut *image),
                image.length as usize * 4,
            );
            let width = attr.width as i32;
            let height = attr.height as i32;
            let (cursor, cwidth, cheight, cx, cy) = self.get_cursor();
            for row in 0..cheight {
                if row + cy < 0 {
                    continue;
                }
                if row + cy >= width {
                    break;
                }
                for col in 0..cwidth {
                    if col + cx < 0 {
                        continue;
                    }
                    if col + cx >= width {
                        break;
                    }
                    let data_off = 4 * ((cy + row) * width + cx + col) as usize;
                    let cpixel = cursor[(row * cwidth + col) as usize];
                    let alpha = (cpixel >> 24) & 0xff;
                    let red = (cpixel >> 16) & 0xff;
                    let green = (cpixel >> 8) & 0xff;
                    let blue = (cpixel >> 0) & 0xff;
                    data[data_off + 0] =
                        ((alpha * blue + (256 - alpha) * data[data_off + 0] as u32) >> 8) as u8;
                    data[data_off + 1] =
                        ((alpha * green + (256 - alpha) * data[data_off + 1] as u32) >> 8) as u8;
                    data[data_off + 2] =
                        ((alpha * red + (256 - alpha) * data[data_off + 2] as u32) >> 8) as u8;
                }
            }
            crate::screenshot::log_image(data, width as _, height as _);
        }
    }

    fn before_poll(&self) {
        let els = self.event_loops.lock();
        for el in &*els {
            if let Some(el2) = el.upgrade() {
                el2.run();
            }
        }
    }

    fn enable_second_monitor(&self, enabled: bool) {
        unsafe {
            let mut msg = Message {
                enable_second_monitor: EnableSecondMonitor {
                    ty: MT_ENABLE_SECOND_MONITOR as _,
                    enable: enabled as _,
                },
            };
            uapi::write(self.data.sock.raw(), &msg).unwrap();
            uapi::read(self.data.sock.raw(), &mut msg).unwrap();
            assert_eq!(msg.ty, MT_ENABLE_SECOND_MONITOR_REPLY as _);
            let xrandr = &self.data.backend.xrandr;
            let xcb = &self.data.backend.xcb;
            let mut err = ptr::null_mut();
            for step in 0..2 {
                if step == 0 && enabled || step == 1 && !enabled {
                    let cookie = xrandr.xcb_randr_set_screen_size_checked(
                        self.c.c,
                        self.c.screen.root,
                        if enabled { 1024 + 800 } else { 1024 },
                        768,
                        1,
                        1,
                    );
                    self.c.errors.check_cookie(xcb, cookie).unwrap();
                } else {
                    let cookie = xrandr.xcb_randr_set_crtc_config(
                        self.c.c,
                        self.data.second_crtc,
                        0,
                        0,
                        if enabled { 1024 } else { 0 },
                        0,
                        if enabled { self.data.small_mode_id } else { 0 },
                        ffi::XCB_RANDR_ROTATION_ROTATE_0 as _,
                        if enabled { 1 } else { 0 },
                        &self.data.second_output,
                    );
                    let reply = xrandr.xcb_randr_set_crtc_config_reply(self.c.c, cookie, &mut err);
                    self.c.errors.check(xcb, reply, err).unwrap();
                }
            }
            let cookie = xrandr.xcb_randr_set_output_primary_checked(
                self.c.c,
                self.c.screen.root,
                if enabled {
                    self.data.second_output
                } else {
                    self.data.first_output
                },
            );
            self.c.errors.check_cookie(xcb, cookie).unwrap();
        }
    }

    fn start_dnd_process(&self, path: &Path) -> Box<dyn DndProcess> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::task::spawn_local(dnd::run(self.data.clone(), rx, path));
        Box::new(XDndProcess {
            tx,
            dropped: Cell::new(false),
            _instance: self.clone(),
        })
    }

    fn create_dnd_path(&self, file: &str) -> PathBuf {
        with_test_data(|td| {
            let path = td.test_dir.join(file);
            File::create(&path).unwrap();
            path.canonicalize().unwrap()
        })
    }

    fn cursor_grabbed<'a>(&'a self, grab: bool) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
        Box::pin(async move {
            loop {
                if self.cursor_grab_status() == grab {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
    }

    fn redraw_requested_scenarios(&self) -> usize {
        1
    }
}

struct XDndProcess {
    tx: UnboundedSender<DndMsg>,
    dropped: Cell<bool>,
    _instance: Arc<XInstance>,
}

impl DndProcess for XDndProcess {
    fn drag_to(&self, x: i32, y: i32) {
        if self.dropped.get() {
            panic!("drag_to called after drop");
        }
        self.tx.send(DndMsg::Move(x as u32, y as u32)).unwrap();
    }

    fn do_drop(&self) {
        if self.dropped.get() {
            panic!("drop called multiple times");
        }
        self.dropped.set(true);
        self.tx.send(DndMsg::Drop).unwrap();
    }
}

impl Drop for XDndProcess {
    fn drop(&mut self) {
        if !self.dropped.get() {
            self.tx.send(DndMsg::Cancel).unwrap();
        }
        self.tx.send(DndMsg::Stop).unwrap();
    }
}

struct WmData {
    wakers: Vec<Waker>,
    windows: HashMap<ffi::xcb_window_t, Weak<XWindow>>,
    parents: HashMap<ffi::xcb_window_t, Weak<XWindow>>,
    window_to_parent: HashMap<ffi::xcb_window_t, ffi::xcb_window_t>,
    pongs: HashSet<ffi::xcb_window_t>,
}

impl WmData {
    fn changed(&mut self) {
        for waker in self.wakers.drain(..) {
            waker.wake();
        }
    }

    fn window(&self, win: ffi::xcb_window_t) -> Option<Arc<XWindow>> {
        if let Some(win) = self.windows.get(&win) {
            return win.upgrade();
        }
        None
    }

    fn parent(&self, win: ffi::xcb_window_t) -> Option<Arc<XWindow>> {
        if let Some(win) = self.parents.get(&win) {
            return win.upgrade();
        }
        None
    }
}

impl Drop for XInstanceData {
    fn drop(&mut self) {
        log::info!("Killing the X server");
        uapi::kill(self.xserver_pid, libc::SIGKILL).unwrap();
        log::info!("Waiting for the X server to terminate");
        uapi::waitpid(self.xserver_pid, 0).unwrap();
    }
}

impl Drop for XInstance {
    fn drop(&mut self) {
        self.wm.take().unwrap().abort();
    }
}

struct XEventLoopData {
    instance: Arc<XInstance>,
    el: Mutex<WEventLoop<UserEvent>>,
    waiters: Mutex<Vec<Waker>>,
    events: Mutex<VecDeque<Event>>,
    version: Cell<u32>,
    cached_num_monitors: Cell<usize>,
    barrier_kb: Arc<XKeyboard>,
}

impl XEventLoopData {
    fn run(&self) {
        let mut el = self.el.lock();
        let mut events = self.events.lock();
        let mut wake = false;
        el.run_return(|ev, _, cf| {
            *cf = ControlFlow::Exit;
            if let Some(ev) = map_event(ev) {
                log::debug!("winit event: {:?}", ev);
                events.push_back(ev);
                wake = true;
            }
        });
        if !wake {
            let num_monitors = el.available_monitors().count();
            if num_monitors != self.cached_num_monitors.get() {
                self.cached_num_monitors.set(num_monitors);
                wake = true;
            }
        }
        if wake {
            self.version.set(self.version.get() + 1);
            let mut waiters = self.waiters.lock();
            for waiter in waiters.drain(..) {
                waiter.wake();
            }
        }
    }
}

struct XEventLoop {
    data: Arc<XEventLoopData>,
    jh: Option<JoinHandle<()>>,
}

impl Drop for XEventLoop {
    fn drop(&mut self) {
        self.jh.take().unwrap().abort();
    }
}

impl XEventLoop {
    fn event2<'a>(&'a self) -> Pin<Box<dyn Future<Output = Event> + 'a>> {
        struct Changed<'b>(&'b XEventLoopData);
        impl<'b> Future for Changed<'b> {
            type Output = Event;
            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                if let Some(e) = self.0.events.lock().pop_front() {
                    Poll::Ready(e)
                } else {
                    self.0.waiters.lock().push(cx.waker().clone());
                    Poll::Pending
                }
            }
        }
        Box::pin(Changed(&self.data))
    }

    fn get_window_format(&self, id: ffi::xcb_window_t) -> ffi::xcb_render_directformat_t {
        unsafe {
            let instance = &self.data.instance;
            let xcb = &instance.data.backend.xcb;
            let render = &instance.data.backend.render;
            let mut err = ptr::null_mut();
            let window_visual = {
                let reply = xcb.xcb_get_window_attributes_reply(
                    instance.c.c,
                    xcb.xcb_get_window_attributes(instance.c.c, id),
                    &mut err,
                );
                instance.c.errors.check(xcb, reply, err).unwrap().visual
            };
            let formats = render.xcb_render_query_pict_formats_reply(
                instance.c.c,
                render.xcb_render_query_pict_formats(instance.c.c),
                &mut err,
            );
            let formats = instance.c.errors.check(xcb, formats, err).unwrap();
            let mut screens = render.xcb_render_query_pict_formats_screens_iterator(&*formats);
            while screens.rem > 0 {
                let mut depths = render.xcb_render_pictscreen_depths_iterator(screens.data);
                while depths.rem > 0 {
                    let visuals = std::slice::from_raw_parts(
                        render.xcb_render_pictdepth_visuals(depths.data),
                        render.xcb_render_pictdepth_visuals_length(depths.data) as _,
                    );
                    for visual in visuals {
                        if visual.visual == window_visual {
                            let formats = std::slice::from_raw_parts(
                                render.xcb_render_query_pict_formats_formats(&*formats),
                                formats.num_formats as _,
                            );
                            for format in formats {
                                if format.id == visual.format {
                                    return format.direct;
                                }
                            }
                        }
                    }
                    render.xcb_render_pictdepth_next(&mut depths);
                }
                render.xcb_render_pictscreen_next(&mut screens);
            }
            unreachable!();
        }
    }
}

impl EventStream for Arc<XEventLoop> {
    fn event<'a>(&'a mut self) -> Pin<Box<dyn Future<Output = Event> + 'a>> {
        self.event2()
    }
}

impl EventLoop for Arc<XEventLoop> {
    fn events(&self) -> Box<dyn EventStream> {
        Box::new(self.clone())
    }

    fn changed<'a>(&'a self) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
        struct Changed<'b>(&'b XEventLoopData, u32);
        impl<'b> Future for Changed<'b> {
            type Output = ();
            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                if self.1 != self.0.version.get() {
                    Poll::Ready(())
                } else {
                    self.0.waiters.lock().push(cx.waker().clone());
                    Poll::Pending
                }
            }
        }
        Box::pin(Changed(&self.data, self.data.version.get()))
    }

    fn create_window(&self, builder: WindowBuilder) -> Box<dyn Window> {
        let winit = builder.build(&*self.data.el.lock()).unwrap();
        let id = winit.x11_window().unwrap();
        let format = self.get_window_format(id);
        log::info!("Created window {}", id);
        log::info!("Pixel format: {:?}", format);
        let win = Arc::new(XWindow {
            el: self.clone(),
            id,
            format,
            parent_id: Cell::new(0),
            winit,
            property_generation: Cell::new(0),
            created: Cell::new(false),
            destroyed: Cell::new(false),
            mapped: Cell::new(false),
            always_on_top: Cell::new(false),
            maximized_vert: Cell::new(false),
            maximized_horz: Cell::new(false),
            fullscreen: Cell::new(false),
            pre_fs_x: Cell::new(0),
            pre_fs_y: Cell::new(0),
            pre_fs_width: Cell::new(0),
            pre_fs_height: Cell::new(0),
            pre_fs_border: Cell::new(0),
            decorations: Cell::new(true),
            border: Cell::new(0),
            x: Cell::new(0),
            y: Cell::new(0),
            x_to_be: Cell::new(0),
            y_to_be: Cell::new(0),
            width_to_be: Cell::new(0),
            height_to_be: Cell::new(0),
            border_to_be: Cell::new(0),
            width: Cell::new(0),
            height: Cell::new(0),
            min_size: Cell::new(None),
            max_size: Cell::new(None),
            wm_name: RefCell::new("".to_string()),
            utf8_title: RefCell::new("".to_string()),
            urgency: Cell::new(false),
            class: RefCell::new(None),
            instance: RefCell::new(None),
            protocols: Cell::new(Protocols::empty()),
            desired_state: Cell::new(WindowState::Withdrawn),
            current_state: Cell::new(WindowState::Withdrawn),
            maximizable: Cell::new(true),
            icon: RefCell::new(None),
            dragging: Cell::new(false),
        });
        self.data
            .instance
            .data
            .wm_data
            .lock()
            .windows
            .insert(win.id, Arc::downgrade(&win));
        Box::new(win)
    }

    fn with_winit<'a>(&self, f: Box<dyn FnOnce(&mut WEventLoop<UserEvent>) + 'a>) {
        f(&mut *self.data.el.lock());
    }

    fn barrier<'a>(&'a self) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
        log::info!("Creating event barrier");
        Box::pin(async {
            self.data.barrier_kb.press(Key::KeyEsc);
            loop {
                let ev = self.event2().await;
                if let Event::DeviceEvent(DeviceEventExt { device_id, event }) = ev {
                    if device_id.xinput_id() == Some(self.data.barrier_kb.dev.id as u32) {
                        if let DeviceEvent::Key(RawKeyEvent {
                            physical_key: KeyCode::Escape,
                            state: ElementState::Released,
                        }) = event
                        {
                            return;
                        }
                    }
                }
            }
        })
    }
}

bitflags::bitflags! {
    struct Protocols: u32 {
        const DELETE_WINDOW = 1 << 0;
        const PING = 1 << 1;
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum WindowState {
    Withdrawn,
    Normal,
    Iconic,
}

struct XWindow {
    el: Arc<XEventLoop>,
    id: ffi::xcb_window_t,
    format: ffi::xcb_render_directformat_t,
    parent_id: Cell<ffi::xcb_window_t>,
    winit: WWindow,
    property_generation: Cell<u32>,
    created: Cell<bool>,
    destroyed: Cell<bool>,
    mapped: Cell<bool>,
    always_on_top: Cell<bool>,
    maximized_vert: Cell<bool>,
    maximized_horz: Cell<bool>,
    fullscreen: Cell<bool>,
    pre_fs_x: Cell<i32>,
    pre_fs_y: Cell<i32>,
    pre_fs_width: Cell<u32>,
    pre_fs_height: Cell<u32>,
    pre_fs_border: Cell<u32>,
    decorations: Cell<bool>,
    border: Cell<u32>,
    x: Cell<i32>,
    y: Cell<i32>,
    x_to_be: Cell<i32>,
    y_to_be: Cell<i32>,
    width_to_be: Cell<u32>,
    height_to_be: Cell<u32>,
    border_to_be: Cell<u32>,
    width: Cell<u32>,
    height: Cell<u32>,
    min_size: Cell<Option<(u32, u32)>>,
    max_size: Cell<Option<(u32, u32)>>,
    wm_name: RefCell<String>,
    utf8_title: RefCell<String>,
    urgency: Cell<bool>,
    class: RefCell<Option<String>>,
    instance: RefCell<Option<String>>,
    protocols: Cell<Protocols>,
    desired_state: Cell<WindowState>,
    current_state: Cell<WindowState>,
    maximizable: Cell<bool>,
    icon: RefCell<Option<BackendIcon>>,
    dragging: Cell<bool>,
}

impl XWindow {
    fn upgade(&self) {
        self.property_generation
            .set(self.property_generation.get() + 1);
    }
}

impl Window for Arc<XWindow> {
    fn id(&self) -> &dyn Display {
        &self.id
    }

    fn backend(&self) -> &dyn Backend {
        &self.el.data.instance.data.backend
    }

    fn event_loop(&self) -> &dyn EventLoop {
        &self.el
    }

    fn winit(&self) -> &WWindow {
        &self.winit
    }

    fn properties_changed<'a>(&'a self) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
        struct Changed<'b>(&'b XWindow, u32);
        impl<'b> Future for Changed<'b> {
            type Output = ();
            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                if self.1 != self.0.property_generation.get() {
                    Poll::Ready(())
                } else {
                    let mut data = self.0.el.data.instance.data.wm_data.lock();
                    data.wakers.push(cx.waker().clone());
                    Poll::Pending
                }
            }
        }
        Box::pin(Changed(&self, self.property_generation.get()))
    }

    fn properties(&self) -> &dyn WindowProperties {
        self
    }

    fn set_inner_size(&self, width: u32, height: u32) {
        unsafe {
            let instance = &self.el.data.instance;
            let xcb = &instance.data.backend.xcb;
            let cookie = xcb.xcb_configure_window_checked(
                instance.c.c,
                self.id,
                (ffi::XCB_CONFIG_WINDOW_WIDTH | ffi::XCB_CONFIG_WINDOW_HEIGHT) as _,
                [width, height].as_ptr() as _,
            );
            if let Err(e) = instance.c.errors.check_cookie(xcb, cookie) {
                log::warn!("Could not resize window: {}", e);
            }
        }
    }

    fn set_background_color(&self, r: u8, g: u8, b: u8) {
        let color = b as u32 | (g as u32) << 8 | (r as u32) << 16;
        let instance = &self.el.data.instance;
        let backend = &instance.data.backend;
        unsafe {
            let cookie = backend.xcb.xcb_change_window_attributes_checked(
                self.el.data.instance.c.c,
                self.id,
                ffi::XCB_CW_BACK_PIXEL,
                &color as *const u32 as *const _,
            );
            if let Err(e) = instance.c.errors.check_cookie(&backend.xcb, cookie) {
                panic!("Could not change back pixel: {}", e);
            }
            let cookie = backend
                .xcb
                .xcb_clear_area(instance.c.c, 0, self.id, 0, 0, 0, 0);
            if let Err(e) = instance.c.errors.check_cookie(&backend.xcb, cookie) {
                panic!("Could not clear window: {}", e);
            }
        }
    }

    fn any(&self) -> &dyn Any {
        self
    }

    fn delete(&self) {
        log::info!("Deleting window {}", self.id);
        unsafe {
            let instance = &self.el.data.instance;
            let xcb = &instance.data.backend.xcb;
            let protocols = self.protocols.get();
            let cookie = if protocols.contains(Protocols::DELETE_WINDOW) {
                let event = ffi::xcb_client_message_event_t {
                    response_type: ffi::XCB_CLIENT_MESSAGE,
                    format: 32,
                    window: self.id,
                    type_: instance.data.atoms.wm_protocols,
                    data: ffi::xcb_client_message_data_t {
                        data32: [instance.data.atoms.wm_delete_window, 0, 0, 0, 0],
                    },
                    ..Default::default()
                };
                xcb.xcb_send_event_checked(instance.c.c, 0, self.id, 0, &event as *const _ as _)
            } else {
                xcb.xcb_destroy_window_checked(instance.c.c, self.id)
            };
            if let Err(e) = instance.c.errors.check_cookie(xcb, cookie) {
                log::warn!("Could not destroy window: {}", e);
            }
        }
    }

    fn frame_extents(&self) -> (u32, u32, u32, u32) {
        (
            self.border.get(),
            self.border.get(),
            self.border.get() + TITLE_HEIGHT as u32,
            self.border.get(),
        )
    }

    fn set_outer_position(&self, x: i32, y: i32) {
        log::info!("Setting outer position of {} to {}x{}", self.id, x, y);
        unsafe {
            let instance = &self.el.data.instance;
            let xcb = &instance.data.backend.xcb;
            let cookie = xcb.xcb_configure_window_checked(
                instance.c.c,
                self.parent_id.get(),
                (ffi::XCB_CONFIG_WINDOW_X | ffi::XCB_CONFIG_WINDOW_Y) as _,
                [x, y].as_ptr() as _,
            );
            if let Err(e) = instance.c.errors.check_cookie(xcb, cookie) {
                log::warn!("Could not configure window: {}", e);
            }
        }
    }

    fn ping<'a>(&'a self) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
        struct Changed<'b>(&'b XWindow);
        impl<'b> Future for Changed<'b> {
            type Output = ();
            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                let mut data = self.0.el.data.instance.data.wm_data.lock();
                if data.pongs.remove(&self.0.id) {
                    Poll::Ready(())
                } else {
                    data.wakers.push(cx.waker().clone());
                    Poll::Pending
                }
            }
        }
        log::info!("Pinging {}", self.id);
        self.el
            .data
            .instance
            .data
            .wm_data
            .lock()
            .pongs
            .remove(&self.id);
        unsafe {
            let instance = &self.el.data.instance;
            let xcb = &instance.data.backend.xcb;
            let msg = ffi::xcb_client_message_event_t {
                response_type: ffi::XCB_CLIENT_MESSAGE,
                format: 32,
                window: self.id,
                type_: instance.data.atoms.wm_protocols,
                data: ffi::xcb_client_message_data_t {
                    data32: [instance.data.atoms.net_wm_ping, 0, self.id, 0, 0],
                },
                ..Default::default()
            };
            xcb.xcb_send_event(instance.c.c, 0, self.id, 0, &msg as *const _ as _);
            xcb.xcb_flush(instance.c.c);
        }
        Box::pin(Changed(&self))
    }

    fn request_redraw(&self, _scenario: usize) {
        let msg = ffi::xcb_expose_event_t {
            response_type: ffi::XCB_EXPOSE,
            window: self.id,
            x: 0,
            y: 0,
            width: 1,
            height: 1,
            count: 0,
            ..Default::default()
        };
        unsafe {
            let xcb = &self.el.data.instance.data.backend.xcb;
            let c = &self.el.data.instance.c;
            let cookie = xcb.xcb_send_event_checked(
                c.c,
                0,
                self.id,
                ffi::XCB_EVENT_MASK_EXPOSURE,
                &msg as *const _ as _,
            );
            if let Err(e) = c.errors.check_cookie(xcb, cookie) {
                panic!("Could not send exposure event: {}", e);
            }
        }
    }
}

impl WindowProperties for Arc<XWindow> {
    fn mapped(&self) -> bool {
        self.mapped.get()
    }

    fn always_on_top(&self) -> bool {
        self.always_on_top.get()
    }

    fn decorations(&self) -> bool {
        self.decorations.get()
    }

    fn x(&self) -> i32 {
        self.x.get()
    }

    fn y(&self) -> i32 {
        self.y.get()
    }

    fn width(&self) -> u32 {
        self.width.get()
    }

    fn height(&self) -> u32 {
        self.height.get()
    }

    fn min_size(&self) -> Option<(u32, u32)> {
        self.min_size.get()
    }

    fn max_size(&self) -> Option<(u32, u32)> {
        self.max_size.get()
    }

    fn title(&self) -> Option<String> {
        let title = self.wm_name.borrow();
        let utf8_title = self.utf8_title.borrow();
        if *title == *utf8_title {
            return Some(title.to_string());
        }
        None
    }

    fn maximized(&self) -> Option<bool> {
        if self.maximized_vert.get() == self.maximized_horz.get() {
            Some(self.maximized_vert.get())
        } else {
            None
        }
    }

    fn minimized(&self) -> Option<bool> {
        Some(self.current_state.get() == WindowState::Iconic)
    }

    fn resizable(&self) -> Option<bool> {
        Some(
            self.max_size() != Some((self.width(), self.height()))
                || self.max_size() != self.min_size(),
        )
    }

    fn attention(&self) -> bool {
        self.urgency.get()
    }

    fn class(&self) -> Option<String> {
        self.class.borrow().clone()
    }

    fn instance(&self) -> Option<String> {
        self.instance.borrow().clone()
    }

    fn supports_transparency(&self) -> bool {
        self.format.alpha_mask != 0
    }

    fn dragging(&self) -> bool {
        self.dragging.get()
    }

    fn icon(&self) -> Option<BackendIcon> {
        self.icon.borrow().clone()
    }

    fn fullscreen(&self) -> bool {
        self.fullscreen.get()
    }
}

impl Drop for XWindow {
    fn drop(&mut self) {
        let data = &self.el.data.instance;
        data.data.wm_data.lock().windows.remove(&self.id);
    }
}

struct XSeat {
    instance: Arc<XInstance>,
    pointer: ffi::xcb_input_device_id_t,
    keyboard: ffi::xcb_input_device_id_t,
    layout: Arc<Cell<Layout>>,
}

impl XSeat {
    fn focus2(&self, window: ffi::xcb_window_t) {
        unsafe {
            let cookie = self
                .instance
                .data
                .backend
                .xinput
                .xcb_input_xi_set_focus_checked(self.instance.c.c, window, 0, self.keyboard);
            if let Err(e) = self
                .instance
                .c
                .errors
                .check_cookie(&self.instance.data.backend.xcb, cookie)
            {
                panic!("Could not set focus: {}", e);
            }
        }
    }
}

fn add_keyboard(seat: &Arc<XSeat>) -> Arc<XKeyboard> {
    let id = seat.instance.add_keyboard();
    log::info!("Created keyboard {} on seat {}", id, seat.keyboard);
    seat.instance.assign_slave(id, seat.keyboard);
    seat.instance.set_layout(id, seat.layout.get(), None);
    seat.instance
        .set_layout(seat.keyboard, seat.layout.get(), None);
    Arc::new(XKeyboard {
        pressed_keys: Default::default(),
        dev: XDevice {
            seat: seat.clone(),
            id,
        },
    })
}

impl Seat for Arc<XSeat> {
    fn add_keyboard(&self) -> Box<dyn Keyboard> {
        Box::new(add_keyboard(self))
    }

    fn add_mouse(&self) -> Box<dyn Mouse> {
        let id = self.instance.add_mouse();
        log::info!("Created mouse {} on seat {}", id, self.keyboard);
        self.instance.assign_slave(id, self.pointer);
        Box::new(Arc::new(XMouse {
            pressed_buttons: Default::default(),
            dev: XDevice {
                seat: self.clone(),
                id,
            },
        }))
    }

    fn add_touchscreen(&self) -> Box<dyn Touchscreen> {
        let id = self.instance.add_touchscreen();
        log::info!("Created touchscreen {} on seat {}", id, self.keyboard);
        self.instance.assign_slave(id, self.pointer);
        Box::new(Arc::new(XTouch {
            dev: XDevice {
                seat: self.clone(),
                id,
            },
        }))
    }

    fn focus(&self, window: &dyn Window) {
        let window: &Arc<XWindow> = window.any().downcast_ref().unwrap();
        log::info!("Focusing seat {} on window {}", self.keyboard, window.id);
        self.focus2(window.id);
    }

    fn un_focus(&self) {
        log::info!("Unfocusing seat {}", self.keyboard);
        self.focus2(0);
    }

    fn set_layout(&self, layout: Layout) {
        log::info!("Setting layout of seat {} to {:?}", self.keyboard, layout);
        self.instance
            .set_layout(self.keyboard, layout, Some(self.layout.get()));
        self.layout.set(layout);
    }

    fn set_cursor_position(&self, x: i32, y: i32) {
        log::info!("Moving cursor of seat {} to {}x{}", self.keyboard, x, y);
        let xinput = &self.instance.data.backend.xinput;
        let xcb = &self.instance.data.backend.xcb;
        let c = &self.instance.c;
        unsafe {
            let cookie = xinput.xcb_input_xi_warp_pointer_checked(
                c.c,
                0,
                c.screen.root,
                0,
                0,
                0,
                0,
                x << 16,
                y << 16,
                self.pointer,
            );
            if let Err(e) = c.errors.check_cookie(xcb, cookie) {
                panic!("Could not warp pointer: {}", e);
            }
        }
    }

    fn cursor_position(&self) -> (i32, i32) {
        unsafe {
            let xcb = &self.instance.data.backend.xcb;
            let xinput = &self.instance.data.backend.xinput;
            let mut err = ptr::null_mut();
            let reply = xinput.xcb_input_xi_query_pointer_reply(
                self.instance.c.c,
                xinput.xcb_input_xi_query_pointer(
                    self.instance.c.c,
                    self.instance.c.screen.root,
                    self.pointer,
                ),
                &mut err,
            );
            let reply = match self.instance.c.errors.check(xcb, reply, err) {
                Ok(r) => r,
                Err(e) => panic!("Could not query pointer: {}", e),
            };
            (reply.root_x >> 16, reply.root_y >> 16)
        }
    }

    fn is(&self, device_id: DeviceId) -> bool {
        Some(self.pointer as u32) == device_id.xinput_id()
            || Some(self.keyboard as u32) == device_id.xinput_id()
    }
}

impl Drop for XSeat {
    fn drop(&mut self) {
        if self.keyboard == self.instance.core_kb {
            return;
        }
        unsafe {
            let instance = &self.instance;
            let xinput = &instance.data.backend.xinput;
            let xcb = &instance.data.backend.xcb;
            #[repr(C)]
            struct Change {
                hc: ffi::xcb_input_hierarchy_change_t,
                data: ffi::xcb_input_hierarchy_change_data_t__remove_master,
            }
            let change = Change {
                hc: ffi::xcb_input_hierarchy_change_t {
                    type_: ffi::XCB_INPUT_HIERARCHY_CHANGE_TYPE_REMOVE_MASTER as _,
                    len: (mem::size_of::<Change>() / 4) as _,
                },
                data: ffi::xcb_input_hierarchy_change_data_t__remove_master {
                    deviceid: self.keyboard,
                    return_mode: ffi::XCB_INPUT_CHANGE_MODE_FLOAT as _,
                    ..Default::default()
                },
            };
            let cookie = xinput.xcb_input_xi_change_hierarchy_checked(instance.c.c, 1, &change.hc);
            if let Err(e) = instance.c.errors.check_cookie(xcb, cookie) {
                log::warn!("Could not remove master: {}", e);
            }
        }
    }
}

struct XDevice {
    seat: Arc<XSeat>,
    id: ffi::xcb_input_device_id_t,
}

impl Drop for XDevice {
    fn drop(&mut self) {
        let msg = Message {
            remove_device: RemoveDevice {
                ty: MT_REMOVE_DEVICE as _,
                id: self.id as _,
            },
        };
        uapi::write(self.seat.instance.data.sock.raw(), &msg).unwrap();
    }
}

struct XDeviceId {
    id: ffi::xcb_input_device_id_t,
}

impl BackendDeviceId for XDeviceId {
    fn is(&self, device: DeviceId) -> bool {
        device.xinput_id() == Some(self.id as u32)
    }
}

struct XMouse {
    pressed_buttons: Mutex<HashMap<Button, Weak<XPressedButton>>>,
    dev: XDevice,
}

impl Device for Arc<XMouse> {
    fn id(&self) -> Box<dyn BackendDeviceId> {
        Box::new(XDeviceId { id: self.dev.id })
    }
}

impl Mouse for Arc<XMouse> {
    fn press(&self, button: Button) -> Box<dyn PressedButton> {
        log::info!(
            "Pressing button {:?} of mouse {} of seat {}",
            button,
            self.dev.id,
            self.dev.seat.keyboard
        );
        let mut buttons = self.pressed_buttons.lock();
        if let Some(p) = buttons.get(&button) {
            if let Some(p) = p.upgrade() {
                return Box::new(p);
            }
        }
        let msg = Message {
            key_press: KeyPress {
                ty: MT_BUTTON_PRESS as _,
                id: self.dev.id as _,
                key: map_button(button),
            },
        };
        uapi::write(self.dev.seat.instance.data.sock.raw(), &msg).unwrap();
        let p = Arc::new(XPressedButton {
            mouse: self.clone(),
            button,
        });
        buttons.insert(button, Arc::downgrade(&p));
        Box::new(p)
    }

    fn move_(&self, dx: i32, dy: i32) {
        log::info!(
            "Moving mouse {} of seat {} by {}x{}",
            self.dev.id,
            self.dev.seat.keyboard,
            dx,
            dy
        );
        let msg = Message {
            mouse_move: MouseMove {
                ty: MT_MOUSE_MOVE as _,
                id: self.dev.id as _,
                dx,
                dy,
            },
        };
        uapi::write(self.dev.seat.instance.data.sock.raw(), &msg).unwrap();
    }

    fn scroll(&self, dx: i32, dy: i32) {
        log::info!(
            "Scrolling mouse {} of seat {} by {}x{}",
            self.dev.id,
            self.dev.seat.keyboard,
            dx,
            dy
        );
        let msg = Message {
            mouse_move: MouseMove {
                ty: MT_MOUSE_SCROLL as _,
                id: self.dev.id as _,
                dx,
                dy: -dy,
            },
        };
        uapi::write(self.dev.seat.instance.data.sock.raw(), &msg).unwrap();
    }
}

struct XKeyboard {
    pressed_keys: Mutex<HashMap<Key, Weak<XPressedKey>>>,
    dev: XDevice,
}

impl Device for Arc<XKeyboard> {
    fn id(&self) -> Box<dyn BackendDeviceId> {
        Box::new(XDeviceId { id: self.dev.id })
    }
}

impl Keyboard for Arc<XKeyboard> {
    fn press(&self, key: Key) -> Box<dyn PressedKey> {
        log::info!(
            "Pressing key {:?} of keyboard {} of seat {}",
            key,
            self.dev.id,
            self.dev.seat.keyboard
        );
        let mut keys = self.pressed_keys.lock();
        if let Some(p) = keys.get(&key) {
            if let Some(p) = p.upgrade() {
                log::info!("Key already pressed");
                return Box::new(p);
            }
        }
        let msg = Message {
            key_press: KeyPress {
                ty: MT_KEY_PRESS as _,
                id: self.dev.id as _,
                key: evdev::map_key(key),
            },
        };
        uapi::write(self.dev.seat.instance.data.sock.raw(), &msg).unwrap();
        let p = Arc::new(XPressedKey {
            kb: self.clone(),
            key,
        });
        keys.insert(key, Arc::downgrade(&p));
        Box::new(p)
    }
}

struct XPressedButton {
    mouse: Arc<XMouse>,
    button: Button,
}

impl PressedButton for Arc<XPressedButton> {}

impl Drop for XPressedButton {
    fn drop(&mut self) {
        let msg = Message {
            key_press: KeyPress {
                ty: MT_BUTTON_RELEASE as _,
                id: self.mouse.dev.id as _,
                key: map_button(self.button),
            },
        };
        uapi::write(self.mouse.dev.seat.instance.data.sock.raw(), &msg).unwrap();
    }
}

struct XPressedKey {
    kb: Arc<XKeyboard>,
    key: Key,
}

impl PressedKey for Arc<XPressedKey> {}

impl Drop for XPressedKey {
    fn drop(&mut self) {
        log::info!("Releasing key {:?}", self.key);
        let msg = Message {
            key_press: KeyPress {
                ty: MT_KEY_RELEASE as _,
                id: self.kb.dev.id as _,
                key: evdev::map_key(self.key),
            },
        };
        uapi::write(self.kb.dev.seat.instance.data.sock.raw(), &msg).unwrap();
    }
}

struct XTouch {
    dev: XDevice,
}

impl Device for Arc<XTouch> {
    fn id(&self) -> Box<dyn BackendDeviceId> {
        Box::new(XDeviceId { id: self.dev.id })
    }
}

impl Touchscreen for Arc<XTouch> {
    fn down(&self, x: i32, y: i32) -> Box<dyn Finger> {
        let mut msg = Message {
            touch_down: TouchDown {
                ty: MT_TOUCH_DOWN as _,
                id: self.dev.id as _,
                x,
                y,
            },
        };
        uapi::write(self.dev.seat.instance.data.sock.raw(), &msg).unwrap();
        uapi::read(self.dev.seat.instance.data.sock.raw(), &mut msg).unwrap();
        unsafe {
            assert_eq!(msg.ty, MT_TOUCH_DOWN_REPLY as _);
            Box::new(XFinger {
                touch: self.clone(),
                touch_id: msg.touch_down_reply.touch_id,
            })
        }
    }
}

struct XFinger {
    touch: Arc<XTouch>,
    touch_id: u32,
}

impl Finger for XFinger {
    fn move_(&self, x: i32, y: i32) {
        let msg = Message {
            touch_move: TouchMove {
                ty: MT_TOUCH_MOVE as _,
                id: self.touch.dev.id as _,
                touch_id: self.touch_id,
                x,
                y,
            },
        };
        uapi::write(self.touch.dev.seat.instance.data.sock.raw(), &msg).unwrap();
    }
}

impl Drop for XFinger {
    fn drop(&mut self) {
        let msg = Message {
            touch_up: TouchUp {
                ty: MT_TOUCH_UP as _,
                id: self.touch.dev.id as _,
                touch_id: self.touch_id,
            },
        };
        uapi::write(self.touch.dev.seat.instance.data.sock.raw(), &msg).unwrap();
    }
}

fn map_button(button: Button) -> u32 {
    match button {
        Button::Left => 1,
        Button::Right => 2,
        Button::Middle => 3,
        Button::Back => 8,
        Button::Forward => 9,
    }
}

const CONFIG: &str = r#"
Section "Device"
    Identifier  "winit device"
    Driver      "winit"
EndSection

Section "Screen"
    Identifier  "winit screen"
    Device      "winit device"
EndSection

Section "Serverlayout"
    Identifier  "winit layout"
    Screen      "winit screen"
EndSection
"#;

#[repr(u32)]
#[allow(dead_code, non_camel_case_types)]
enum MessageType {
    MT_NONE,
    MT_CREATE_KEYBOARD,
    MT_CREATE_KEYBOARD_REPLY,
    MT_KEY_PRESS,
    MT_KEY_RELEASE,
    MT_REMOVE_DEVICE,
    MT_ENABLE_SECOND_MONITOR,
    MT_ENABLE_SECOND_MONITOR_REPLY,
    MT_GET_VIDEO_INFO,
    MT_GET_VIDEO_INFO_REPLY,
    MT_CREATE_MOUSE,
    MT_CREATE_MOUSE_REPLY,
    MT_BUTTON_PRESS,
    MT_BUTTON_RELEASE,
    MT_MOUSE_MOVE,
    MT_MOUSE_SCROLL,
    MT_CREATE_TOUCH,
    MT_CREATE_TOUCH_REPLY,
    MT_TOUCH_DOWN,
    MT_TOUCH_DOWN_REPLY,
    MT_TOUCH_UP,
    MT_TOUCH_MOVE,
}

#[repr(C)]
#[derive(Copy, Clone)]
union Message {
    ty: u32,
    create_keyboard_reply: CreateKeyboardReply,
    key_press: KeyPress,
    remove_device: RemoveDevice,
    enable_second_monitor: EnableSecondMonitor,
    get_video_info_reply: GetVideoInfoReply,
    mouse_move: MouseMove,
    touch_move: TouchMove,
    touch_down: TouchDown,
    touch_down_reply: TouchDownReply,
    touch_up: TouchUp,
}

unsafe impl Pod for Message {}

#[repr(C)]
#[derive(Copy, Clone)]
struct CreateKeyboardReply {
    ty: u32,
    id: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct TouchDownReply {
    ty: u32,
    touch_id: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct TouchUp {
    ty: u32,
    id: u32,
    touch_id: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct TouchDown {
    ty: u32,
    id: u32,
    x: i32,
    y: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct TouchMove {
    ty: u32,
    id: u32,
    touch_id: u32,
    x: i32,
    y: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct EnableSecondMonitor {
    ty: u32,
    enable: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct GetVideoInfoReply {
    ty: u32,
    second_crtc: u32,
    second_output: u32,
    first_output: u32,
    large_mode_id: u32,
    small_mode_id: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct KeyPress {
    ty: u32,
    id: u32,
    key: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct MouseMove {
    ty: u32,
    id: u32,
    dx: i32,
    dy: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct RemoveDevice {
    ty: u32,
    id: u32,
}

#[derive(Default)]
struct Atoms {
    net_wm_state: ffi::xcb_atom_t,
    wm_change_state: ffi::xcb_atom_t,
    wm_state: ffi::xcb_atom_t,
    net_wm_name: ffi::xcb_atom_t,
    net_wm_icon: ffi::xcb_atom_t,
    wm_delete_window: ffi::xcb_atom_t,
    net_wm_ping: ffi::xcb_atom_t,
    utf8_string: ffi::xcb_atom_t,
    net_wm_state_above: ffi::xcb_atom_t,
    net_wm_state_fullscreen: ffi::xcb_atom_t,
    net_frame_extents: ffi::xcb_atom_t,
    net_wm_state_maximized_horz: ffi::xcb_atom_t,
    net_wm_state_maximized_vert: ffi::xcb_atom_t,
    motif_wm_hints: ffi::xcb_atom_t,
    wm_name: ffi::xcb_atom_t,
    wm_normal_hints: ffi::xcb_atom_t,
    wm_hints: ffi::xcb_atom_t,
    wm_class: ffi::xcb_atom_t,
    wm_protocols: ffi::xcb_atom_t,
    net_active_window: ffi::xcb_atom_t,
    net_supported: ffi::xcb_atom_t,
    net_client_list: ffi::xcb_atom_t,
    net_client_list_stacking: ffi::xcb_atom_t,
    net_supporting_wm_check: ffi::xcb_atom_t,
    net_wm_moveresize: ffi::xcb_atom_t,
    x_dnd_aware: ffi::xcb_atom_t,
    x_dnd_selection: ffi::xcb_atom_t,
    x_dnd_enter: ffi::xcb_atom_t,
    x_dnd_type_list: ffi::xcb_atom_t,
    x_dnd_position: ffi::xcb_atom_t,
    x_dnd_action_copy: ffi::xcb_atom_t,
    x_dnd_action_private: ffi::xcb_atom_t,
    x_dnd_status: ffi::xcb_atom_t,
    x_dnd_leave: ffi::xcb_atom_t,
    x_dnd_drop: ffi::xcb_atom_t,
    uri_list: ffi::xcb_atom_t,
}
