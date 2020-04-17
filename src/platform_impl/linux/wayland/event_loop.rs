use std::{collections::VecDeque, fmt, sync::{Arc, Mutex}, time::Instant};
use smithay_client_toolkit::{
    reexports::calloop::{self, Source, channel::{channel as unbounded, Sender, Channel as Receiver}},
    environment::{Environment, SimpleGlobal},
    default_environment,
    reexports::{
        client::{ConnectError, Display, Main, protocol::{wl_output, wl_surface::WlSurface}},
        protocols::unstable::{
            pointer_constraints::v1::client::{
                zwp_pointer_constraints_v1::{ZwpPointerConstraintsV1 as PointerConstraints, Lifetime},
                zwp_locked_pointer_v1::ZwpLockedPointerV1 as LockedPointer
            },
            relative_pointer::v1::client::{
                zwp_relative_pointer_manager_v1::ZwpRelativePointerManagerV1 as RelativePointerManager,
                zwp_relative_pointer_v1 as relative_pointer
            }
        },
    },
    output::with_output_info,
    seat::pointer::ThemedPointer,
    window::{self as sctk, ConceptFrame},
    get_surface_scale_factor
};
use crate::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{DeviceEvent, StartCause, Event},
    event_loop::{ControlFlow, EventLoopClosed},
    platform_impl::platform,
};
use super::window;

pub trait Sink<T> = FnMut(crate::event::Event<T>, &crate::event_loop::EventLoopWindowTarget<T>, &mut crate::event_loop::ControlFlow);

// Application state update
struct Update<'t, T:'static> {
    sink: &'t mut dyn Sink<T>,
}

default_environment!{Env, desktop,
    fields = [
        relative_pointer_manager: SimpleGlobal<RelativePointerManager>,
        pointer_constraints: SimpleGlobal<PointerConstraints>
    ],
    singles = [
        RelativePointerManager => relative_pointer_manager,
        PointerConstraints => pointer_constraints
    ]
}

pub struct Window {
    pub surface: WlSurface,
    pub size: (u32,u32), // Detect identity set size
    pub scale_factor: u32,
    pub states: Vec<sctk::State>,
    pub current_cursor: &'static str,
    pub locked_pointers: Vec<Main<LockedPointer>>,
}

impl PartialEq<WlSurface> for Window { fn eq(&self, surface: &WlSurface) -> bool { self.surface == *surface } }
impl PartialEq for Window { fn eq(&self, other: &Self) -> bool { *self == other.surface } }

pub struct Context<T: 'static> {
    pub display: Display,
    pub env: Environment<Env>,
    // Split from Window (!Send,!Sync)
    pub sctk_windows: Mutex<Vec<sctk::Window<ConceptFrame>>>, // RefCell ?
    // Arc<Mutex> shared with all window::WindowHandle so size changes in Event::Configure are reflected back on the handle state (WindowHandle::inner_size)
    pub windows: Arc<Mutex<Vec<Window>>>,
    pub command: (Sender<window::Command>, Source<Receiver<window::Command>>),
    _marker: std::marker::PhantomData<T>
}
pub type EventLoopWindowTarget<T> = &'static Context<T>; // Erase lifetime to 'static because winit::EventLoopWindowTarget is missing <'lifetime>
//impl<T> EventLoopWindowTarget<T> { pub fn display(&self) -> &Display { &*self.display } } // EventLoopWindowTargetExtUnix

// required by linux/mod.rs for crate::EventLoop::Deref
pub fn window_target<T>(event_loop_window_target: &Context<T>) -> crate::event_loop::EventLoopWindowTarget<T> {
    crate::event_loop::EventLoopWindowTarget{
        p: crate::platform_impl::EventLoopWindowTarget::Wayland(
            //unsafe{std::mem::transmute::<&Context<T>, &'static Context<T>>(&event_loop_window_target)}
            unsafe{&*(&event_loop_window_target as *const &Context<T> as *const Context<T>)}
            /*'EventLoopWindowTarget:'EventLoop*/
        ),
        _marker: Default::default()
    }
}

