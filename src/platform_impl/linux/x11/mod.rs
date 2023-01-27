#![cfg(x11_platform)]

mod atoms;
mod dnd;
mod event_processor;
mod events;
pub mod ffi;
mod ime;
mod monitor;
pub mod util;
mod window;
mod xdisplay;

pub(crate) use self::{
    monitor::{MonitorHandle, VideoMode},
    window::UnownedWindow,
    xdisplay::XConnection,
};

pub use self::xdisplay::{XError, XNotSupported};

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    fmt,
    ops::Deref,
    rc::Rc,
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    sync::{Arc, Weak},
    time::{Duration, Instant},
};
use x11rb::{
    errors::{ConnectionError, ReplyError, ReplyOrIdError},
    protocol::{
        xinput::{self, ConnectionExt as _},
        xproto::{self, ConnectionExt as _},
    },
    x11_utils::X11Error,
};

use mio::{unix::SourceFd, Events, Interest, Poll, Token, Waker};
use raw_window_handle::{RawDisplayHandle, XlibDisplayHandle};

use self::{
    dnd::{Dnd, DndState},
    event_processor::EventProcessor,
    util::modifiers::ModifierKeymap,
};
use crate::{
    error::OsError as RootOsError,
    event::{Event, StartCause},
    event_loop::{
        ControlFlow, DeviceEventFilter, EventLoopClosed, EventLoopWindowTarget as RootELW,
    },
    platform_impl::{
        platform::{sticky_exit_callback, WindowId},
        PlatformSpecificWindowBuilderAttributes,
    },
    window::WindowAttributes,
};

/// Sum error for X11 errors.
pub(crate) enum PlatformError {
    /// An error that can occur during connection operation.
    Connection(ConnectionError),

    /// An error that can occur as a result of a protocol.
    Protocol(X11Error),

    /// An error that can occur during Xlib operation.
    Xlib(XError),
}

impl fmt::Debug for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlatformError::Connection(e) => fmt::Debug::fmt(e, f),
            PlatformError::Protocol(e) => fmt::Debug::fmt(e, f),
            PlatformError::Xlib(e) => fmt::Debug::fmt(e, f),
        }
    }
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlatformError::Connection(e) => fmt::Display::fmt(e, f),
            PlatformError::Protocol(_) => f.write_str("Encountered a libX11 error"),
            PlatformError::Xlib(e) => fmt::Display::fmt(e, f),
        }
    }
}

impl std::error::Error for PlatformError {}

impl From<ConnectionError> for PlatformError {
    fn from(value: ConnectionError) -> Self {
        PlatformError::Connection(value)
    }
}

impl From<X11Error> for PlatformError {
    fn from(value: X11Error) -> Self {
        PlatformError::Protocol(value)
    }
}

impl From<XError> for PlatformError {
    fn from(value: XError) -> Self {
        PlatformError::Xlib(value)
    }
}

impl From<ReplyError> for PlatformError {
    fn from(value: ReplyError) -> Self {
        match value {
            ReplyError::ConnectionError(e) => PlatformError::Connection(e),
            ReplyError::X11Error(e) => PlatformError::Protocol(e),
        }
    }
}

impl From<ReplyOrIdError> for PlatformError {
    fn from(value: ReplyOrIdError) -> Self {
        match value {
            ReplyOrIdError::ConnectionError(e) => PlatformError::Connection(e),
            ReplyOrIdError::X11Error(e) => PlatformError::Protocol(e),
            ReplyOrIdError::IdsExhausted => panic!("XID space exhausted"),
        }
    }
}

const X_TOKEN: Token = Token(0);
const USER_REDRAW_TOKEN: Token = Token(1);

struct WakeSender<T> {
    sender: Sender<T>,
    waker: Arc<Waker>,
}

struct PeekableReceiver<T> {
    recv: Receiver<T>,
    first: Option<T>,
}

