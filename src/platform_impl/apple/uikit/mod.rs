#![allow(clippy::let_unit_value)]

mod app_state;
mod event_loop;
mod monitor;
mod view;
mod view_controller;
mod window;

use std::fmt;

pub(crate) use self::event_loop::{
    ActiveEventLoop, EventLoop, PlatformSpecificEventLoopAttributes,
};
pub(crate) use self::monitor::MonitorHandle;
pub(crate) use self::window::{PlatformSpecificWindowAttributes, Window};
pub(crate) use crate::cursor::{
    NoCustomCursor as PlatformCustomCursor, NoCustomCursor as PlatformCustomCursorSource,
};
pub(crate) use crate::icon::NoIcon as PlatformIcon;

#[derive(Debug)]
pub enum OsError {}

impl fmt::Display for OsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "os error")
    }
}
