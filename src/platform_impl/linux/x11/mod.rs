#![cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]

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
    monitor::{MonitorHandle, VideoMode},
    window::UnownedWindow,
    xdisplay::{XConnection, XError, XNotSupported},
};

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    ffi::CStr,
    mem::{self, MaybeUninit},
    ops::Deref,
    os::raw::*,
    ptr,
    rc::Rc,
    slice,
    sync::mpsc::{Receiver, Sender},
    sync::{mpsc, Arc, Weak},
    time::{Duration, Instant},
};

use libc::{self, setlocale, LC_CTYPE};

use mio::{unix::SourceFd, Events, Interest, Poll, Token, Waker};

use self::{
    dnd::{Dnd, DndState},
    event_processor::EventProcessor,
    ime::{Ime, ImeCreationError, ImeReceiver, ImeSender},
    util::modifiers::ModifierKeymap,
};
use crate::{
    error::OsError as RootOsError,
    event::{Event, StartCause},
    event_loop::{ControlFlow, EventLoopClosed, EventLoopWindowTarget as RootELW},
    platform_impl::{platform::sticky_exit_callback, PlatformSpecificWindowBuilderAttributes},
    window::WindowAttributes,
};

const X_TOKEN: Token = Token(0);
const USER_REDRAW_TOKEN: Token = Token(1);

struct WakeSender<T> {
    sender: Sender<T>,
    waker: Arc<Waker>,
}

pub struct EventLoopWindowTarget<T> {
    xconn: Arc<XConnection>,
    wm_delete_window: ffi::Atom,
    net_wm_ping: ffi::Atom,
    ime_sender: ImeSender,
    root: ffi::Window,
    ime: RefCell<Ime>,
    windows: RefCell<HashMap<WindowId, Weak<UnownedWindow>>>,
    redraw_sender: WakeSender<WindowId>,
    _marker: ::std::marker::PhantomData<T>,
}

pub struct EventLoop<T: 'static> {
    poll: Poll,
    waker: Arc<Waker>,
    event_processor: EventProcessor<T>,
    redraw_channel: Receiver<WindowId>,
    user_channel: Receiver<T>, //waker.wake needs to be called whenever something gets sent
    user_sender: Sender<T>,
    target: Rc<RootELW<T>>,
}

pub struct EventLoopProxy<T: 'static> {
    user_sender: Sender<T>,
    waker: Arc<Waker>,
}

