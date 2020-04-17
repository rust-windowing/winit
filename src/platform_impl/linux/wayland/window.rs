use std::{collections::VecDeque, sync::{Arc, Mutex}};
use raw_window_handle::unix::WaylandHandle;
use smithay_client_toolkit::{
    environment::Environment,
    reexports::client::{
        Display,
        protocol::{wl_output::WlOutput, wl_surface::WlSurface},
    },
    get_surface_outputs,
    get_surface_scale_factor,
    window::{ConceptFrame, Decorations},
};
use crate::{
    event::WindowEvent as Event,
    dpi::{LogicalSize, PhysicalPosition, PhysicalSize, Position, Size},
    error::{ExternalError, NotSupportedError, OsError},
    platform_impl::{
        self as platform,
        platform::wayland::event_loop::{available_monitors, primary_monitor},
        PlatformSpecificWindowBuilderAttributes as AttributesExt,
    },
    window::{CursorIcon, Fullscreen, WindowAttributes},
};
use super::{event_loop::{DispatchData, State, Context, Env, MonitorHandle}, EventLoopWindowTarget, conversion};

pub fn id(surface: &WlSurface) -> crate::window::WindowId {
    crate::window::WindowId(super::super::WindowId::Wayland(surface.as_ref().id()))
}

pub fn event<'t, T>(event: crate::event::WindowEvent<'t>, surface: &WlSurface) -> crate::event::Event<'t, T> {
    crate::event::Event::WindowEvent{event, window_id: id(surface)}
}

