#![cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "netbsd", target_os = "openbsd"))]

mod dnd;
mod event_processor;
mod events;
pub mod ffi;
mod ime;
mod monitor;
pub mod util;
mod window;
mod xdisplay;

pub use self::{
    monitor::MonitorHandle,
    window::UnownedWindow,
    xdisplay::{XConnection, XError, XNotSupported},
};

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet, VecDeque},
    ffi::CStr,
    mem,
    ops::Deref,
    os::raw::*,
    rc::Rc,
    slice,
    sync::{mpsc, Arc, Mutex, Weak},
};

use libc::{self, setlocale, LC_CTYPE};

use self::{
    dnd::{Dnd, DndState},
    event_processor::EventProcessor,
    ime::{Ime, ImeCreationError, ImeReceiver, ImeSender},
};
use crate::{
    error::OsError as RootOsError,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopClosed, EventLoopWindowTarget as RootELW},
    platform_impl::{platform::sticky_exit_callback, PlatformSpecificWindowBuilderAttributes},
    window::WindowAttributes,
};

pub struct EventLoopWindowTarget<T> {
    xconn: Arc<XConnection>,
    wm_delete_window: ffi::Atom,
    net_wm_ping: ffi::Atom,
    ime_sender: ImeSender,
    root: ffi::Window,
    ime: RefCell<Ime>,
    windows: RefCell<HashMap<WindowId, Weak<UnownedWindow>>>,
    pending_redraws: Arc<Mutex<HashSet<WindowId>>>,
    _marker: ::std::marker::PhantomData<T>,
}

pub struct EventLoop<T: 'static> {
    inner_loop: ::calloop::EventLoop<()>,
    _x11_source: ::calloop::Source<::calloop::generic::Generic<::calloop::generic::EventedRawFd>>,
    _user_source: ::calloop::Source<::calloop::channel::Channel<T>>,
    pending_user_events: Rc<RefCell<VecDeque<T>>>,
    user_sender: ::calloop::channel::Sender<T>,
    pending_events: Rc<RefCell<VecDeque<Event<T>>>>,
    target: Rc<RootELW<T>>,
}

#[derive(Clone)]
pub struct EventLoopProxy<T: 'static> {
    user_sender: ::calloop::channel::Sender<T>,
}

