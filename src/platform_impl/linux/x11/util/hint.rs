use std::sync::Arc;

use super::*;

#[derive(Debug)]
#[allow(dead_code)]
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
/// [`_NET_WM_WINDOW_TYPE`](https://specifications.freedesktop.org/wm-spec/wm-spec-1.5.html).
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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
    /// A dropdown menu that usually appears when the user clicks on an item in a menu bar.
    /// This property is typically used on override-redirect windows.
    DropdownMenu,
    /// A popup menu that usually appears when the user right clicks on an object.
    /// This property is typically used on override-redirect windows.
    PopupMenu,
    /// A tooltip window. Usually used to show additional information when hovering over an object with the cursor.
    /// This property is typically used on override-redirect windows.
    Tooltip,
    /// The window is a notification.
    /// This property is typically used on override-redirect windows.
    Notification,
    /// This should be used on the windows that are popped up by combo boxes.
    /// This property is typically used on override-redirect windows.
    Combo,
    /// This indicates the the window is being dragged.
    /// This property is typically used on override-redirect windows.
    Dnd,
    /// This is a normal, top-level window.
    #[default]
    Normal,
}

impl WindowType {
    pub(crate) fn as_atom(&self, xconn: &Arc<XConnection>) -> xproto::Atom {
        use self::WindowType::*;
        let atom_name = match *self {
            Desktop => _NET_WM_WINDOW_TYPE_DESKTOP,
            Dock => _NET_WM_WINDOW_TYPE_DOCK,
            Toolbar => _NET_WM_WINDOW_TYPE_TOOLBAR,
            Menu => _NET_WM_WINDOW_TYPE_MENU,
            Utility => _NET_WM_WINDOW_TYPE_UTILITY,
            Splash => _NET_WM_WINDOW_TYPE_SPLASH,
            Dialog => _NET_WM_WINDOW_TYPE_DIALOG,
            DropdownMenu => _NET_WM_WINDOW_TYPE_DROPDOWN_MENU,
            PopupMenu => _NET_WM_WINDOW_TYPE_POPUP_MENU,
            Tooltip => _NET_WM_WINDOW_TYPE_TOOLTIP,
            Notification => _NET_WM_WINDOW_TYPE_NOTIFICATION,
            Combo => _NET_WM_WINDOW_TYPE_COMBO,
            Dnd => _NET_WM_WINDOW_TYPE_DND,
            Normal => _NET_WM_WINDOW_TYPE_NORMAL,
        };

        let atoms = xconn.atoms();
        atoms[atom_name]
    }
}

pub struct MotifHints {
    hints: MwmHints,
}

struct MwmHints {
    flags: u32,
    functions: u32,
    decorations: u32,
    input_mode: u32,
    status: u32,
}

#[allow(dead_code)]
mod mwm {
    // Motif WM hints are obsolete, but still widely supported.
    // https://stackoverflow.com/a/1909708
    pub const MWM_HINTS_FUNCTIONS: u32 = 1 << 0;
    pub const MWM_HINTS_DECORATIONS: u32 = 1 << 1;

    pub const MWM_FUNC_ALL: u32 = 1 << 0;
    pub const MWM_FUNC_RESIZE: u32 = 1 << 1;
    pub const MWM_FUNC_MOVE: u32 = 1 << 2;
    pub const MWM_FUNC_MINIMIZE: u32 = 1 << 3;
    pub const MWM_FUNC_MAXIMIZE: u32 = 1 << 4;
    pub const MWM_FUNC_CLOSE: u32 = 1 << 5;
}

impl MotifHints {
    pub fn new() -> MotifHints {
        MotifHints {
            hints: MwmHints {
                flags: 0,
                functions: 0,
                decorations: 0,
                input_mode: 0,
                status: 0,
            },
        }
    }

    pub fn set_decorations(&mut self, decorations: bool) {
        self.hints.flags |= mwm::MWM_HINTS_DECORATIONS;
        self.hints.decorations = decorations as u32;
    }

    pub fn set_maximizable(&mut self, maximizable: bool) {
        if maximizable {
            self.add_func(mwm::MWM_FUNC_MAXIMIZE);
        } else {
            self.remove_func(mwm::MWM_FUNC_MAXIMIZE);
        }
    }

    fn add_func(&mut self, func: u32) {
        if self.hints.flags & mwm::MWM_HINTS_FUNCTIONS != 0 {
            if self.hints.functions & mwm::MWM_FUNC_ALL != 0 {
                self.hints.functions &= !func;
            } else {
                self.hints.functions |= func;
            }
        }
    }

    fn remove_func(&mut self, func: u32) {
        if self.hints.flags & mwm::MWM_HINTS_FUNCTIONS == 0 {
            self.hints.flags |= mwm::MWM_HINTS_FUNCTIONS;
            self.hints.functions = mwm::MWM_FUNC_ALL;
        }

        if self.hints.functions & mwm::MWM_FUNC_ALL != 0 {
            self.hints.functions |= func;
        } else {
            self.hints.functions &= !func;
        }
    }
}

impl Default for MotifHints {
    fn default() -> Self {
        Self::new()
    }
}

impl XConnection {
    pub fn get_motif_hints(&self, window: xproto::Window) -> MotifHints {
        let atoms = self.atoms();
        let motif_hints = atoms[_MOTIF_WM_HINTS];

        let mut hints = MotifHints::new();

        if let Ok(props) = self.get_property::<u32>(window, motif_hints, motif_hints) {
            hints.hints.flags = props.first().cloned().unwrap_or(0);
            hints.hints.functions = props.get(1).cloned().unwrap_or(0);
            hints.hints.decorations = props.get(2).cloned().unwrap_or(0);
            hints.hints.input_mode = props.get(3).cloned().unwrap_or(0);
            hints.hints.status = props.get(4).cloned().unwrap_or(0);
        }

        hints
    }

    #[allow(clippy::unnecessary_cast)]
    pub fn set_motif_hints(
        &self,
        window: xproto::Window,
        hints: &MotifHints,
    ) -> Result<VoidCookie<'_>, X11Error> {
        let atoms = self.atoms();
        let motif_hints = atoms[_MOTIF_WM_HINTS];

        let hints_data: [u32; 5] = [
            hints.hints.flags as u32,
            hints.hints.functions as u32,
            hints.hints.decorations as u32,
            hints.hints.input_mode as u32,
            hints.hints.status as u32,
        ];

        self.change_property(
            window,
            motif_hints,
            motif_hints,
            xproto::PropMode::REPLACE,
            &hints_data,
        )
    }
}
