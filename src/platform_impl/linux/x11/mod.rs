#![cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]

mod dnd;
mod event_processor;
mod monitor;
pub mod util;
mod window;
mod xdisplay;
pub mod xlib;

pub use self::{
    monitor::{MonitorHandle, VideoMode},
    window::UnownedWindow,
    xdisplay::{XConnection, XError, XNotSupported},
};

use std::sync::atomic::AtomicUsize;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    ops::Deref,
    os::raw::*,
    ptr,
    rc::Rc,
    sync::mpsc::Receiver,
    sync::{Arc, Weak},
    time::{Duration, Instant},
};

use mio::{unix::SourceFd, Events, Interest, Poll, Token, Waker};

use mio_misc::{
    channel::{channel, SendError, Sender},
    queue::NotificationQueue,
    NotificationId,
};

use self::{
    dnd::{Dnd, DndState},
    event_processor::EventProcessor,
};
use crate::{
    error::OsError as RootOsError,
    event::{Event, StartCause},
    event_loop::{ControlFlow, EventLoopClosed, EventLoopWindowTarget as RootELW},
    platform_impl::{platform::sticky_exit_callback, PlatformSpecificWindowBuilderAttributes},
    window::WindowAttributes,
};

use crate::platform_impl::x11::util::EventQueue;
use xcb_dl::{ffi, XcbXinput};
use xcb_dl_util::xcb_box::XcbBox;

const X_TOKEN: Token = Token(0);
const USER_REDRAW_TOKEN: Token = Token(1);

pub struct EventLoopWindowTarget<T> {
    xconn: Arc<XConnection>,
    wm_delete_window: ffi::xcb_atom_t,
    net_wm_ping: ffi::xcb_atom_t,
    windows: RefCell<HashMap<WindowId, Weak<UnownedWindow>>>,
    redraw_sender: Sender<WindowId>,
    reset_dead_keys: Arc<AtomicUsize>,
    _marker: ::std::marker::PhantomData<T>,
}

pub struct EventLoop<T: 'static> {
    poll: Poll,
    event_queue: EventQueue,
    event_processor: EventProcessor<T>,
    redraw_channel: Receiver<WindowId>,
    user_channel: Receiver<T>,
    user_sender: Sender<T>,
    target: Rc<RootELW<T>>,
}

pub struct EventLoopProxy<T: 'static> {
    user_sender: Sender<T>,
}

impl<T: 'static> Clone for EventLoopProxy<T> {
    fn clone(&self) -> Self {
        EventLoopProxy {
            user_sender: self.user_sender.clone(),
        }
    }
}

impl<T: 'static> EventLoop<T> {
    pub fn new(xconn: Arc<XConnection>) -> EventLoop<T> {
        let wm_delete_window = xconn.get_atom("WM_DELETE_WINDOW");

        let net_wm_ping = xconn.get_atom("_NET_WM_PING");

        let dnd = Dnd::new(Arc::clone(&xconn));

        xconn.update_cached_wm_info();

        let poll = Poll::new().unwrap();
        let waker = Arc::new(Waker::new(poll.registry(), USER_REDRAW_TOKEN).unwrap());
        let queue = Arc::new(NotificationQueue::new(waker));

        poll.registry()
            .register(&mut SourceFd(&xconn.fd), X_TOKEN, Interest::READABLE)
            .unwrap();

        let (user_sender, user_channel) = channel(queue.clone(), NotificationId::gen_next());

        let (redraw_sender, redraw_channel) = channel(queue, NotificationId::gen_next());

        let event_queue = EventQueue::new(&xconn);

        let target = Rc::new(RootELW {
            p: super::EventLoopWindowTarget::X(EventLoopWindowTarget {
                windows: Default::default(),
                _marker: ::std::marker::PhantomData,
                xconn,
                wm_delete_window,
                net_wm_ping,
                redraw_sender,
                reset_dead_keys: Arc::new(AtomicUsize::new(0)),
            }),
            _marker: ::std::marker::PhantomData,
        });

        let mut event_processor = EventProcessor {
            target: target.clone(),
            dnd,
            devices: Default::default(),
            num_touch: 0,
            first_touch: None,
            seats: Default::default(),
        };

        // Register for device hotplug events
        let wt = get_xtarget(&target);
        for screen in &wt.xconn.screens {
            let pending = wt.xconn.select_xinput_events(
                screen.root,
                ffi::XCB_INPUT_DEVICE_ALL as _,
                ffi::XCB_INPUT_XI_EVENT_MASK_HIERARCHY,
            );
            if let Err(e) = wt.xconn.check_pending1(pending) {
                log::error!("Cannot listen for device hotplug events: {}", e);
            }
        }

        EventProcessor::init_device(
            &target,
            &mut event_processor.devices,
            &mut event_processor.seats,
            ffi::XCB_INPUT_DEVICE_ALL as _,
        );

        let result = EventLoop {
            poll,
            event_queue,
            redraw_channel,
            user_channel,
            user_sender,
            event_processor,
            target,
        };

        result
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

            // Send unchecked requests
            self.flush_requests();

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

            if self.event_queue.has_pending_events() {
                // If there are pending events that have already been read from the socket
                // but not yet dispatched, we HAVE to handle them now. Otherwise, if the
                // application is using run_return, it has no way to get notified that it
                // should call run_return again. The application will probably try to wait
                // for the socket to become readable but that's no good because the socket
                // might be empty while we already have events queued.
                //
                // TODO: Should we change `cause`?
                continue;
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

            self.poll.poll(&mut events, timeout).unwrap();
            events.clear();

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

        let wt = get_xtarget(&self.target);

        while let Some(mut event) = self.event_queue.poll_for_event() {
            self.event_processor.process_event(&mut *event, |event| {
                sticky_exit_callback(
                    event,
                    target,
                    control_flow,
                    &mut |event, window_target, control_flow| {
                        if let Event::RedrawRequested(crate::window::WindowId(
                            super::WindowId::X(wid),
                        )) = event
                        {
                            wt.redraw_sender.send(wid).unwrap();
                        } else {
                            callback(event, window_target, control_flow);
                        }
                    },
                );
            });
        }
    }

    fn flush_requests(&self) {
        let wt = get_xtarget(&self.target);
        if let Err(e) = wt.xconn.flush() {
            panic!("The connection to the X server failed: {}", e);
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
        self.user_sender.send(event).map_err(|e| {
            EventLoopClosed(if let SendError::Disconnected(x) = e {
                x
            } else {
                unreachable!()
            })
        })
    }
}

struct DeviceInfo<'a> {
    xconn: &'a XConnection,
    _reply: XcbBox<ffi::xcb_input_xi_query_device_reply_t>,
    iter: ffi::xcb_input_xi_device_info_iterator_t,
}

