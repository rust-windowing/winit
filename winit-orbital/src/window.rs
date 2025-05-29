use std::collections::VecDeque;
use std::iter;
use std::sync::{Arc, Mutex};

use dpi::{PhysicalInsets, PhysicalPosition, PhysicalSize, Position, Size};
use winit_core::cursor::Cursor;
use winit_core::error::{NotSupportedError, RequestError};
use winit_core::impl_surface_downcast;
use winit_core::monitor::{Fullscreen, MonitorHandle as CoreMonitorHandle};
use winit_core::window::{self, ImePurpose, Surface as CoreSurface, Window as CoreWindow, SurfaceId};

use crate::event_loop::{ActiveEventLoop, EventLoopProxy};
use crate::{RedoxSocket, WindowProperties};

// These values match the values uses in the `window_new` function in orbital:
// https://gitlab.redox-os.org/redox-os/orbital/-/blob/master/src/scheme.rs
const ORBITAL_FLAG_ASYNC: char = 'a';
const ORBITAL_FLAG_BACK: char = 'b';
const ORBITAL_FLAG_FRONT: char = 'f';
const ORBITAL_FLAG_HIDDEN: char = 'h';
const ORBITAL_FLAG_BORDERLESS: char = 'l';
const ORBITAL_FLAG_MAXIMIZED: char = 'm';
const ORBITAL_FLAG_RESIZABLE: char = 'r';
const ORBITAL_FLAG_TRANSPARENT: char = 't';

#[derive(Debug)]
pub struct Window {
    window_socket: Arc<RedoxSocket>,
    redraws: Arc<Mutex<VecDeque<SurfaceId>>>,
    destroys: Arc<Mutex<VecDeque<SurfaceId>>>,
    event_loop_proxy: Arc<EventLoopProxy>,
}

impl Window {
    pub(crate) fn new(
        el: &ActiveEventLoop,
        attrs: window::WindowAttributes,
    ) -> Result<Self, RequestError> {
        let scale = 1.;

        let (x, y) = if let Some(pos) = attrs.position {
            pos.to_physical::<i32>(scale).into()
        } else {
            // These coordinates are a special value to center the window.
            (-1, -1)
        };

        let (w, h): (u32, u32) = if let Some(size) = attrs.surface_size {
            size.to_physical::<u32>(scale).into()
        } else {
            (1024, 768)
        };

        // TODO: min/max surface_size

        // Async by default.
        let mut flag_str = ORBITAL_FLAG_ASYNC.to_string();

        if attrs.maximized {
            flag_str.push(ORBITAL_FLAG_MAXIMIZED);
        }

        if attrs.resizable {
            flag_str.push(ORBITAL_FLAG_RESIZABLE);
        }

        // TODO: fullscreen

        if attrs.transparent {
            flag_str.push(ORBITAL_FLAG_TRANSPARENT);
        }

        if !attrs.decorations {
            flag_str.push(ORBITAL_FLAG_BORDERLESS);
        }

        if !attrs.visible {
            flag_str.push(ORBITAL_FLAG_HIDDEN);
        }

        match attrs.window_level {
            window::WindowLevel::AlwaysOnBottom => {
                flag_str.push(ORBITAL_FLAG_BACK);
            },
            window::WindowLevel::Normal => {},
            window::WindowLevel::AlwaysOnTop => {
                flag_str.push(ORBITAL_FLAG_FRONT);
            },
        }

        // TODO: window_icon

        // Open window.
        let window = RedoxSocket::orbital(&WindowProperties {
            flags: &flag_str,
            x,
            y,
            w,
            h,
            title: &attrs.title,
        })
        .expect("failed to open window");

        // Add to event socket.
        el.event_socket
            .write(&syscall::Event {
                id: window.fd,
                flags: syscall::EventFlags::EVENT_READ,
                data: window.fd,
            })
            .unwrap();

        let window_socket = Arc::new(window);

        // Notify event thread that this window was created, it will send some default events.
        {
            let mut creates = el.creates.lock().unwrap();
            creates.push_back(window_socket.clone());
        }

        el.event_loop_proxy.wake_socket.wake().unwrap();

        Ok(Self {
            window_socket,
            redraws: el.redraws.clone(),
            destroys: el.destroys.clone(),
            event_loop_proxy: el.event_loop_proxy.clone(),
        })
    }

