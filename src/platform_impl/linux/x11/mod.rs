#![cfg(x11_platform)]

mod activation;
mod atoms;
mod dnd;
mod event_processor;
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

use calloop::generic::Generic;
use calloop::EventLoop as Loop;
use calloop::{ping::Ping, Readiness};

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet, VecDeque},
    ffi::CStr,
    fmt,
    ops::{Deref, DerefMut},
    os::{
        raw::*,
        unix::io::{AsRawFd, RawFd},
    },
    ptr,
    rc::Rc,
    str,
    sync::mpsc::{Receiver, Sender, TryRecvError},
    sync::{mpsc, Arc, Weak},
    time::{Duration, Instant},
};

use libc::{self, setlocale, LC_CTYPE};

use atoms::*;
use raw_window_handle::{RawDisplayHandle, XlibDisplayHandle};

use x11rb::protocol::{
    xinput::{self, ConnectionExt as _},
    xproto::{self, ConnectionExt as _},
};
use x11rb::x11_utils::X11Error as LogicalError;
use x11rb::{
    errors::{ConnectError, ConnectionError, IdsExhausted, ReplyError},
    xcb_ffi::ReplyOrIdError,
};

use self::{
    dnd::{Dnd, DndState},
    event_processor::EventProcessor,
    ime::{Ime, ImeCreationError, ImeReceiver, ImeRequest, ImeSender},
};
use super::{common::xkb_state::KbdState, OsError};
use crate::{
    error::{OsError as RootOsError, RunLoopError},
    event::{Event, StartCause},
    event_loop::{ControlFlow, DeviceEvents, EventLoopClosed, EventLoopWindowTarget as RootELW},
    platform::pump_events::PumpStatus,
    platform_impl::{
        platform::{min_timeout, sticky_exit_callback, WindowId},
        PlatformSpecificWindowBuilderAttributes,
    },
    window::WindowAttributes,
};

type X11Source = Generic<RawFd>;

struct WakeSender<T> {
    sender: Sender<T>,
    waker: Ping,
}

impl<T> Clone for WakeSender<T> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            waker: self.waker.clone(),
        }
    }
}

impl<T> WakeSender<T> {
    pub fn send(&self, t: T) -> Result<(), EventLoopClosed<T>> {
        let res = self.sender.send(t).map_err(|e| EventLoopClosed(e.0));
        if res.is_ok() {
            self.waker.ping();
        }
        res
    }
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
    target: WindowTarget,
    _marker: ::std::marker::PhantomData<T>,
}

impl<T> Deref for EventLoopWindowTarget<T> {
    type Target = WindowTarget;

    fn deref(&self) -> &Self::Target {
        &self.target
    }
}

impl<T> DerefMut for EventLoopWindowTarget<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.target
    }
}

/// The window target, sans any generics.
pub struct WindowTarget {
    xconn: Arc<XConnection>,
    wm_delete_window: xproto::Atom,
    net_wm_ping: xproto::Atom,
    ime_sender: ImeSender,
    root: xproto::Window,
    ime: RefCell<Ime>,
    windows: RefCell<HashMap<WindowId, Weak<UnownedWindow>>>,
    redraw_sender: WakeSender<WindowId>,
    activation_sender: WakeSender<ActivationToken>,
    device_events: Cell<DeviceEvents>,
}

pub struct EventLoop<T: 'static> {
    loop_running: bool,
    control_flow: ControlFlow,
    event_loop: Loop<'static, EventLoopState>,
    waker: calloop::ping::Ping,
    event_processor: EventProcessor,
    redraw_receiver: PeekableReceiver<WindowId>,
    user_receiver: PeekableReceiver<T>,
    activation_receiver: PeekableReceiver<ActivationToken>,
    user_sender: Sender<T>,
    target: Rc<RootELW<T>>,

    /// The current state of the event loop.
    state: EventLoopState,
}