fn send<T>(sink: &mut dyn Sink<T>, context: &Context<T>, control_flow: &mut ControlFlow, event: crate::event::Event<T>) {
    let mut exit = ControlFlow::Exit;
    sink(event, &window_target(&context), if *control_flow == ControlFlow::Exit { &mut exit } else { control_flow })
}

impl<T> Update<'_, T> {
    fn send(&mut self, context: &Context<T>, control_flow: &mut ControlFlow, event: crate::event::Event<T>) {
        send(self.sink, context, control_flow, event);
    }
}

/// Mutable state, time shared by handlers on main thread
pub struct State<T: 'static> {
    pub context: Context<T>,
    keyboard: super::keyboard::Keyboard,
    pointers: Vec<ThemedPointer>, // Window::set_pointer
    control_flow: ControlFlow,
    redraw_events: Vec<Event<'static, T>>,
}

pub(crate) struct DispatchData<'t, T:'static> {
    update: Update<'t, T>,
    pub state: &'t mut State<T>,
}

// wayland-client requires DispatchData:Any:'static (i.e erases lifetimes)
unsafe fn erase_lifetime<'t,T:'static>(data: DispatchData<'t,T>) -> DispatchData<'static,T> {
    std::mem::transmute::<DispatchData::<'t,T>, DispatchData::<'static,T>>(data)
}
/*// todo: actualy restore lifetimes, not just allow whatever
unsafe fn restore_erased_lifetime<'t,T:'static>(data: &mut DispatchData::<'static,T>) -> &'t mut DispatchData::<'t,T> {
    std::mem::transmute::<&mut DispatchData::<'static,T>, &mut DispatchData::<'t,T>>(data)
}*/

/*fn send<T>(sink: &dyn Sink<T>, state: &mut State<T>, event: crate::event::Event<T>) {
    sink(event, &window_target(&state), if state.control_flow == ControlFlow::Exit { &mut ControlFlow::Exit } else { &mut state.control_flow })
}*/

impl<T> DispatchData<'_, T> {
    pub fn send(&mut self, event: crate::event::Event<T>) { send(&mut self.update.sink, &self.state.context, &mut self.state.control_flow, event) }
}

pub struct EventLoop<T: 'static> {
    event_loop: calloop::EventLoop<DispatchData<'static,T>>,
    user: (Sender<T>, Source<Receiver<T>>), // User messages (EventProxy)
    state: State<T>,
    window_target: Option<crate::event_loop::EventLoopWindowTarget<T>>, // crate::EventLoop::Deref -> &EventLoopWindowTarget
}

