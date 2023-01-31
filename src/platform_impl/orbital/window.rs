use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use raw_window_handle::{
    OrbitalDisplayHandle, OrbitalWindowHandle, RawDisplayHandle, RawWindowHandle,
};

use crate::{
    dpi::{PhysicalPosition, PhysicalSize, Position, Size},
    error,
    platform_impl::Fullscreen,
    window,
    window::ImePurpose,
};

use super::{
    EventLoopWindowTarget, MonitorHandle, PlatformSpecificWindowBuilderAttributes, RedoxSocket,
    TimeSocket, WindowId, WindowProperties,
};

// These values match the values uses in the `window_new` function in orbital:
// https://gitlab.redox-os.org/redox-os/orbital/-/blob/master/src/scheme.rs
const ORBITAL_FLAG_ASYNC: char = 'a';
const ORBITAL_FLAG_BACK: char = 'b';
const ORBITAL_FLAG_FRONT: char = 'f';
const ORBITAL_FLAG_BORDERLESS: char = 'l';
const ORBITAL_FLAG_RESIZABLE: char = 'r';
const ORBITAL_FLAG_TRANSPARENT: char = 't';

pub struct Window {
    window_socket: Arc<RedoxSocket>,
    redraws: Arc<Mutex<VecDeque<WindowId>>>,
    destroys: Arc<Mutex<VecDeque<WindowId>>>,
    wake_socket: Arc<TimeSocket>,
}