type ActivationToken = (WindowId, crate::event_loop::AsyncRequestSerial);

struct EventLoopState {
    /// The latest readiness state for the x11 file descriptor
    x11_readiness: Readiness,
}

pub struct EventLoopProxy<T: 'static> {
    user_sender: WakeSender<T>,
}

impl<T: 'static> Clone for EventLoopProxy<T> {
    fn clone(&self) -> Self {
        EventLoopProxy {
            user_sender: self.user_sender.clone(),
        }
    }
}

impl<T: 'static> EventLoop<T> {
    pub(crate) fn new(xconn: Arc<XConnection>) -> EventLoop<T> {
        let root = xconn.default_root().root;
        let atoms = xconn.atoms();

        let wm_delete_window = atoms[WM_DELETE_WINDOW];
        let net_wm_ping = atoms[_NET_WM_PING];

        let dnd = Dnd::new(Arc::clone(&xconn))
            .expect("Failed to call XInternAtoms when initializing drag and drop");

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

        xconn
            .select_xrandr_input(root as ffi::Window)
            .expect("Failed to query XRandR extension");

        unsafe {
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
        }

        {
            let mut ext = XExtension::default();

            let res = unsafe {
                (xconn.xlib.XkbQueryExtension)(
                    xconn.display,
                    &mut ext.opcode,
                    &mut ext.first_event_id,
                    &mut ext.first_error_id,
                    &mut 1,
                    &mut 0,
                )
            };

            if res == ffi::False {
                panic!("X server missing XKB extension");
            }

            // Enable detectable auto repeat.
            let mut supported = 0;
            unsafe {
                (xconn.xlib.XkbSetDetectableAutoRepeat)(xconn.display, 1, &mut supported);
            }
            if supported == 0 {
                warn!("Detectable auto repeart is not supported");
            }
        }

        unsafe {
            let mut xinput_major_ver = ffi::XI_2_Major;
            let mut xinput_minor_ver = ffi::XI_2_Minor;
            if (xconn.xinput2.XIQueryVersion)(
                xconn.display,
                &mut xinput_major_ver,
                &mut xinput_minor_ver,
            ) != ffi::Success as std::os::raw::c_int
            {
                panic!(
                    "X server has XInput extension {xinput_major_ver}.{xinput_minor_ver} but does not support XInput2",
                );
            }
        }

        xconn.update_cached_wm_info(root);

        // Create an event loop.
        let event_loop =
            Loop::<EventLoopState>::try_new().expect("Failed to initialize the event loop");
        let handle = event_loop.handle();

        // Create the X11 event dispatcher.
        let source = X11Source::new(
            xconn.xcb_connection().as_raw_fd(),
            calloop::Interest::READ,
            calloop::Mode::Level,
        );
        handle
            .insert_source(source, |readiness, _, state| {
                state.x11_readiness = readiness;
                Ok(calloop::PostAction::Continue)
            })
            .expect("Failed to register the X11 event dispatcher");

        let (waker, waker_source) =
            calloop::ping::make_ping().expect("Failed to create event loop waker");
        event_loop
            .handle()
            .insert_source(waker_source, move |_, _, _| {
                // No extra handling is required, we just need to wake-up.
            })
            .expect("Failed to register the event loop waker source");

        // Create a channel for handling redraw requests.
        let (redraw_sender, redraw_channel) = mpsc::channel();

        // Create a channel for sending activation tokens.
        let (activation_token_sender, activation_token_channel) = mpsc::channel();

        // Create a channel for sending user events.
        let (user_sender, user_channel) = mpsc::channel();

        let kb_state =
            KbdState::from_x11_xkb(xconn.xcb_connection().get_raw_xcb_connection()).unwrap();

        let window_target = EventLoopWindowTarget {
            target: WindowTarget {
                ime,
                root,
                windows: Default::default(),
                ime_sender,
                xconn,
                wm_delete_window,
                net_wm_ping,
                redraw_sender: WakeSender {
                    sender: redraw_sender, // not used again so no clone
                    waker: waker.clone(),
                },
                activation_sender: WakeSender {
                    sender: activation_token_sender, // not used again so no clone
                    waker: waker.clone(),
                },
                device_events: Default::default(),
            },
            _marker: ::std::marker::PhantomData,
        };

        // Set initial device event filter.
        window_target.update_listen_device_events(true);

        let event_processor = EventProcessor {
            dnd,
            devices: Default::default(),
            ime_receiver,
            ime_event_receiver,
            kb_state,
            num_touch: 0,
            held_key_press: None,
            first_touch: None,
            active_window: None,
            is_composing: false,
            enqueued_events: VecDeque::new().into(),
            event_handlers: event_processor::EventHandlers::new(
                window_target.xconn.xcb_connection(),
            )
            .expect("Failed to load event handlers")
            .into(),
        };

        let target = Rc::new(RootELW {
            p: super::EventLoopWindowTarget::X(window_target),
            _marker: ::std::marker::PhantomData,
        });

        // Register for device hotplug events
        // (The request buffer is flushed during `init_device`)
        get_xtarget(&target)
            .xconn
            .select_xinput_events(
                root,
                ffi::XIAllDevices as _,
                x11rb::protocol::xinput::XIEventMask::HIERARCHY,
            )
            .expect_then_ignore_error("Failed to register for XInput2 device hotplug events");

        get_xtarget(&target)
            .xconn
            .select_xkb_events(
                0x100, // Use the "core keyboard device"
                ffi::XkbNewKeyboardNotifyMask | ffi::XkbStateNotifyMask,
            )
            .unwrap();

        event_processor.init_device(get_xtarget(&target), ALL_DEVICES);

        EventLoop {
            loop_running: false,
            control_flow: ControlFlow::default(),
            event_loop,
            waker,
            event_processor,
            redraw_receiver: PeekableReceiver::from_recv(redraw_channel),
            activation_receiver: PeekableReceiver::from_recv(activation_token_channel),
            user_receiver: PeekableReceiver::from_recv(user_channel),
            user_sender,
            target,
            state: EventLoopState {
                x11_readiness: Readiness::EMPTY,
            },
        }
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy {
            user_sender: WakeSender {
                sender: self.user_sender.clone(),
                waker: self.waker.clone(),
            },
        }
    }

    pub(crate) fn window_target(&self) -> &RootELW<T> {
        &self.target
    }

    pub fn run_ondemand<F>(&mut self, mut event_handler: F) -> Result<(), RunLoopError>
    where
        F: FnMut(Event<T>, &RootELW<T>, &mut ControlFlow),
    {
        if self.loop_running {
            return Err(RunLoopError::AlreadyRunning);
        }

        let exit = loop {
            match self.pump_events(None, &mut event_handler) {
                PumpStatus::Exit(0) => {
                    break Ok(());
                }
                PumpStatus::Exit(code) => {
                    break Err(RunLoopError::ExitFailure(code));
                }
                _ => {
                    continue;
                }
            }
        };

        // Applications aren't allowed to carry windows between separate
        // `run_ondemand` calls but if they have only just dropped their
        // windows we need to make sure those last requests are sent to the
        // X Server.
        let wt = get_xtarget(&self.target);
        wt.x_connection().sync_with_server().map_err(|x_err| {
            RunLoopError::Os(os_error!(OsError::XError(Arc::new(X11Error::Xlib(x_err)))))
        })?;

        exit
    }

    pub fn pump_events<F>(&mut self, timeout: Option<Duration>, mut callback: F) -> PumpStatus
    where
        F: FnMut(Event<T>, &RootELW<T>, &mut ControlFlow),
    {
        if !self.loop_running {
            self.loop_running = true;

            // Reset the internal state for the loop as we start running to
            // ensure consistent behaviour in case the loop runs and exits more
            // than once.
            self.control_flow = ControlFlow::Poll;

            // run the initial loop iteration
            self.single_iteration(&mut callback, StartCause::Init);
        }

        // Consider the possibility that the `StartCause::Init` iteration could
        // request to Exit.
        if !matches!(self.control_flow, ControlFlow::ExitWithCode(_)) {
            self.poll_events_with_timeout(timeout, &mut callback);
        }
        if let ControlFlow::ExitWithCode(code) = self.control_flow {
            self.loop_running = false;

            let mut dummy = self.control_flow;
            sticky_exit_callback(
                Event::LoopExiting,
                self.window_target(),
                &mut dummy,
                &mut callback,
            );

            PumpStatus::Exit(code)
        } else {
            PumpStatus::Continue
        }
    }

    fn has_pending(&mut self) -> bool {
        let wt = get_xtarget(&self.target);
        self.event_processor.poll(wt)
            || self.user_receiver.has_incoming()
            || self.redraw_receiver.has_incoming()
    }

    pub fn poll_events_with_timeout<F>(&mut self, mut timeout: Option<Duration>, mut callback: F)
    where
        F: FnMut(Event<T>, &RootELW<T>, &mut ControlFlow),
    {
        let start = Instant::now();

        let has_pending = self.has_pending();

        timeout = if has_pending {
            // If we already have work to do then we don't want to block on the next poll.
            Some(Duration::ZERO)
        } else {
            let control_flow_timeout = match self.control_flow {
                ControlFlow::Wait => None,
                ControlFlow::Poll => Some(Duration::ZERO),
                ControlFlow::WaitUntil(wait_deadline) => {
                    Some(wait_deadline.saturating_duration_since(start))
                }
                // This function shouldn't have to handle any requests to exit
                // the application (there should be no need to poll for events
                // if the application has requested to exit) so we consider
                // it a bug in the backend if we ever see `ExitWithCode` here.
                ControlFlow::ExitWithCode(_code) => unreachable!(),
            };

            min_timeout(control_flow_timeout, timeout)
        };

        self.state.x11_readiness = Readiness::EMPTY;
        if let Err(error) = self
            .event_loop
            .dispatch(timeout, &mut self.state)
            .map_err(std::io::Error::from)
        {
            log::error!("Failed to poll for events: {error:?}");
            let exit_code = error.raw_os_error().unwrap_or(1);
            self.control_flow = ControlFlow::ExitWithCode(exit_code);
            return;
        }

        // False positive / spurious wake ups could lead to us spamming
        // redundant iterations of the event loop with no new events to
        // dispatch.
        //
        // If there's no readable event source then we just double check if we
        // have any pending `_receiver` events and if not we return without
        // running a loop iteration.
        // If we don't have any pending `_receiver`
        if !self.has_pending() && !self.state.x11_readiness.readable {
            return;
        }

        // NB: `StartCause::Init` is handled as a special case and doesn't need
        // to be considered here
        let cause = match self.control_flow {
            ControlFlow::Poll => StartCause::Poll,
            ControlFlow::Wait => StartCause::WaitCancelled {
                start,
                requested_resume: None,
            },
            ControlFlow::WaitUntil(deadline) => {
                if Instant::now() < deadline {
                    StartCause::WaitCancelled {
                        start,
                        requested_resume: Some(deadline),
                    }
                } else {
                    StartCause::ResumeTimeReached {
                        start,
                        requested_resume: deadline,
                    }
                }
            }
            // This function shouldn't have to handle any requests to exit
            // the application (there should be no need to poll for events
            // if the application has requested to exit) so we consider
            // it a bug in the backend if we ever see `ExitWithCode` here.
            ControlFlow::ExitWithCode(_code) => unreachable!(),
        };

        self.single_iteration(&mut callback, cause);
    }

    fn single_iteration<F>(&mut self, callback: &mut F, cause: StartCause)
    where
        F: FnMut(Event<T>, &RootELW<T>, &mut ControlFlow),
    {
        let mut control_flow = self.control_flow;

        sticky_exit_callback(
            crate::event::Event::NewEvents(cause),
            &self.target,
            &mut control_flow,
            callback,
        );

        // NB: For consistency all platforms must emit a 'resumed' event even though X11
        // applications don't themselves have a formal suspend/resume lifecycle.
        if cause == StartCause::Init {
            sticky_exit_callback(
                crate::event::Event::Resumed,
                &self.target,
                &mut control_flow,
                callback,
            );
        }

        // Process all pending events
        self.drain_events(callback, &mut control_flow)
            .expect("Failed to drain events");

        // Empty activation tokens.
        while let Ok((window_id, serial)) = self.activation_receiver.try_recv() {
            let token = self.event_processor.with_window(
                get_xtarget(&self.target),
                window_id.0 as xproto::Window,
                |window| window.generate_activation_token(),
            );

            match token {
                Some(Ok(token)) => sticky_exit_callback(
                    crate::event::Event::WindowEvent {
                        window_id: crate::window::WindowId(window_id),
                        event: crate::event::WindowEvent::ActivationTokenDone {
                            serial,
                            token: crate::window::ActivationToken::_new(token),
                        },
                    },
                    &self.target,
                    &mut control_flow,
                    callback,
                ),
                Some(Err(e)) => {
                    log::error!("Failed to get activation token: {}", e);
                }
                None => {}
            }
        }

        // Empty the user event buffer
        {
            while let Ok(event) = self.user_receiver.try_recv() {
                sticky_exit_callback(
                    crate::event::Event::UserEvent(event),
                    &self.target,
                    &mut control_flow,
                    callback,
                );
            }
        }

        // Empty the redraw requests
        {
            let mut windows = HashSet::new();

            while let Ok(window_id) = self.redraw_receiver.try_recv() {
                windows.insert(window_id);
            }

            for window_id in windows {
                let window_id = crate::window::WindowId(window_id);
                sticky_exit_callback(
                    Event::RedrawRequested(window_id),
                    &self.target,
                    &mut control_flow,
                    callback,
                );
            }
        }

        // This is always the last event we dispatch before poll again
        {
            sticky_exit_callback(
                crate::event::Event::AboutToWait,
                &self.target,
                &mut control_flow,
                callback,
            );
        }

        self.control_flow = control_flow;
    }

    fn drain_events<F>(
        &mut self,
        callback: &mut F,
        control_flow: &mut ControlFlow,
    ) -> Result<(), X11Error>
    where
        F: FnMut(Event<T>, &RootELW<T>, &mut ControlFlow),
    {
        let target = &self.target;
        let wt = get_xtarget(&self.target);

        while let Some(event) = self.event_processor.pop_single_event(wt)? {
            self.event_processor.process_event(wt, &event, |event| {
                sticky_exit_callback(
                    event,
                    target,
                    control_flow,
                    &mut |event, window_target, control_flow| {
                        if let Event::RedrawRequested(crate::window::WindowId(wid)) = event {
                            wt.redraw_sender.send(wid).unwrap();
                        } else {
                            callback(event, window_target, control_flow);
                        }
                    },
                );
            });
        }

        Ok(())
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

    pub fn set_listen_device_events(&self, allowed: DeviceEvents) {
        self.device_events.set(allowed);
    }

    pub fn raw_display_handle(&self) -> raw_window_handle::RawDisplayHandle {
        let mut display_handle = XlibDisplayHandle::empty();
        display_handle.display = self.xconn.display as *mut _;
        display_handle.screen = self.xconn.default_screen_index() as c_int;
        RawDisplayHandle::Xlib(display_handle)
    }
}