    fn get_flag(&self, flag: char) -> Result<bool, RequestError> {
        let mut buf: [u8; 4096] = [0; 4096];
        let path = self.window_socket.fpath(&mut buf).map_err(|err| os_error!(format!("{err}")))?;
        let properties = WindowProperties::new(path);
        Ok(properties.flags.contains(flag))
    }

    fn set_flag(&self, flag: char, value: bool) -> Result<(), RequestError> {
        self.window_socket
            .write(format!("F,{flag},{}", if value { 1 } else { 0 }).as_bytes())
            .map_err(|err| os_error!(format!("{err}")))?;
        Ok(())
    }

    #[inline]
    fn raw_window_handle_rwh_06(&self) -> Result<rwh_06::RawWindowHandle, rwh_06::HandleError> {
        let handle = rwh_06::OrbitalWindowHandle::new({
            let window = self.window_socket.fd as *mut _;
            std::ptr::NonNull::new(window).expect("orbital fd should never be null")
        });
        Ok(rwh_06::RawWindowHandle::Orbital(handle))
    }

    #[inline]
    fn raw_display_handle_rwh_06(&self) -> Result<rwh_06::RawDisplayHandle, rwh_06::HandleError> {
        Ok(rwh_06::RawDisplayHandle::Orbital(rwh_06::OrbitalDisplayHandle::new()))
    }
}

impl CoreSurface for Window {
    impl_surface_downcast!(Window);
    
    fn id(&self) -> SurfaceId {
        SurfaceId::from_raw(self.window_socket.fd)
    }

    #[inline]
    fn primary_monitor(&self) -> Option<CoreMonitorHandle> {
        None
    }

    #[inline]
    fn available_monitors(&self) -> Box<dyn Iterator<Item = CoreMonitorHandle>> {
        Box::new(iter::empty())
    }

    #[inline]
    fn current_monitor(&self) -> Option<CoreMonitorHandle> {
        None
    }

    #[inline]
    fn scale_factor(&self) -> f64 {
        1.
    }

    #[inline]
    fn request_redraw(&self) {
        let window_id = self.id();
        let mut redraws = self.redraws.lock().unwrap();
        if !redraws.contains(&window_id) {
            redraws.push_back(window_id);

            self.event_loop_proxy.wake_socket.wake().unwrap();
        }
    }

    #[inline]
    fn pre_present_notify(&self) {}

    #[inline]
    fn surface_size(&self) -> PhysicalSize<u32> {
        let mut buf: [u8; 4096] = [0; 4096];
        let path = self.window_socket.fpath(&mut buf).expect("failed to read properties");
        let properties = WindowProperties::new(path);
        (properties.w, properties.h).into()
    }

    #[inline]
    fn request_surface_size(&self, size: Size) -> Option<PhysicalSize<u32>> {
        let (w, h): (u32, u32) = size.to_physical::<u32>(self.scale_factor()).into();
        self.window_socket.write(format!("S,{w},{h}").as_bytes()).expect("failed to set size");
        None
    }

    #[inline]
    fn set_transparent(&self, transparent: bool) {
        let _ = self.set_flag(ORBITAL_FLAG_TRANSPARENT, transparent);
    }

    #[inline]
    fn set_cursor(&self, _: Cursor) {}

    #[inline]
    fn set_cursor_position(&self, _: Position) -> Result<(), RequestError> {
        Err(NotSupportedError::new("set_cursor_position is not supported").into())
    }

    #[inline]
    fn set_cursor_grab(&self, mode: window::CursorGrabMode) -> Result<(), RequestError> {
        let (grab, relative) = match mode {
            window::CursorGrabMode::None => (false, false),
            window::CursorGrabMode::Confined => (true, false),
            window::CursorGrabMode::Locked => (true, true),
        };
        self.window_socket
            .write(format!("M,G,{}", if grab { 1 } else { 0 }).as_bytes())
            .map_err(|err| os_error!(format!("{err}")))?;
        self.window_socket
            .write(format!("M,R,{}", if relative { 1 } else { 0 }).as_bytes())
            .map_err(|err| os_error!(format!("{err}")))?;
        Ok(())
    }

