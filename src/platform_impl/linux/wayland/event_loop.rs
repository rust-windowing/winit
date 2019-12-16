use std::{
    cell::RefCell,
    collections::VecDeque,
    fmt,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Instant,
};

use smithay_client_toolkit::reexports::protocols::unstable::pointer_constraints::v1::client::{
    zwp_locked_pointer_v1::ZwpLockedPointerV1, zwp_pointer_constraints_v1::ZwpPointerConstraintsV1,
};
use smithay_client_toolkit::reexports::protocols::unstable::relative_pointer::v1::client::{
    zwp_relative_pointer_manager_v1::ZwpRelativePointerManagerV1,
    zwp_relative_pointer_v1::ZwpRelativePointerV1,
};

use smithay_client_toolkit::pointer::{AutoPointer, AutoThemer};
use smithay_client_toolkit::reexports::client::protocol::{
    wl_compositor::WlCompositor, wl_shm::WlShm, wl_surface::WlSurface,
};

use crate::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::ModifiersState,
    event_loop::{ControlFlow, EventLoopClosed, EventLoopWindowTarget as RootELW},
    monitor::{MonitorHandle as RootMonitorHandle, VideoMode as RootVideoMode},
    platform_impl::platform::{
        sticky_exit_callback, MonitorHandle as PlatformMonitorHandle,
        VideoMode as PlatformVideoMode,
    },
    window::CursorIcon,
};

use super::{window::WindowStore, DeviceId, WindowId};

use smithay_client_toolkit::{
    output::OutputMgr,
    reexports::client::{
        protocol::{wl_keyboard, wl_output, wl_pointer, wl_registry, wl_seat, wl_touch},
        ConnectError, Display, EventQueue, GlobalEvent,
    },
    Environment,
};

pub struct WindowEventsSink<T> {
    buffer: VecDeque<crate::event::Event<T>>,
}

impl<T> WindowEventsSink<T> {
    pub fn new() -> WindowEventsSink<T> {
        WindowEventsSink {
            buffer: VecDeque::new(),
        }
    }

    pub fn send_event(&mut self, evt: crate::event::Event<T>) {
        self.buffer.push_back(evt);
    }

    pub fn send_window_event(&mut self, evt: crate::event::WindowEvent, wid: WindowId) {
        self.buffer.push_back(crate::event::Event::WindowEvent {
            event: evt,
            window_id: crate::window::WindowId(crate::platform_impl::WindowId::Wayland(wid)),
        });
    }

    pub fn send_device_event(&mut self, evt: crate::event::DeviceEvent, dev_id: DeviceId) {
        self.buffer.push_back(crate::event::Event::DeviceEvent {
            event: evt,
            device_id: crate::event::DeviceId(crate::platform_impl::DeviceId::Wayland(dev_id)),
        });
    }

    fn empty_with<F>(&mut self, mut callback: F)
    where
        F: FnMut(crate::event::Event<T>),
    {
        for evt in self.buffer.drain(..) {
            callback(evt)
        }
    }
}

pub struct CursorManager {
    pointer_constraints_proxy: Arc<Mutex<Option<ZwpPointerConstraintsV1>>>,
    auto_themer: Option<AutoThemer>,
    pointers: Vec<AutoPointer>,
    locked_pointers: Vec<ZwpLockedPointerV1>,
    cursor_visible: bool,
    current_cursor: CursorIcon,
}

impl CursorManager {
    fn new(constraints: Arc<Mutex<Option<ZwpPointerConstraintsV1>>>) -> CursorManager {
        CursorManager {
            pointer_constraints_proxy: constraints,
            auto_themer: None,
            pointers: Vec::new(),
            locked_pointers: Vec::new(),
            cursor_visible: true,
            current_cursor: CursorIcon::default(),
        }
    }

    fn register_pointer(&mut self, pointer: wl_pointer::WlPointer) {
        let auto_themer = self
            .auto_themer
            .as_ref()
            .expect("AutoThemer not initialized. Server did not advertise shm or compositor?");
        self.pointers.push(auto_themer.theme_pointer(pointer));
    }

    fn set_auto_themer(&mut self, auto_themer: AutoThemer) {
        self.auto_themer = Some(auto_themer);
    }