impl<T> PeekableReceiver<T> {
    pub fn from_recv(recv: Receiver<T>) -> Self {
        Self { recv, first: None }
    }
    pub fn has_incoming(&mut self) -> bool {
        if self.first.is_some() {
            return true;
        }

        match self.recv.try_recv() {
            Ok(v) => {
                self.first = Some(v);
                true
            }
            Err(TryRecvError::Empty) => false,
            Err(TryRecvError::Disconnected) => {
                warn!("Channel was disconnected when checking incoming");
                false
            }
        }
    }
    pub fn try_recv(&mut self) -> Result<T, TryRecvError> {
        if let Some(first) = self.first.take() {
            return Ok(first);
        }
        self.recv.try_recv()
    }
}

pub struct EventLoopWindowTarget<T> {
    xconn: Arc<XConnection>,
    root: xproto::Window,
    ime: Option<ime::ImeData>,
    ime_sender: Sender<ime::ImeRequest>,
    ime_receiver: Receiver<ime::ImeRequest>,
    windows: RefCell<HashMap<WindowId, Weak<UnownedWindow>>>,
    redraw_sender: WakeSender<WindowId>,
    device_event_filter: Cell<DeviceEventFilter>,
    _marker: ::std::marker::PhantomData<T>,
}

pub struct EventLoop<T: 'static> {
    poll: Poll,
    waker: Arc<Waker>,
    event_processor: EventProcessor<T>,
    redraw_receiver: PeekableReceiver<WindowId>,
    user_receiver: PeekableReceiver<T>, //waker.wake needs to be called whenever something gets sent
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
    pub(crate) fn new(xconn: Arc<XConnection>) -> EventLoop<T> {
        let root = xconn.default_screen().root;

        let dnd = Dnd::new(Arc::clone(&xconn));

        let (ime_sender, ime_receiver) = mpsc::channel();
        /*
        let (ime_sender, ime_receiver) = mpsc::channel();
        let (ime_event_sender, ime_event_receiver) = mpsc::channel();
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
            let result = Ime::new(Arc::clone(&xconn), ime_event_sender);
            if let Err(ImeCreationError::OpenFailure(ref state)) = result {
                panic!("Failed to open input method: {state:#?}");
            }
            result.expect("Failed to set input method destruction callback")
        });
        */

        // Check if XInput2 is available.
        if xconn
            .connection
            .xinput_xi_query_version(2, 3)
            .unwrap()
            .reply()
            .is_err()
        {
            panic!("X server missing XInput2 extension");
        }

        xconn
            .update_cached_wm_info(root)
            .expect("Failed to update cached WM info");

        let mut mod_keymap = ModifierKeymap::new();
        mod_keymap
            .reset_from_x_connection(&xconn)
            .expect("Failed to reset modifier keymap");

        let poll = Poll::new().unwrap();
        let waker = Arc::new(Waker::new(poll.registry(), USER_REDRAW_TOKEN).unwrap());

        poll.registry()
            .register(&mut SourceFd(&xconn.x11_fd), X_TOKEN, Interest::READABLE)
            .unwrap();

        let (user_sender, user_channel) = std::sync::mpsc::channel();
        let (redraw_sender, redraw_channel) = std::sync::mpsc::channel();

        let window_target = EventLoopWindowTarget {
            root,
            windows: Default::default(),
            ime: ime::ImeData::new(&xconn, xconn.default_screen).ok(),
            ime_sender,
            ime_receiver,
            _marker: ::std::marker::PhantomData,
            xconn,
            redraw_sender: WakeSender {
                sender: redraw_sender, // not used again so no clone
                waker: waker.clone(),
            },
            device_event_filter: Default::default(),
        };

        // Set initial device event filter.
        window_target
            .update_device_event_filter(true)
            .expect("Failed to update device event filter");

        let target = Rc::new(RootELW {
            p: super::EventLoopWindowTarget::X(window_target),
            _marker: ::std::marker::PhantomData,
        });

        let event_processor = EventProcessor {
            target: target.clone(),
            dnd,
            devices: Default::default(),
            mod_keymap,
            device_mod_state: Default::default(),
            num_touch: 0,
            first_touch: None,
            active_window: None,
            is_composing: false,
        };

        // Register for device hotplug events
        // (The request buffer is flushed during `init_device`)
        let all_devices = 0;
        get_xtarget(&target)
            .xconn
            .connection
            .xinput_xi_select_events(
                root,
                &[xinput::EventMask {
                    deviceid: all_devices,
                    mask: vec![xinput::XIEventMask::HIERARCHY],
                }],
            )
            .expect("Failed to select XI_HIERARCHY events")
            .ignore_error();

        event_processor.init_device(all_devices);

        EventLoop {
            poll,
            waker,
            event_processor,
            redraw_receiver: PeekableReceiver::from_recv(redraw_channel),
            user_receiver: PeekableReceiver::from_recv(user_channel),
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

    pub fn run_return<F>(&mut self, mut callback: F) -> i32
    where
        F: FnMut(Event<'_, T>, &RootELW<T>, &mut ControlFlow),
    {
        struct IterationResult {
            deadline: Option<Instant>,
            timeout: Option<Duration>,
            wait_start: Instant,
        }
        fn single_iteration<T, F>(
            this: &mut EventLoop<T>,
            control_flow: &mut ControlFlow,
            cause: &mut StartCause,
            callback: &mut F,
        ) -> IterationResult
        where
            F: FnMut(Event<'_, T>, &RootELW<T>, &mut ControlFlow),
        {
            sticky_exit_callback(
                crate::event::Event::NewEvents(*cause),
                &this.target,
                control_flow,
                callback,
            );

            // NB: For consistency all platforms must emit a 'resumed' event even though X11
            // applications don't themselves have a formal suspend/resume lifecycle.
            if *cause == StartCause::Init {
                sticky_exit_callback(
                    crate::event::Event::Resumed,
                    &this.target,
                    control_flow,
                    callback,
                );
            }

            // Process all pending events
            this.drain_events(callback, control_flow);

            // Empty the user event buffer
            {
                while let Ok(event) = this.user_receiver.try_recv() {
                    sticky_exit_callback(
                        crate::event::Event::UserEvent(event),
                        &this.target,
                        control_flow,
                        callback,
                    );
                }
            }
            // send MainEventsCleared
            {
                sticky_exit_callback(
                    crate::event::Event::MainEventsCleared,
                    &this.target,
                    control_flow,
                    callback,
                );
            }
            // Empty the redraw requests
            {
                let mut windows = HashSet::new();

                while let Ok(window_id) = this.redraw_receiver.try_recv() {
                    windows.insert(window_id);
                }

                for window_id in windows {
                    let window_id = crate::window::WindowId(window_id);
                    sticky_exit_callback(
                        Event::RedrawRequested(window_id),
                        &this.target,
                        control_flow,
                        callback,
                    );
                }
            }
            // send RedrawEventsCleared
            {
                sticky_exit_callback(
                    crate::event::Event::RedrawEventsCleared,
                    &this.target,
                    control_flow,
                    callback,
                );
            }

            let start = Instant::now();
            let (deadline, timeout);

            match control_flow {
                ControlFlow::ExitWithCode(_) => {
                    return IterationResult {
                        wait_start: start,
                        deadline: None,
                        timeout: None,
                    };
                }
                ControlFlow::Poll => {
                    *cause = StartCause::Poll;
                    deadline = None;
                    timeout = Some(Duration::from_millis(0));
                }
                ControlFlow::Wait => {
                    *cause = StartCause::WaitCancelled {
                        start,
                        requested_resume: None,
                    };
                    deadline = None;
                    timeout = None;
                }
                ControlFlow::WaitUntil(wait_deadline) => {
                    *cause = StartCause::ResumeTimeReached {
                        start,
                        requested_resume: *wait_deadline,
                    };
                    timeout = if *wait_deadline > start {
                        Some(*wait_deadline - start)
                    } else {
                        Some(Duration::from_millis(0))
                    };
                    deadline = Some(*wait_deadline);
                }
            }

            IterationResult {
                wait_start: start,
                deadline,
                timeout,
            }
        }

        let mut control_flow = ControlFlow::default();
        let mut events = Events::with_capacity(8);
        let mut cause = StartCause::Init;

        // run the initial loop iteration
        let mut iter_result = single_iteration(self, &mut control_flow, &mut cause, &mut callback);

        let exit_code = loop {
            if let ControlFlow::ExitWithCode(code) = control_flow {
                break code;
            }
            let has_pending = self.event_processor.poll()
                || self.user_receiver.has_incoming()
                || self.redraw_receiver.has_incoming();
            if !has_pending {
                // Wait until
                if let Err(e) = self.poll.poll(&mut events, iter_result.timeout) {
                    if e.raw_os_error() != Some(libc::EINTR) {
                        panic!("epoll returned an error: {e:?}");
                    }
                }
                events.clear();

                if control_flow == ControlFlow::Wait {
                    // We don't go straight into executing the event loop iteration, we instead go
                    // to the start of this loop and check again if there's any pending event. We
                    // must do this because during the execution of the iteration we sometimes wake
                    // the mio waker, and if the waker is already awaken before we call poll(),
                    // then poll doesn't block, but it returns immediately. This caused the event
                    // loop to run continuously even if the control_flow was `Wait`
                    continue;
                }
            }

            let wait_cancelled = iter_result
                .deadline
                .map_or(false, |deadline| Instant::now() < deadline);

            if wait_cancelled {
                cause = StartCause::WaitCancelled {
                    start: iter_result.wait_start,
                    requested_resume: iter_result.deadline,
                };
            }

            iter_result = single_iteration(self, &mut control_flow, &mut cause, &mut callback);
        };

        callback(
            crate::event::Event::LoopDestroyed,
            &self.target,
            &mut control_flow,
        );
        exit_code
    }

    pub fn run<F>(mut self, callback: F) -> !
    where
        F: 'static + FnMut(Event<'_, T>, &RootELW<T>, &mut ControlFlow),
    {
        let exit_code = self.run_return(callback);
        ::std::process::exit(exit_code);
    }

    fn drain_events<F>(&mut self, callback: &mut F, control_flow: &mut ControlFlow)
    where
        F: FnMut(Event<'_, T>, &RootELW<T>, &mut ControlFlow),
    {
        let target = &self.target;
        let wt = get_xtarget(&self.target);

        while let Some(mut xev) = self
            .event_processor
            .poll_one_event()
            .expect("Failed to pump X11 events")
        {
            self.event_processor.process_event(&mut xev, |event| {
                sticky_exit_callback(
                    event,
                    target,
                    control_flow,
                    &mut |event, window_target, control_flow| {
                        if let Event::RedrawRequested(crate::window::WindowId(wid)) = event {
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
        #[cfg(wayland_platform)]
        _ => unreachable!(),
    }
}

impl<T> EventLoopWindowTarget<T> {
    /// Returns the `XConnection` of this events loop.
    #[inline]
    pub(crate) fn x_connection(&self) -> &Arc<XConnection> {
        &self.xconn
    }

    pub fn set_device_event_filter(&self, filter: DeviceEventFilter) {
        self.device_event_filter.set(filter);
    }

    /// Update the device event filter based on window focus.
    pub(crate) fn update_device_event_filter(&self, focus: bool) -> Result<(), PlatformError> {
        let filter_events = self.device_event_filter.get() == DeviceEventFilter::Never
            || (self.device_event_filter.get() == DeviceEventFilter::Unfocused && !focus);

        let mut mask = xinput::XIEventMask::from(0u16);
        if !filter_events {
            mask = xinput::XIEventMask::RAW_MOTION
                | xinput::XIEventMask::RAW_BUTTON_PRESS
                | xinput::XIEventMask::RAW_BUTTON_RELEASE
                | xinput::XIEventMask::RAW_KEY_PRESS
                | xinput::XIEventMask::RAW_KEY_RELEASE;
        }

        let all_master_devices = 1;
        let events = [xinput::EventMask {
            deviceid: all_master_devices,
            mask: vec![mask],
        }];

        self.xconn
            .connection
            .xinput_xi_select_events(self.root, &events)?
            .ignore_error();
        Ok(())
    }

    pub fn raw_display_handle(&self) -> raw_window_handle::RawDisplayHandle {
        let mut display_handle = XlibDisplayHandle::empty();
        display_handle.display = self.xconn.display.as_ptr() as *mut _;
        display_handle.screen = self.xconn.default_screen as _;
        RawDisplayHandle::Xlib(display_handle)
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

struct DeviceInfo {
    info: Vec<xinput::XIDeviceInfo>,
}

impl DeviceInfo {
    fn get(xconn: &XConnection, device: xinput::DeviceId) -> Option<Self> {
        let info = xconn
            .connection
            .xinput_xi_query_device(device)
            .ok()?
            .reply()
            .ok()?;

        if info.infos.is_empty() {
            None
        } else {
            Some(DeviceInfo { info: info.infos })
        }
    }
}

impl Deref for DeviceInfo {
    type Target = [xinput::XIDeviceInfo];
    fn deref(&self) -> &Self::Target {
        &self.info
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId(xinput::DeviceId);

impl DeviceId {
    #[allow(unused)]
    pub const unsafe fn dummy() -> Self {
        DeviceId(0)
    }
}

pub(crate) struct Window(Arc<UnownedWindow>);

impl Deref for Window {
    type Target = UnownedWindow;
    #[inline]
    fn deref(&self) -> &UnownedWindow {
        &self.0
    }
}

impl Window {
    pub(crate) fn new<T>(
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
        xconn
            .connection
            .destroy_window(window.id().0 as _)
            .map(|r| r.ignore_error())
            .ok();
    }
}

fn mkwid(w: xproto::Window) -> crate::window::WindowId {
    crate::window::WindowId(crate::platform_impl::platform::WindowId(w as u64))
}
fn mkdid(w: xinput::DeviceId) -> crate::event::DeviceId {
    crate::event::DeviceId(crate::platform_impl::DeviceId::X(DeviceId(w)))
}

#[derive(Debug)]
struct Device {
    _name: String,
    scroll_axes: Vec<(u16, ScrollAxis)>,
    // For master devices, this is the paired device (pointer <-> keyboard).
    // For slave devices, this is the master.
    attachment: u16,
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
    fn new(info: &xinput::XIDeviceInfo) -> Self {
        let name = String::from_utf8_lossy(&info.name).into_owned();
        let mut scroll_axes = Vec::new();

        if Device::physical_device(info) {
            // Identify scroll axes
            for class in info.classes.iter() {
                if let xinput::DeviceClassData::Scroll(ref info) = class.data {
                    scroll_axes.push((
                        info.number,
                        ScrollAxis {
                            increment: fp3232(info.increment),
                            orientation: match info.scroll_type {
                                xinput::ScrollType::HORIZONTAL => ScrollOrientation::Horizontal,
                                xinput::ScrollType::VERTICAL => ScrollOrientation::Vertical,
                                _ => unreachable!(),
                            },
                            position: 0.0,
                        },
                    ));
                }
            }
        }

        let mut device = Device {
            _name: name,
            scroll_axes,
            attachment: info.attachment,
        };
        device.reset_scroll_position(info);
        device
    }

    fn reset_scroll_position(&mut self, info: &xinput::XIDeviceInfo) {
        if Device::physical_device(info) {
            for class in info.classes.iter() {
                if let xinput::DeviceClassData::Valuator(ref info) = class.data {
                    if let Some(&mut (_, ref mut axis)) = self
                        .scroll_axes
                        .iter_mut()
                        .find(|&&mut (axis, _)| axis == info.number as _)
                    {
                        axis.position = fp3232(info.value);
                    }
                }
            }
        }
    }

    #[inline]
    fn physical_device(info: &xinput::XIDeviceInfo) -> bool {
        use xinput::DeviceType;

        matches!(
            info.type_,
            DeviceType::SLAVE_KEYBOARD | DeviceType::SLAVE_POINTER | DeviceType::FLOATING_SLAVE
        )
    }
}

#[inline]
fn fp3232(fp: xinput::Fp3232) -> f64 {
    // Fixed point fractional representation.
    let int = fp.integral as f64;
    let frac = fp.frac as f64 / (1u64 << 32) as f64;
    int + frac
}

#[inline]
fn fp1616(fp: xinput::Fp1616) -> f64 {
    (fp as f64) / (0x10000 as f64)
}