pub enum Set {
    Size((u32,u32)),
    RequestRedraw,
    Minimized,
    Maximized(bool),
    Fullscreen(Option<WlOutput>),
    Decorate(bool),
    Resizable(bool),
    Title(String),
    Cursor(&'static str),
    MinSize(Option<(u32,u32)>),
    MaxSize(Option<(u32,u32)>),
    CursorGrab(bool),
    Drop
}
pub struct Command { pub surface: WlSurface, pub set: Set }

pub struct Handle {
    display: &'static Display,
    env: &'static Environment<Env>,
    surface: WlSurface,
    // Arc<Mutex> shared with EventLoop::windows so editions in Configure are reflected back on the handle state for size changes
    windows: &'static Arc<Mutex<Vec<super::event_loop::Window>>>,
    command: &'static smithay_client_toolkit::reexports::calloop::channel::Sender<Command>, // Sends (foreign thread) user commands to EventLoop
    // todo: futures::channel ^
    // Asynchronous state on the handle, sent on the update channel by all setters
    //state: State,
    //cursor: &'static str,
    //cursor_visible: bool,
}

fn output(fullscreen: Fullscreen) -> WlOutput {
    match fullscreen {
        Fullscreen::Exclusive(_) => panic!("Wayland doesn't support exclusive fullscreen"),
        Fullscreen::Borderless(crate::monitor::MonitorHandle {inner: platform::MonitorHandle::Wayland(monitor_id)}) => monitor_id.0,
        //#[allow(unreachable_patterns)] Fullscreen::Borderless(_) => unreachable!(),
    }
}

use Set::*;
impl Handle {
    pub fn new<T>(
        context: &EventLoopWindowTarget<T>,
        attributes: WindowAttributes,
        attributes_ext: AttributesExt,
    ) -> Result<Self, OsError> {
        let Context{env,display,sctk_windows,windows,command,..} = context;
        let surface = env.create_surface_with_scale_callback(
            |scale_factor, surface, mut data| {
                surface.set_buffer_scale(scale_factor);
                let mut data = data.get::<DispatchData<T>>().unwrap();
                fn with<T, R>(data: &mut DispatchData<T>, surface: &WlSurface, f:impl Fn(&mut super::event_loop::Window) -> R) -> R {
                    f(data.state.context.windows.lock().unwrap().iter_mut().find(|w| w == &surface).unwrap())
                }
                let mut size = with(&mut data, &surface, |window| {
                    window.scale_factor = scale_factor as u32;
                    LogicalSize::<f64>::from(window.size).to_physical::<u32>(scale_factor as f64)
                });
                data.send(event(Event::ScaleFactorChanged{scale_factor: scale_factor as f64, new_inner_size: &mut size}, &surface));
                with(&mut data, &surface, |window| {
                    window.size = size.to_logical::<f64>(scale_factor as f64).into();
                });
                // Also send a Resized event though logical size stays identical since Resized size argument is given as 'physical'
                data.send(self::event(Event::Resized(size), &surface)); // fixme: windows lock
            }
        );

        let identity_scale_factor = get_surface_scale_factor(&surface) as u32; // Always 1.
        let size = attributes.inner_size.map(|size| size.to_logical::<f64>(identity_scale_factor as f64).into()).unwrap_or((800, 600));
        let mut window = env.create_window::<ConceptFrame, _>(surface.clone(), size, {
            let surface = surface.clone();
            move |event, mut data| {
                let data = data.get().unwrap();
                let event = {
                    let DispatchData::<T>{state:State{context:Context{windows,sctk_windows,..},..},..} = data;
                    use smithay_client_toolkit::window::Event::*;
                    match event {
                        Configure { new_size, states } => if let Some(window) = windows.lock().unwrap().iter_mut().find(|w| *w == &surface) /*=>*/ {
                            window.states = states;
                            let size = new_size.unwrap_or(window.size);
                            let event = if window.size != size || window.scale_factor != get_surface_scale_factor(&surface) as u32 {
                                window.size = size;
                                window.scale_factor = get_surface_scale_factor(&surface) as u32;
                                let size = LogicalSize::<f64>::from(window.size).to_physical(window.scale_factor as f64);
                                //redraw_events.push(crate::event::Event::RedrawRequested(id(&surface))); // is RedrawRequested expected after Resized ?
                                Some(Event::Resized(size))
                            } else { None };
                            let mut sctk_windows = sctk_windows.lock().unwrap();
                            let sctk_window = sctk_windows.iter_mut().find(|w| w.surface() == &surface).unwrap();
                            {let (w, h) = size; sctk_window.resize(w, h);}
                            sctk_window.refresh();
                            event
                        } else { unreachable!() },
                        Refresh => if let Some(window) = sctk_windows.lock().unwrap().iter_mut().find(|w| w.surface() == &surface) /*=>*/ { window.refresh(); None } else { unreachable!() },
                        Close => Some(Event::CloseRequested),
                    }
                };
                if let Some(event) = event { data.send(self::event(event, &surface)); }
            }
        }).unwrap();

        if let Some(app_id) = attributes_ext.app_id { window.set_app_id(app_id); }
        if let Some(fullscreen) = attributes.fullscreen { window.set_fullscreen(Some(&output(fullscreen))); }
        else if attributes.maximized { window.set_maximized(); }
        window.set_resizable(attributes.resizable);
        window.set_decorate( if attributes.decorations { Decorations::FollowServer } else { Decorations::None });
        window.set_title(attributes.title);
        window.set_min_size(attributes.min_inner_size.map(|size| size.to_logical::<f64>(identity_scale_factor as f64).into()));
        window.set_max_size(attributes.max_inner_size.map(|size| size.to_logical::<f64>(identity_scale_factor as f64).into()));

        sctk_windows.lock().unwrap().push(window);

        windows.lock().unwrap().push(super::event_loop::Window{
            surface: surface.clone(),
            size: Default::default(), // until Configure
            scale_factor: identity_scale_factor, // until surface enter output
            states: Default::default(), // until Configure
            current_cursor: "left_ptr",
            locked_pointers: Default::default(),
        });

        Ok(Self{display, env, surface, windows, command: &command.0, /*cursor: "left_ptr", cursor_visible: true*/})
    }

    pub fn id(&self) -> super::WindowId { self.surface.as_ref().id() }

    pub fn scale_factor(&self) -> i32 { get_surface_scale_factor(&self.surface) }