impl<T: 'static> EventLoop<T> {
    pub fn new(xconn: Arc<XConnection>) -> EventLoop<T> {
        let root = unsafe { (xconn.xlib.XDefaultRootWindow)(xconn.display) };

        let wm_delete_window = unsafe { xconn.get_atom_unchecked(b"WM_DELETE_WINDOW\0") };

        let net_wm_ping = unsafe { xconn.get_atom_unchecked(b"_NET_WM_PING\0") };

        let dnd = Dnd::new(Arc::clone(&xconn))
            .expect("Failed to call XInternAtoms when initializing drag and drop");

        let (ime_sender, ime_receiver) = mpsc::channel();
        // Input methods will open successfully without setting the locale, but it won't be
        // possible to actually commit pre-edit sequences.
        unsafe {
            setlocale(LC_CTYPE, b"\0".as_ptr() as *const _);
        }
        let ime = RefCell::new({
            let result = Ime::new(Arc::clone(&xconn));
            if let Err(ImeCreationError::OpenFailure(ref state)) = result {
                panic!(format!("Failed to open input method: {:#?}", state));
            }
            result.expect("Failed to set input method destruction callback")
        });

        let randr_event_offset = xconn
            .select_xrandr_input(root)
            .expect("Failed to query XRandR extension");

        let xi2ext = unsafe {
            let mut result = XExtension {
                opcode: mem::uninitialized(),
                first_event_id: mem::uninitialized(),
                first_error_id: mem::uninitialized(),
            };
            let res = (xconn.xlib.XQueryExtension)(
                xconn.display,
                b"XInputExtension\0".as_ptr() as *const c_char,
                &mut result.opcode as *mut c_int,
                &mut result.first_event_id as *mut c_int,
                &mut result.first_error_id as *mut c_int,
            );
            if res == ffi::False {
                panic!("X server missing XInput extension");
            }
            result
        };

        unsafe {
            let mut xinput_major_ver = ffi::XI_2_Major;
            let mut xinput_minor_ver = ffi::XI_2_Minor;
            if (xconn.xinput2.XIQueryVersion)(
                xconn.display,
                &mut xinput_major_ver,
                &mut xinput_minor_ver,
            ) != ffi::Success as libc::c_int
            {
                panic!(
                    "X server has XInput extension {}.{} but does not support XInput2",
                    xinput_major_ver, xinput_minor_ver,
                );
            }
        }

        xconn.update_cached_wm_info(root);

        let target = Rc::new(RootELW {
            p: super::EventLoopWindowTarget::X(EventLoopWindowTarget {
                ime,
                root,
                windows: Default::default(),
                _marker: ::std::marker::PhantomData,
                ime_sender,
                xconn,
                wm_delete_window,
                net_wm_ping,
                pending_redraws: Default::default(),
            }),
            _marker: ::std::marker::PhantomData,
        });

        // A calloop event loop to drive us
        let inner_loop = ::calloop::EventLoop::new().unwrap();

        // Handle user events
        let pending_user_events = Rc::new(RefCell::new(VecDeque::new()));
        let pending_user_events2 = pending_user_events.clone();

        let (user_sender, user_channel) = ::calloop::channel::channel();

        let _user_source = inner_loop
            .handle()
            .insert_source(user_channel, move |evt, &mut ()| {
                if let ::calloop::channel::Event::Msg(msg) = evt {
                    pending_user_events2.borrow_mut().push_back(msg);
                }
            })
            .unwrap();

        // Handle X11 events
        let pending_events: Rc<RefCell<VecDeque<_>>> = Default::default();

        let mut processor = EventProcessor {
            target: target.clone(),
            dnd,
            devices: Default::default(),
            randr_event_offset,
            ime_receiver,
            xi2ext,
        };

        // Register for device hotplug events
        // (The request buffer is flushed during `init_device`)
        get_xtarget(&target)
            .xconn
            .select_xinput_events(root, ffi::XIAllDevices, ffi::XI_HierarchyChangedMask)
            .queue();

        processor.init_device(ffi::XIAllDevices);

        // Setup the X11 event source
        let mut x11_events =
            ::calloop::generic::Generic::from_raw_fd(get_xtarget(&target).xconn.x11_fd);
        x11_events.set_interest(::calloop::mio::Ready::readable());
        let _x11_source = inner_loop
            .handle()
            .insert_source(x11_events, {
                let pending_events = pending_events.clone();
                let mut callback = move |event| {
                    pending_events.borrow_mut().push_back(event);
                };
                move |evt, &mut ()| {
                    if evt.readiness.is_readable() {
                        // process all pending events
                        let mut xev = unsafe { mem::uninitialized() };
                        while unsafe { processor.poll_one_event(&mut xev) } {
                            processor.process_event(&mut xev, &mut callback);
                        }
                    }
                }
            })
            .unwrap();

        let result = EventLoop {
            inner_loop,
            pending_events,
            _x11_source,
            _user_source,
            user_sender,
            pending_user_events,
            target,
        };

        result
    }

    /// Returns the `XConnection` of this events loop.
    #[inline]
    pub fn x_connection(&self) -> &Arc<XConnection> {
        &get_xtarget(&self.target).xconn
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy {
            user_sender: self.user_sender.clone(),
        }
    }

    pub(crate) fn window_target(&self) -> &RootELW<T> {
        &self.target
    }

    pub fn run_return<F>(&mut self, mut callback: F)
    where
        F: FnMut(Event<T>, &RootELW<T>, &mut ControlFlow),
    {
        let mut control_flow = ControlFlow::default();
        let wt = get_xtarget(&self.target);

        callback(
            crate::event::Event::NewEvents(crate::event::StartCause::Init),
            &self.target,
            &mut control_flow,
        );

        loop {
            // Empty the event buffer
            {
                let mut guard = self.pending_events.borrow_mut();
                for evt in guard.drain(..) {
                    sticky_exit_callback(evt, &self.target, &mut control_flow, &mut callback);
                }
            }

            // Empty the user event buffer
            {
                let mut guard = self.pending_user_events.borrow_mut();
                for evt in guard.drain(..) {
                    sticky_exit_callback(
                        crate::event::Event::UserEvent(evt),
                        &self.target,
                        &mut control_flow,
                        &mut callback,
                    );
                }
            }
            // Empty the redraw requests
            {
                let mut guard = wt.pending_redraws.lock().unwrap();
                for wid in guard.drain() {
                    sticky_exit_callback(
                        Event::WindowEvent {
                            window_id: crate::window::WindowId(super::WindowId::X(wid)),
                            event: WindowEvent::RedrawRequested,
                        },
                        &self.target,
                        &mut control_flow,
                        &mut callback,
                    );
                }
            }
            // send Events cleared
            {
                sticky_exit_callback(
                    crate::event::Event::EventsCleared,
                    &self.target,
                    &mut control_flow,
                    &mut callback,
                );
            }

            match control_flow {
                ControlFlow::Exit => break,
                ControlFlow::Poll => {
                    // non-blocking dispatch
                    self.inner_loop
                        .dispatch(Some(::std::time::Duration::from_millis(0)), &mut ())
                        .unwrap();
                    callback(
                        crate::event::Event::NewEvents(crate::event::StartCause::Poll),
                        &self.target,
                        &mut control_flow,
                    );
                }
                ControlFlow::Wait => {
                    self.inner_loop.dispatch(None, &mut ()).unwrap();
                    callback(
                        crate::event::Event::NewEvents(crate::event::StartCause::WaitCancelled {
                            start: ::std::time::Instant::now(),
                            requested_resume: None,
                        }),
                        &self.target,
                        &mut control_flow,
                    );
                }
                ControlFlow::WaitUntil(deadline) => {
                    let start = ::std::time::Instant::now();
                    // compute the blocking duration
                    let duration = if deadline > start {
                        deadline - start
                    } else {
                        ::std::time::Duration::from_millis(0)
                    };
                    self.inner_loop.dispatch(Some(duration), &mut ()).unwrap();
                    let now = std::time::Instant::now();
                    if now < deadline {
                        callback(
                            crate::event::Event::NewEvents(
                                crate::event::StartCause::WaitCancelled {
                                    start,
                                    requested_resume: Some(deadline),
                                },
                            ),
                            &self.target,
                            &mut control_flow,
                        );
                    } else {
                        callback(
                            crate::event::Event::NewEvents(
                                crate::event::StartCause::ResumeTimeReached {
                                    start,
                                    requested_resume: deadline,
                                },
                            ),
                            &self.target,
                            &mut control_flow,
                        );
                    }
                }
            }
        }

        callback(
            crate::event::Event::LoopDestroyed,
            &self.target,
            &mut control_flow,
        );
    }

    pub fn run<F>(mut self, callback: F) -> !
    where
        F: 'static + FnMut(Event<T>, &RootELW<T>, &mut ControlFlow),
    {
        self.run_return(callback);
        ::std::process::exit(0);
    }
}

