#![cfg(free_unix)]

#[cfg(all(not(x11_platform), not(wayland_platform)))]
compile_error!("Please select a feature to build for unix: `x11`, `wayland`");

use std::env;

pub(crate) use winit_common::xkb::{physicalkey_to_scancode, scancode_to_physicalkey};
use winit_core::error::{EventLoopError, NotSupportedError};
use winit_core::event_loop::EventLoopProvider;
#[cfg(wayland_platform)]
pub(crate) use winit_wayland as wayland;
#[cfg(x11_platform)]
pub(crate) use winit_x11 as x11;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Backend {
    #[cfg(x11_platform)]
    X,
    #[cfg(wayland_platform)]
    Wayland,
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PlatformSpecificEventLoopAttributes {
    pub(crate) forced_backend: Option<Backend>,
    pub(crate) any_thread: bool,
}

pub(crate) fn create_event_loop(
    attributes: &mut PlatformSpecificEventLoopAttributes,
) -> Result<Box<dyn EventLoopProvider>, EventLoopError> {
    if !attributes.any_thread && !is_main_thread() {
        panic!(
            "Initializing the event loop outside of the main thread is a significant \
             cross-platform compatibility hazard. If you absolutely need to create an EventLoop \
             on a different thread, you can use the `EventLoopBuilderExtX11::with_any_thread` or \
             `EventLoopBuilderExtWayland::with_any_thread` functions."
        );
    }

    // NOTE: Wayland first because of X11 could be present under Wayland as well. Empty
    // variables are also treated as not set.
    let backend = match (
        attributes.forced_backend,
        env::var("WAYLAND_DISPLAY")
            .ok()
            .filter(|var| !var.is_empty())
            .or_else(|| env::var("WAYLAND_SOCKET").ok())
            .filter(|var| !var.is_empty())
            .is_some(),
        env::var("DISPLAY").map(|var| !var.is_empty()).unwrap_or(false),
    ) {
        // User is forcing a backend.
        (Some(backend), ..) => backend,
        // Wayland is present.
        #[cfg(wayland_platform)]
        (None, true, _) => Backend::Wayland,
        // X11 is present.
        #[cfg(x11_platform)]
        (None, _, true) => Backend::X,
        // No backend is present.
        (_, wayland_display, x11_display) => {
            let msg = if wayland_display && !cfg!(wayland_platform) {
                "DISPLAY is not set; note: enable the `winit/wayland` feature to support Wayland"
            } else if x11_display && !cfg!(x11_platform) {
                "neither WAYLAND_DISPLAY nor WAYLAND_SOCKET is set; note: enable the `winit/x11` \
                 feature to support X11"
            } else {
                "neither WAYLAND_DISPLAY nor WAYLAND_SOCKET nor DISPLAY is set."
            };
            return Err(NotSupportedError::new(msg).into());
        },
    };

    // Create the display based on the backend.
    match backend {
        #[cfg(wayland_platform)]
        Backend::Wayland => Ok(Box::new(wayland::EventLoop::new()?)),
        #[cfg(x11_platform)]
        Backend::X => Ok(Box::new(x11::EventLoop::new()?)),
    }
}

#[cfg(target_os = "linux")]
fn is_main_thread() -> bool {
    rustix::thread::gettid() == rustix::process::getpid()
}

#[cfg(any(target_os = "dragonfly", target_os = "freebsd", target_os = "openbsd"))]
fn is_main_thread() -> bool {
    use libc::pthread_main_np;

    unsafe { pthread_main_np() == 1 }
}

#[cfg(target_os = "netbsd")]
fn is_main_thread() -> bool {
    std::thread::current().name() == Some("main")
}