    pub fn set_cursor_visible(&mut self, visible: bool) {
        if !visible {
            for pointer in self.pointers.iter() {
                (**pointer).set_cursor(0, None, 0, 0);
            }
        } else {
            self.set_cursor_icon_impl(self.current_cursor);
        }
        self.cursor_visible = visible;
    }

    /// A helper function to restore cursor styles on PtrEvent::Enter.
    pub fn reload_cursor_style(&mut self) {
        if !self.cursor_visible {
            self.set_cursor_visible(false);
        } else {
            self.set_cursor_icon_impl(self.current_cursor);
        }
    }

    pub fn set_cursor_icon(&mut self, cursor: CursorIcon) {
        if cursor != self.current_cursor {
            self.current_cursor = cursor;
            if self.cursor_visible {
                self.set_cursor_icon_impl(cursor);
            }
        }
    }

    fn set_cursor_icon_impl(&mut self, cursor: CursorIcon) {
        let cursor = match cursor {
            CursorIcon::Alias => "link",
            CursorIcon::Arrow => "arrow",
            CursorIcon::Cell => "plus",
            CursorIcon::Copy => "copy",
            CursorIcon::Crosshair => "crosshair",
            CursorIcon::Default => "left_ptr",
            CursorIcon::Hand => "hand",
            CursorIcon::Help => "question_arrow",
            CursorIcon::Move => "move",
            CursorIcon::Grab => "grab",
            CursorIcon::Grabbing => "grabbing",
            CursorIcon::Progress => "progress",
            CursorIcon::AllScroll => "all-scroll",
            CursorIcon::ContextMenu => "context-menu",

            CursorIcon::NoDrop => "no-drop",
            CursorIcon::NotAllowed => "crossed_circle",

            // Resize cursors
            CursorIcon::EResize => "right_side",
            CursorIcon::NResize => "top_side",
            CursorIcon::NeResize => "top_right_corner",
            CursorIcon::NwResize => "top_left_corner",
            CursorIcon::SResize => "bottom_side",
            CursorIcon::SeResize => "bottom_right_corner",
            CursorIcon::SwResize => "bottom_left_corner",
            CursorIcon::WResize => "left_side",
            CursorIcon::EwResize => "h_double_arrow",
            CursorIcon::NsResize => "v_double_arrow",
            CursorIcon::NwseResize => "bd_double_arrow",
            CursorIcon::NeswResize => "fd_double_arrow",
            CursorIcon::ColResize => "h_double_arrow",
            CursorIcon::RowResize => "v_double_arrow",

            CursorIcon::Text => "text",
            CursorIcon::VerticalText => "vertical-text",

            CursorIcon::Wait => "watch",

            CursorIcon::ZoomIn => "zoom-in",
            CursorIcon::ZoomOut => "zoom-out",
        };

        for pointer in self.pointers.iter() {
            // Ignore erros, since we don't want to fail hard in case we can't find a proper cursor
            // in a given theme.
            let _ = pointer.set_cursor(cursor, None);
        }
    }

    // This function can only be called from a thread on which `pointer_constraints_proxy` event
    // queue is located, so calling it directly from a Window doesn't work well, in case
    // you've sent your window to another thread, so we need to pass cursor grab updates to
    // the event loop and call this function from there.
    fn grab_pointer(&mut self, surface: Option<&WlSurface>) {
        for locked_pointer in self.locked_pointers.drain(..) {
            locked_pointer.destroy();
        }

        if let Some(surface) = surface {
            for pointer in self.pointers.iter() {
                let locked_pointer = self
                    .pointer_constraints_proxy
                    .try_lock()
                    .unwrap()
                    .as_ref()
                    .and_then(|pointer_constraints| {
                        super::pointer::implement_locked_pointer(
                            surface,
                            &**pointer,
                            pointer_constraints,
                        )
                        .ok()
                    });

                if let Some(locked_pointer) = locked_pointer {
                    self.locked_pointers.push(locked_pointer);
                }
            }
        }
    }
}

pub struct EventLoop<T: 'static> {
    // The loop
    inner_loop: ::calloop::EventLoop<()>,
    // The wayland display
    pub display: Arc<Display>,
    // The output manager
    pub outputs: OutputMgr,
    // Our sink, shared with some handlers, buffering the events
    sink: Arc<Mutex<WindowEventsSink<T>>>,
    pending_user_events: Rc<RefCell<VecDeque<T>>>,
    // The cursor manager
    cursor_manager: Arc<Mutex<CursorManager>>,
    // Utility for grabbing the cursor and changing visibility
    _user_source: ::calloop::Source<::calloop::channel::Channel<T>>,
    user_sender: ::calloop::channel::Sender<T>,
    _kbd_source: ::calloop::Source<::calloop::channel::Channel<crate::event::Event<()>>>,
    window_target: RootELW<T>,
}

// A handle that can be sent across threads and used to wake up the `EventLoop`.
//
// We should only try and wake up the `EventLoop` if it still exists, so we hold Weak ptrs.
pub struct EventLoopProxy<T: 'static> {
    user_sender: calloop::channel::Sender<T>,
}

pub struct EventLoopWindowTarget<T> {
    // The event queue
    pub evq: RefCell<::calloop::Source<EventQueue>>,
    // The window store
    pub store: Arc<Mutex<WindowStore>>,
    // The cursor manager
    pub cursor_manager: Arc<Mutex<CursorManager>>,
    // The env
    pub env: Environment,
    // A cleanup switch to prune dead windows
    pub cleanup_needed: Arc<Mutex<bool>>,
    // The wayland display
    pub display: Arc<Display>,
    // The list of seats
    pub seats: Arc<Mutex<Vec<(u32, wl_seat::WlSeat)>>>,
    _marker: ::std::marker::PhantomData<T>,
}

impl<T: 'static> Clone for EventLoopProxy<T> {
    fn clone(&self) -> Self {
        EventLoopProxy {
            user_sender: self.user_sender.clone(),
        }
    }
}

impl<T: 'static> EventLoopProxy<T> {
    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed<T>> {
        self.user_sender.send(event).map_err(|e| {
            EventLoopClosed(if let ::calloop::channel::SendError::Disconnected(x) = e {
                x
            } else {
                unreachable!()
            })
        })
    }
}

impl<T: 'static> EventLoop<T> {
    pub fn new() -> Result<EventLoop<T>, ConnectError> {
        let (display, mut event_queue) = Display::connect_to_env()?;

        let display = Arc::new(display);
        let sink = Arc::new(Mutex::new(WindowEventsSink::new()));
        let store = Arc::new(Mutex::new(WindowStore::new()));
        let seats = Arc::new(Mutex::new(Vec::new()));

        let inner_loop = ::calloop::EventLoop::new().unwrap();

        let (kbd_sender, kbd_channel) = ::calloop::channel::channel::<crate::event::Event<()>>();
        let kbd_sink = sink.clone();
        let kbd_source = inner_loop
            .handle()
            .insert_source(kbd_channel, move |evt, &mut ()| {
                if let ::calloop::channel::Event::Msg(evt) = evt {
                    let evt = evt.map_nonuser_event().ok().unwrap();
                    kbd_sink.lock().unwrap().send_event(evt);
                }
            })
            .unwrap();

        let pointer_constraints_proxy = Arc::new(Mutex::new(None));

        let mut seat_manager = SeatManager {
            sink: sink.clone(),
            relative_pointer_manager_proxy: Rc::new(RefCell::new(None)),
            pointer_constraints_proxy: pointer_constraints_proxy.clone(),
            store: store.clone(),
            seats: seats.clone(),
            kbd_sender,
            cursor_manager: Arc::new(Mutex::new(CursorManager::new(pointer_constraints_proxy))),
        };

        let cursor_manager = seat_manager.cursor_manager.clone();
        let cursor_manager_clone = cursor_manager.clone();

        let shm_cell = Rc::new(RefCell::new(None));
        let compositor_cell = Rc::new(RefCell::new(None));

        let env = Environment::from_display_with_cb(
            &display,
            &mut event_queue,
            move |event, registry| match event {
                GlobalEvent::New {
                    id,
                    ref interface,
                    version,
                } => {
                    if interface == "zwp_relative_pointer_manager_v1" {
                        let relative_pointer_manager_proxy = registry
                            .bind(version, id, move |pointer_manager| {
                                pointer_manager.implement_closure(|_, _| (), ())
                            })
                            .unwrap();

                        *seat_manager
                            .relative_pointer_manager_proxy
                            .try_borrow_mut()
                            .unwrap() = Some(relative_pointer_manager_proxy);
                    }
                    if interface == "zwp_pointer_constraints_v1" {
                        let pointer_constraints_proxy = registry
                            .bind(version, id, move |pointer_constraints| {
                                pointer_constraints.implement_closure(|_, _| (), ())
                            })
                            .unwrap();

                        *seat_manager.pointer_constraints_proxy.lock().unwrap() =
                            Some(pointer_constraints_proxy);
                    }
                    if interface == "wl_shm" {
                        let shm: WlShm = registry
                            .bind(version, id, move |shm| shm.implement_closure(|_, _| (), ()))
                            .unwrap();

                        (*shm_cell.borrow_mut()) = Some(shm);
                    }
                    if interface == "wl_compositor" {
                        let compositor: WlCompositor = registry
                            .bind(version, id, move |compositor| {
                                compositor.implement_closure(|_, _| (), ())
                            })
                            .unwrap();
                        (*compositor_cell.borrow_mut()) = Some(compositor);
                    }

                    if compositor_cell.borrow().is_some() && shm_cell.borrow().is_some() {
                        let compositor = compositor_cell.borrow_mut().take().unwrap();
                        let shm = shm_cell.borrow_mut().take().unwrap();
                        let auto_themer = AutoThemer::init(None, compositor, &shm);
                        cursor_manager_clone
                            .lock()
                            .unwrap()
                            .set_auto_themer(auto_themer);
                    }

                    if interface == "wl_seat" {
                        seat_manager.add_seat(id, version, registry)
                    }
                }
                GlobalEvent::Removed { id, ref interface } => {
                    if interface == "wl_seat" {
                        seat_manager.remove_seat(id)
                    }
                }
            },
        )
        .unwrap();

        let source = inner_loop
            .handle()
            .insert_source(event_queue, |(), &mut ()| {})
            .unwrap();

        let pending_user_events = Rc::new(RefCell::new(VecDeque::new()));
        let pending_user_events2 = pending_user_events.clone();

        let (user_sender, user_channel) = ::calloop::channel::channel();

        let user_source = inner_loop
            .handle()
            .insert_source(user_channel, move |evt, &mut ()| {
                if let ::calloop::channel::Event::Msg(msg) = evt {
                    pending_user_events2.borrow_mut().push_back(msg);
                }
            })
            .unwrap();

        let cursor_manager_clone = cursor_manager.clone();
        Ok(EventLoop {
            inner_loop,
            sink,
            pending_user_events,
            display: display.clone(),
            outputs: env.outputs.clone(),
            _user_source: user_source,
            user_sender,
            cursor_manager,
            _kbd_source: kbd_source,
            window_target: RootELW {
                p: crate::platform_impl::EventLoopWindowTarget::Wayland(EventLoopWindowTarget {
                    evq: RefCell::new(source),
                    store,
                    env,
                    cursor_manager: cursor_manager_clone,
                    cleanup_needed: Arc::new(Mutex::new(false)),
                    seats,
                    display,
                    _marker: ::std::marker::PhantomData,
                }),
                _marker: ::std::marker::PhantomData,
            },
        })
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy {
            user_sender: self.user_sender.clone(),
        }
    }

    pub fn run<F>(mut self, callback: F) -> !
    where
        F: 'static + FnMut(crate::event::Event<T>, &RootELW<T>, &mut ControlFlow),
    {
        self.run_return(callback);
        std::process::exit(0);
    }

    pub fn run_return<F>(&mut self, mut callback: F)
    where
        F: FnMut(crate::event::Event<T>, &RootELW<T>, &mut ControlFlow),
    {
        // send pending events to the server
        self.display.flush().expect("Wayland connection lost.");

        let mut control_flow = ControlFlow::default();

        let sink = self.sink.clone();
        let user_events = self.pending_user_events.clone();

        callback(
            crate::event::Event::NewEvents(crate::event::StartCause::Init),
            &self.window_target,
            &mut control_flow,
        );

        loop {
            self.post_dispatch_triggers();

            // empty buffer of events
            {
                let mut guard = sink.lock().unwrap();
                guard.empty_with(|evt| {
                    sticky_exit_callback(
                        evt,
                        &self.window_target,
                        &mut control_flow,
                        &mut callback,
                    );
                });
            }
            // empty user events
            {
                let mut guard = user_events.borrow_mut();
                for evt in guard.drain(..) {
                    sticky_exit_callback(
                        crate::event::Event::UserEvent(evt),
                        &self.window_target,
                        &mut control_flow,
                        &mut callback,
                    );
                }
            }
            // do a second run of post-dispatch-triggers, to handle user-generated "request-redraw"
            // in response of resize & friends
            self.post_dispatch_triggers();
            {
                let mut guard = sink.lock().unwrap();
                guard.empty_with(|evt| {
                    sticky_exit_callback(
                        evt,
                        &self.window_target,
                        &mut control_flow,
                        &mut callback,
                    );
                });
            }
            // send Events cleared
            {
                sticky_exit_callback(
                    crate::event::Event::EventsCleared,
                    &self.window_target,
                    &mut control_flow,
                    &mut callback,
                );
            }

            // send pending events to the server
            self.display.flush().expect("Wayland connection lost.");

            // During the run of the user callback, some other code monitoring and reading the
            // wayland socket may have been run (mesa for example does this with vsync), if that
            // is the case, some events may have been enqueued in our event queue.
            //
            // If some messages are there, the event loop needs to behave as if it was instantly
            // woken up by messages arriving from the wayland socket, to avoid getting stuck.
            let instant_wakeup = {
                let window_target = match self.window_target.p {
                    crate::platform_impl::EventLoopWindowTarget::Wayland(ref wt) => wt,
                    _ => unreachable!(),
                };
                let dispatched = window_target
                    .evq
                    .borrow_mut()
                    .dispatch_pending()
                    .expect("Wayland connection lost.");
                dispatched > 0
            };

            match control_flow {
                ControlFlow::Exit => break,
                ControlFlow::Poll => {
                    // non-blocking dispatch
                    self.inner_loop
                        .dispatch(Some(::std::time::Duration::from_millis(0)), &mut ())
                        .unwrap();
                    callback(
                        crate::event::Event::NewEvents(crate::event::StartCause::Poll),
                        &self.window_target,
                        &mut control_flow,
                    );
                }
                ControlFlow::Wait => {
                    let timeout = if instant_wakeup {
                        Some(::std::time::Duration::from_millis(0))
                    } else {
                        None
                    };
                    self.inner_loop.dispatch(timeout, &mut ()).unwrap();
                    callback(
                        crate::event::Event::NewEvents(crate::event::StartCause::WaitCancelled {
                            start: Instant::now(),
                            requested_resume: None,
                        }),
                        &self.window_target,
                        &mut control_flow,
                    );
                }
                ControlFlow::WaitUntil(deadline) => {
                    let start = Instant::now();
                    // compute the blocking duration
                    let duration = if deadline > start && !instant_wakeup {
                        deadline - start
                    } else {
                        ::std::time::Duration::from_millis(0)
                    };
                    self.inner_loop.dispatch(Some(duration), &mut ()).unwrap();
                    let now = Instant::now();
                    if now < deadline {
                        callback(
                            crate::event::Event::NewEvents(
                                crate::event::StartCause::WaitCancelled {
                                    start,
                                    requested_resume: Some(deadline),
                                },
                            ),
                            &self.window_target,
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
                            &self.window_target,
                            &mut control_flow,
                        );
                    }
                }
            }
        }

        callback(
            crate::event::Event::LoopDestroyed,
            &self.window_target,
            &mut control_flow,
        );
    }

    pub fn primary_monitor(&self) -> MonitorHandle {
        primary_monitor(&self.outputs)
    }

    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        available_monitors(&self.outputs)
    }

    pub fn window_target(&self) -> &RootELW<T> {
        &self.window_target
    }
}

impl<T> EventLoopWindowTarget<T> {
    pub fn display(&self) -> &Display {
        &*self.display
    }
}

/*
 * Private EventLoop Internals
 */

impl<T> EventLoop<T> {
    fn post_dispatch_triggers(&mut self) {
        let mut sink = self.sink.lock().unwrap();
        let window_target = match self.window_target.p {
            crate::platform_impl::EventLoopWindowTarget::Wayland(ref wt) => wt,
            _ => unreachable!(),
        };
        // prune possible dead windows
        {
            let mut cleanup_needed = window_target.cleanup_needed.lock().unwrap();
            if *cleanup_needed {
                let pruned = window_target.store.lock().unwrap().cleanup();
                *cleanup_needed = false;
                for wid in pruned {
                    sink.send_window_event(crate::event::WindowEvent::Destroyed, wid);
                }
            }
        }
        // process pending resize/refresh
        window_target.store.lock().unwrap().for_each(|window| {
            if let Some(frame) = window.frame {
                if let Some(newsize) = window.newsize {
                    // Drop resize events equaled to the current size
                    if newsize != *window.size {
                        let (w, h) = newsize;
                        frame.resize(w, h);
                        frame.refresh();
                        let logical_size = crate::dpi::LogicalSize::new(w as f64, h as f64);
                        sink.send_window_event(
                            crate::event::WindowEvent::Resized(logical_size),
                            window.wid,
                        );
                        *window.size = (w, h);
                    } else {
                        // Refresh csd, etc, otherwise
                        frame.refresh();
                    }
                } else if window.frame_refresh {
                    frame.refresh();
                    if !window.refresh {
                        frame.surface().commit()
                    }
                }
            }
            if let Some(dpi) = window.new_dpi {
                sink.send_window_event(
                    crate::event::WindowEvent::HiDpiFactorChanged(dpi as f64),
                    window.wid,
                );
            }
            if window.refresh {
                sink.send_window_event(crate::event::WindowEvent::RedrawRequested, window.wid);
            }
            if window.closed {
                sink.send_window_event(crate::event::WindowEvent::CloseRequested, window.wid);
            }

            if let Some(grab_cursor) = window.grab_cursor {
                let surface = if grab_cursor {
                    Some(window.surface)
                } else {
                    None
                };
                self.cursor_manager.lock().unwrap().grab_pointer(surface);
            }
        })
    }
}

