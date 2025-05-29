use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::CStr;
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::os::raw::*;
use std::os::unix::io::{AsFd, AsRawFd, BorrowedFd, RawFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, LazyLock, Mutex, Weak};
use std::time::{Duration, Instant};
use std::{fmt, mem, ptr, slice, str};

use calloop::generic::Generic;
use calloop::ping::Ping;
use calloop::{EventLoop as Loop, Readiness};
use libc::{setlocale, LC_CTYPE};
use tracing::warn;
use winit_common::xkb::Context;
use winit_core::application::ApplicationHandler;
use winit_core::cursor::{CustomCursor as CoreCustomCursor, CustomCursorSource};
use winit_core::error::{EventLoopError, RequestError};
use winit_core::event::{DeviceId, StartCause, WindowEvent};
use winit_core::event_loop::pump_events::PumpStatus;
use winit_core::event_loop::{
    ActiveEventLoop as RootActiveEventLoop, ControlFlow, DeviceEvents,
    EventLoopProxy as CoreEventLoopProxy, EventLoopProxyProvider,
    OwnedDisplayHandle as CoreOwnedDisplayHandle,
};
use winit_core::monitor::MonitorHandle as CoreMonitorHandle;
use winit_core::window::{Theme, Window as CoreWindow, WindowAttributes, SurfaceId};
use x11rb::connection::RequestConnection;
use x11rb::errors::{ConnectError, ConnectionError, IdsExhausted, ReplyError};
use x11rb::protocol::xinput::{self, ConnectionExt as _};
use x11rb::protocol::{xkb, xproto};
use x11rb::x11_utils::X11Error as LogicalError;
use x11rb::xcb_ffi::ReplyOrIdError;

use crate::atoms::*;
use crate::dnd::Dnd;
use crate::event_processor::{EventProcessor, MAX_MOD_REPLAY_LEN};
use crate::ime::{self, Ime, ImeCreationError, ImeSender};
use crate::util::{self, CustomCursor};
use crate::window::{UnownedWindow, Window};
use crate::xdisplay::{XConnection, XError, XNotSupported};
use crate::{ffi, xsettings, XlibErrorHook};

// Xinput constants not defined in x11rb
pub(crate) const ALL_DEVICES: u16 = 0;
pub(crate) const ALL_MASTER_DEVICES: u16 = 1;
pub(crate) const ICONIC_STATE: u32 = 3;

/// The underlying x11rb connection that we are using.
type X11rbConnection = x11rb::xcb_ffi::XCBConnection;

type X11Source = Generic<BorrowedFd<'static>>;

pub(crate) static X11_BACKEND: LazyLock<Mutex<Result<Arc<XConnection>, XNotSupported>>> =
    LazyLock::new(|| Mutex::new(XConnection::new(Some(x_error_callback)).map(Arc::new)));

/// Hooks for X11 errors.
pub(crate) static XLIB_ERROR_HOOKS: Mutex<Vec<XlibErrorHook>> = Mutex::new(Vec::new());

unsafe extern "C" fn x_error_callback(
    display: *mut ffi::Display,
    event: *mut ffi::XErrorEvent,
) -> c_int {
    let xconn_lock = X11_BACKEND.lock().unwrap_or_else(|e| e.into_inner());
    if let Ok(ref xconn) = *xconn_lock {
        // Call all the hooks.
        let mut error_handled = false;
        for hook in XLIB_ERROR_HOOKS.lock().unwrap().iter() {
            error_handled |= hook(display as *mut _, event as *mut _);
        }

        // `assume_init` is safe here because the array consists of `MaybeUninit` values,
        // which do not require initialization.
        let mut buf: [MaybeUninit<c_char>; 1024] = unsafe { MaybeUninit::uninit().assume_init() };
        unsafe {
            (xconn.xlib.XGetErrorText)(
                display,
                (*event).error_code as c_int,
                buf.as_mut_ptr() as *mut c_char,
                buf.len() as c_int,
            )
        };
        let description =
            unsafe { CStr::from_ptr(buf.as_ptr() as *const c_char) }.to_string_lossy();

        let error = unsafe {
            XError {
                description: description.into_owned(),
                error_code: (*event).error_code,
                request_code: (*event).request_code,
                minor_code: (*event).minor_code,
            }
        };

        // Don't log error.
        if !error_handled {
            tracing::error!("X11 error: {:#?}", error);
            // XXX only update the error, if it wasn't handled by any of the hooks.
            *xconn.latest_error.lock().unwrap() = Some(error);
        }
    }
    // Fun fact: this return value is completely ignored.
    0
}

#[derive(Debug)]
pub(crate) struct WakeSender<T> {
    sender: Sender<T>,
    waker: Ping,
}

impl<T> Clone for WakeSender<T> {
    fn clone(&self) -> Self {
        Self { sender: self.sender.clone(), waker: self.waker.clone() }
    }
}

impl<T> WakeSender<T> {
    pub fn send(&self, t: T) {
        let res = self.sender.send(t);
        if res.is_ok() {
            self.waker.ping();
        }
    }
}

#[derive(Debug)]
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
            },
            Err(TryRecvError::Empty) => false,
            Err(TryRecvError::Disconnected) => {
                warn!("Channel was disconnected when checking incoming");
                false
            },
        }
    }

    pub fn try_recv(&mut self) -> Result<T, TryRecvError> {
        if let Some(first) = self.first.take() {
            return Ok(first);
        }
        self.recv.try_recv()
    }
}

#[derive(Debug)]
pub struct ActiveEventLoop {
    pub(crate) xconn: Arc<XConnection>,
    pub(crate) wm_delete_window: xproto::Atom,
    pub(crate) net_wm_ping: xproto::Atom,
    pub(crate) net_wm_sync_request: xproto::Atom,
    pub(crate) ime_sender: ImeSender,
    control_flow: Cell<ControlFlow>,
    exit: Cell<Option<i32>>,
    pub(crate) root: xproto::Window,
    pub(crate) ime: Option<RefCell<Ime>>,
    pub(crate) windows: RefCell<HashMap<SurfaceId, Weak<UnownedWindow>>>,
    pub(crate) redraw_sender: WakeSender<SurfaceId>,
    pub(crate) activation_sender: WakeSender<ActivationItem>,
    event_loop_proxy: CoreEventLoopProxy,
    device_events: Cell<DeviceEvents>,
}

#[derive(Debug)]
pub struct EventLoop {
    loop_running: bool,
    event_loop: Loop<'static, EventLoopState>,
    event_processor: EventProcessor,
    redraw_receiver: PeekableReceiver<SurfaceId>,
    activation_receiver: PeekableReceiver<ActivationItem>,

    /// The current state of the event loop.
    state: EventLoopState,
}

pub(crate) type ActivationItem = (SurfaceId, winit_core::event_loop::AsyncRequestSerial);

#[derive(Debug)]
struct EventLoopState {
    /// The latest readiness state for the x11 file descriptor
    x11_readiness: Readiness,

    /// User requested a wake up.
    proxy_wake_up: bool,
}

impl EventLoop {
    pub fn new() -> Result<EventLoop, EventLoopError> {
        static EVENT_LOOP_CREATED: AtomicBool = AtomicBool::new(false);
        if EVENT_LOOP_CREATED.swap(true, Ordering::Relaxed) {
            // Required?
            return Err(EventLoopError::RecreationAttempt);
        }

        let xconn = match X11_BACKEND.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            Ok(xconn) => xconn.clone(),
            Err(err) => return Err(os_error!(err.clone()).into()),
        };

        let root = xconn.default_root().root;
        let atoms = xconn.atoms();

        let wm_delete_window = atoms[WM_DELETE_WINDOW];
        let net_wm_ping = atoms[_NET_WM_PING];
        let net_wm_sync_request = atoms[_NET_WM_SYNC_REQUEST];

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
            setlocale(LC_CTYPE, c"".as_ptr() as *const _);

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

        let ime = Ime::new(Arc::clone(&xconn), ime_event_sender);
        if let Err(ImeCreationError::OpenFailure(state)) = ime.as_ref() {
            warn!("Failed to open input method: {state:#?}");
        } else if let Err(err) = ime.as_ref() {
            warn!("Failed to set input method destruction callback: {err:?}");
        }

        let ime = ime.ok().map(RefCell::new);

        let randr_event_offset =
            xconn.select_xrandr_input(root).expect("Failed to query XRandR extension");

        let xi2ext = xconn
            .xcb_connection()
            .extension_information(xinput::X11_EXTENSION_NAME)
            .expect("Failed to query XInput extension")
            .expect("X server missing XInput extension");
        let xkbext = xconn
            .xcb_connection()
            .extension_information(xkb::X11_EXTENSION_NAME)
            .expect("Failed to query XKB extension")
            .expect("X server missing XKB extension");

        // Check for XInput2 support.
        xconn
            .xcb_connection()
            .xinput_xi_query_version(2, 3)
            .expect("Failed to send XInput2 query version request")
            .reply()
            .expect("Error while checking for XInput2 query version reply");

        xconn.update_cached_wm_info(root);

        // Create an event loop.
        let event_loop =
            Loop::<EventLoopState>::try_new().expect("Failed to initialize the event loop");
        let handle = event_loop.handle();

        // Create the X11 event dispatcher.
        let source = X11Source::new(
            // SAFETY: xcb owns the FD and outlives the source.
            unsafe { BorrowedFd::borrow_raw(xconn.xcb_connection().as_raw_fd()) },
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
        let (user_waker, user_waker_source) =
            calloop::ping::make_ping().expect("Failed to create user event loop waker.");
        event_loop
            .handle()
            .insert_source(user_waker_source, move |_, _, state| {
                // No extra handling is required, we just need to wake-up.
                state.proxy_wake_up = true;
            })
            .expect("Failed to register the event loop waker source");
        let event_loop_proxy = EventLoopProxy::new(user_waker);

        let xkb_context =
            Context::from_x11_xkb(xconn.xcb_connection().get_raw_xcb_connection()).unwrap();

        let mut xmodmap = util::ModifierKeymap::new();
        xmodmap.reload_from_x_connection(&xconn);

        let window_target = ActiveEventLoop {
            ime,
            root,
            control_flow: Cell::new(ControlFlow::default()),
            exit: Cell::new(None),
            windows: Default::default(),
            ime_sender,
            xconn,
            wm_delete_window,
            net_wm_ping,
            net_wm_sync_request,
            redraw_sender: WakeSender {
                sender: redraw_sender, // not used again so no clone
                waker: waker.clone(),
            },
            activation_sender: WakeSender {
                sender: activation_token_sender, // not used again so no clone
                waker: waker.clone(),
            },
            event_loop_proxy: event_loop_proxy.into(),
            device_events: Default::default(),
        };

        // Set initial device event filter.
        window_target.update_listen_device_events(true);

        let event_processor = EventProcessor {
            target: window_target,
            dnd,
            devices: Default::default(),
            randr_event_offset,
            ime_receiver,
            ime_event_receiver,
            xi2ext,
            xfiltered_modifiers: VecDeque::with_capacity(MAX_MOD_REPLAY_LEN),
            xmodmap,
            xkbext,
            xkb_context,
            num_touch: 0,
            held_key_press: None,
            first_touch: None,
            active_window: None,
            modifiers: Default::default(),
            is_composing: false,
        };

        // Register for device hotplug events
        // (The request buffer is flushed during `init_device`)
        event_processor
            .target
            .xconn
            .select_xinput_events(
                root,
                ALL_DEVICES,
                x11rb::protocol::xinput::XIEventMask::HIERARCHY,
            )
            .expect_then_ignore_error("Failed to register for XInput2 device hotplug events");

        event_processor
            .target
            .xconn
            .select_xkb_events(
                0x100, // Use the "core keyboard device"
                xkb::EventType::NEW_KEYBOARD_NOTIFY
                    | xkb::EventType::MAP_NOTIFY
                    | xkb::EventType::STATE_NOTIFY,
            )
            .unwrap();

        event_processor.init_device(ALL_DEVICES);

        let event_loop = EventLoop {
            loop_running: false,
            event_loop,
            event_processor,
            redraw_receiver: PeekableReceiver::from_recv(redraw_channel),
            activation_receiver: PeekableReceiver::from_recv(activation_token_channel),
            state: EventLoopState { x11_readiness: Readiness::EMPTY, proxy_wake_up: false },
        };

        Ok(event_loop)
    }

    pub fn window_target(&self) -> &dyn RootActiveEventLoop {
        &self.event_processor.target
    }

    pub fn run_app<A: ApplicationHandler>(mut self, app: A) -> Result<(), EventLoopError> {
        self.run_app_on_demand(app)
    }

    pub fn run_app_on_demand<A: ApplicationHandler>(
        &mut self,
        mut app: A,
    ) -> Result<(), EventLoopError> {
        self.event_processor.target.clear_exit();
        let exit = loop {
            match self.pump_app_events(None, &mut app) {
                PumpStatus::Exit(0) => {
                    break Ok(());
                },
                PumpStatus::Exit(code) => {
                    break Err(EventLoopError::ExitFailure(code));
                },
                _ => {
                    continue;
                },
            }
        };

        // Applications aren't allowed to carry windows between separate
        // `run_on_demand` calls but if they have only just dropped their
        // windows we need to make sure those last requests are sent to the
        // X Server.
        self.event_processor
            .target
            .x_connection()
            .sync_with_server()
            .map_err(|x_err| EventLoopError::Os(os_error!(X11Error::Xlib(x_err))))?;

        exit
    }

    pub fn pump_app_events<A: ApplicationHandler>(
        &mut self,
        timeout: Option<Duration>,
        mut app: A,
    ) -> PumpStatus {
        if !self.loop_running {
            self.loop_running = true;

            // run the initial loop iteration
            self.single_iteration(&mut app, StartCause::Init);
        }

        // Consider the possibility that the `StartCause::Init` iteration could
        // request to Exit.
        if !self.exiting() {
            self.poll_events_with_timeout(timeout, &mut app);
        }
        if let Some(code) = self.exit_code() {
            self.loop_running = false;

            PumpStatus::Exit(code)
        } else {
            PumpStatus::Continue
        }
    }

    fn has_pending(&mut self) -> bool {
        self.event_processor.poll()
            || self.state.proxy_wake_up
            || self.redraw_receiver.has_incoming()
    }

    fn poll_events_with_timeout<A: ApplicationHandler>(
        &mut self,
        mut timeout: Option<Duration>,
        app: &mut A,
    ) {
        let start = Instant::now();

        let has_pending = self.has_pending();

        timeout = if has_pending {
            // If we already have work to do then we don't want to block on the next poll.
            Some(Duration::ZERO)
        } else {
            let control_flow_timeout = match self.control_flow() {
                ControlFlow::Wait => None,
                ControlFlow::Poll => Some(Duration::ZERO),
                ControlFlow::WaitUntil(wait_deadline) => {
                    Some(wait_deadline.saturating_duration_since(start))
                },
            };

            min_timeout(control_flow_timeout, timeout)
        };

        self.state.x11_readiness = Readiness::EMPTY;
        if let Err(error) =
            self.event_loop.dispatch(timeout, &mut self.state).map_err(std::io::Error::from)
        {
            tracing::error!("Failed to poll for events: {error:?}");
            let exit_code = error.raw_os_error().unwrap_or(1);
            self.set_exit_code(exit_code);
            return;
        }

        // NB: `StartCause::Init` is handled as a special case and doesn't need
        // to be considered here
        let cause = match self.control_flow() {
            ControlFlow::Poll => StartCause::Poll,
            ControlFlow::Wait => StartCause::WaitCancelled { start, requested_resume: None },
            ControlFlow::WaitUntil(deadline) => {
                if Instant::now() < deadline {
                    StartCause::WaitCancelled { start, requested_resume: Some(deadline) }
                } else {
                    StartCause::ResumeTimeReached { start, requested_resume: deadline }
                }
            },
        };

        // False positive / spurious wake ups could lead to us spamming
        // redundant iterations of the event loop with no new events to
        // dispatch.
        //
        // If there's no readable event source then we just double check if we
        // have any pending `_receiver` events and if not we return without
        // running a loop iteration.
        // If we don't have any pending `_receiver`
        if !self.has_pending()
            && !matches!(&cause, StartCause::ResumeTimeReached { .. } | StartCause::Poll)
            && timeout.is_none()
        {
            return;
        }

        self.single_iteration(app, cause);
    }

    fn single_iteration<A: ApplicationHandler>(&mut self, app: &mut A, cause: StartCause) {
        app.new_events(&self.event_processor.target, cause);

        // NB: For consistency all platforms must call `can_create_surfaces` even though X11
        // applications don't themselves have a formal surface destroy/create lifecycle.
        if cause == StartCause::Init {
            app.can_create_surfaces(&self.event_processor.target)
        }

        // Process all pending events
        self.drain_events(app);

        // Empty activation tokens.
        while let Ok((window_id, serial)) = self.activation_receiver.try_recv() {
            let token = self
                .event_processor
                .with_window(window_id.into_raw() as xproto::Window, |window| {
                    window.generate_activation_token()
                });

            match token {
                Some(Ok(token)) => {
                    let event = WindowEvent::ActivationTokenDone {
                        serial,
                        token: winit_core::window::ActivationToken::from_raw(token),
                    };
                    app.window_event(&self.event_processor.target, window_id, event);
                },
                Some(Err(e)) => {
                    tracing::error!("Failed to get activation token: {}", e);
                },
                None => {},
            }
        }

        // Empty the user event buffer
        if mem::take(&mut self.state.proxy_wake_up) {
            app.proxy_wake_up(&self.event_processor.target);
        }

        // Empty the redraw requests
        {
            let mut windows = HashSet::new();

            while let Ok(window_id) = self.redraw_receiver.try_recv() {
                windows.insert(window_id);
            }

            for window_id in windows {
                app.window_event(
                    &self.event_processor.target,
                    window_id,
                    WindowEvent::RedrawRequested,
                );
            }
        }

        // This is always the last event we dispatch before poll again
        app.about_to_wait(&self.event_processor.target);
    }

    fn drain_events<A: ApplicationHandler>(&mut self, app: &mut A) {
        let mut xev = MaybeUninit::uninit();

        while unsafe { self.event_processor.poll_one_event(xev.as_mut_ptr()) } {
            let mut xev = unsafe { xev.assume_init() };
            self.event_processor.process_event(&mut xev, app);
        }
    }

    fn control_flow(&self) -> ControlFlow {
        self.event_processor.target.control_flow()
    }

    fn exiting(&self) -> bool {
        self.event_processor.target.exiting()
    }

    fn set_exit_code(&self, code: i32) {
        self.event_processor.target.set_exit_code(code);
    }

    fn exit_code(&self) -> Option<i32> {
        self.event_processor.target.exit_code()
    }
}

impl AsFd for EventLoop {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.event_loop.as_fd()
    }
}

impl AsRawFd for EventLoop {
    fn as_raw_fd(&self) -> RawFd {
        self.event_loop.as_raw_fd()
    }
}

impl ActiveEventLoop {
    /// Returns the `XConnection` of this events loop.
    #[inline]
    pub(crate) fn x_connection(&self) -> &Arc<XConnection> {
        &self.xconn
    }

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
            .select_xinput_events(self.root, ALL_MASTER_DEVICES, mask)
            .expect_then_ignore_error("Failed to update device event filter");
    }

    pub(crate) fn clear_exit(&self) {
        self.exit.set(None)
    }

    pub(crate) fn set_exit_code(&self, code: i32) {
        self.exit.set(Some(code))
    }

    pub(crate) fn exit_code(&self) -> Option<i32> {
        self.exit.get()
    }
}

impl RootActiveEventLoop for ActiveEventLoop {
    fn create_proxy(&self) -> CoreEventLoopProxy {
        self.event_loop_proxy.clone()
    }

    fn create_window(
        &self,
        window_attributes: WindowAttributes,
    ) -> Result<Box<dyn CoreWindow>, RequestError> {
        Ok(Box::new(Window::new(self, window_attributes)?))
    }

    fn create_custom_cursor(
        &self,
        custom_cursor: CustomCursorSource,
    ) -> Result<CoreCustomCursor, RequestError> {
        Ok(CoreCustomCursor(Arc::new(CustomCursor::new(self, custom_cursor)?)))
    }

    fn available_monitors(&self) -> Box<dyn Iterator<Item = CoreMonitorHandle>> {
        Box::new(
            self.xconn
                .available_monitors()
                .into_iter()
                .flatten()
                .map(|monitor| CoreMonitorHandle(Arc::new(monitor))),
        )
    }

    fn primary_monitor(&self) -> Option<CoreMonitorHandle> {
        self.xconn.primary_monitor().ok().map(|monitor| CoreMonitorHandle(Arc::new(monitor)))
    }

    fn system_theme(&self) -> Option<Theme> {
        None
    }

    fn listen_device_events(&self, allowed: DeviceEvents) {
        self.device_events.set(allowed);
    }

    fn set_control_flow(&self, control_flow: ControlFlow) {
        self.control_flow.set(control_flow)
    }

    fn control_flow(&self) -> ControlFlow {
        self.control_flow.get()
    }

    fn exit(&self) {
        self.exit.set(Some(0))
    }

    fn exiting(&self) -> bool {
        self.exit.get().is_some()
    }

    fn owned_display_handle(&self) -> CoreOwnedDisplayHandle {
        CoreOwnedDisplayHandle::new(self.x_connection().clone())
    }

    fn rwh_06_handle(&self) -> &dyn rwh_06::HasDisplayHandle {
        self
    }
}

impl rwh_06::HasDisplayHandle for ActiveEventLoop {
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        self.xconn.display_handle()
    }
}

pub(crate) struct DeviceInfo<'a> {
    xconn: &'a XConnection,
    info: *const ffi::XIDeviceInfo,
    count: usize,
}

impl<'a> DeviceInfo<'a> {
    pub(crate) fn get(xconn: &'a XConnection, device: c_int) -> Option<Self> {
        unsafe {
            let mut count = 0;
            let info = (xconn.xinput2.XIQueryDevice)(xconn.display, device, &mut count);
            xconn.check_errors().ok()?;

            if info.is_null() || count == 0 {
                None
            } else {
                Some(DeviceInfo { xconn, info, count: count as usize })
            }
        }
    }
}

impl Drop for DeviceInfo<'_> {
    fn drop(&mut self) {
        assert!(!self.info.is_null());
        unsafe { (self.xconn.xinput2.XIFreeDeviceInfo)(self.info as *mut _) };
    }
}

impl Deref for DeviceInfo<'_> {
    type Target = [ffi::XIDeviceInfo];

    fn deref(&self) -> &Self::Target {
        unsafe { slice::from_raw_parts(self.info, self.count) }
    }
}

#[derive(Clone, Debug)]
pub struct EventLoopProxy {
    ping: Ping,
}

impl EventLoopProxyProvider for EventLoopProxy {
    fn wake_up(&self) {
        self.ping.ping();
    }
}

impl EventLoopProxy {
    fn new(ping: Ping) -> Self {
        Self { ping }
    }
}

