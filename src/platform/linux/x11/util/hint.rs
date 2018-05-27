use std::sync::Arc;

use super::*;

pub const MWM_HINTS_DECORATIONS: c_ulong = 2;

#[derive(Debug)]
pub enum StateOperation {
    Remove = 0, // _NET_WM_STATE_REMOVE
    Add = 1,    // _NET_WM_STATE_ADD
    Toggle = 2, // _NET_WM_STATE_TOGGLE
}

impl From<bool> for StateOperation {
    fn from(op: bool) -> Self {
        if op {
            StateOperation::Add
        } else {
            StateOperation::Remove
        }
    }
}

/// X window type. Maps directly to
/// [`_NET_WM_WINDOW_TYPE`](https://specifications.freedesktop.org/wm-spec/1.3/ar01s05.html).
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum WindowType {
    /// A desktop feature. This can include a single window containing desktop icons with the same dimensions as the
    /// screen, allowing the desktop environment to have full control of the desktop, without the need for proxying
    /// root window clicks.
    Desktop,
    /// A dock or panel feature. Typically a Window Manager would keep such windows on top of all other windows.
    Dock,
    /// Toolbar windows. "Torn off" from the main application.
    Toolbar,
    /// Pinnable menu windows. "Torn off" from the main application.
    Menu,
    /// A small persistent utility window, such as a palette or toolbox.
    Utility,
    /// The window is a splash screen displayed as an application is starting up.
    Splash,
    /// This is a dialog window.
    Dialog,
    /// This is a normal, top-level window.
    Normal,
}

impl Default for WindowType {
    fn default() -> Self {
        WindowType::Normal
    }
}

impl WindowType {
    pub(crate) fn as_atom(&self, xconn: &Arc<XConnection>) -> ffi::Atom {
        use self::WindowType::*;
        let atom_name: &[u8] = match self {
            &Desktop => b"_NET_WM_WINDOW_TYPE_DESKTOP\0",
            &Dock => b"_NET_WM_WINDOW_TYPE_DOCK\0",
            &Toolbar => b"_NET_WM_WINDOW_TYPE_TOOLBAR\0",
            &Menu => b"_NET_WM_WINDOW_TYPE_MENU\0",
            &Utility => b"_NET_WM_WINDOW_TYPE_UTILITY\0",
            &Splash => b"_NET_WM_WINDOW_TYPE_SPLASH\0",
            &Dialog => b"_NET_WM_WINDOW_TYPE_DIALOG\0",
            &Normal => b"_NET_WM_WINDOW_TYPE_NORMAL\0",
        };
        unsafe { xconn.get_atom_unchecked(atom_name) }
    }
}

impl XConnection {
    pub fn get_wm_hints(&self, window: ffi::Window) -> Result<XSmartPointer<ffi::XWMHints>, XError> {
        let wm_hints = unsafe { (self.xlib.XGetWMHints)(self.display, window) };
        self.check_errors()?;
        let wm_hints = if wm_hints.is_null() {
            self.alloc_wm_hints()
        } else {
            XSmartPointer::new(self, wm_hints).unwrap()
        };
        Ok(wm_hints)
    }

    pub fn set_wm_hints(&self, window: ffi::Window, wm_hints: XSmartPointer<ffi::XWMHints>) -> Flusher {
        unsafe {
            (self.xlib.XSetWMHints)(
                self.display,
                window,
                wm_hints.ptr,
            );
        }
        Flusher::new(self)
    }
}
