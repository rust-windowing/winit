#[macro_use]
mod util;

mod app;
mod app_state;
mod cursor;
mod event;
mod event_loop;
mod ffi;
mod menu;
mod monitor;
mod observer;
mod view;
mod window;
mod window_delegate;

pub(crate) use self::cursor::CustomCursor as PlatformCustomCursor;
pub(crate) use self::event::{physicalkey_to_scancode, scancode_to_physicalkey, KeyEventExtra};
pub(crate) use self::event_loop::{
    ActiveEventLoop, EventLoop, EventLoopProxy, OwnedDisplayHandle,
    PlatformSpecificEventLoopAttributes,
};
pub(crate) use self::monitor::{MonitorHandle, VideoModeHandle};
pub(crate) use self::window::Window;
pub(crate) use self::window_delegate::PlatformSpecificWindowAttributes;
pub(crate) use crate::cursor::OnlyCursorImageSource as PlatformCustomCursorSource;
pub(crate) use crate::icon::NoIcon as PlatformIcon;
pub(crate) use crate::platform_impl::Fullscreen;