impl WindowTarget {
    /// Update the device event based on window focus.
    pub fn update_listen_device_events(&self, focus: bool) {
        let device_events = self.device_events.get() == DeviceEvents::Always
            || (focus && self.device_events.get() == DeviceEvents::WhenFocused);

        let mut mask = xinput::XIEventMask::from(0u32);
        if device_events {
            mask = xinput::XIEventMask::RAW_MOTION
                | xinput::XIEventMask::RAW_BUTTON_PRESS
                | xinput::XIEventMask::RAW_BUTTON_RELEASE
                | xinput::XIEventMask::RAW_KEY_PRESS
                | xinput::XIEventMask::RAW_KEY_RELEASE;
        }

        self.xconn
            .select_xinput_events(self.root, ffi::XIAllMasterDevices as _, mask)
            .expect_then_ignore_error("Failed to update device event filter");
    }
}

impl<T: 'static> EventLoopProxy<T> {
    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed<T>> {
        self.user_sender
            .send(event)
            .map_err(|e| EventLoopClosed(e.0))
    }
}

struct DeviceInfo {
    info: Vec<xinput::XIDeviceInfo>,
}

impl DeviceInfo {
    fn get(xconn: &XConnection, device: xinput::DeviceId) -> Option<Self> {
        let device_data = xconn
            .xcb_connection()
            .xinput_xi_query_device(device)
            .ok()?
            .reply()
            .ok()?;

        Some(DeviceInfo {
            info: device_data.infos,
        })
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId(u16);

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

        if let Ok(c) = xconn
            .xcb_connection()
            .destroy_window(window.id().0 as xproto::Window)
        {
            c.ignore_error();
        }
    }
}

