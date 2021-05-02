#![cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]

use crate::window::Window;

/// Additional methods on `Window` that are specific to Unix.
pub trait WindowExtUnix {
    /// Returns the `ApplicatonWindow` from gtk crate that is used by this window.
    ///
    /// Returns `None` if the window doesn't use xlib (if it uses wayland for example).
    fn gtk_window(&self) -> &gtk::ApplicationWindow;

    /// Not to display window icon in the task bar.
    fn skip_taskbar(&self);
}

impl WindowExtUnix for Window {
    fn gtk_window(&self) -> &gtk::ApplicationWindow {
        &self.window.window
    }

    fn skip_taskbar(&self) {
        self.window.skip_taskbar()
    }
}
