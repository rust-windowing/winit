use std::sync::Arc;

use super::*;
use std::convert::TryInto;
use thiserror::Error;
use xcb_dl_util::hint::{XcbHints, XcbHintsError, XcbSizeHints, XcbSizeHintsError};
use xcb_dl_util::property::XcbGetPropertyError;

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
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
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
    Normal,
}

impl Default for WindowType {
    fn default() -> Self {
        WindowType::Normal
    }
}

impl WindowType {
    pub(crate) fn as_atom(&self, xconn: &Arc<XConnection>) -> ffi::xcb_atom_t {
        use self::WindowType::*;
        let atom_name: &str = match *self {
            Desktop => "_NET_WM_WINDOW_TYPE_DESKTOP",
            Dock => "_NET_WM_WINDOW_TYPE_DOCK",
            Toolbar => "_NET_WM_WINDOW_TYPE_TOOLBAR",
            Menu => "_NET_WM_WINDOW_TYPE_MENU",
            Utility => "_NET_WM_WINDOW_TYPE_UTILITY",
            Splash => "_NET_WM_WINDOW_TYPE_SPLASH",
            Dialog => "_NET_WM_WINDOW_TYPE_DIALOG",
            DropdownMenu => "_NET_WM_WINDOW_TYPE_DROPDOWN_MENU",
            PopupMenu => "_NET_WM_WINDOW_TYPE_POPUP_MENU",
            Tooltip => "_NET_WM_WINDOW_TYPE_TOOLTIP",
            Notification => "_NET_WM_WINDOW_TYPE_NOTIFICATION",
            Combo => "_NET_WM_WINDOW_TYPE_COMBO",
            Dnd => "_NET_WM_WINDOW_TYPE_DND",
            Normal => "_NET_WM_WINDOW_TYPE_NORMAL",
        };
        xconn.get_atom(atom_name)
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

impl MwmHints {
    fn as_array(&self) -> [u32; 5] {
        [
            self.flags,
            self.functions,
            self.decorations,
            self.input_mode,
            self.status,
        ]
    }
}

#[derive(Debug, Error)]
pub enum HintsError {
    #[error("Could not convert the property contents to XcbHints: {0}")]
    Contents(#[from] XcbHintsError),
    #[error("Could not convert the property contents to XcbSizeHints: {0}")]
    SizeContents(#[from] XcbSizeHintsError),
    #[error("Could not retrieve the property: {0}")]
    Property(#[from] XcbGetPropertyError),
    #[error("An xcb error occurred: {0}")]
    Xcb(#[from] XcbError),
}

impl XConnection {
    pub fn get_wm_hints(&self, window: ffi::xcb_window_t) -> Result<XcbHints, HintsError> {
        let prop = self.get_property::<u32>(window, ffi::XCB_ATOM_WM_HINTS, ffi::XCB_ATOM_WM_HINTS);
        let bytes = match prop {
            Ok(b) => b,
            Err(XcbGetPropertyError::Unset) => return Ok(XcbHints::default()),
            Err(e) => return Err(e.into()),
        };
        Ok((&*bytes).try_into()?)
    }

    pub fn set_wm_hints(&self, window: ffi::xcb_window_t, wm_hints: XcbHints) -> XcbPendingCommand {
        self.change_property(
            window,
            ffi::XCB_ATOM_WM_HINTS,
            ffi::XCB_ATOM_WM_HINTS,
            PropMode::Replace,
            wm_hints.as_bytes(),
        )
    }

    pub fn get_normal_hints(&self, window: ffi::xcb_window_t) -> Result<XcbSizeHints, HintsError> {
        let bytes = self.get_property::<u32>(
            window,
            ffi::XCB_ATOM_WM_NORMAL_HINTS,
            ffi::XCB_ATOM_WM_SIZE_HINTS,
        )?;
        Ok((&*bytes).try_into()?)
    }

    pub fn set_normal_hints(
        &self,
        window: ffi::xcb_window_t,
        normal_hints: XcbSizeHints,
    ) -> XcbPendingCommand {
        self.change_property(
            window,
            ffi::XCB_ATOM_WM_NORMAL_HINTS,
            ffi::XCB_ATOM_WM_SIZE_HINTS,
            PropMode::Replace,
            normal_hints.as_bytes(),
        )
    }

    pub fn get_motif_hints(&self, window: ffi::xcb_window_t) -> MotifHints {
        let motif_hints = self.get_atom("_MOTIF_WM_HINTS");

        let mut hints = MotifHints::new();

        if let Ok(props) = self.get_property::<u32>(window, motif_hints, motif_hints) {
            hints.hints.flags = props.get(0).cloned().unwrap_or(0);
            hints.hints.functions = props.get(1).cloned().unwrap_or(0);
            hints.hints.decorations = props.get(2).cloned().unwrap_or(0);
            hints.hints.input_mode = props.get(3).cloned().unwrap_or(0);
            hints.hints.status = props.get(4).cloned().unwrap_or(0);
        }

        hints
    }

    pub fn set_motif_hints(
        &self,
        window: ffi::xcb_window_t,
        hints: &MotifHints,
    ) -> XcbPendingCommand {
        let motif_hints = self.get_atom("_MOTIF_WM_HINTS");

        self.change_property(
            window,
            motif_hints,
            motif_hints,
            PropMode::Replace,
            &hints.hints.as_array(),
        )
        .into()
    }
}