/// Generic sum error type for X11 errors.
#[derive(Debug)]
pub enum X11Error {
    /// An error from the Xlib library.
    Xlib(XError),

    /// An error that occurred while trying to connect to the X server.
    Connect(ConnectError),

    /// An error that occurred over the connection medium.
    Connection(ConnectionError),

    /// An error that occurred logically on the X11 end.
    X11(LogicalError),

    /// The XID range has been exhausted.
    XidsExhausted(IdsExhausted),

    /// Got `null` from an Xlib function without a reason.
    UnexpectedNull(&'static str),

    /// Got an invalid activation token.
    InvalidActivationToken(Vec<u8>),
}

impl fmt::Display for X11Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            X11Error::Xlib(e) => write!(f, "Xlib error: {}", e),
            X11Error::Connect(e) => write!(f, "X11 connection error: {}", e),
            X11Error::Connection(e) => write!(f, "X11 connection error: {}", e),
            X11Error::XidsExhausted(e) => write!(f, "XID range exhausted: {}", e),
            X11Error::X11(e) => write!(f, "X11 error: {:?}", e),
            X11Error::UnexpectedNull(s) => write!(f, "Xlib function returned null: {}", s),
            X11Error::InvalidActivationToken(s) => write!(
                f,
                "Invalid activation token: {}",
                std::str::from_utf8(s).unwrap_or("<invalid utf8>")
            ),
        }
    }
}

