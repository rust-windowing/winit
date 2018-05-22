use super::*;

pub const MWM_HINTS_DECORATIONS: c_ulong = 2;

#[derive(Debug)]
pub enum StateOperation {
    Remove = 0, // _NET_WM_STATE_REMOVE
    Add = 1,    // _NET_WM_STATE_ADD
    _Toggle = 2, // _NET_WM_STATE_TOGGLE
}

impl From<bool> for StateOperation {
    fn from(b: bool) -> Self {
        if b {
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
        unsafe { get_atom(xconn, atom_name) }
            .expect("Failed to get atom for `WindowType`")
    }
}
