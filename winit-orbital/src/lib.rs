//! # Orbital / Redox OS
//!
//! Redox OS has some functionality not yet present that will be implemented
//! when its orbital display server provides it.

use std::fs::{File, OpenOptions};
use std::io::{Read, Result, Write};
use std::os::fd::AsRawFd;
use std::{fmt, mem, slice, str};

use libredox::data::TimeSpec;

pub use self::event_loop::{EventLoop, PlatformSpecificEventLoopAttributes};

macro_rules! os_error {
    ($error:expr) => {{ winit_core::error::OsError::new(line!(), file!(), $error) }};
}

pub mod event_loop;
pub mod window;

#[derive(Debug)]
struct RedoxSocket {
    fd: File,
}

impl RedoxSocket {
    fn orbital(properties: &WindowProperties<'_>) -> Result<Self> {
        Self::open_raw(&format!("{properties}"))
    }

    // Paths should be checked to ensure they are actually sockets and not normal files. If a
    // non-socket path is used, it could cause read and write to not function as expected. For
    // example, the seek would change in a potentially unpredictable way if either read or write
    // were called at the same time by multiple threads.
    fn open_raw(path: &str) -> Result<Self> {
        let fd = OpenOptions::new().read(true).write(true).open(path)?;
        Ok(Self { fd })
    }

    fn fd(&self) -> usize {
        self.fd.as_raw_fd() as usize
    }

    fn read(&self, buf: &mut [u8]) -> Result<()> {
        (&self.fd).read_exact(buf)
    }

    fn write(&self, buf: &[u8]) -> Result<()> {
        (&self.fd).write_all(buf)
    }

    fn fpath<'a>(&self, buf: &'a mut [u8]) -> Result<&'a str> {
        let count = libredox::call::fpath(self.fd(), buf)?;
        str::from_utf8(&buf[..count])
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidData))
    }
}

#[derive(Debug)]
struct TimeSocket(RedoxSocket);

impl TimeSocket {
    fn open() -> Result<Self> {
        RedoxSocket::open_raw("/scheme/time/4").map(Self)
    }

    // Read current time.
    fn current_time(&self) -> Result<TimeSpec> {
        let mut timespec: libredox::data::TimeSpec = unsafe { mem::zeroed() };
        let timespec_bytes = unsafe {
            slice::from_raw_parts_mut(
                &mut timespec as *mut _ as *mut u8,
                mem::size_of::<TimeSpec>(),
            )
        };
        self.0.read(timespec_bytes)?;
        Ok(timespec)
    }

    // Write a timeout.
    fn timeout(&self, timespec: &TimeSpec) -> Result<()> {
        let timespec_bytes = unsafe {
            slice::from_raw_parts(timespec as *const _ as *const u8, mem::size_of::<TimeSpec>())
        };
        self.0.write(timespec_bytes)
    }

    // Wake immediately.
    fn wake(&self) -> Result<()> {
        // Writing a default TimeSpec will always trigger a time event.
        let timespec: TimeSpec = unsafe { mem::zeroed() };
        self.timeout(&timespec)
    }
}

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
        // /scheme/orbital/flags/x/y/w/h/t
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

impl fmt::Display for WindowProperties<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "/scheme/orbital/{}/{}/{}/{}/{}/{}",
            self.flags, self.x, self.y, self.w, self.h, self.title
        )
    }
}