impl std::error::Error for X11Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            X11Error::Xlib(e) => Some(e),
            X11Error::Connect(e) => Some(e),
            X11Error::Connection(e) => Some(e),
            X11Error::XidsExhausted(e) => Some(e),
            _ => None,
        }
    }
}

impl From<XError> for X11Error {
    fn from(e: XError) -> Self {
        X11Error::Xlib(e)
    }
}

impl From<ConnectError> for X11Error {
    fn from(e: ConnectError) -> Self {
        X11Error::Connect(e)
    }
}

impl From<ConnectionError> for X11Error {
    fn from(e: ConnectionError) -> Self {
        X11Error::Connection(e)
    }
}

impl From<LogicalError> for X11Error {
    fn from(e: LogicalError) -> Self {
        X11Error::X11(e)
    }
}

impl From<ReplyError> for X11Error {
    fn from(value: ReplyError) -> Self {
        match value {
            ReplyError::ConnectionError(e) => e.into(),
            ReplyError::X11Error(e) => e.into(),
        }
    }
}

impl From<ime::ImeContextCreationError> for X11Error {
    fn from(value: ime::ImeContextCreationError) -> Self {
        match value {
            ime::ImeContextCreationError::XError(e) => e.into(),
            ime::ImeContextCreationError::Null => Self::UnexpectedNull("XOpenIM"),
        }
    }
}

