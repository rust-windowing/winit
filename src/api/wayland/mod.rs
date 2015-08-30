#![cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd"))]
#![allow(unused_variables, dead_code)]

use self::wayland::egl::{EGLSurface, is_egl_available};
use self::wayland::core::Surface;
use self::wayland::core::output::Output;
use self::wayland::core::shell::{ShellSurface, ShellFullscreenMethod};

use self::wayland_window::{DecoratedSurface, SurfaceGuard, substract_borders};

use libc;
use api::dlopen;
use api::egl;
use api::egl::Context as EglContext;

use BuilderAttribs;
use ContextError;
use CreationError;
use Event;
use PixelFormat;
use CursorState;
use MouseCursor;
use GlContext;

use std::collections::VecDeque;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};
use std::ffi::CString;

use platform::MonitorID as PlatformMonitorID;

use self::context::WaylandContext;

extern crate wayland_client as wayland;
extern crate wayland_kbd;
extern crate wayland_window;

mod context;
mod keyboard;

lazy_static! {
    static ref WAYLAND_CONTEXT: Option<WaylandContext> = {
        WaylandContext::new()
    };
}

pub fn is_available() -> bool {
    WAYLAND_CONTEXT.is_some()
}

enum ShellWindow {
    Plain(ShellSurface<EGLSurface>),
    Decorated(DecoratedSurface<EGLSurface>)
}

impl ShellWindow {
    fn get_shell(&mut self) -> ShellGuard {
        match self {
            &mut ShellWindow::Plain(ref mut s) => {
                ShellGuard::Plain(s)
            },
            &mut ShellWindow::Decorated(ref mut s) => {
                ShellGuard::Decorated(s.get_shell())
            }
        }
    }

    fn resize(&mut self, w: i32, h: i32, x: i32, y: i32) {
        match self {
            &mut ShellWindow::Plain(ref s) => s.resize(w, h, x, y),
            &mut ShellWindow::Decorated(ref mut s) => {
                s.resize(w, h);
                s.get_shell().resize(w, h, x, y);
            }
        }
    }

    fn set_cfg_callback(&mut self, arc: Arc<Mutex<(i32, i32, bool)>>) {
        match self {
            &mut ShellWindow::Decorated(ref mut s) => {
                s.get_shell().set_configure_callback(move |_, w, h| {
                    let (w, h) = substract_borders(w, h);
                    let mut guard = arc.lock().unwrap();
                    *guard = (w, h, true);
                })
            }
            _ => {}
        }
    }
}

enum ShellGuard<'a> {
    Plain(&'a mut ShellSurface<EGLSurface>),
    Decorated(SurfaceGuard<'a, EGLSurface>)
}

impl<'a> Deref for ShellGuard<'a> {
    type Target = ShellSurface<EGLSurface>;
    fn deref(&self) -> &ShellSurface<EGLSurface> {
        match self {
            &ShellGuard::Plain(ref s) => s,
            &ShellGuard::Decorated(ref s) => s.deref()
        }
    }
}

impl<'a> DerefMut for ShellGuard<'a> {
    fn deref_mut(&mut self) -> &mut ShellSurface<EGLSurface> {
        match self {
            &mut ShellGuard::Plain(ref mut s) => s,
            &mut ShellGuard::Decorated(ref mut s) => s.deref_mut()
        }
    }
}

pub struct Window {
    shell_window: Mutex<ShellWindow>,
    pending_events: Arc<Mutex<VecDeque<Event>>>,
    need_resize: Arc<Mutex<(i32, i32, bool)>>,
    resize_callback: Option<fn(u32, u32)>,
    pub context: EglContext,
}

// private methods of wayalnd windows

impl Window {
    fn resize_if_needed(&self) -> bool {
        let mut guard = self.need_resize.lock().unwrap();
        let (w, h, b) = *guard;
        *guard = (0, 0, false);
        if b {
            let mut guard = self.shell_window.lock().unwrap();
            guard.resize(w, h, 0, 0);
            if let Some(f) = self.resize_callback {
                f(w as u32, h as u32);
            }
            if let Some(ref ctxt) = *WAYLAND_CONTEXT {
                let mut window_guard = self.shell_window.lock().unwrap();
                ctxt.push_event_for(
                    window_guard.get_shell().get_wsurface().get_id(),
                    Event::Resized(w as u32, h as u32)
                );
            }
        }
        b
    }
}

#[derive(Clone)]
pub struct WindowProxy;

impl WindowProxy {
    pub fn wakeup_event_loop(&self) {
        if let Some(ref ctxt) = *WAYLAND_CONTEXT {
            ctxt.display.sync();
        }
    }
}

#[derive(Clone)]
pub struct MonitorID {
    output: Arc<Output>
}

pub fn get_available_monitors() -> VecDeque<MonitorID> {
    WAYLAND_CONTEXT.as_ref().unwrap().outputs.iter().map(|o| MonitorID::new(o.clone())).collect()
}
pub fn get_primary_monitor() -> MonitorID {
    match WAYLAND_CONTEXT.as_ref().unwrap().outputs.iter().next() {
        Some(o) => MonitorID::new(o.clone()),
        None => panic!("No monitor is available.")
    }
}

impl MonitorID {
    fn new(output: Arc<Output>) -> MonitorID {
        MonitorID {
            output: output
        }
    }

    pub fn get_name(&self) -> Option<String> {
        Some(format!("{} - {}", self.output.manufacturer(), self.output.model()))
    }

    pub fn get_native_identifier(&self) -> ::native_monitor::NativeMonitorId {
        ::native_monitor::NativeMonitorId::Unavailable
    }

    pub fn get_dimensions(&self) -> (u32, u32) {
        let (w, h) = self.output.modes()
                                .into_iter()
                                .find(|m| m.is_current())
                                .map(|m| (m.width, m.height))
                                .unwrap();
        (w as u32, h as u32)
    }
}


pub struct PollEventsIterator<'a> {
    window: &'a Window,
}