impl Window {
    pub(crate) fn new<T: 'static>(
        el: &EventLoopWindowTarget<T>,
        attrs: window::WindowAttributes,
        _: PlatformSpecificWindowBuilderAttributes,
    ) -> Result<Self, error::OsError> {
        let scale = MonitorHandle.scale_factor();

        let (x, y) = if let Some(pos) = attrs.position {
            pos.to_physical::<i32>(scale).into()
        } else {
            // These coordinates are a special value to center the window.
            (-1, -1)
        };

        let (w, h): (u32, u32) = if let Some(size) = attrs.inner_size {
            size.to_physical::<u32>(scale).into()
        } else {
            (1024, 768)
        };

        //TODO: min/max inner_size

        // Async by default.
        let mut flag_str = ORBITAL_FLAG_ASYNC.to_string();

        if attrs.resizable {
            flag_str.push(ORBITAL_FLAG_RESIZABLE);
        }

        //TODO: maximized, fullscreen, visible

        if attrs.transparent {
            flag_str.push(ORBITAL_FLAG_TRANSPARENT);
        }

        if !attrs.decorations {
            flag_str.push(ORBITAL_FLAG_BORDERLESS);
        }

        match attrs.window_level {
            window::WindowLevel::AlwaysOnBottom => {
                flag_str.push(ORBITAL_FLAG_BACK);
            }
            window::WindowLevel::Normal => {}
            window::WindowLevel::AlwaysOnTop => {
                flag_str.push(ORBITAL_FLAG_FRONT);
            }
        }

        //TODO: window_icon

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

        el.wake_socket.wake().unwrap();

        Ok(Self {
            window_socket,
            redraws: el.redraws.clone(),
            destroys: el.destroys.clone(),
            wake_socket: el.wake_socket.clone(),
        })
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        WindowId {
            fd: self.window_socket.fd as u64,
        }
    }

    #[inline]
    pub fn primary_monitor(&self) -> Option<MonitorHandle> {
        Some(MonitorHandle)
    }

    #[inline]
    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        let mut v = VecDeque::with_capacity(1);
        v.push_back(MonitorHandle);
        v
    }

    #[inline]
    pub fn current_monitor(&self) -> Option<MonitorHandle> {
        Some(MonitorHandle)
    }

    #[inline]
    pub fn scale_factor(&self) -> f64 {
        MonitorHandle.scale_factor()
    }

    #[inline]
    pub fn request_redraw(&self) {
        let window_id = self.id();
        let mut redraws = self.redraws.lock().unwrap();
        if !redraws.contains(&window_id) {
            redraws.push_back(window_id);

            self.wake_socket.wake().unwrap();
        }
    }

    #[inline]
    pub fn inner_position(&self) -> Result<PhysicalPosition<i32>, error::NotSupportedError> {
        let mut buf: [u8; 4096] = [0; 4096];
        let path = self
            .window_socket
            .fpath(&mut buf)
            .expect("failed to read properties");
        let properties = WindowProperties::new(path);
        Ok((properties.x, properties.y).into())
    }

    #[inline]
    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, error::NotSupportedError> {
        //TODO: adjust for window decorations
        self.inner_position()
    }

    #[inline]
    pub fn set_outer_position(&self, position: Position) {
        //TODO: adjust for window decorations
        let (x, y): (i32, i32) = position.to_physical::<i32>(self.scale_factor()).into();
        self.window_socket
            .write(format!("P,{x},{y}").as_bytes())
            .expect("failed to set position");
    }

    #[inline]
    pub fn inner_size(&self) -> PhysicalSize<u32> {
        let mut buf: [u8; 4096] = [0; 4096];
        let path = self
            .window_socket
            .fpath(&mut buf)
            .expect("failed to read properties");
        let properties = WindowProperties::new(path);
        (properties.w, properties.h).into()
    }

    #[inline]
    pub fn set_inner_size(&self, size: Size) {
        let (w, h): (u32, u32) = size.to_physical::<u32>(self.scale_factor()).into();
        self.window_socket
            .write(format!("S,{w},{h}").as_bytes())
            .expect("failed to set size");
    }

    #[inline]
    pub fn outer_size(&self) -> PhysicalSize<u32> {
        //TODO: adjust for window decorations
        self.inner_size()
    }

    #[inline]
    pub fn set_min_inner_size(&self, _: Option<Size>) {}

    #[inline]
    pub fn set_max_inner_size(&self, _: Option<Size>) {}

    #[inline]
    pub fn title(&self) -> String {
        let mut buf: [u8; 4096] = [0; 4096];
        let path = self
            .window_socket
            .fpath(&mut buf)
            .expect("failed to read properties");
        let properties = WindowProperties::new(path);
        properties.title.to_string()
    }

    #[inline]
    pub fn set_title(&self, title: &str) {
        self.window_socket
            .write(format!("T,{title}").as_bytes())
            .expect("failed to set title");
    }

    #[inline]
    pub fn set_transparent(&self, _transparent: bool) {}

    #[inline]
    pub fn set_visible(&self, _visibility: bool) {}

    #[inline]
    pub fn is_visible(&self) -> Option<bool> {
        None
    }

    #[inline]
    pub fn resize_increments(&self) -> Option<PhysicalSize<u32>> {
        None
    }

    #[inline]
    pub fn set_resize_increments(&self, _increments: Option<Size>) {}

    #[inline]
    pub fn set_resizable(&self, _resizeable: bool) {}

    #[inline]
    pub fn is_resizable(&self) -> bool {
        let mut buf: [u8; 4096] = [0; 4096];
        let path = self
            .window_socket
            .fpath(&mut buf)
            .expect("failed to read properties");
        let properties = WindowProperties::new(path);
        properties.flags.contains(ORBITAL_FLAG_RESIZABLE)
    }

    #[inline]
    pub fn set_minimized(&self, _minimized: bool) {}

    #[inline]
    pub fn is_minimized(&self) -> Option<bool> {
        None
    }

    #[inline]
    pub fn set_maximized(&self, _maximized: bool) {}

    #[inline]
    pub fn is_maximized(&self) -> bool {
        false
    }

    #[inline]
    pub(crate) fn set_fullscreen(&self, _monitor: Option<Fullscreen>) {}

    #[inline]
    pub(crate) fn fullscreen(&self) -> Option<Fullscreen> {
        None
    }

    #[inline]
    pub fn set_decorations(&self, _decorations: bool) {}

    #[inline]
    pub fn is_decorated(&self) -> bool {
        let mut buf: [u8; 4096] = [0; 4096];
        let path = self
            .window_socket
            .fpath(&mut buf)
            .expect("failed to read properties");
        let properties = WindowProperties::new(path);
        !properties.flags.contains(ORBITAL_FLAG_BORDERLESS)
    }

    #[inline]
    pub fn set_window_level(&self, _level: window::WindowLevel) {}

    #[inline]
    pub fn set_window_icon(&self, _window_icon: Option<crate::icon::Icon>) {}

    #[inline]
    pub fn set_ime_position(&self, _position: Position) {}

    #[inline]
    pub fn set_ime_allowed(&self, _allowed: bool) {}

    #[inline]
    pub fn set_ime_purpose(&self, _purpose: ImePurpose) {}

    #[inline]
    pub fn focus_window(&self) {}

    #[inline]
    pub fn request_user_attention(&self, _request_type: Option<window::UserAttentionType>) {}

    #[inline]
    pub fn set_cursor_icon(&self, _: window::CursorIcon) {}

    #[inline]
    pub fn set_cursor_position(&self, _: Position) -> Result<(), error::ExternalError> {
        Err(error::ExternalError::NotSupported(
            error::NotSupportedError::new(),
        ))
    }

    #[inline]
    pub fn set_cursor_grab(&self, _: window::CursorGrabMode) -> Result<(), error::ExternalError> {
        Err(error::ExternalError::NotSupported(
            error::NotSupportedError::new(),
        ))
    }

    #[inline]
    pub fn set_cursor_visible(&self, _: bool) {}

    #[inline]
    pub fn drag_window(&self) -> Result<(), error::ExternalError> {
        Err(error::ExternalError::NotSupported(
            error::NotSupportedError::new(),
        ))
    }

    #[inline]
    pub fn drag_resize_window(
        &self,
        _direction: window::ResizeDirection,
    ) -> Result<(), error::ExternalError> {
        Err(error::ExternalError::NotSupported(
            error::NotSupportedError::new(),
        ))
    }

    #[inline]
    pub fn set_cursor_hittest(&self, _hittest: bool) -> Result<(), error::ExternalError> {
        Err(error::ExternalError::NotSupported(
            error::NotSupportedError::new(),
        ))
    }

    #[inline]
    pub fn raw_window_handle(&self) -> RawWindowHandle {
        let mut handle = OrbitalWindowHandle::empty();
        handle.window = self.window_socket.fd as *mut _;
        RawWindowHandle::Orbital(handle)
    }

    #[inline]
    pub fn raw_display_handle(&self) -> RawDisplayHandle {
        RawDisplayHandle::Orbital(OrbitalDisplayHandle::empty())
    }

    #[inline]
    pub fn set_enabled_buttons(&self, _buttons: window::WindowButtons) {}

    #[inline]
    pub fn enabled_buttons(&self) -> window::WindowButtons {
        window::WindowButtons::all()
    }

    #[inline]
    pub fn theme(&self) -> Option<window::Theme> {
        None
    }

    #[inline]
    pub fn has_focus(&self) -> bool {
        false
    }

    #[inline]
    pub fn set_theme(&self, _theme: Option<window::Theme>) {}
}

impl Drop for Window {
    fn drop(&mut self) {
        {
            let mut destroys = self.destroys.lock().unwrap();
            destroys.push_back(self.id());
        }

        self.wake_socket.wake().unwrap();
    }
}