    #[inline]
    fn set_cursor_visible(&self, visible: bool) {
        let _ = self.window_socket.write(format!("M,C,{}", if visible { 1 } else { 0 }).as_bytes());
    }

    #[inline]
    fn set_cursor_hittest(&self, _hittest: bool) -> Result<(), RequestError> {
        Err(NotSupportedError::new("set_cursor_hittest is not supported").into())
    }

    fn rwh_06_window_handle(&self) -> &dyn rwh_06::HasWindowHandle {
        self
    }

    fn rwh_06_display_handle(&self) -> &dyn rwh_06::HasDisplayHandle {
        self
    }
}

impl CoreWindow for Window {

    #[inline]
    fn reset_dead_keys(&self) {
        // TODO?
    }

    #[inline]
    fn surface_position(&self) -> PhysicalPosition<i32> {
        // TODO: adjust for window decorations
        (0, 0).into()
    }

    #[inline]
    fn outer_position(&self) -> Result<PhysicalPosition<i32>, RequestError> {
        let mut buf: [u8; 4096] = [0; 4096];
        let path = self.window_socket.fpath(&mut buf).expect("failed to read properties");
        let properties = WindowProperties::new(path);
        Ok((properties.x, properties.y).into())
    }

    #[inline]
    fn set_outer_position(&self, position: Position) {
        // TODO: adjust for window decorations
        let (x, y): (i32, i32) = position.to_physical::<i32>(self.scale_factor()).into();
        self.window_socket.write(format!("P,{x},{y}").as_bytes()).expect("failed to set position");
    }

    #[inline]
    fn outer_size(&self) -> PhysicalSize<u32> {
        // TODO: adjust for window decorations
        self.surface_size()
    }

    fn safe_area(&self) -> PhysicalInsets<u32> {
        PhysicalInsets::new(0, 0, 0, 0)
    }

    #[inline]
    fn set_min_surface_size(&self, _: Option<Size>) {}

    #[inline]
    fn set_max_surface_size(&self, _: Option<Size>) {}

    #[inline]
    fn title(&self) -> String {
        let mut buf: [u8; 4096] = [0; 4096];
        let path = self.window_socket.fpath(&mut buf).expect("failed to read properties");
        let properties = WindowProperties::new(path);
        properties.title.to_string()
    }

    #[inline]
    fn set_title(&self, title: &str) {
        self.window_socket.write(format!("T,{title}").as_bytes()).expect("failed to set title");
    }

    #[inline]
    fn set_blur(&self, _blur: bool) {}

    #[inline]
    fn set_visible(&self, visible: bool) {
        let _ = self.set_flag(ORBITAL_FLAG_HIDDEN, !visible);
    }

    #[inline]
    fn is_visible(&self) -> Option<bool> {
        Some(!self.get_flag(ORBITAL_FLAG_HIDDEN).unwrap_or(false))
    }

    #[inline]
    fn surface_resize_increments(&self) -> Option<PhysicalSize<u32>> {
        None
    }

    #[inline]
    fn set_surface_resize_increments(&self, _increments: Option<Size>) {}

    #[inline]
    fn set_resizable(&self, resizeable: bool) {
        let _ = self.set_flag(ORBITAL_FLAG_RESIZABLE, resizeable);
    }

    #[inline]
    fn is_resizable(&self) -> bool {
        self.get_flag(ORBITAL_FLAG_RESIZABLE).unwrap_or(false)
    }

    #[inline]
    fn set_minimized(&self, _minimized: bool) {}

    #[inline]
    fn is_minimized(&self) -> Option<bool> {
        None
    }

    #[inline]
    fn set_maximized(&self, maximized: bool) {
        let _ = self.set_flag(ORBITAL_FLAG_MAXIMIZED, maximized);
    }

