//! # Orbital / Redox OS
//!
//! Redox OS has some functionality not yet present that will be implemented
//! when its orbital display server provides it.

#[macro_use]
mod util;
mod event_loop;
mod window;

pub use self::event_loop::{EventLoop, PlatformSpecificEventLoopAttributes};
