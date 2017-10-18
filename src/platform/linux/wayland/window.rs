use std::sync::Arc;

use wayland_client::protocol::{wl_display,wl_surface};
use wayland_client::{Proxy, StateToken};

use {CreationError, MouseCursor, CursorState, WindowAttributes};
use platform::MonitorId as PlatformMonitorId;
use window::MonitorId as RootMonitorId;
use platform::wayland::MonitorId as WaylandMonitorId;

use super::{EventsLoop, WindowId, make_wid};
use super::wayland_window::{DecoratedSurface, DecoratedSurfaceImplementation};

pub struct Window {
    surface: wl_surface::WlSurface,
    decorated: DecoratedSurface,
}

impl Window {
    pub fn new(evlp: &EventsLoop, attributes: &WindowAttributes) -> Result<Window, CreationError>
    {
        let (width, height) = attributes.dimensions.unwrap_or((800,600));

        let (surface, decorated, xdg) = evlp.create_window(
            width, height, attributes.decorations, decorated_impl(), ());

        let mut fullscreen_monitor =
        if let Some(RootMonitorId { inner: PlatformMonitorId::Wayland(ref monitor_id) }) = attributes.fullscreen {
            monitor_id.info.lock().unwrap().output.clone()
        } else {
            None
        };

        Ok(Window {
            surface: surface,
            decorated: decorated
        })
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        unimplemented!()
    }

    pub fn set_title(&self, title: &str) {
        unimplemented!()
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
        unimplemented!()
    }

    #[inline]
    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        unimplemented!()
    }

    #[inline]
    // NOTE: This will only resize the borders, the contents must be updated by the user
    pub fn set_inner_size(&self, x: u32, y: u32) {
        unimplemented!()
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
        // TODO
        1.0
    }

    #[inline]
    pub fn set_cursor_position(&self, _x: i32, _y: i32) -> Result<(), ()> {
        // TODO: not yet possible on wayland
        Err(())
    }
    
    pub fn get_display(&self) -> &wl_display::WlDisplay {
        unimplemented!()
    }
    
    pub fn get_surface(&self) -> &wl_surface::WlSurface {
        unimplemented!()
    }

    pub fn get_current_monitor(&self) -> WaylandMonitorId {
        unimplemented!()
    }

    pub fn is_ready(&self) -> bool {
        unimplemented!()
    }
}

/*
 * Internal store for windows
 */

struct InternalWindow {
    surface: wl_surface::WlSurface
}

pub struct WindowStore {
    windows: Vec<InternalWindow>
}

impl WindowStore {
    pub fn new() -> WindowStore {
        WindowStore { windows: Vec::new() }
    }

    pub fn find_wid(&self, surface: &wl_surface::WlSurface) -> Option<WindowId> {
        for window in &self.windows {
            if surface.equals(&window.surface) {
                return Some(make_wid(surface));
            }
        }
        None
    }
}

/*
 * Protocol implementation
 */

fn decorated_impl() -> DecoratedSurfaceImplementation<()> {
    DecoratedSurfaceImplementation {
        configure: |_, _, _, _| {},
        close: |_, _| {}
    }
}