    #[inline]
    fn is_maximized(&self) -> bool {
        self.get_flag(ORBITAL_FLAG_MAXIMIZED).unwrap_or(false)
    }

    fn set_fullscreen(&self, _monitor: Option<Fullscreen>) {}

    fn fullscreen(&self) -> Option<Fullscreen> {
        None
    }

    #[inline]
    fn set_decorations(&self, decorations: bool) {
        let _ = self.set_flag(ORBITAL_FLAG_BORDERLESS, !decorations);
    }

    #[inline]
    fn is_decorated(&self) -> bool {
        !self.get_flag(ORBITAL_FLAG_BORDERLESS).unwrap_or(false)
    }

    #[inline]
    fn set_window_level(&self, level: window::WindowLevel) {
        match level {
            window::WindowLevel::AlwaysOnBottom => {
                let _ = self.set_flag(ORBITAL_FLAG_BACK, true);
            },
            window::WindowLevel::Normal => {
                let _ = self.set_flag(ORBITAL_FLAG_BACK, false);
                let _ = self.set_flag(ORBITAL_FLAG_FRONT, false);
            },
            window::WindowLevel::AlwaysOnTop => {
                let _ = self.set_flag(ORBITAL_FLAG_FRONT, true);
            },
        }
    }

    #[inline]
    fn set_window_icon(&self, _window_icon: Option<winit_core::icon::Icon>) {}

    #[inline]
    fn set_ime_cursor_area(&self, _position: Position, _size: Size) {}

    #[inline]
    fn set_ime_allowed(&self, _allowed: bool) {}

    #[inline]
    fn set_ime_purpose(&self, _purpose: ImePurpose) {}

    #[inline]
    fn focus_window(&self) {}

    #[inline]
    fn request_user_attention(&self, _request_type: Option<window::UserAttentionType>) {}

    #[inline]
    fn drag_window(&self) -> Result<(), RequestError> {
        self.window_socket.write(b"D").map_err(|err| os_error!(format!("{err}")))?;
        Ok(())
    }

    #[inline]
    fn drag_resize_window(&self, direction: window::ResizeDirection) -> Result<(), RequestError> {
        let arg = match direction {
            window::ResizeDirection::East => "R",
            window::ResizeDirection::North => "T",
            window::ResizeDirection::NorthEast => "T,R",
            window::ResizeDirection::NorthWest => "T,L",
            window::ResizeDirection::South => "B",
            window::ResizeDirection::SouthEast => "B,R",
            window::ResizeDirection::SouthWest => "B,L",
            window::ResizeDirection::West => "L",
        };
        self.window_socket
            .write(format!("D,{arg}").as_bytes())
            .map_err(|err| os_error!(format!("{err}")))?;
        Ok(())
    }

    #[inline]
    fn show_window_menu(&self, _position: Position) {}

    #[inline]
    fn set_enabled_buttons(&self, _buttons: window::WindowButtons) {}

    #[inline]
    fn enabled_buttons(&self) -> window::WindowButtons {
        window::WindowButtons::all()
    }

    #[inline]
    fn theme(&self) -> Option<window::Theme> {
        None
    }

    #[inline]
    fn has_focus(&self) -> bool {
        false
    }

    #[inline]
    fn set_theme(&self, _theme: Option<window::Theme>) {}

    fn set_content_protected(&self, _protected: bool) {}
}

impl rwh_06::HasWindowHandle for Window {
    fn window_handle(&self) -> Result<rwh_06::WindowHandle<'_>, rwh_06::HandleError> {
        let raw = self.raw_window_handle_rwh_06()?;
        unsafe { Ok(rwh_06::WindowHandle::borrow_raw(raw)) }
    }
}

impl rwh_06::HasDisplayHandle for Window {
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        let raw = self.raw_display_handle_rwh_06()?;
        unsafe { Ok(rwh_06::DisplayHandle::borrow_raw(raw)) }
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        {
            let mut destroys = self.destroys.lock().unwrap();
            destroys.push_back(self.id());
        }

        self.event_loop_proxy.wake_socket.wake().unwrap();
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlatformSpecificWindowAttributes;
