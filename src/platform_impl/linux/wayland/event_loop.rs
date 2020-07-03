use std::{
    cell::RefCell,
    collections::VecDeque,
    fmt,
    io::ErrorKind,
    rc::Rc,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use mio::{Events, Poll, PollOpt, Ready, Token};

use mio_extras::channel::{channel, Receiver, SendError, Sender};

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
    dpi::{LogicalSize, PhysicalPosition, PhysicalSize},
    event::{
        DeviceEvent, DeviceId as RootDeviceId, Event, ModifiersState, StartCause, WindowEvent,
    },
    event_loop::{ControlFlow, EventLoopClosed, EventLoopWindowTarget as RootELW},
    monitor::{MonitorHandle as RootMonitorHandle, VideoMode as RootVideoMode},
    platform_impl::platform::{
        sticky_exit_callback, DeviceId as PlatformDeviceId, MonitorHandle as PlatformMonitorHandle,
        VideoMode as PlatformVideoMode, WindowId as PlatformWindowId,
    },
    window::{CursorIcon, WindowId as RootWindowId},
};

use super::{
    window::{DecorationsAction, WindowStore},
    DeviceId, WindowId,
};

use smithay_client_toolkit::{
    output::OutputMgr,
    reexports::client::{
        protocol::{wl_keyboard, wl_output, wl_pointer, wl_registry, wl_seat, wl_touch},
        ConnectError, Display, EventQueue, GlobalEvent,
    },
    Environment,
};

const KBD_TOKEN: Token = Token(0);
const USER_TOKEN: Token = Token(1);
const EVQ_TOKEN: Token = Token(2);

#[derive(Clone)]
pub struct EventsSink {
    sender: Sender<Event<'static, ()>>,
}

impl EventsSink {
    pub fn new(sender: Sender<Event<'static, ()>>) -> EventsSink {
        EventsSink { sender }
    }

    pub fn send_event(&self, event: Event<'static, ()>) {
        self.sender.send(event).unwrap()
    }

    pub fn send_device_event(&self, event: DeviceEvent, device_id: DeviceId) {
        self.send_event(Event::DeviceEvent {
            event,
            device_id: RootDeviceId(PlatformDeviceId::Wayland(device_id)),
        });
    }

    pub fn send_window_event(&self, event: WindowEvent<'static>, window_id: WindowId) {
        self.send_event(Event::WindowEvent {
            event,
            window_id: RootWindowId(PlatformWindowId::Wayland(window_id)),
        });
    }
}

pub struct CursorManager {
    pointer_constraints_proxy: Arc<Mutex<Option<ZwpPointerConstraintsV1>>>,
    auto_themer: Option<AutoThemer>,
    pointers: Vec<AutoPointer>,
    locked_pointers: Vec<ZwpLockedPointerV1>,
    cursor_visible: bool,
    current_cursor: CursorIcon,
    scale_factor: u32,
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
            scale_factor: 1,
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

    pub fn update_scale_factor(&mut self, scale: u32) {
        self.scale_factor = scale;
        self.reload_cursor_style();
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
            let _ = pointer.set_cursor_with_scale(cursor, self.scale_factor, None);
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
    // Poll instance
    poll: Poll,
    // The wayland display
    pub display: Arc<Display>,
    // The cursor manager
    cursor_manager: Arc<Mutex<CursorManager>>,
    kbd_channel: Receiver<Event<'static, ()>>,
    user_channel: Receiver<T>,
    user_sender: Sender<T>,
    window_target: RootELW<T>,
}

// A handle that can be sent across threads and used to wake up the `EventLoop`.
//
// We should only try and wake up the `EventLoop` if it still exists, so we hold Weak ptrs.
pub struct EventLoopProxy<T: 'static> {
    user_sender: Sender<T>,
}