fn get_xtarget<T>(rt: &RootELW<T>) -> &EventLoopWindowTarget<T> {
    if let super::EventLoopWindowTarget::X(ref target) = rt.p {
        target
    } else {
        unreachable!();
    }
}

impl<T: 'static> EventLoopProxy<T> {
    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed> {
        self.user_sender.send(event).map_err(|_| EventLoopClosed)
    }
}

struct DeviceInfo<'a> {
    xconn: &'a XConnection,
    info: *const ffi::XIDeviceInfo,
    count: usize,
}

impl<'a> DeviceInfo<'a> {
    fn get(xconn: &'a XConnection, device: c_int) -> Option<Self> {
        unsafe {
            let mut count = mem::uninitialized();
            let info = (xconn.xinput2.XIQueryDevice)(xconn.display, device, &mut count);
            xconn.check_errors().ok().and_then(|_| {
                if info.is_null() || count == 0 {
                    None
                } else {
                    Some(DeviceInfo {
                        xconn,
                        info,
                        count: count as usize,
                    })
                }
            })
        }
    }
}

impl<'a> Drop for DeviceInfo<'a> {
    fn drop(&mut self) {
        assert!(!self.info.is_null());
        unsafe { (self.xconn.xinput2.XIFreeDeviceInfo)(self.info as *mut _) };
    }
}