impl<T: 'static> Clone for EventLoopProxy<T> {
    fn clone(&self) -> Self {
        EventLoopProxy {
            user_sender: self.user_sender.clone(),
            waker: self.waker.clone(),
        }
    }
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
            // Remember default locale to restore it if target locale is unsupported
            // by Xlib
            let default_locale = setlocale(LC_CTYPE, ptr::null());
            setlocale(LC_CTYPE, b"\0".as_ptr() as *const _);

            // Check if set locale is supported by Xlib.
            // If not, calls to some Xlib functions like `XSetLocaleModifiers`
            // will fail.
            let locale_supported = (xconn.xlib.XSupportsLocale)() == 1;
            if !locale_supported {
                let unsupported_locale = setlocale(LC_CTYPE, ptr::null());
                warn!(
                    "Unsupported locale \"{}\". Restoring default locale \"{}\".",
                    CStr::from_ptr(unsupported_locale).to_string_lossy(),
                    CStr::from_ptr(default_locale).to_string_lossy()
                );
                // Restore default locale
                setlocale(LC_CTYPE, default_locale);
            }
        }
        let ime = RefCell::new({
            let result = Ime::new(Arc::clone(&xconn));
            if let Err(ImeCreationError::OpenFailure(ref state)) = result {
                panic!("Failed to open input method: {:#?}", state);
            }
            result.expect("Failed to set input method destruction callback")
        });

        let randr_event_offset = xconn
            .select_xrandr_input(root)
            .expect("Failed to query XRandR extension");

        let xi2ext = unsafe {
            let mut ext = XExtension::default();

            let res = (xconn.xlib.XQueryExtension)(
                xconn.display,
                b"XInputExtension\0".as_ptr() as *const c_char,
                &mut ext.opcode,
                &mut ext.first_event_id,
                &mut ext.first_error_id,
            );

            if res == ffi::False {
                panic!("X server missing XInput extension");
            }

            ext
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

        let mut mod_keymap = ModifierKeymap::new();
        mod_keymap.reset_from_x_connection(&xconn);

        let poll = Poll::new().unwrap();
        let waker = Arc::new(Waker::new(poll.registry(), USER_REDRAW_TOKEN).unwrap());

        poll.registry()
            .register(&mut SourceFd(&xconn.x11_fd), X_TOKEN, Interest::READABLE)
            .unwrap();

        let (user_sender, user_channel) = std::sync::mpsc::channel();
        let (redraw_sender, redraw_channel) = std::sync::mpsc::channel();

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
                redraw_sender: WakeSender {
                    sender: redraw_sender, // not used again so no clone
                    waker: waker.clone(),
                },
            }),
            _marker: ::std::marker::PhantomData,
        });

        let event_processor = EventProcessor {
            target: target.clone(),
            dnd,
            devices: Default::default(),
            randr_event_offset,
            ime_receiver,
            xi2ext,
            mod_keymap,
            device_mod_state: Default::default(),
            num_touch: 0,
            first_touch: None,
            active_window: None,
        };

        // Register for device hotplug events
        // (The request buffer is flushed during `init_device`)
        get_xtarget(&target)
            .xconn
            .select_xinput_events(root, ffi::XIAllDevices, ffi::XI_HierarchyChangedMask)
            .queue();

        event_processor.init_device(ffi::XIAllDevices);

        EventLoop {
            poll,
            waker,
            event_processor,
            redraw_channel,
            user_channel,
            user_sender,
            target,
        }
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy {
            user_sender: self.user_sender.clone(),
            waker: self.waker.clone(),
        }
    }

    pub(crate) fn window_target(&self) -> &RootELW<T> {
        &self.target
    }

    pub fn run_return<F>(&mut self, mut callback: F)
    where
        F: FnMut(Event<'_, T>, &RootELW<T>, &mut ControlFlow),
    {
        let mut control_flow = ControlFlow::default();
        let mut events = Events::with_capacity(8);
        let mut cause = StartCause::Init;

        loop {
            sticky_exit_callback(
                crate::event::Event::NewEvents(cause),
                &self.target,
                &mut control_flow,
                &mut callback,
            );

            // Process all pending events
            self.drain_events(&mut callback, &mut control_flow);

            // Empty the user event buffer
            {
                while let Ok(event) = self.user_channel.try_recv() {
                    sticky_exit_callback(
                        crate::event::Event::UserEvent(event),
                        &self.target,
                        &mut control_flow,
                        &mut callback,
                    );
                }
            }
            // send MainEventsCleared
            {
                sticky_exit_callback(
                    crate::event::Event::MainEventsCleared,
                    &self.target,
                    &mut control_flow,
                    &mut callback,
                );
            }
            // Empty the redraw requests
            {
                let mut windows = HashSet::new();

                while let Ok(window_id) = self.redraw_channel.try_recv() {
                    windows.insert(window_id);
                }

                for window_id in windows {
                    let window_id = crate::window::WindowId(super::WindowId::X(window_id));
                    sticky_exit_callback(
                        Event::RedrawRequested(window_id),
                        &self.target,
                        &mut control_flow,
                        &mut callback,
                    );
                }
            }
            // send RedrawEventsCleared
            {
                sticky_exit_callback(
                    crate::event::Event::RedrawEventsCleared,
                    &self.target,
                    &mut control_flow,
                    &mut callback,
                );
            }

            let start = Instant::now();
            let (deadline, timeout);

            match control_flow {
                ControlFlow::Exit => break,
                ControlFlow::Poll => {
                    cause = StartCause::Poll;
                    deadline = None;
                    timeout = Some(Duration::from_millis(0));
                }
                ControlFlow::Wait => {
                    cause = StartCause::WaitCancelled {
                        start,
                        requested_resume: None,
                    };
                    deadline = None;
                    timeout = None;
                }
                ControlFlow::WaitUntil(wait_deadline) => {
                    cause = StartCause::ResumeTimeReached {
                        start,
                        requested_resume: wait_deadline,
                    };
                    timeout = if wait_deadline > start {
                        Some(wait_deadline - start)
                    } else {
                        Some(Duration::from_millis(0))
                    };
                    deadline = Some(wait_deadline);
                }
            }

            // If the XConnection already contains buffered events, we don't
            // need to wait for data on the socket.
            if !self.event_processor.poll() {
                if let Err(e) = self.poll.poll(&mut events, timeout) {
                    if e.raw_os_error() != Some(libc::EINTR) {
                        panic!("epoll returned an error: {:?}", e);
                    }
                }
                events.clear();
            }

            let wait_cancelled = deadline.map_or(false, |deadline| Instant::now() < deadline);

            if wait_cancelled {
                cause = StartCause::WaitCancelled {
                    start,
                    requested_resume: deadline,
                };
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
        F: 'static + FnMut(Event<'_, T>, &RootELW<T>, &mut ControlFlow),
    {
        self.run_return(callback);
        ::std::process::exit(0);
    }

    fn drain_events<F>(&mut self, callback: &mut F, control_flow: &mut ControlFlow)
    where
        F: FnMut(Event<'_, T>, &RootELW<T>, &mut ControlFlow),
    {
        let target = &self.target;
        let mut xev = MaybeUninit::uninit();
        let wt = get_xtarget(&self.target);

        while unsafe { self.event_processor.poll_one_event(xev.as_mut_ptr()) } {
            let mut xev = unsafe { xev.assume_init() };
            self.event_processor.process_event(&mut xev, |event| {
                sticky_exit_callback(
                    event,
                    target,
                    control_flow,
                    &mut |event, window_target, control_flow| {
                        if let Event::RedrawRequested(crate::window::WindowId(
                            super::WindowId::X(wid),
                        )) = event
                        {
                            wt.redraw_sender.sender.send(wid).unwrap();
                            wt.redraw_sender.waker.wake().unwrap();
                        } else {
                            callback(event, window_target, control_flow);
                        }
                    },
                );
            });
        }
    }
}

pub(crate) fn get_xtarget<T>(target: &RootELW<T>) -> &EventLoopWindowTarget<T> {
    match target.p {
        super::EventLoopWindowTarget::X(ref target) => target,
        #[cfg(feature = "wayland")]
        _ => unreachable!(),
    }
}

impl<T> EventLoopWindowTarget<T> {
    /// Returns the `XConnection` of this events loop.
    #[inline]
    pub fn x_connection(&self) -> &Arc<XConnection> {
        &self.xconn
    }
}

impl<T: 'static> EventLoopProxy<T> {
    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed<T>> {
        self.user_sender
            .send(event)
            .map_err(|e| EventLoopClosed(e.0))
            .map(|_| self.waker.wake().unwrap())
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
            let mut count = 0;
            let info = (xconn.xinput2.XIQueryDevice)(xconn.display, device, &mut count);
            xconn.check_errors().ok()?;

            if info.is_null() || count == 0 {
                None
            } else {
                Some(DeviceInfo {
                    xconn,
                    info,
                    count: count as usize,
                })
            }
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
    pub const unsafe fn dummy() -> Self {
        WindowId(0)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId(c_int);

impl DeviceId {
    pub const unsafe fn dummy() -> Self {
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
        let window = Arc::new(UnownedWindow::new(event_loop, attribs, pl_attribs)?);
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

#[derive(Debug, Default, Copy, Clone)]
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