impl<'a> Iterator for PollEventsIterator<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        if let Some(ref ctxt) = *WAYLAND_CONTEXT {
            ctxt.display.dispatch_pending();
        }
        if self.window.resize_if_needed() {
            Some(Event::Refresh)
        } else {
            self.window.pending_events.lock().unwrap().pop_front()
        }
    }
}

pub struct WaitEventsIterator<'a> {
    window: &'a Window,
}

impl<'a> Iterator for WaitEventsIterator<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        let mut evt = None;
        while evt.is_none() {
            if let Some(ref ctxt) = *WAYLAND_CONTEXT {
                ctxt.display.dispatch();
            }
            evt = if self.window.resize_if_needed() {
                Some(Event::Refresh)
            } else {
                self.window.pending_events.lock().unwrap().pop_front()
            };
        }
        evt
    }
}

impl Window {
    pub fn new(builder: BuilderAttribs) -> Result<Window, CreationError> {
        use self::wayland::internals::FFI;

        let wayland_context = match *WAYLAND_CONTEXT {
            Some(ref c) => c,
            None => return Err(CreationError::NotSupported),
        };

        if !is_egl_available() { return Err(CreationError::NotSupported) }

        let (w, h) = builder.dimensions.unwrap_or((800, 600));

        let surface = EGLSurface::new(
            wayland_context.compositor.create_surface(),
            w as i32,
            h as i32
        );

        let mut shell_window = if let Some(PlatformMonitorID::Wayland(ref monitor)) = builder.monitor {
            let shell_surface = wayland_context.shell.get_shell_surface(surface);
            shell_surface.set_fullscreen(ShellFullscreenMethod::Default, Some(&monitor.output));
            ShellWindow::Plain(shell_surface)
        } else {
            if builder.decorations {
                ShellWindow::Decorated(match DecoratedSurface::new(
                    surface,
                    w as i32,
                    h as i32,
                    &wayland_context.registry,
                    Some(&wayland_context.seat)
                ) {
                    Ok(s) => s,
                    Err(_) => return Err(CreationError::NotSupported)
                })
            } else {
                ShellWindow::Plain(wayland_context.shell.get_shell_surface(surface))
            }
        };

        let context = {
            let libegl = unsafe { dlopen::dlopen(b"libEGL.so\0".as_ptr() as *const _, dlopen::RTLD_NOW) };
            if libegl.is_null() {
                return Err(CreationError::NotSupported);
            }
            let egl = ::api::egl::ffi::egl::Egl::load_with(|sym| {
                let sym = CString::new(sym).unwrap();
                unsafe { dlopen::dlsym(libegl, sym.as_ptr()) }
            });
            try!(EglContext::new(
                egl,
                &builder,
                egl::NativeDisplay::Wayland(Some(wayland_context.display.ptr() as *const _)))
                .and_then(|p| p.finish((**shell_window.get_shell()).ptr() as *const _))
            )
        };

        // create a queue already containing a refresh event to trigger first draw
        // it's harmless and a removes the need to do a first swap_buffers() before
        // starting the event loop
        let events = Arc::new(Mutex::new({
            let mut v = VecDeque::new();
            v.push_back(Event::Refresh);
            v
        }));

        wayland_context.register_surface(shell_window.get_shell().get_wsurface().get_id(),
                                         events.clone());

        let need_resize = Arc::new(Mutex::new((0, 0, false)));

        shell_window.set_cfg_callback(need_resize.clone());

        wayland_context.display.flush().unwrap();

        Ok(Window {
            shell_window: Mutex::new(shell_window),
            pending_events: events,
            need_resize: need_resize,
            resize_callback: None,
            context: context
        })
    }