    //pub fn display(&self) -> *mut Display { self.display }
    pub fn surface(&self) -> &WlSurface { &self.surface }
    pub fn current_monitor(&self) -> MonitorHandle { MonitorHandle(get_surface_outputs(&self.surface).last().unwrap().clone()) }
    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> { available_monitors(&self.env) }
    pub fn primary_monitor(&self) -> MonitorHandle { primary_monitor(&self.env) }

    pub fn raw_window_handle(&self) -> WaylandHandle {
        WaylandHandle {
            surface: self.surface().as_ref().c_ptr() as *mut _,
            display: self.display.c_ptr() as *mut _,
            ..WaylandHandle::empty()
        }
    }

    fn with<R>(&self, f: impl Fn(&super::event_loop::Window) -> R) -> R { f(self.windows.lock().unwrap().iter().find(|&w| w==&self.surface).unwrap()) }

    pub fn inner_size(&self) -> PhysicalSize<u32> { self.with(|window| LogicalSize::<f64>::from(window.size).to_physical(self.scale_factor() as f64)) }
    pub fn outer_size(&self) -> PhysicalSize<u32> { self.inner_size() /*fixme*/ }

    pub fn inner_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> { Err(NotSupportedError::new()) }
    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> { Err(NotSupportedError::new()) }

    pub fn fullscreen(&self) -> Option<Fullscreen> {
        if self.with(|window| window.states.contains(&smithay_client_toolkit::window::State::Fullscreen)) {
            Some(Fullscreen::Borderless(crate::monitor::MonitorHandle {inner: platform::MonitorHandle::Wayland(self.current_monitor())}))
        } else { None }
    }

    pub fn set(&self, set: Set) { self.command.send(Command{surface: self.surface.clone(), set}).unwrap() }
    pub fn request_redraw(&self) { self.set(RequestRedraw); }
    pub fn set_title(&self, title: &str) { self.set(Title(title.into())); }
    pub fn set_visible(&self, _visible: bool) { /*todo*/ }

    // note: This will only resize the borders, the contents must be updated by the user
    pub fn set_inner_size(&self, size: Size) { self.set(Size(size.to_logical::<u32>(self.scale_factor() as f64).into())); }
    pub fn set_min_inner_size(&self, dimensions: Option<Size>) { self.set(MinSize(dimensions.map(|dim| dim.to_logical::<f64>(self.scale_factor() as f64).into()))); }
    pub fn set_max_inner_size(&self, dimensions: Option<Size>) { self.set(MaxSize(dimensions.map(|dim| dim.to_logical::<f64>(self.scale_factor() as f64).into()))); }
    pub fn set_resizable(&self, resizable: bool) { self.set(Resizable(resizable)); }
    pub fn set_decorations(&self, decorate: bool) { self.set(Decorate(decorate)); }
    pub fn set_minimized(&self, minimized: bool) { if minimized { self.set(Minimized); } }
    pub fn set_maximized(&self, maximized: bool) { self.set(Maximized(maximized)); }
    pub fn set_fullscreen(&self, fullscreen: Option<Fullscreen>) { self.set(Fullscreen(fullscreen.map(output))); }
    pub fn set_cursor_icon(&self, cursor_icon: CursorIcon) { // why is this not &mut ?
        //self.cursor = conversion::cursor(cursor_icon); if self.cursor_visible { self.set(Cursor(self.cursor)); }
        self.set(Cursor(conversion::cursor(cursor_icon))); // Assume visible
    }
    pub fn set_cursor_visible(&self, visible: bool) {
        //self.cursor_visible = visible; self.set(Cursor(if self.cursor_visible { self.cursor } else { "" }))
        self.set(Cursor(if visible { "left_ptr" } else { "" })) // Assume default icon
    }
    pub fn set_cursor_grab(&self, grab: bool) -> Result<(), ExternalError> { self.set(CursorGrab(grab)); Ok(()) }
    pub fn set_cursor_position(&self, _pos: Position) -> Result<(), ExternalError> { Err(ExternalError::NotSupported(NotSupportedError::new())) }
    pub fn set_outer_position(&self, _pos: Position) { /*todo*/ }
}

impl std::ops::Drop for Handle { fn drop(&mut self) { self.set(Drop); } }