impl From<ReplyOrIdError> for X11Error {
    fn from(value: ReplyOrIdError) -> Self {
        match value {
            ReplyOrIdError::ConnectionError(e) => e.into(),
            ReplyOrIdError::X11Error(e) => e.into(),
            ReplyOrIdError::IdsExhausted => Self::XidsExhausted(IdsExhausted),
        }
    }
}

/// The underlying x11rb connection that we are using.
type X11rbConnection = x11rb::xcb_ffi::XCBConnection;

/// Type alias for a void cookie.
type VoidCookie<'a> = x11rb::cookie::VoidCookie<'a, X11rbConnection>;

/// Extension trait for `Result<VoidCookie, E>`.
trait CookieResultExt {
    /// Unwrap the send error and ignore the result.
    fn expect_then_ignore_error(self, msg: &str);
}

impl<'a, E: fmt::Debug> CookieResultExt for Result<VoidCookie<'a>, E> {
    fn expect_then_ignore_error(self, msg: &str) {
        self.expect(msg).ignore_error()
    }
}

#[derive(Debug, Default, Copy, Clone)]
struct XExtension {
    opcode: c_int,
    first_event_id: c_int,
    first_error_id: c_int,
}

fn mkwid(w: xproto::Window) -> crate::window::WindowId {
    crate::window::WindowId(crate::platform_impl::platform::WindowId(w as _))
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
    fn new(info: &xinput::XIDeviceInfo) -> Result<Self, X11Error> {
        let name = str::from_utf8(&info.name).expect("device name is not valid utf8");
        let mut scroll_axes = Vec::new();

        if Device::physical_device(info) {
            // Identify scroll axes
            for class in info.classes.iter() {
                if let xinput::DeviceClassData::Scroll(info) = &class.data {
                    scroll_axes.push((
                        info.number,
                        ScrollAxis {
                            increment: xinput_fp3232_to_float(info.increment),
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
            _name: name.to_string(),
            scroll_axes,
            attachment: info.attachment,
        };
        device.reset_scroll_position(info);
        Ok(device)
    }

    fn reset_scroll_position(&mut self, info: &xinput::XIDeviceInfo) {
        if Device::physical_device(info) {
            for class in info.classes.iter() {
                if let xinput::DeviceClassData::Valuator(info) = &class.data {
                    if let Some(&mut (_, ref mut axis)) = self
                        .scroll_axes
                        .iter_mut()
                        .find(|&&mut (axis, _)| axis == info.number)
                    {
                        axis.position = xinput_fp3232_to_float(info.value);
                    }
                }
            }
        }
    }

    #[inline]
    fn physical_device(info: &xinput::XIDeviceInfo) -> bool {
        use xinput::DeviceType;
        info.type_ == DeviceType::SLAVE_KEYBOARD
            || info.type_ == DeviceType::SLAVE_POINTER
            || info.type_ == DeviceType::FLOATING_SLAVE
    }
}

// Xinput constants not defined in x11rb
const ALL_DEVICES: u16 = 0;

fn xinput_fp3232_to_float(fp: xinput::Fp3232) -> f64 {
    let xinput::Fp3232 { integral, frac } = fp;
    integral as f64 + (frac as f64 / (1u64 << 32) as f64)
}

fn xinput_fp1616_to_float(fp: xinput::Fp1616) -> f64 {
    (fp as f64) / ((1 << 16) as f64)
}