impl<T> EventLoop<T> {
    pub fn new() -> Result<Self, ConnectError> {
        let (env, display, queue) = smithay_client_toolkit::init_default_environment!(
            Env,
            desktop,
            fields = [
                relative_pointer_manager: SimpleGlobal::new(),
                pointer_constraints: SimpleGlobal::new()
            ]
        )?;

        let event_loop = calloop::EventLoop/*::<DispatchData<T>>*/::new().unwrap();
        smithay_client_toolkit::WaylandSource::new(queue).quick_insert(event_loop.handle()).unwrap();

        let user = { // Push user messages
            let (sender, receiver) = unbounded::<T>();
            (
                sender,
                event_loop.handle().insert_source(receiver, |event, _, sink:&mut DispatchData<T>| { // calloop::sources::channel::Event<_> ? should be T
                    if let calloop::channel::Event::Msg(item) = event { sink.send(crate::event::Event::UserEvent(item)); }
                }).unwrap()
            )
        };

        let command = { // Window handle command sender
            let (sender, receiver) = unbounded::<window::Command>();
            (sender,
            event_loop.handle().insert_source(receiver, |calloop_channel_event, _, data| { // fixme: use a standard futures::channel
                if let calloop::channel::Event::Msg(command) = calloop_channel_event {
                    let event = {
                        let DispatchData{state:State{context:Context{env, windows, sctk_windows,..},redraw_events, pointers,..},..} = data;
                        let mut windows = windows.lock().unwrap();
                        let window::Command{surface,set} = command;
                        let windows_index = windows.iter().position(|w| w==&surface).unwrap();
                        let window = &mut windows[windows_index];
                        let sctk_windows_index = sctk_windows.lock().unwrap().iter().position(|w| w.surface()==&surface).unwrap();
                        let sctk_window = &mut sctk_windows.lock().unwrap()[sctk_windows_index];
                        use window::Set::*;
                        if let Drop = set {
                            surface.destroy();
                            windows.remove(windows_index);
                            sctk_windows.lock().unwrap().remove(sctk_windows_index);
                            Some(window::event(crate::event::WindowEvent::Destroyed, &surface))
                        } else {
                            match set {
                                Drop => unreachable!(),
                                Size(size) => {
                                    if window.size != size || window.scale_factor != get_surface_scale_factor(&surface) as u32 {
                                        window.size = size;
                                        window.scale_factor = get_surface_scale_factor(&surface) as u32;
                                        redraw_events.push(Event::RedrawRequested(window::id(&surface)));
                                    }
                                    {let (w, h) = size; sctk_window.resize(w, h);}
                                    sctk_window.refresh();
                                }
                                RequestRedraw => redraw_events.push(Event::RedrawRequested(window::id(&surface))),
                                Minimized => sctk_window.set_minimized(),
                                Maximized(true) => sctk_window.set_maximized(),
                                Maximized(false) => sctk_window.unset_maximized(),
                                Fullscreen(Some(output)) => sctk_window.set_fullscreen(Some(&output)),
                                Fullscreen(None) => sctk_window.unset_fullscreen(),
                                MinSize(size) => sctk_window.set_min_size(size),
                                MaxSize(size) => sctk_window.set_max_size(size),
                                Title(title) => sctk_window.set_title(title),
                                Cursor(cursor) => { window.current_cursor = cursor; for pointer in pointers { pointer.set_cursor(window.current_cursor, None).unwrap(); } },
                                Decorate(true) => sctk_window.set_decorate(sctk::Decorations::FollowServer),
                                Decorate(false) => sctk_window.set_decorate(sctk::Decorations::None),
                                Resizable(resizable) => sctk_window.set_resizable(resizable),
                                CursorGrab(true) => {
                                    if let Some(pointer_constraints) = env.get_global::<PointerConstraints>() {
                                        window.locked_pointers = pointers.iter().map(
                                            |pointer| pointer_constraints.lock_pointer(&surface, pointer, None, Lifetime::Persistent.to_raw())
                                        ).collect();
                                    }
                                }
                                CursorGrab(false) => { window.locked_pointers.clear(); }
                            };
                            None
                        }
                    };
                    if let Some(event) = event { data.send(event); }
                }
            }).unwrap()
            )
        };

        let mut self_ = Self{
            event_loop,
            user,
            state: State{
                context: Context{
                    display, env,
                    sctk_windows: Default::default(),
                    windows: Default::default(),
                    command,
                    _marker: Default::default(),
                },
                keyboard: Default::default(),
                pointers: Default::default(),
                control_flow: ControlFlow::Wait,
                redraw_events: Default::default(),
            },
            window_target: None,
        };
        self_.window_target = Some(window_target(&self_.state.context)); // Workaround self-reference (could use rental instead)
        Ok(self_)
    }

    // required by linux/mod.rs for crate::EventLoop::Deref
    pub fn window_target(&self) -> &crate::event_loop::EventLoopWindowTarget<T> { self.window_target.as_ref().unwrap() }
}

pub struct EventLoopProxy<T>(Sender<T>);
impl<T> Clone for EventLoopProxy<T> { fn clone(&self) -> Self { Self(self.0.clone()) } }
impl<T: 'static> EventLoopProxy<T> {
    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed<T>> { self.0.send(event).map_err(|std::sync::mpsc::SendError(e)| EventLoopClosed(e)) }
}

impl<T> EventLoop<T> {
    pub fn create_proxy(&self) -> EventLoopProxy<T> { EventLoopProxy(self.user.0.clone()) }

    pub fn run<S:Sink<T>>(mut self, sink: S) -> ! {
        self.run_return(sink);
        std::process::exit(0);
    }

    pub fn run_return<S:Sink<T>>(&mut self, mut sink: S) {
        let Self{event_loop, state, ..} = self;

        let _seat_handler = { // for a simple setup
            let (loop_handle, env) = (event_loop.handle(), &state.context.env);

            use smithay_client_toolkit::seat::{
                pointer::{ThemeManager, ThemeSpec},
                keyboard::{map_keyboard_repeat, RepeatKind},
            };

            let theme_manager = ThemeManager::init(
                ThemeSpec::System,
                env.require_global(),
                env.require_global(),
            );

            let relative_pointer_manager = env.get_global::<RelativePointerManager>();

            env.listen_for_seats(move |seat, seat_data, mut data| {
                let DispatchData::<T>{state:State{pointers,..}, ..} = data.get().unwrap();
                if seat_data.has_pointer {
                    let pointer = theme_manager.theme_pointer_with_impl(&seat,
                        {
                            let mut pointer = super::pointer::Pointer::default(); // Track focus and reconstruct scroll events
                            move/*pointer*/ |event, themed_pointer, mut data| {
                                let DispatchData::<T>{update, state:State{context,mut control_flow,..},..} = data.get().unwrap();
                                pointer.handle(themed_pointer, |e,s| update.send(&context, &mut control_flow, super::window::event(e, s)), &context.windows.lock().unwrap(), event);
                            }
                        }
                    );

                    if let Some(manager) = &relative_pointer_manager {
                        use relative_pointer::Event::*;
                        manager.get_relative_pointer(&pointer).quick_assign(move |_, event, mut data| match event {
                            RelativeMotion { dx, dy, .. } => {
                                let data = data.get::<DispatchData<T>>().unwrap();
                                let device_id = crate::event::DeviceId(super::super::DeviceId::Wayland(super::DeviceId));
                                data.send(crate::event::Event::DeviceEvent{event: DeviceEvent::MouseMotion { delta: (dx, dy) }, device_id});
                            }
                            _ => unreachable!(),
                        });
                    }

                    pointers.push(pointer);
                }

                if seat_data.has_keyboard {
                    let _ = map_keyboard_repeat(loop_handle.clone(), &seat, None, RepeatKind::System,
                        |event, _, mut data| {
                            let DispatchData::<T>{update, state:State{context,keyboard,control_flow,..},..} = data.get().unwrap();
                            keyboard.handle(|e,s| update.send(context, control_flow, super::window::event(e, s)), event, false);
                        }
                    ).unwrap();
                }

                if seat_data.has_touch {
                    seat.get_touch().quick_assign({
                        let mut touch = super::touch::Touch::default(); // Track touch points
                        move |_, event, mut data| {
                            let data = data.get::<DispatchData<T>>().unwrap();
                            touch.handle(|e,s| data.send(super::window::event(e, s)), event);
                        }
                    });
                }
            });
        };

        send(&mut sink, &state.context, &mut state.control_flow, crate::event::Event::NewEvents(StartCause::Init));
        loop {
            match state.control_flow {
                ControlFlow::Exit => break,
                ControlFlow::Poll => {
                    event_loop.dispatch(std::time::Duration::new(0,0), &mut unsafe{erase_lifetime(DispatchData{update: Update{sink: &mut sink}, state})}).unwrap();
                    send(&mut sink, &state.context, &mut state.control_flow, Event::NewEvents(StartCause::Poll));
                }
                ControlFlow::Wait => {
                    event_loop.dispatch(None, &mut unsafe{erase_lifetime(DispatchData{update: Update{sink: &mut sink}, state})}).unwrap();
                    send(&mut sink, &state.context, &mut state.control_flow, Event::NewEvents(StartCause::WaitCancelled{start: Instant::now(), requested_resume: None}));
                }
                ControlFlow::WaitUntil(deadline) => {
                    let start = Instant::now();
                    let duration = deadline.saturating_duration_since(start);
                    event_loop.dispatch(Some(duration), &mut unsafe{erase_lifetime(DispatchData{update: Update{sink: &mut sink}, state})}).unwrap();

                    let now = Instant::now();
                    if now < deadline {
                        send(&mut sink, &state.context, &mut state.control_flow, Event::NewEvents(StartCause::WaitCancelled{start, requested_resume: Some(deadline)}));
                    } else {
                        send(&mut sink, &state.context, &mut state.control_flow, Event::NewEvents(StartCause::ResumeTimeReached{start, requested_resume: deadline}));
                    }
                }
            }
            send(&mut sink, &state.context, &mut state.control_flow, Event::MainEventsCleared);
            // sink.send_all(state.redraw_events.drain(..));
            for event in state.redraw_events.drain(..) { send(&mut sink, &state.context, &mut state.control_flow, event); }
            send(&mut sink, &state.context, &mut state.control_flow, Event::RedrawEventsCleared);
        }
        //drop(seat_handler);
        send(&mut sink, &state.context, &mut state.control_flow, Event::LoopDestroyed);
    }

    pub fn primary_monitor(&self) -> MonitorHandle {
        primary_monitor(&self.state.context.env)
    }

    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        available_monitors(&self.state.context.env)
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
    pub fn monitor(&self) -> crate::monitor::MonitorHandle {
        crate::monitor::MonitorHandle {
            inner: platform::MonitorHandle::Wayland(self.monitor.clone()),
        }
    }
}

#[derive(Clone)]
pub struct MonitorHandle(pub(crate) wl_output::WlOutput);

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
        with_output_info(&self.0, |info| format!("{} ({})", info.model, info.make))
    }

    #[inline]
    pub fn native_identifier(&self) -> u32 {
        with_output_info(&self.0, |info| info.id).unwrap_or(0)
    }

    pub fn size(&self) -> PhysicalSize<u32> {
        match with_output_info(&self.0, |info| {
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
        with_output_info(&self.0, |info| info.location)
            .unwrap_or((0, 0))
            .into()
    }

    #[inline]
    pub fn scale_factor(&self) -> i32 {
        with_output_info(&self.0, |info| info.scale_factor).unwrap_or(1)
    }

    #[inline]
    pub fn video_modes(&self) -> impl Iterator<Item = crate::monitor::VideoMode> {
        let monitor = self.clone();

        with_output_info(&self.0, |info| info.modes.clone())
            .unwrap_or_default()
            .into_iter()
            .map(move |x| crate::monitor::VideoMode {
                video_mode: platform::VideoMode::Wayland(VideoMode {
                    size: (x.dimensions.0 as u32, x.dimensions.1 as u32),
                    refresh_rate: (x.refresh_rate as f32 / 1000.0).round() as u16,
                    bit_depth: 32,
                    monitor: monitor.clone(),
                }),
            })
    }
}

pub fn primary_monitor(env: &Environment<Env>) -> MonitorHandle {
    MonitorHandle(
        env.get_all_outputs()
            .first()
            .expect("No monitor is available.")
            .clone(),
    )
}

pub fn available_monitors(env: &Environment<Env>) -> VecDeque<MonitorHandle> {
    env.get_all_outputs()
        .iter()
        .map(|proxy| MonitorHandle(proxy.clone()))
        .collect()
}
