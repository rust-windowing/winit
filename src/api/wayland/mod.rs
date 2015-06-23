#![cfg(target_os = "linux")]
#![allow(unused_variables, dead_code)]

use self::wayland::egl::{EGLSurface, is_egl_available};
use self::wayland::core::Surface;
use self::wayland::core::output::Output;
use self::wayland::core::shell::{ShellSurface, ShellFullscreenMethod};

use libc;
use api::dlopen;
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
use std::sync::{Arc, Mutex};
use std::ffi::CString;

use platform::MonitorID as PlatformMonitorID;

use self::context::WaylandContext;

extern crate wayland_client as wayland;
extern crate wayland_kbd;

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

pub struct Window {
    shell_surface: ShellSurface<EGLSurface>,
    pending_events: Arc<Mutex<VecDeque<Event>>>,
    pub context: EglContext,
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
        self.window.pending_events.lock().unwrap().pop_front()
    }
}

pub struct WaitEventsIterator<'a> {
    window: &'a Window,
}

impl<'a> Iterator for WaitEventsIterator<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        if let Some(ref ctxt) = *WAYLAND_CONTEXT {
            ctxt.display.dispatch();
        }
        self.window.pending_events.lock().unwrap().pop_front()
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

        let shell_surface = wayland_context.shell.get_shell_surface(surface);
        if let Some(PlatformMonitorID::Wayland(ref monitor)) = builder.monitor {
            shell_surface.set_fullscreen(ShellFullscreenMethod::Default, Some(&monitor.output));
        } else {
            shell_surface.set_toplevel();
        }

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
                Some(wayland_context.display.ptr() as *const _),
                (*shell_surface).ptr() as *const _
            ))
        };

        let events = Arc::new(Mutex::new(VecDeque::new()));

        wayland_context.register_surface(shell_surface.get_wsurface().get_id(), events.clone());

        wayland_context.display.flush().unwrap();

        Ok(Window {
            shell_surface: shell_surface,
            pending_events: events,
            context: context
        })
    }

    pub fn set_title(&self, title: &str) {
        let ctitle = CString::new(title).unwrap();
        self.shell_surface.set_title(&ctitle);
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
        let (w, h) = self.shell_surface.get_attached_size();
        Some((w as u32, h as u32))
    }

    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        // maybe available if we draw the border ourselves ?
        // but for now, no.
        None
    }

    pub fn set_inner_size(&self, x: u32, y: u32) {
        self.shell_surface.resize(x as i32, y as i32, 0, 0)
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
        if let Some(callback) = callback {
            self.shell_surface.set_configure_callback(
                move |_,w,h| { callback(w as u32, h as u32) }
            );
        } else {
            self.shell_surface.set_configure_callback(
                move |_,_,_| {}
            );
        }
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
            ctxt.deregister_surface(self.shell_surface.get_wsurface().get_id())
        }
    }
}
