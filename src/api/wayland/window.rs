use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use libc;

use {CreationError, CursorState, Event, MouseCursor, WindowAttributes};
use platform::MonitorId as PlatformMonitorId;

use wayland_client::EventIterator;
use wayland_client::wayland::compositor::WlSurface;
use wayland_client::wayland::shell::WlShellSurface;
use super::wayland_window::{DecoratedSurface, add_borders, substract_borders};
use super::context::{WaylandContext, WAYLAND_CONTEXT};

#[derive(Clone)]
pub struct WindowProxy;

impl WindowProxy {
    #[inline]
    pub fn wakeup_event_loop(&self) {
        unimplemented!()
    }
}

pub struct Window {
    wayland_context: &'static WaylandContext,
    surface: WlSurface,
    shell_window: Mutex<ShellWindow>,
    evt_queue: Arc<Mutex<VecDeque<Event>>>,
    inner_size: Mutex<(i32, i32)>,
    resize_callback: Option<fn(u32, u32)>,
}

impl Window {
    fn next_event(&self) -> Option<Event> {
        use wayland_client::Event as WEvent;
        use wayland_client::wayland::WaylandProtocolEvent;
        use wayland_client::wayland::shell::WlShellSurfaceEvent;

        let mut newsize = None;
        let mut evt_queue_guard = self.evt_queue.lock().unwrap();

        let mut shell_window_guard = self.shell_window.lock().unwrap();
        match *shell_window_guard {
            ShellWindow::Decorated(ref mut deco) => {
                for (_, w, h) in deco {
                    newsize = Some((w, h));
                }
            },
            ShellWindow::Plain(ref plain, ref mut evtiter) => {
                for evt in evtiter {
                    if let WEvent::Wayland(WaylandProtocolEvent::WlShellSurface(_, ssevt)) = evt {
                        match ssevt {
                            WlShellSurfaceEvent::Ping(u) => {
                                plain.pong(u);
                            },
                            WlShellSurfaceEvent::Configure(_, w, h) => {
                                newsize = Some((w, h));
                            },
                            _ => {}
                        }
                    }
                }
            }
        }

        if let Some((w, h)) = newsize {
            let (w, h) = substract_borders(w, h);
            *self.inner_size.lock().unwrap() = (w, h);
            if let ShellWindow::Decorated(ref mut deco) = *shell_window_guard {
                deco.resize(w, h);
            }
            if let Some(f) = self.resize_callback {
                f(w as u32, h as u32);
            }
            Some(Event::Resized(w as u32, h as u32))
        } else {
            evt_queue_guard.pop_front()
        }
    }
}

pub struct PollEventsIterator<'a> {
    window: &'a Window,
}

impl<'a> Iterator for PollEventsIterator<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        match self.window.next_event() {
            Some(evt) => return Some(evt),
            None => {}
        }
        // the queue was empty, try a dispatch and see the result
        self.window.wayland_context.dispatch_events();
        return self.window.next_event();
    }
}

pub struct WaitEventsIterator<'a> {
    window: &'a Window,
}

impl<'a> Iterator for WaitEventsIterator<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        loop {
            match self.window.next_event() {
                Some(evt) => return Some(evt),
                None => {}
            }
            // the queue was empty, try a dispatch & read and see the result
            self.window.wayland_context.flush_events().expect("Connexion with the wayland compositor lost.");
            match self.window.wayland_context.read_events() {
                Ok(_) => {
                    // events were read or dispatch is needed, in both cases, we dispatch
                    self.window.wayland_context.dispatch_events()
                }
                Err(_) => panic!("Connexion with the wayland compositor lost.")
            }
        }
    }
}

enum ShellWindow {
    Plain(WlShellSurface, EventIterator),
    Decorated(DecoratedSurface)
}

impl Window {
    pub fn new(window: &WindowAttributes) -> Result<Window, CreationError>
    {
        use wayland_client::Proxy;
        // not implemented
        assert!(window.min_dimensions.is_none());
        assert!(window.max_dimensions.is_none());

        let wayland_context = match *WAYLAND_CONTEXT {
            Some(ref c) => c,
            None => return Err(CreationError::NotSupported),
        };

        let (w, h) = window.dimensions.unwrap_or((800, 600));

        let (surface, evt_queue) = match wayland_context.new_surface() {
            Some(t) => t,
            None => return Err(CreationError::NotSupported)
        };

        let shell_window = if let Some(PlatformMonitorId::Wayland(ref monitor_id)) = window.monitor {
            let pid = super::monitor::proxid_from_monitorid(monitor_id);
            match wayland_context.plain_from(&surface, Some(pid)) {
                Some(mut s) => {
                    let iter = EventIterator::new();
                    s.set_evt_iterator(&iter);
                    ShellWindow::Plain(s, iter)
                },
                None => return Err(CreationError::NotSupported)
            }
        } else if window.decorations {
            match wayland_context.decorated_from(&surface, w as i32, h as i32) {
                Some(s) => ShellWindow::Decorated(s),
                None => return Err(CreationError::NotSupported)
            }
        } else {
            match wayland_context.plain_from(&surface, None) {
                Some(mut s) => {
                    let iter = EventIterator::new();
                    s.set_evt_iterator(&iter);
                    ShellWindow::Plain(s, iter)
                },
                None => return Err(CreationError::NotSupported)
            }
        };

        Ok(Window {
            wayland_context: wayland_context,
            surface: surface,
            shell_window: Mutex::new(shell_window),
            evt_queue: evt_queue,
            inner_size: Mutex::new((w as i32, h as i32)),
            resize_callback: None,
        })
    }

    pub fn set_title(&self, title: &str) {
        let guard = self.shell_window.lock().unwrap();
        match *guard {
            ShellWindow::Plain(ref plain, _) => { plain.set_title(title.into()); },
            ShellWindow::Decorated(ref deco) => { deco.set_title(title.into()); }
        }
    }

    #[inline]
    pub fn show(&self) {
        // TODO
    }

    #[inline]
    pub fn hide(&self) {
        // TODO
    }

    #[inline]
    pub fn get_position(&self) -> Option<(i32, i32)> {
        // Not possible with wayland
        None
    }

    #[inline]
    pub fn set_position(&self, _x: i32, _y: i32) {
        // Not possible with wayland
    }

    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        let (w, h) = *self.inner_size.lock().unwrap();
        Some((w as u32, h as u32))
    }

    #[inline]
    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        let (w, h) = *self.inner_size.lock().unwrap();
        let (w, h) = add_borders(w, h);
        Some((w as u32, h as u32))
    }

    #[inline]
    pub fn set_inner_size(&self, x: u32, y: u32) {
        let mut guard = self.shell_window.lock().unwrap();
        match *guard {
            ShellWindow::Decorated(ref mut deco) => { deco.resize(x as i32, y as i32); },
            _ => {}
        }
    }

    #[inline]
    pub fn create_window_proxy(&self) -> WindowProxy {
        WindowProxy
    }

    #[inline]
    pub fn poll_events(&self) -> PollEventsIterator {
        PollEventsIterator {
            window: self
        }
    }

    #[inline]
    pub fn wait_events(&self) -> WaitEventsIterator {
        WaitEventsIterator {
            window: self
        }
    }

    #[inline]
    pub fn set_window_resize_callback(&mut self, callback: Option<fn(u32, u32)>) {
        self.resize_callback = callback;
    }

    #[inline]
    pub fn set_cursor(&self, _cursor: MouseCursor) {
        // TODO
    }

    #[inline]
    pub fn set_cursor_state(&self, state: CursorState) -> Result<(), String> {
        use CursorState::{Grab, Normal, Hide};
        // TODO : not yet possible on wayland to grab cursor
        match state {
            Grab => Err("Cursor cannot be grabbed on wayland yet.".to_string()),
            Hide => Err("Cursor cannot be hidden on wayland yet.".to_string()),
            Normal => Ok(())
        }
    }

    #[inline]
    pub fn hidpi_factor(&self) -> f32 {
        1.0
    }

    #[inline]
    pub fn set_cursor_position(&self, _x: i32, _y: i32) -> Result<(), ()> {
        // TODO: not yet possible on wayland
        Err(())
    }

    #[inline]
    pub fn get_wayland_display(&self) -> *mut libc::c_void {
        WAYLAND_CONTEXT.as_ref().unwrap() // context exists if window was created
                       .display_ptr() as *mut libc::c_void
    }

    #[inline]
    pub fn get_wayland_surface(&self) -> *mut libc::c_void {
        use wayland_client::Proxy;
        self.surface.ptr() as *mut libc::c_void
    }

    #[inline]
    pub fn platform_display(&self) -> *mut libc::c_void {
        WAYLAND_CONTEXT.as_ref().unwrap() // context exists if window was created
                       .display_ptr() as *mut libc::c_void
    }

    #[inline]
    pub fn platform_window(&self) -> *mut libc::c_void {
        use wayland_client::Proxy;
        self.surface.ptr() as *mut libc::c_void
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        use wayland_client::Proxy;
        self.wayland_context.dropped_surface(self.surface.id());
    }
}