/*
 * Wayland protocol implementations
 */

struct SeatManager<T: 'static> {
    sink: Arc<Mutex<WindowEventsSink<T>>>,
    store: Arc<Mutex<WindowStore>>,
    seats: Arc<Mutex<Vec<(u32, wl_seat::WlSeat)>>>,
    kbd_sender: ::calloop::channel::Sender<crate::event::Event<()>>,
    relative_pointer_manager_proxy: Rc<RefCell<Option<ZwpRelativePointerManagerV1>>>,
    pointer_constraints_proxy: Arc<Mutex<Option<ZwpPointerConstraintsV1>>>,
    cursor_manager: Arc<Mutex<CursorManager>>,
}

impl<T: 'static> SeatManager<T> {
    fn add_seat(&mut self, id: u32, version: u32, registry: wl_registry::WlRegistry) {
        use std::cmp::min;

        let mut seat_data = SeatData {
            sink: self.sink.clone(),
            store: self.store.clone(),
            pointer: None,
            relative_pointer: None,
            relative_pointer_manager_proxy: self.relative_pointer_manager_proxy.clone(),
            keyboard: None,
            touch: None,
            kbd_sender: self.kbd_sender.clone(),
            modifiers_tracker: Arc::new(Mutex::new(ModifiersState::default())),
            cursor_manager: self.cursor_manager.clone(),
        };
        let seat = registry
            .bind(min(version, 5), id, move |seat| {
                seat.implement_closure(move |event, seat| seat_data.receive(event, seat), ())
            })
            .unwrap();
        self.store.lock().unwrap().new_seat(&seat);
        self.seats.lock().unwrap().push((id, seat));
    }

    fn remove_seat(&mut self, id: u32) {
        let mut seats = self.seats.lock().unwrap();
        if let Some(idx) = seats.iter().position(|&(i, _)| i == id) {
            let (_, seat) = seats.swap_remove(idx);
            if seat.as_ref().version() >= 5 {
                seat.release();
            }
        }
    }
}

