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
            &DropdownMenu => b"_NET_WM_WINDOW_TYPE_DROPDOWN_MENU\0",
            &PopupMenu => b"_NET_WM_WINDOW_TYPE_POPUP_MENU\0",
            &Tooltip => b"_NET_WM_WINDOW_TYPE_TOOLTIP\0",
            &Notification => b"_NET_WM_WINDOW_TYPE_NOTIFICATION\0",
            &Combo => b"_NET_WM_WINDOW_TYPE_COMBO\0",
            &Dnd => b"_NET_WM_WINDOW_TYPE_DND\0",
            &Normal => b"_NET_WM_WINDOW_TYPE_NORMAL\0",
        };
        unsafe { xconn.get_atom_unchecked(atom_name) }
    }
}

pub struct NormalHints<'a> {
    size_hints: XSmartPointer<'a, ffi::XSizeHints>,
}

impl<'a> NormalHints<'a> {
    pub fn new(xconn: &'a XConnection) -> Self {
        NormalHints { size_hints: xconn.alloc_size_hints() }
    }

    pub fn has_flag(&self, flag: c_long) -> bool {
        has_flag(self.size_hints.flags, flag)
    }

    fn getter(&self, flag: c_long, field1: &c_int, field2: &c_int) -> Option<(u32, u32)> {
        if self.has_flag(flag) {
            Some((*field1 as _, *field2 as _))
        } else {
            None
        }
    }

    pub fn get_size(&self) -> Option<(u32, u32)> {
        self.getter(ffi::PSize, &self.size_hints.width, &self.size_hints.height)
    }

    // WARNING: This hint is obsolete
    pub fn set_size(&mut self, size: Option<(u32, u32)>) {
        if let Some((width, height)) = size {
            self.size_hints.flags |= ffi::PSize;
            self.size_hints.width = width as c_int;
            self.size_hints.height = height as c_int;
        } else {
            self.size_hints.flags &= !ffi::PSize;
        }
    }

    pub fn get_max_size(&self) -> Option<(u32, u32)> {
        self.getter(ffi::PMaxSize, &self.size_hints.max_width, &self.size_hints.max_height)
    }

    pub fn set_max_size(&mut self, max_size: Option<(u32, u32)>) {
        if let Some((max_width, max_height)) = max_size {
            self.size_hints.flags |= ffi::PMaxSize;
            self.size_hints.max_width = max_width as c_int;
            self.size_hints.max_height = max_height as c_int;
        } else {
            self.size_hints.flags &= !ffi::PMaxSize;
        }
    }

    pub fn get_min_size(&self) -> Option<(u32, u32)> {
        self.getter(ffi::PMinSize, &self.size_hints.min_width, &self.size_hints.min_height)
    }

    pub fn set_min_size(&mut self, min_size: Option<(u32, u32)>) {
        if let Some((min_width, min_height)) = min_size {
            self.size_hints.flags |= ffi::PMinSize;
            self.size_hints.min_width = min_width as c_int;
            self.size_hints.min_height = min_height as c_int;
        } else {
            self.size_hints.flags &= !ffi::PMinSize;
        }
    }

    pub fn get_resize_increments(&self) -> Option<(u32, u32)> {
        self.getter(ffi::PResizeInc, &self.size_hints.width_inc, &self.size_hints.height_inc)
    }

    pub fn set_resize_increments(&mut self, resize_increments: Option<(u32, u32)>) {
        if let Some((width_inc, height_inc)) = resize_increments {
            self.size_hints.flags |= ffi::PResizeInc;
            self.size_hints.width_inc = width_inc as c_int;
            self.size_hints.height_inc = height_inc as c_int;
        } else {
            self.size_hints.flags &= !ffi::PResizeInc;
        }
    }

    pub fn get_base_size(&self) -> Option<(u32, u32)> {
        self.getter(ffi::PBaseSize, &self.size_hints.base_width, &self.size_hints.base_height)
    }

    pub fn set_base_size(&mut self, base_size: Option<(u32, u32)>) {
        if let Some((base_width, base_height)) = base_size {
            self.size_hints.flags |= ffi::PBaseSize;
            self.size_hints.base_width = base_width as c_int;
            self.size_hints.base_height = base_height as c_int;
        } else {
            self.size_hints.flags &= !ffi::PBaseSize;
        }
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

    pub fn get_normal_hints(&self, window: ffi::Window) -> Result<NormalHints, XError> {
        let size_hints = self.alloc_size_hints();
        let mut supplied_by_user: c_long = unsafe { mem::uninitialized() };
        unsafe {
            (self.xlib.XGetWMNormalHints)(
                self.display,
                window,
                size_hints.ptr,
                &mut supplied_by_user,
            );
        }
        self.check_errors().map(|_| NormalHints { size_hints })
    }

    pub fn set_normal_hints(&self, window: ffi::Window, normal_hints: NormalHints) -> Flusher {
        unsafe {
            (self.xlib.XSetWMNormalHints)(
                self.display,
                window,
                normal_hints.size_hints.ptr,
            );
        }
        Flusher::new(self)
    }
}