pub struct EventLoopWindowTarget<T> {
    // the event queue
    pub evq: RefCell<EventQueue>,
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
    // The output manager
    pub outputs: OutputMgr,
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
            EventLoopClosed(if let SendError::Disconnected(x) = e {
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
        let store = Arc::new(Mutex::new(WindowStore::new()));
        let seats = Arc::new(Mutex::new(Vec::new()));

        let poll = Poll::new().unwrap();

        let (kbd_sender, kbd_channel) = channel();

        let sink = EventsSink::new(kbd_sender);

        poll.register(&kbd_channel, KBD_TOKEN, Ready::readable(), PollOpt::level())
            .unwrap();

        let pointer_constraints_proxy = Arc::new(Mutex::new(None));

        let mut seat_manager = SeatManager {
            sink,
            store: store.clone(),
            seats: seats.clone(),
            relative_pointer_manager_proxy: Rc::new(RefCell::new(None)),
            pointer_constraints_proxy: pointer_constraints_proxy.clone(),
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

        poll.register(&event_queue, EVQ_TOKEN, Ready::readable(), PollOpt::level())
            .unwrap();

        let (user_sender, user_channel) = channel();

        poll.register(
            &user_channel,
            USER_TOKEN,
            Ready::readable(),
            PollOpt::level(),
        )
        .unwrap();

        let cursor_manager_clone = cursor_manager.clone();
        let outputs = env.outputs.clone();
        Ok(EventLoop {
            poll,
            display: display.clone(),
            user_sender,
            user_channel,
            kbd_channel,
            cursor_manager,
            window_target: RootELW {
                p: crate::platform_impl::EventLoopWindowTarget::Wayland(EventLoopWindowTarget {
                    evq: RefCell::new(event_queue),
                    store,
                    env,
                    cursor_manager: cursor_manager_clone,
                    cleanup_needed: Arc::new(Mutex::new(false)),
                    seats,
                    display,
                    outputs,
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
        F: 'static + FnMut(Event<'_, T>, &RootELW<T>, &mut ControlFlow),
    {
        self.run_return(callback);
        std::process::exit(0);
    }

    pub fn run_return<F>(&mut self, mut callback: F)
    where
        F: FnMut(Event<'_, T>, &RootELW<T>, &mut ControlFlow),
    {
        // send pending events to the server
        self.display.flush().expect("Wayland connection lost.");

        let mut control_flow = ControlFlow::default();
        let mut events = Events::with_capacity(8);

        callback(
            Event::NewEvents(StartCause::Init),
            &self.window_target,
            &mut control_flow,
        );

        loop {
            // Read events from the event queue
            {
                let mut evq = get_target(&self.window_target).evq.borrow_mut();

                evq.dispatch_pending()
                    .expect("failed to dispatch wayland events");

                if let Some(read) = evq.prepare_read() {
                    if let Err(e) = read.read_events() {
                        if e.kind() != ErrorKind::WouldBlock {
                            panic!("failed to read wayland events: {}", e);
                        }
                    }

                    evq.dispatch_pending()
                        .expect("failed to dispatch wayland events");
                }
            }

            self.post_dispatch_triggers(&mut callback, &mut control_flow);

            while let Ok(event) = self.kbd_channel.try_recv() {
                let event = event.map_nonuser_event().unwrap();
                sticky_exit_callback(event, &self.window_target, &mut control_flow, &mut callback);
            }

            while let Ok(event) = self.user_channel.try_recv() {
                sticky_exit_callback(
                    Event::UserEvent(event),
                    &self.window_target,
                    &mut control_flow,
                    &mut callback,
                );
            }

            // send Events cleared
            {
                sticky_exit_callback(
                    Event::MainEventsCleared,
                    &self.window_target,
                    &mut control_flow,
                    &mut callback,
                );
            }

            // handle request-redraw
            {
                self.redraw_triggers(|wid, window_target| {
                    sticky_exit_callback(
                        Event::RedrawRequested(crate::window::WindowId(
                            crate::platform_impl::WindowId::Wayland(wid),
                        )),
                        window_target,
                        &mut control_flow,
                        &mut callback,
                    );
                });
            }

            // send RedrawEventsCleared
            {
                sticky_exit_callback(
                    Event::RedrawEventsCleared,
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
                    #[cfg(feature = "x11")]
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
                    self.poll
                        .poll(&mut events, Some(Duration::from_millis(0)))
                        .unwrap();
                    events.clear();

                    callback(
                        Event::NewEvents(StartCause::Poll),
                        &self.window_target,
                        &mut control_flow,
                    );
                }
                ControlFlow::Wait => {
                    if !instant_wakeup {
                        self.poll.poll(&mut events, None).unwrap();
                        events.clear();
                    }

                    callback(
                        Event::NewEvents(StartCause::WaitCancelled {
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
                        Duration::from_millis(0)
                    };
                    self.poll.poll(&mut events, Some(duration)).unwrap();
                    events.clear();

                    let now = Instant::now();
                    if now < deadline {
                        callback(
                            Event::NewEvents(StartCause::WaitCancelled {
                                start,
                                requested_resume: Some(deadline),
                            }),
                            &self.window_target,
                            &mut control_flow,
                        );
                    } else {
                        callback(
                            Event::NewEvents(StartCause::ResumeTimeReached {
                                start,
                                requested_resume: deadline,
                            }),
                            &self.window_target,
                            &mut control_flow,
                        );
                    }
                }
            }
        }

        callback(Event::LoopDestroyed, &self.window_target, &mut control_flow);
    }

    pub fn window_target(&self) -> &RootELW<T> {
        &self.window_target
    }
}

impl<T> EventLoopWindowTarget<T> {
    pub fn display(&self) -> &Display {
        &*self.display
    }

    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        available_monitors(&self.outputs)
    }

    pub fn primary_monitor(&self) -> MonitorHandle {
        primary_monitor(&self.outputs)
    }
}

/*
 * Private EventLoop Internals
 */

impl<T> EventLoop<T> {
    fn redraw_triggers<F>(&mut self, mut callback: F)
    where
        F: FnMut(WindowId, &RootELW<T>),
    {
        let window_target = match self.window_target.p {
            crate::platform_impl::EventLoopWindowTarget::Wayland(ref wt) => wt,
            #[cfg(feature = "x11")]
            _ => unreachable!(),
        };
        window_target.store.lock().unwrap().for_each_redraw_trigger(
            |refresh, frame_refresh, wid, frame| {
                if let Some(frame) = frame {
                    let mut frame = frame.lock().unwrap();

                    if frame_refresh {
                        frame.refresh();
                        if !refresh {
                            frame.surface().commit()
                        }
                    }
                }
                if refresh {
                    callback(wid, &self.window_target);
                }
            },
        )
    }

    fn post_dispatch_triggers<F>(&mut self, mut callback: F, control_flow: &mut ControlFlow)
    where
        F: FnMut(Event<'_, T>, &RootELW<T>, &mut ControlFlow),
    {
        let window_target = match self.window_target.p {
            crate::platform_impl::EventLoopWindowTarget::Wayland(ref wt) => wt,
            #[cfg(feature = "x11")]
            _ => unreachable!(),
        };

        let mut callback = |event: Event<'_, T>| {
            sticky_exit_callback(event, &self.window_target, control_flow, &mut callback);
        };

        // prune possible dead windows
        {
            let mut cleanup_needed = window_target.cleanup_needed.lock().unwrap();
            if *cleanup_needed {
                let pruned = window_target.store.lock().unwrap().cleanup();
                *cleanup_needed = false;
                for wid in pruned {
                    callback(Event::WindowEvent {
                        window_id: crate::window::WindowId(
                            crate::platform_impl::WindowId::Wayland(wid),
                        ),
                        event: WindowEvent::Destroyed,
                    });
                }
            }
        }
        // process pending resize/refresh
        window_target.store.lock().unwrap().for_each(|window| {
            let window_id =
                crate::window::WindowId(crate::platform_impl::WindowId::Wayland(window.wid));

            // Update window logical .size field (for callbacks using .inner_size)
            let (old_logical_size, mut logical_size) = {
                let mut window_size = window.size.lock().unwrap();
                let old_logical_size = *window_size;
                *window_size = window.new_size.unwrap_or(old_logical_size);
                (old_logical_size, *window_size)
            };

            if let Some(scale_factor) = window.new_scale_factor {
                // Update cursor scale factor
                self.cursor_manager
                    .lock()
                    .unwrap()
                    .update_scale_factor(scale_factor as u32);
                let new_logical_size = {
                    let scale_factor = scale_factor as f64;
                    let mut physical_size =
                        LogicalSize::<f64>::from(logical_size).to_physical(scale_factor);
                    callback(Event::WindowEvent {
                        window_id,
                        event: WindowEvent::ScaleFactorChanged {
                            scale_factor,
                            new_inner_size: &mut physical_size,
                        },
                    });
                    physical_size.to_logical::<u32>(scale_factor).into()
                };
                // Update size if changed by callback
                if new_logical_size != logical_size {
                    logical_size = new_logical_size;
                    *window.size.lock().unwrap() = logical_size.into();
                }
            }

            if window.new_size.is_some() || window.new_scale_factor.is_some() {
                if let Some(frame) = window.frame {
                    let mut frame = frame.lock().unwrap();
                    // Update decorations state
                    match window.decorations_action {
                        Some(DecorationsAction::Hide) => frame.set_decorate(false),
                        Some(DecorationsAction::Show) => frame.set_decorate(true),
                        None => (),
                    }

                    // mutter (GNOME Wayland) relies on `set_geometry` to reposition window in case
                    // it overlaps mutter's `bounding box`, so we can't avoid this resize call,
                    // which calls `set_geometry` under the hood, for now.
                    let (w, h) = logical_size;
                    frame.resize(w, h);
                    frame.refresh();
                }
                // Don't send resize event downstream if the new logical size and scale is identical to the
                // current one
                if logical_size != old_logical_size || window.new_scale_factor.is_some() {
                    let physical_size = LogicalSize::<f64>::from(logical_size).to_physical(
                        window.new_scale_factor.unwrap_or(window.prev_scale_factor) as f64,
                    );
                    callback(Event::WindowEvent {
                        window_id,
                        event: WindowEvent::Resized(physical_size),
                    });
                }
            }

            if window.closed {
                callback(Event::WindowEvent {
                    window_id,
                    event: WindowEvent::CloseRequested,
                });
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

fn get_target<T>(target: &RootELW<T>) -> &EventLoopWindowTarget<T> {
    match target.p {
        crate::platform_impl::EventLoopWindowTarget::Wayland(ref wt) => wt,
        #[cfg(feature = "x11")]
        _ => unreachable!(),
    }
}

/*
 * Wayland protocol implementations
 */

struct SeatManager {
    sink: EventsSink,
    store: Arc<Mutex<WindowStore>>,
    seats: Arc<Mutex<Vec<(u32, wl_seat::WlSeat)>>>,
    relative_pointer_manager_proxy: Rc<RefCell<Option<ZwpRelativePointerManagerV1>>>,
    pointer_constraints_proxy: Arc<Mutex<Option<ZwpPointerConstraintsV1>>>,
    cursor_manager: Arc<Mutex<CursorManager>>,
}

impl SeatManager {
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

struct SeatData {
    sink: EventsSink,
    store: Arc<Mutex<WindowStore>>,
    pointer: Option<wl_pointer::WlPointer>,
    relative_pointer: Option<ZwpRelativePointerV1>,
    relative_pointer_manager_proxy: Rc<RefCell<Option<ZwpRelativePointerManagerV1>>>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    touch: Option<wl_touch::WlTouch>,
    modifiers_tracker: Arc<Mutex<ModifiersState>>,
    cursor_manager: Arc<Mutex<CursorManager>>,
}

impl SeatData {
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
                        self.sink.clone(),
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

impl Drop for SeatData {
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
    pub fn size(&self) -> PhysicalSize<u32> {
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
            size: PhysicalSize<u32>,
            position: PhysicalPosition<i32>,
            scale_factor: i32,
        }

        let monitor_id_proxy = MonitorHandle {
            name: self.name(),
            native_identifier: self.native_identifier(),
            size: self.size(),
            position: self.position(),
            scale_factor: self.scale_factor(),
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

    pub fn size(&self) -> PhysicalSize<u32> {
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

    pub fn position(&self) -> PhysicalPosition<i32> {
        self.mgr
            .with_info(&self.proxy, |_, info| info.location)
            .unwrap_or((0, 0))
            .into()
    }

    #[inline]
    pub fn scale_factor(&self) -> i32 {
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