impl<'a> Deref for DeviceInfo<'a> {
    type Target = [ffi::XIDeviceInfo];
    fn deref(&self) -> &Self::Target {
        unsafe { slice::from_raw_parts(self.info, self.count) }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(ffi::Window);

impl WindowId {
    pub unsafe fn dummy() -> Self {
        WindowId(0)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId(c_int);

impl DeviceId {
    pub unsafe fn dummy() -> Self {
        DeviceId(0)
    }
}

pub struct Window(Arc<UnownedWindow>);

impl Deref for Window {
    type Target = UnownedWindow;
    #[inline]
    fn deref(&self) -> &UnownedWindow {
        &*self.0
    }
}

impl Window {
    pub fn new<T>(
        event_loop: &EventLoopWindowTarget<T>,
        attribs: WindowAttributes,
        pl_attribs: PlatformSpecificWindowBuilderAttributes,
    ) -> Result<Self, RootOsError> {
        let window = Arc::new(UnownedWindow::new(&event_loop, attribs, pl_attribs)?);
        event_loop
            .windows
            .borrow_mut()
            .insert(window.id(), Arc::downgrade(&window));
        Ok(Window(window))
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        let window = self.deref();
        let xconn = &window.xconn;
        unsafe {
            (xconn.xlib.XDestroyWindow)(xconn.display, window.id().0);
            // If the window was somehow already destroyed, we'll get a `BadWindow` error, which we don't care about.
            let _ = xconn.check_errors();
        }
    }
}

/// XEvents of type GenericEvent store their actual data in an XGenericEventCookie data structure. This is a wrapper to
/// extract the cookie from a GenericEvent XEvent and release the cookie data once it has been processed
struct GenericEventCookie<'a> {
    xconn: &'a XConnection,
    cookie: ffi::XGenericEventCookie,
}

impl<'a> GenericEventCookie<'a> {
    fn from_event<'b>(
        xconn: &'b XConnection,
        event: ffi::XEvent,
    ) -> Option<GenericEventCookie<'b>> {
        unsafe {
            let mut cookie: ffi::XGenericEventCookie = From::from(event);
            if (xconn.xlib.XGetEventData)(xconn.display, &mut cookie) == ffi::True {
                Some(GenericEventCookie { xconn, cookie })
            } else {
                None
            }
        }
    }
}

impl<'a> Drop for GenericEventCookie<'a> {
    fn drop(&mut self) {
        unsafe {
            (self.xconn.xlib.XFreeEventData)(self.xconn.display, &mut self.cookie);
        }
    }
}

#[derive(Debug, Copy, Clone)]
struct XExtension {
    opcode: c_int,
    first_event_id: c_int,
    first_error_id: c_int,
}

fn mkwid(w: ffi::Window) -> crate::window::WindowId {
    crate::window::WindowId(crate::platform_impl::WindowId::X(WindowId(w)))
}
fn mkdid(w: c_int) -> crate::event::DeviceId {
    crate::event::DeviceId(crate::platform_impl::DeviceId::X(DeviceId(w)))
}

#[derive(Debug)]
struct Device {
    name: String,
    scroll_axes: Vec<(i32, ScrollAxis)>,
    // For master devices, this is the paired device (pointer <-> keyboard).
    // For slave devices, this is the master.
    attachment: c_int,
}

#[derive(Debug, Copy, Clone)]
struct ScrollAxis {
    increment: f64,
    orientation: ScrollOrientation,
    position: f64,
}

#[derive(Debug, Copy, Clone)]
enum ScrollOrientation {
    Vertical,
    Horizontal,
}

impl Device {
    fn new<T: 'static>(el: &EventProcessor<T>, info: &ffi::XIDeviceInfo) -> Self {
        let name = unsafe { CStr::from_ptr(info.name).to_string_lossy() };
        let mut scroll_axes = Vec::new();

        let wt = get_xtarget(&el.target);

        if Device::physical_device(info) {
            // Register for global raw events
            let mask = ffi::XI_RawMotionMask
                | ffi::XI_RawButtonPressMask
                | ffi::XI_RawButtonReleaseMask
                | ffi::XI_RawKeyPressMask
                | ffi::XI_RawKeyReleaseMask;
            // The request buffer is flushed when we poll for events
            wt.xconn
                .select_xinput_events(wt.root, info.deviceid, mask)
                .queue();

            // Identify scroll axes
            for class_ptr in Device::classes(info) {
                let class = unsafe { &**class_ptr };
                match class._type {
                    ffi::XIScrollClass => {
                        let info = unsafe {
                            mem::transmute::<&ffi::XIAnyClassInfo, &ffi::XIScrollClassInfo>(class)
                        };
                        scroll_axes.push((
                            info.number,
                            ScrollAxis {
                                increment: info.increment,
                                orientation: match info.scroll_type {
                                    ffi::XIScrollTypeHorizontal => ScrollOrientation::Horizontal,
                                    ffi::XIScrollTypeVertical => ScrollOrientation::Vertical,
                                    _ => unreachable!(),
                                },
                                position: 0.0,
                            },
                        ));
                    }
                    _ => {}
                }
            }
        }

        let mut device = Device {
            name: name.into_owned(),
            scroll_axes,
            attachment: info.attachment,
        };
        device.reset_scroll_position(info);
        device
    }

    fn reset_scroll_position(&mut self, info: &ffi::XIDeviceInfo) {
        if Device::physical_device(info) {
            for class_ptr in Device::classes(info) {
                let class = unsafe { &**class_ptr };
                match class._type {
                    ffi::XIValuatorClass => {
                        let info = unsafe {
                            mem::transmute::<&ffi::XIAnyClassInfo, &ffi::XIValuatorClassInfo>(class)
                        };
                        if let Some(&mut (_, ref mut axis)) = self
                            .scroll_axes
                            .iter_mut()
                            .find(|&&mut (axis, _)| axis == info.number)
                        {
                            axis.position = info.value;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    #[inline]
    fn physical_device(info: &ffi::XIDeviceInfo) -> bool {
        info._use == ffi::XISlaveKeyboard
            || info._use == ffi::XISlavePointer
            || info._use == ffi::XIFloatingSlave
    }

    #[inline]
    fn classes(info: &ffi::XIDeviceInfo) -> &[*const ffi::XIAnyClassInfo] {
        unsafe {
            slice::from_raw_parts(
                info.classes as *const *const ffi::XIAnyClassInfo,
                info.num_classes as usize,
            )
        }
    }
}