    pub fn set_title(&self, title: &str) {
        let ctitle = CString::new(title).unwrap();
        // intermediate variable is forced,
        // see https://github.com/rust-lang/rust/issues/22921
        let mut guard = self.shell_window.lock().unwrap();
        guard.get_shell().set_title(&ctitle);
    }

    pub fn show(&self) {
        // TODO
    }

    pub fn hide(&self) {
        // TODO
    }

    pub fn get_position(&self) -> Option<(i32, i32)> {
        // not available with wayland
        None
    }

    pub fn set_position(&self, _x: i32, _y: i32) {
        // not available with wayland
    }

    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        // intermediate variables are forced,
        // see https://github.com/rust-lang/rust/issues/22921
        let mut guard = self.shell_window.lock().unwrap();
        let shell = guard.get_shell();
        let (w, h) = shell.get_attached_size();
        Some((w as u32, h as u32))
    }

    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        // maybe available if we draw the border ourselves ?
        // but for now, no.
        None
    }

    pub fn set_inner_size(&self, x: u32, y: u32) {
        self.shell_window.lock().unwrap().resize(x as i32, y as i32, 0, 0)
    }

    pub fn create_window_proxy(&self) -> WindowProxy {
        WindowProxy
    }

    pub fn poll_events(&self) -> PollEventsIterator {
        PollEventsIterator {
            window: self
        }
    }

    pub fn wait_events(&self) -> WaitEventsIterator {
        WaitEventsIterator {
            window: self
        }
    }

    pub fn set_window_resize_callback(&mut self, callback: Option<fn(u32, u32)>) {
        self.resize_callback = callback;
    }

    pub fn set_cursor(&self, cursor: MouseCursor) {
        // TODO
    }

    pub fn set_cursor_state(&self, state: CursorState) -> Result<(), String> {
        // TODO
        Ok(())
    }

    pub fn hidpi_factor(&self) -> f32 {
        1.0
    }

    pub fn set_cursor_position(&self, x: i32, y: i32) -> Result<(), ()> {
        // TODO
        Ok(())
    }

    pub fn platform_display(&self) -> *mut libc::c_void {
        unimplemented!()
    }

    pub fn platform_window(&self) -> *mut libc::c_void {
        unimplemented!()
    }
}

impl GlContext for Window {
    unsafe fn make_current(&self) -> Result<(), ContextError> {
        self.context.make_current()
    }

    fn is_current(&self) -> bool {
        self.context.is_current()
    }

    fn get_proc_address(&self, addr: &str) -> *const libc::c_void {
        self.context.get_proc_address(addr)
    }

    fn swap_buffers(&self) -> Result<(), ContextError> {
        self.context.swap_buffers()
    }

    fn get_api(&self) -> ::Api {
        self.context.get_api()
    }

    fn get_pixel_format(&self) -> PixelFormat {
        self.context.get_pixel_format().clone()
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        if let Some(ref ctxt) = *WAYLAND_CONTEXT {
            // intermediate variable is forced,
            // see https://github.com/rust-lang/rust/issues/22921
            let mut guard = self.shell_window.lock().unwrap();
            let shell = guard.get_shell();
            ctxt.deregister_surface(
                shell.get_wsurface().get_id()
            )
        }
    }
}
