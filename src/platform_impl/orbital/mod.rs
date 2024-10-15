#![cfg(target_os = "redox")]

use std::num::{NonZeroU16, NonZeroU32};
use std::{fmt, str};

use smol_str::SmolStr;

pub(crate) use self::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy, OwnedDisplayHandle};
use crate::dpi::{PhysicalPosition, PhysicalSize};
use crate::keyboard::Key;
mod event_loop;

pub use self::window::Window;
mod window;

pub(crate) use crate::cursor::{
    NoCustomCursor as PlatformCustomCursor, NoCustomCursor as PlatformCustomCursorSource,
};
pub(crate) use crate::icon::NoIcon as PlatformIcon;

struct RedoxSocket {
    fd: usize,
}

impl RedoxSocket {
    fn event() -> syscall::Result<Self> {
        Self::open_raw("event:")
    }

    fn orbital(properties: &WindowProperties<'_>) -> syscall::Result<Self> {
        Self::open_raw(&format!("{properties}"))
    }

    // Paths should be checked to ensure they are actually sockets and not normal files. If a
    // non-socket path is used, it could cause read and write to not function as expected. For
    // example, the seek would change in a potentially unpredictable way if either read or write
    // were called at the same time by multiple threads.
    fn open_raw(path: &str) -> syscall::Result<Self> {
        let fd = syscall::open(path, syscall::O_RDWR | syscall::O_CLOEXEC)?;
        Ok(Self { fd })
    }

    fn read(&self, buf: &mut [u8]) -> syscall::Result<()> {
        let count = syscall::read(self.fd, buf)?;
        if count == buf.len() {
            Ok(())
        } else {
            Err(syscall::Error::new(syscall::EINVAL))
        }
    }

    fn write(&self, buf: &[u8]) -> syscall::Result<()> {
        let count = syscall::write(self.fd, buf)?;
        if count == buf.len() {
            Ok(())
        } else {
            Err(syscall::Error::new(syscall::EINVAL))
        }
    }

    fn fpath<'a>(&self, buf: &'a mut [u8]) -> syscall::Result<&'a str> {
        let count = syscall::fpath(self.fd, buf)?;
        str::from_utf8(&buf[..count]).map_err(|_err| syscall::Error::new(syscall::EINVAL))
    }
}

impl Drop for RedoxSocket {
    fn drop(&mut self) {
        let _ = syscall::close(self.fd);
    }
}

pub struct TimeSocket(RedoxSocket);

impl TimeSocket {
    fn open() -> syscall::Result<Self> {
        RedoxSocket::open_raw("time:4").map(Self)
    }

    // Read current time.
    fn current_time(&self) -> syscall::Result<syscall::TimeSpec> {
        let mut timespec = syscall::TimeSpec::default();
        self.0.read(&mut timespec)?;
        Ok(timespec)
    }

    // Write a timeout.
    fn timeout(&self, timespec: &syscall::TimeSpec) -> syscall::Result<()> {
        self.0.write(timespec)
    }

    // Wake immediately.
    fn wake(&self) -> syscall::Result<()> {
        // Writing a default TimeSpec will always trigger a time event.
        self.timeout(&syscall::TimeSpec::default())
    }
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PlatformSpecificEventLoopAttributes {}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlatformSpecificWindowAttributes;

struct WindowProperties<'a> {
    flags: &'a str,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    title: &'a str,
}

impl<'a> WindowProperties<'a> {
    fn new(path: &'a str) -> Self {
        // orbital:flags/x/y/w/h/t
        let mut parts = path.splitn(6, '/');
        let flags = parts.next().unwrap_or("");
        let x = parts.next().map_or(0, |part| part.parse::<i32>().unwrap_or(0));
        let y = parts.next().map_or(0, |part| part.parse::<i32>().unwrap_or(0));
        let w = parts.next().map_or(0, |part| part.parse::<u32>().unwrap_or(0));
        let h = parts.next().map_or(0, |part| part.parse::<u32>().unwrap_or(0));
        let title = parts.next().unwrap_or("");
        Self { flags, x, y, w, h, title }
    }
}

impl<'a> fmt::Display for WindowProperties<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "orbital:{}/{}/{}/{}/{}/{}",
            self.flags, self.x, self.y, self.w, self.h, self.title
        )
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MonitorHandle;

impl MonitorHandle {
    pub fn name(&self) -> Option<String> {
        None
    }

    pub fn position(&self) -> Option<PhysicalPosition<i32>> {
        None
    }

    pub fn scale_factor(&self) -> f64 {
        1.0 // TODO
    }

    pub fn current_video_mode(&self) -> Option<VideoModeHandle> {
        // (it is guaranteed to support 32 bit color though)
        Some(VideoModeHandle { monitor: self.clone() })
    }

    pub fn video_modes(&self) -> impl Iterator<Item = VideoModeHandle> {
        self.current_video_mode().into_iter()
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct VideoModeHandle {
    monitor: MonitorHandle,
}

impl VideoModeHandle {
    pub fn size(&self) -> PhysicalSize<u32> {
        // TODO
        PhysicalSize::default()
    }

    pub fn bit_depth(&self) -> Option<NonZeroU16> {
        None
    }

    pub fn refresh_rate_millihertz(&self) -> Option<NonZeroU32> {
        // TODO
        None
    }

    pub fn monitor(&self) -> MonitorHandle {
        self.monitor.clone()
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct KeyEventExtra {
    pub key_without_modifiers: Key,
    pub text_with_all_modifiers: Option<SmolStr>,
}