impl<'a> DeviceInfo<'a> {
    fn get(xconn: &'a XConnection, device: ffi::xcb_input_device_id_t) -> Option<Self> {
        unsafe {
            let mut err = ptr::null_mut();
            let reply = xconn.xinput.xcb_input_xi_query_device_reply(
                xconn.c,
                xconn.xinput.xcb_input_xi_query_device(xconn.c, device),
                &mut err,
            );
            let reply = match xconn.check(reply, err) {
                Ok(i) => i,
                Err(e) => {
                    log::error!("Could not query device data: {}", e);
                    return None;
                }
            };
            let iter = xconn
                .xinput
                .xcb_input_xi_query_device_infos_iterator(&*reply);
            Some(DeviceInfo {
                xconn,
                _reply: reply,
                iter,
            })
        }
    }
}

impl<'a> Iterator for DeviceInfo<'a> {
    type Item = *mut ffi::xcb_input_xi_device_info_t;

    fn next(&mut self) -> Option<Self::Item> {
        if self.iter.rem > 0 {
            let data = self.iter.data;
            unsafe {
                self.xconn
                    .xinput
                    .xcb_input_xi_device_info_next(&mut self.iter);
            }
            Some(data)
        } else {
            None
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(ffi::xcb_window_t);

impl WindowId {
    #[cfg(not(feature = "wayland"))]
    pub unsafe fn dummy() -> Self {
        WindowId(0)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceId(pub(crate) ffi::xcb_input_device_id_t);

impl DeviceId {
    #[cfg(not(feature = "wayland"))]
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
            let cookie = xconn.xcb.xcb_destroy_window_checked(xconn.c, window.id().0);
            if let Err(e) = xconn.check_cookie(cookie) {
                log::error!("Could not destroy window: {}", e);
            }
        }
    }
}

#[derive(Debug, Default, Copy, Clone)]
struct XExtension {
    opcode: c_int,
    first_event_id: c_int,
    first_error_id: c_int,
}

fn mkwid(w: ffi::xcb_window_t) -> crate::window::WindowId {
    crate::window::WindowId(crate::platform_impl::WindowId::X(WindowId(w)))
}
fn mkdid(w: ffi::xcb_input_device_id_t) -> crate::event::DeviceId {
    crate::event::DeviceId(crate::platform_impl::DeviceId::X(DeviceId(w)))
}

#[derive(Debug)]
struct Device {
    name: String,
    scroll_axes: Vec<(u16, ScrollAxis)>,
    // For master devices, this is the paired device (pointer <-> keyboard).
    // For slave devices, this is the master.
    attachment: ffi::xcb_input_device_id_t,
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
    fn new<T: 'static>(
        wt: &EventLoopWindowTarget<T>,
        info: &ffi::xcb_input_xi_device_info_t,
    ) -> Self {
        let name = unsafe {
            let name = std::slice::from_raw_parts(
                wt.xconn.xinput.xcb_input_xi_device_info_name(info) as _,
                info.name_len as _,
            );
            String::from_utf8_lossy(name)
        };
        let mut scroll_axes = Vec::new();

        if info.type_ == ffi::XCB_INPUT_DEVICE_TYPE_MASTER_KEYBOARD as u16 {
            let pending = wt.xconn.select_xkb_events(
                info.deviceid,
                ffi::XCB_XKB_EVENT_TYPE_NEW_KEYBOARD_NOTIFY | ffi::XCB_XKB_EVENT_TYPE_MAP_NOTIFY,
            );
            if let Err(e) = wt.xconn.check_pending1(pending) {
                log::error!(
                    "Cannot listen for keyboard layout changes of keyboard {}: {}",
                    info.deviceid,
                    e
                );
            }
        }

        if Device::physical_device(info) {
            // Register for global raw events
            let mask = ffi::XCB_INPUT_XI_EVENT_MASK_RAW_MOTION
                | ffi::XCB_INPUT_XI_EVENT_MASK_RAW_BUTTON_PRESS
                | ffi::XCB_INPUT_XI_EVENT_MASK_RAW_BUTTON_RELEASE
                | ffi::XCB_INPUT_XI_EVENT_MASK_RAW_KEY_PRESS
                | ffi::XCB_INPUT_XI_EVENT_MASK_RAW_KEY_RELEASE;
            for screen in &wt.xconn.screens {
                let pending = wt
                    .xconn
                    .select_xinput_events(screen.root, info.deviceid, mask);
                if let Err(e) = wt.xconn.check_pending1(pending) {
                    log::error!(
                        "Cannot listen for raw input events of device {}: {}",
                        info.deviceid,
                        e
                    );
                }
            }

            // Identify scroll axes
            let classes = unsafe { Classes::new(&wt.xconn.xinput, info) };
            for class in classes {
                match class.type_ as ffi::xcb_input_device_class_type_t {
                    ffi::XCB_INPUT_DEVICE_CLASS_TYPE_SCROLL => {
                        let info = unsafe {
                            &*(class as *const _ as *const ffi::xcb_input_scroll_class_t)
                        };
                        scroll_axes.push((
                            info.number,
                            ScrollAxis {
                                increment: util::fp3232_to_f64(info.increment),
                                orientation: match info.scroll_type as ffi::xcb_input_scroll_type_t
                                {
                                    ffi::XCB_INPUT_SCROLL_TYPE_HORIZONTAL => {
                                        ScrollOrientation::Horizontal
                                    }
                                    ffi::XCB_INPUT_SCROLL_TYPE_VERTICAL => {
                                        ScrollOrientation::Vertical
                                    }
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
        device.reset_scroll_position(wt, info);
        device
    }

    fn reset_scroll_position<T: 'static>(
        &mut self,
        wt: &EventLoopWindowTarget<T>,
        info: &ffi::xcb_input_xi_device_info_t,
    ) {
        if Device::physical_device(info) {
            let classes = unsafe { Classes::new(&wt.xconn.xinput, info) };
            for class in classes {
                match class.type_ as ffi::xcb_input_device_class_type_t {
                    ffi::XCB_INPUT_DEVICE_CLASS_TYPE_VALUATOR => {
                        let info = unsafe {
                            &*(class as *const _ as *const ffi::xcb_input_valuator_class_t)
                        };
                        if let Some(&mut (_, ref mut axis)) = self
                            .scroll_axes
                            .iter_mut()
                            .find(|&&mut (axis, _)| axis == info.number)
                        {
                            axis.position = util::fp3232_to_f64(info.value);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    #[inline]
    fn physical_device(info: &ffi::xcb_input_xi_device_info_t) -> bool {
        let ty = info.type_ as ffi::xcb_input_device_type_t;
        ty == ffi::XCB_INPUT_DEVICE_TYPE_SLAVE_KEYBOARD
            || ty == ffi::XCB_INPUT_DEVICE_TYPE_SLAVE_POINTER
            || ty == ffi::XCB_INPUT_DEVICE_TYPE_FLOATING_SLAVE
    }
}

struct Classes<'a> {
    xinput: &'a XcbXinput,
    iter: ffi::xcb_input_device_class_iterator_t,
}

impl<'a> Classes<'a> {
    unsafe fn new(xinput: &'a XcbXinput, info: &'a ffi::xcb_input_xi_device_info_t) -> Self {
        Self {
            xinput,
            iter: xinput.xcb_input_xi_device_info_classes_iterator(info),
        }
    }
}

impl<'a> Iterator for Classes<'a> {
    type Item = &'a ffi::xcb_input_device_class_t;

    fn next(&mut self) -> Option<Self::Item> {
        if self.iter.rem > 0 {
            unsafe {
                let res = &*self.iter.data;
                self.xinput.xcb_input_device_class_next(&mut self.iter);
                Some(res)
            }
        } else {
            None
        }
    }
}