struct SeatData<T> {
    sink: Arc<Mutex<WindowEventsSink<T>>>,
    store: Arc<Mutex<WindowStore>>,
    kbd_sender: ::calloop::channel::Sender<crate::event::Event<()>>,
    pointer: Option<wl_pointer::WlPointer>,
    relative_pointer: Option<ZwpRelativePointerV1>,
    relative_pointer_manager_proxy: Rc<RefCell<Option<ZwpRelativePointerManagerV1>>>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    touch: Option<wl_touch::WlTouch>,
    modifiers_tracker: Arc<Mutex<ModifiersState>>,
    cursor_manager: Arc<Mutex<CursorManager>>,
}

impl<T: 'static> SeatData<T> {
    fn receive(&mut self, evt: wl_seat::Event, seat: wl_seat::WlSeat) {
        match evt {
            wl_seat::Event::Name { .. } => (),
            wl_seat::Event::Capabilities { capabilities } => {
                // create pointer if applicable
                if capabilities.contains(wl_seat::Capability::Pointer) && self.pointer.is_none() {
                    self.pointer = Some(super::pointer::implement_pointer(
                        &seat,
                        self.sink.clone(),
                        self.store.clone(),
                        self.modifiers_tracker.clone(),
                        self.cursor_manager.clone(),
                    ));

                    self.cursor_manager
                        .lock()
                        .unwrap()
                        .register_pointer(self.pointer.as_ref().unwrap().clone());

                    self.relative_pointer = self
                        .relative_pointer_manager_proxy
                        .try_borrow()
                        .unwrap()
                        .as_ref()
                        .and_then(|manager| {
                            super::pointer::implement_relative_pointer(
                                self.sink.clone(),
                                self.pointer.as_ref().unwrap(),
                                manager,
                            )
                            .ok()
                        })
                }
                // destroy pointer if applicable
                if !capabilities.contains(wl_seat::Capability::Pointer) {
                    if let Some(pointer) = self.pointer.take() {
                        if pointer.as_ref().version() >= 3 {
                            pointer.release();
                        }
                    }
                }
                // create keyboard if applicable
                if capabilities.contains(wl_seat::Capability::Keyboard) && self.keyboard.is_none() {
                    self.keyboard = Some(super::keyboard::init_keyboard(
                        &seat,
                        self.kbd_sender.clone(),
                        self.modifiers_tracker.clone(),
                    ))
                }
                // destroy keyboard if applicable
                if !capabilities.contains(wl_seat::Capability::Keyboard) {
                    if let Some(kbd) = self.keyboard.take() {
                        if kbd.as_ref().version() >= 3 {
                            kbd.release();
                        }
                    }
                }
                // create touch if applicable
                if capabilities.contains(wl_seat::Capability::Touch) && self.touch.is_none() {
                    self.touch = Some(super::touch::implement_touch(
                        &seat,
                        self.sink.clone(),
                        self.store.clone(),
                    ))
                }
                // destroy touch if applicable
                if !capabilities.contains(wl_seat::Capability::Touch) {
                    if let Some(touch) = self.touch.take() {
                        if touch.as_ref().version() >= 3 {
                            touch.release();
                        }
                    }
                }
            }
            _ => unreachable!(),
        }
    }
}

impl<T> Drop for SeatData<T> {
    fn drop(&mut self) {
        if let Some(pointer) = self.pointer.take() {
            if pointer.as_ref().version() >= 3 {
                pointer.release();
            }
        }
        if let Some(kbd) = self.keyboard.take() {
            if kbd.as_ref().version() >= 3 {
                kbd.release();
            }
        }
        if let Some(touch) = self.touch.take() {
            if touch.as_ref().version() >= 3 {
                touch.release();
            }
        }
    }
}

/*
 * Monitor stuff
 */

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VideoMode {
    pub(crate) size: (u32, u32),
    pub(crate) bit_depth: u16,
    pub(crate) refresh_rate: u16,
    pub(crate) monitor: MonitorHandle,
}

