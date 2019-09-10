use std::{
    cell::RefCell,
    collections::VecDeque,
    fmt,
    io::ErrorKind,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use mio::{Events, Poll, PollOpt, Ready, Token};

use mio_extras::channel::{channel, Receiver, Sender};

use crate::{
    dpi::{LogicalSize, PhysicalPosition, PhysicalSize},
    event::{Event, ModifiersState, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoopClosed, EventLoopWindowTarget as RootELW},
    monitor::VideoMode,
    platform_impl::platform::sticky_exit_callback,
};

use super::{window::WindowStore, WindowId};

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
pub struct WindowEventsSink {
    sender: Sender<(WindowEvent<'static>, crate::window::WindowId)>,
}

impl WindowEventsSink {
    pub fn new(
        sender: Sender<(WindowEvent<'static>, crate::window::WindowId)>,
    ) -> WindowEventsSink {
        WindowEventsSink { sender }
    }

    pub fn send_event(&self, evt: WindowEvent<'static>, wid: WindowId) {
        self.sender
            .send((
                evt,
                crate::window::WindowId(crate::platform_impl::WindowId::Wayland(wid)),
            ))
            .unwrap();
    }
}

pub struct EventLoop<T: 'static> {
    // Poll instance
    poll: Poll,
    // The wayland display
    pub display: Arc<Display>,
    // the output manager
    pub outputs: OutputMgr,
    kbd_channel: Receiver<(WindowEvent<'static>, crate::window::WindowId)>,
    user_channel: Receiver<T>,
    user_sender: Sender<T>,
    window_target: RootELW<T>,
}

// A handle that can be sent across threads and used to wake up the `EventLoop`.
//
// We should only try and wake up the `EventLoop` if it still exists, so we hold Weak ptrs.
#[derive(Clone)]
pub struct EventLoopProxy<T: 'static> {
    user_sender: Sender<T>,
}

pub struct EventLoopWindowTarget<T> {
    // the event queue
    pub evq: RefCell<EventQueue>,
    // The window store
    pub store: Arc<Mutex<WindowStore>>,
    // the env
    pub env: Environment,
    // a cleanup switch to prune dead windows
    pub cleanup_needed: Arc<Mutex<bool>>,
    // The wayland display
    pub display: Arc<Display>,
    // The list of seats
    pub seats: Arc<Mutex<Vec<(u32, wl_seat::WlSeat)>>>,
    _marker: ::std::marker::PhantomData<T>,
}

impl<T: 'static> EventLoopProxy<T> {
    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed> {
        self.user_sender.send(event).map_err(|_| EventLoopClosed)
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

        let sink = WindowEventsSink::new(kbd_sender);

        poll.register(&kbd_channel, KBD_TOKEN, Ready::readable(), PollOpt::level())
            .unwrap();

        let mut seat_manager = SeatManager {
            sink,
            store: store.clone(),
            seats: seats.clone(),
        };

        let env = Environment::from_display_with_cb(
            &display,
            &mut event_queue,
            move |event, registry| match event {
                GlobalEvent::New {
                    id,
                    ref interface,
                    version,
                } => {
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

        Ok(EventLoop {
            poll,
            display: display.clone(),
            outputs: env.outputs.clone(),
            user_sender,
            user_channel,
            kbd_channel,
            window_target: RootELW {
                p: crate::platform_impl::EventLoopWindowTarget::Wayland(EventLoopWindowTarget {
                    evq: RefCell::new(event_queue),
                    store,
                    env,
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
        F: 'static + FnMut(Event<'_, T>, &RootELW<T>, &mut ControlFlow),
    {
        self.run_return(callback);
        ::std::process::exit(0);
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

            while let Ok((event, window_id)) = self.kbd_channel.try_recv() {
                sticky_exit_callback(
                    Event::WindowEvent { event, window_id },
                    &self.window_target,
                    &mut control_flow,
                    &mut callback,
                );
            }

            while let Ok(event) = self.user_channel.try_recv() {
                sticky_exit_callback(
                    Event::UserEvent(event),
                    &self.window_target,
                    &mut control_flow,
                    &mut callback,
                );
            }

            // do a second run of post-dispatch-triggers, to handle user-generated "request-redraw"
            // in response of resize & friends
            self.post_dispatch_triggers(&mut callback, &mut control_flow);

            // send Events cleared
            {
                sticky_exit_callback(
                    Event::EventsCleared,
                    &self.window_target,
                    &mut control_flow,
                    &mut callback,
                );
            }

            // send pending events to the server
            self.display.flush().expect("Wayland connection lost.");

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
                    self.poll.poll(&mut events, None).unwrap();
                    events.clear();

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
                    let duration = if deadline > start {
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

    pub fn primary_monitor(&self) -> MonitorHandle {
        primary_monitor(&self.outputs)
    }

    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        available_monitors(&self.outputs)
    }

    pub fn display(&self) -> &Display {
        &*self.display
    }

    pub fn window_target(&self) -> &RootELW<T> {
        &self.window_target
    }
}

/*
 * Private EventLoop Internals
 */

impl<T> EventLoop<T> {
    fn post_dispatch_triggers<F>(&mut self, mut callback: F, control_flow: &mut ControlFlow)
    where
        F: FnMut(Event<'_, T>, &RootELW<T>, &mut ControlFlow),
    {
        let window_target = get_target(&self.window_target);

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
        window_target.store.lock().unwrap().for_each(
            |newsize, size, prev_dpi, new_dpi, refresh, frame_refresh, closed, wid, frame| {
                let window_id =
                    crate::window::WindowId(crate::platform_impl::WindowId::Wayland(wid));
                if let Some(frame) = frame {
                    if let Some((w, h)) = newsize {
                        frame.resize(w, h);
                        frame.refresh();
                        let logical_size = crate::dpi::LogicalSize::new(w as f64, h as f64);
                        let physical_size =
                            logical_size.to_physical(new_dpi.unwrap_or(prev_dpi) as f64);

                        callback(Event::WindowEvent {
                            window_id: crate::window::WindowId(
                                crate::platform_impl::WindowId::Wayland(wid),
                            ),
                            event: WindowEvent::Resized(physical_size),
                        });
                        *size = (w, h);
                    } else if frame_refresh {
                        frame.refresh();
                        if !refresh {
                            frame.surface().commit()
                        }
                    }

                    if let Some(dpi) = new_dpi {
                        let dpi = dpi as f64;
                        let logical_size = LogicalSize::from(*size);
                        let mut new_inner_size = Some(logical_size.to_physical(dpi));

                        callback(Event::WindowEvent {
                            window_id,
                            event: WindowEvent::HiDpiFactorChanged {
                                hidpi_factor: dpi,
                                new_inner_size: &mut new_inner_size,
                            },
                        });

                        if let Some(new_size) = new_inner_size {
                            let (w, h) = new_size.to_logical(dpi).into();
                            frame.resize(w, h);
                            *size = (w, h);
                        }
                    }
                }
                if refresh {
                    callback(Event::WindowEvent {
                        window_id,
                        event: WindowEvent::RedrawRequested,
                    });
                }
                if closed {
                    callback(Event::WindowEvent {
                        window_id,
                        event: WindowEvent::CloseRequested,
                    });
                }
            },
        )
    }
}

fn get_target<T>(target: &RootELW<T>) -> &EventLoopWindowTarget<T> {
    match target.p {
        crate::platform_impl::EventLoopWindowTarget::Wayland(ref wt) => wt,
        _ => unreachable!(),
    }
}

/*
 * Wayland protocol implementations
 */

struct SeatManager {
    sink: WindowEventsSink,
    store: Arc<Mutex<WindowStore>>,
    seats: Arc<Mutex<Vec<(u32, wl_seat::WlSeat)>>>,
}

impl SeatManager {
    fn add_seat(&mut self, id: u32, version: u32, registry: wl_registry::WlRegistry) {
        use std::cmp::min;

        let mut seat_data = SeatData {
            sink: self.sink.clone(),
            store: self.store.clone(),
            pointer: None,
            keyboard: None,
            touch: None,
            modifiers_tracker: Arc::new(Mutex::new(ModifiersState::default())),
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
    sink: WindowEventsSink,
    store: Arc<Mutex<WindowStore>>,
    pointer: Option<wl_pointer::WlPointer>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    touch: Option<wl_touch::WlTouch>,
    modifiers_tracker: Arc<Mutex<ModifiersState>>,
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
                    ))
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

pub struct MonitorHandle {
    pub(crate) proxy: wl_output::WlOutput,
    pub(crate) mgr: OutputMgr,
}

impl Clone for MonitorHandle {
    fn clone(&self) -> MonitorHandle {
        MonitorHandle {
            proxy: self.proxy.clone(),
            mgr: self.mgr.clone(),
        }
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
    pub fn video_modes(&self) -> impl Iterator<Item = VideoMode> {
        self.mgr
            .with_info(&self.proxy, |_, info| info.modes.clone())
            .unwrap_or(vec![])
            .into_iter()
            .map(|x| VideoMode {
                size: (x.dimensions.0 as u32, x.dimensions.1 as u32),
                refresh_rate: (x.refresh_rate as f32 / 1000.0).round() as u16,
                bit_depth: 32,
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