impl From<EventLoopProxy> for CoreEventLoopProxy {
    fn from(value: EventLoopProxy) -> Self {
        CoreEventLoopProxy::new(Arc::new(value))
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

    /// An extension that we rely on is not available.
    MissingExtension(&'static str),

    /// Could not find a matching X11 visual for this visualid
    NoSuchVisual(xproto::Visualid),

    /// Unable to parse xsettings.
    XsettingsParse(xsettings::ParserError),

    /// Failed to get property.
    GetProperty(util::GetPropertyError),

    /// Could not find an ARGB32 pict format.
    NoArgb32Format,
}

impl fmt::Display for X11Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            X11Error::Xlib(e) => write!(f, "Xlib error: {e}"),
            X11Error::Connect(e) => write!(f, "X11 connection error: {e}"),
            X11Error::Connection(e) => write!(f, "X11 connection error: {e}"),
            X11Error::XidsExhausted(e) => write!(f, "XID range exhausted: {e}"),
            X11Error::GetProperty(e) => write!(f, "Failed to get X property {e}"),
            X11Error::X11(e) => write!(f, "X11 error: {e:?}"),
            X11Error::UnexpectedNull(s) => write!(f, "Xlib function returned null: {s}"),
            X11Error::InvalidActivationToken(s) => write!(
                f,
                "Invalid activation token: {}",
                std::str::from_utf8(s).unwrap_or("<invalid utf8>")
            ),
            X11Error::MissingExtension(s) => write!(f, "Missing X11 extension: {s}"),
            X11Error::NoSuchVisual(visualid) => {
                write!(f, "Could not find a matching X11 visual for ID `{visualid:x}`")
            },
            X11Error::XsettingsParse(err) => {
                write!(f, "Failed to parse xsettings: {err:?}")
            },
            X11Error::NoArgb32Format => {
                f.write_str("winit only supports X11 displays with ARGB32 picture formats")
            },
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

impl From<xsettings::ParserError> for X11Error {
    fn from(value: xsettings::ParserError) -> Self {
        Self::XsettingsParse(value)
    }
}

impl From<util::GetPropertyError> for X11Error {
    fn from(value: util::GetPropertyError) -> Self {
        Self::GetProperty(value)
    }
}

/// Type alias for a void cookie.
pub(crate) type VoidCookie<'a> = x11rb::cookie::VoidCookie<'a, X11rbConnection>;

/// Extension trait for `Result<VoidCookie, E>`.
pub(crate) trait CookieResultExt {
    /// Unwrap the send error and ignore the result.
    fn expect_then_ignore_error(self, msg: &str);
}

impl<E: fmt::Debug> CookieResultExt for Result<VoidCookie<'_>, E> {
    fn expect_then_ignore_error(self, msg: &str) {
        self.expect(msg).ignore_error()
    }
}

pub(crate) fn mkwid(w: xproto::Window) -> winit_core::window::SurfaceId {
    winit_core::window::SurfaceId::from_raw(w as _)
}

pub(crate) fn mkdid(w: xinput::DeviceId) -> DeviceId {
    DeviceId::from_raw(w as i64)
}

#[derive(Debug)]
pub struct Device {
    _name: String,
    pub(crate) scroll_axes: Vec<(i32, ScrollAxis)>,
    // For master devices, this is the paired device (pointer <-> keyboard).
    // For slave devices, this is the master.
    pub(crate) attachment: c_int,
}

#[derive(Debug, Copy, Clone)]
pub(crate) struct ScrollAxis {
    pub(crate) increment: f64,
    pub(crate) orientation: ScrollOrientation,
    pub(crate) position: f64,
}

#[derive(Debug, Copy, Clone)]
pub(crate) enum ScrollOrientation {
    Vertical,
    Horizontal,
}

impl Device {
    pub(crate) fn new(info: &ffi::XIDeviceInfo) -> Self {
        let name = unsafe { CStr::from_ptr(info.name).to_string_lossy() };
        let mut scroll_axes = Vec::new();

        if Device::physical_device(info) {
            // Identify scroll axes
            for &class_ptr in Device::classes(info) {
                let ty = unsafe { (*class_ptr)._type };
                if ty == ffi::XIScrollClass {
                    let info = unsafe { &*(class_ptr as *const ffi::XIScrollClassInfo) };
                    scroll_axes.push((info.number, ScrollAxis {
                        increment: info.increment,
                        orientation: match info.scroll_type {
                            ffi::XIScrollTypeHorizontal => ScrollOrientation::Horizontal,
                            ffi::XIScrollTypeVertical => ScrollOrientation::Vertical,
                            _ => unreachable!(),
                        },
                        position: 0.0,
                    }));
                }
            }
        }

        let mut device =
            Device { _name: name.into_owned(), scroll_axes, attachment: info.attachment };
        device.reset_scroll_position(info);
        device
    }

    pub(crate) fn reset_scroll_position(&mut self, info: &ffi::XIDeviceInfo) {
        if Device::physical_device(info) {
            for &class_ptr in Device::classes(info) {
                let ty = unsafe { (*class_ptr)._type };
                if ty == ffi::XIValuatorClass {
                    let info = unsafe { &*(class_ptr as *const ffi::XIValuatorClassInfo) };
                    if let Some(&mut (_, ref mut axis)) =
                        self.scroll_axes.iter_mut().find(|&&mut (axis, _)| axis == info.number)
                    {
                        axis.position = info.value;
                    }
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

/// Convert the raw X11 representation for a 32-bit floating point to a double.
#[inline]
pub(crate) fn xinput_fp1616_to_float(fp: xinput::Fp1616) -> f64 {
    (fp as f64) / ((1 << 16) as f64)
}

/// Returns the minimum `Option<Duration>`, taking into account that `None`
/// equates to an infinite timeout, not a zero timeout (so can't just use
/// `Option::min`)
fn min_timeout(a: Option<Duration>, b: Option<Duration>) -> Option<Duration> {
    a.map_or(b, |a_timeout| b.map_or(Some(a_timeout), |b_timeout| Some(a_timeout.min(b_timeout))))
}