impl VideoMode {
    #[inline]
    pub fn size(&self) -> PhysicalSize {
        self.size.into()
    }

    #[inline]
    pub fn bit_depth(&self) -> u16 {
        self.bit_depth
    }

    #[inline]
    pub fn refresh_rate(&self) -> u16 {
        self.refresh_rate
    }

    #[inline]
    pub fn monitor(&self) -> RootMonitorHandle {
        RootMonitorHandle {
            inner: PlatformMonitorHandle::Wayland(self.monitor.clone()),
        }
    }
}

#[derive(Clone)]
pub struct MonitorHandle {
    pub(crate) proxy: wl_output::WlOutput,
    pub(crate) mgr: OutputMgr,
}

impl PartialEq for MonitorHandle {
    fn eq(&self, other: &Self) -> bool {
        self.native_identifier() == other.native_identifier()
    }
}

impl Eq for MonitorHandle {}

impl PartialOrd for MonitorHandle {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(&other))
    }
}

impl Ord for MonitorHandle {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.native_identifier().cmp(&other.native_identifier())
    }
}

impl std::hash::Hash for MonitorHandle {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.native_identifier().hash(state);
    }
}

impl fmt::Debug for MonitorHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[derive(Debug)]
        struct MonitorHandle {
            name: Option<String>,
            native_identifier: u32,
            size: PhysicalSize,
            position: PhysicalPosition,
            hidpi_factor: i32,
        }

        let monitor_id_proxy = MonitorHandle {
            name: self.name(),
            native_identifier: self.native_identifier(),
            size: self.size(),
            position: self.position(),
            hidpi_factor: self.hidpi_factor(),
        };

        monitor_id_proxy.fmt(f)
    }
}

impl MonitorHandle {
    pub fn name(&self) -> Option<String> {
        self.mgr.with_info(&self.proxy, |_, info| {
            format!("{} ({})", info.model, info.make)
        })
    }

    #[inline]
    pub fn native_identifier(&self) -> u32 {
        self.mgr.with_info(&self.proxy, |id, _| id).unwrap_or(0)
    }

    pub fn size(&self) -> PhysicalSize {
        match self.mgr.with_info(&self.proxy, |_, info| {
            info.modes
                .iter()
                .find(|m| m.is_current)
                .map(|m| m.dimensions)
        }) {
            Some(Some((w, h))) => (w as u32, h as u32),
            _ => (0, 0),
        }
        .into()
    }

    pub fn position(&self) -> PhysicalPosition {
        self.mgr
            .with_info(&self.proxy, |_, info| info.location)
            .unwrap_or((0, 0))
            .into()
    }

    #[inline]
    pub fn hidpi_factor(&self) -> i32 {
        self.mgr
            .with_info(&self.proxy, |_, info| info.scale_factor)
            .unwrap_or(1)
    }

    #[inline]
    pub fn video_modes(&self) -> impl Iterator<Item = RootVideoMode> {
        let monitor = self.clone();

        self.mgr
            .with_info(&self.proxy, |_, info| info.modes.clone())
            .unwrap_or(vec![])
            .into_iter()
            .map(move |x| RootVideoMode {
                video_mode: PlatformVideoMode::Wayland(VideoMode {
                    size: (x.dimensions.0 as u32, x.dimensions.1 as u32),
                    refresh_rate: (x.refresh_rate as f32 / 1000.0).round() as u16,
                    bit_depth: 32,
                    monitor: monitor.clone(),
                }),
            })
    }
}

pub fn primary_monitor(outputs: &OutputMgr) -> MonitorHandle {
    outputs.with_all(|list| {
        if let Some(&(_, ref proxy, _)) = list.first() {
            MonitorHandle {
                proxy: proxy.clone(),
                mgr: outputs.clone(),
            }
        } else {
            panic!("No monitor is available.")
        }
    })
}

pub fn available_monitors(outputs: &OutputMgr) -> VecDeque<MonitorHandle> {
    outputs.with_all(|list| {
        list.iter()
            .map(|&(_, ref proxy, _)| MonitorHandle {
                proxy: proxy.clone(),
                mgr: outputs.clone(),
            })
            .collect()
    })
}
