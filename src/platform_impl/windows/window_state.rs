use crate::{
    dpi::{PhysicalPosition, Size},
    event::ModifiersState,
    icon::Icon,
    platform_impl::platform::{event_loop, util},
    window::{CursorIcon, Fullscreen, Theme, WindowAttributes},
};
use parking_lot::MutexGuard;
use std::{io, ptr};
use winapi::{
    shared::{
        minwindef::DWORD,
        windef::{HWND, RECT},
    },
    um::winuser,
};

/// Contains information about states and the window that the callback is going to use.
pub struct WindowState {
    pub mouse: MouseProperties,

    /// Used by `WM_GETMINMAXINFO`.
    pub min_size: Option<Size>,
    pub max_size: Option<Size>,

    pub window_icon: Option<Icon>,
    pub taskbar_icon: Option<Icon>,

    pub saved_window: Option<SavedWindow>,
    pub scale_factor: f64,

    pub modifiers_state: ModifiersState,
    pub fullscreen: Option<Fullscreen>,
    pub current_theme: Theme,
    pub preferred_theme: Option<Theme>,
    pub high_surrogate: Option<u16>,
    pub window_flags: WindowFlags,
}

#[derive(Clone)]
pub struct SavedWindow {
    pub placement: winuser::WINDOWPLACEMENT,
}

#[derive(Clone)]
pub struct MouseProperties {
    pub cursor: CursorIcon,
    pub capture_count: u32,
    cursor_flags: CursorFlags,
    pub last_position: Option<PhysicalPosition<f64>>,
}

bitflags! {
    pub struct CursorFlags: u8 {
        const GRABBED   = 1 << 0;
        const HIDDEN    = 1 << 1;
        const IN_WINDOW = 1 << 2;
    }
}
bitflags! {
    pub struct WindowFlags: u32 {
        const RESIZABLE      = 1 << 0;
        const DECORATIONS    = 1 << 1;
        const VISIBLE        = 1 << 2;
        const ON_TASKBAR     = 1 << 3;
        const ALWAYS_ON_TOP  = 1 << 4;
        const NO_BACK_BUFFER = 1 << 5;
        const TRANSPARENT    = 1 << 6;
        const CHILD          = 1 << 7;
        const MAXIMIZED      = 1 << 8;
        const POPUP          = 1 << 14;

        /// Marker flag for fullscreen. Should always match `WindowState::fullscreen`, but is
        /// included here to make masking easier.
        const MARKER_EXCLUSIVE_FULLSCREEN = 1 << 9;
        const MARKER_BORDERLESS_FULLSCREEN = 1 << 13;

        /// The `WM_SIZE` event contains some parameters that can effect the state of `WindowFlags`.
        /// In most cases, it's okay to let those parameters change the state. However, when we're
        /// running the `WindowFlags::apply_diff` function, we *don't* want those parameters to
        /// effect our stored state, because the purpose of `apply_diff` is to update the actual
        /// window's state to match our stored state. This controls whether to accept those changes.
        const MARKER_RETAIN_STATE_ON_SIZE = 1 << 10;

        const MARKER_IN_SIZE_MOVE = 1 << 11;

        const MINIMIZED = 1 << 12;

        const EXCLUSIVE_FULLSCREEN_OR_MASK = WindowFlags::ALWAYS_ON_TOP.bits;
        const NO_DECORATIONS_AND_MASK = !WindowFlags::RESIZABLE.bits;
        const INVISIBLE_AND_MASK = !WindowFlags::MAXIMIZED.bits;
    }
}

impl WindowState {
    pub fn new(
        attributes: &WindowAttributes,
        taskbar_icon: Option<Icon>,
        scale_factor: f64,
        current_theme: Theme,
        preferred_theme: Option<Theme>,
    ) -> WindowState {
        WindowState {
            mouse: MouseProperties {
                cursor: CursorIcon::default(),
                capture_count: 0,
                cursor_flags: CursorFlags::empty(),
                last_position: None,
            },

            min_size: attributes.min_inner_size,
            max_size: attributes.max_inner_size,

            window_icon: attributes.window_icon.clone(),
            taskbar_icon,

            saved_window: None,
            scale_factor,

            modifiers_state: ModifiersState::default(),
            fullscreen: None,
            current_theme,
            preferred_theme,
            high_surrogate: None,
            window_flags: WindowFlags::empty(),
        }
    }

    pub fn window_flags(&self) -> WindowFlags {
        self.window_flags
    }

    pub fn set_window_flags<F>(mut this: MutexGuard<'_, Self>, window: HWND, f: F)
    where
        F: FnOnce(&mut WindowFlags),
    {
        let old_flags = this.window_flags;
        f(&mut this.window_flags);
        let new_flags = this.window_flags;

        drop(this);
        old_flags.apply_diff(window, new_flags);
    }

    pub fn set_window_flags_in_place<F>(&mut self, f: F)
    where
        F: FnOnce(&mut WindowFlags),
    {
        f(&mut self.window_flags);
    }
}

impl MouseProperties {
    pub fn cursor_flags(&self) -> CursorFlags {
        self.cursor_flags
    }

    pub fn set_cursor_flags<F>(&mut self, window: HWND, f: F) -> Result<(), io::Error>
    where
        F: FnOnce(&mut CursorFlags),
    {
        let old_flags = self.cursor_flags;
        f(&mut self.cursor_flags);
        match self.cursor_flags.refresh_os_cursor(window) {
            Ok(()) => (),
            Err(e) => {
                self.cursor_flags = old_flags;
                return Err(e);
            }
        }

        Ok(())
    }
}

impl WindowFlags {
    fn mask(mut self) -> WindowFlags {
        if self.contains(WindowFlags::MARKER_EXCLUSIVE_FULLSCREEN) {
            self |= WindowFlags::EXCLUSIVE_FULLSCREEN_OR_MASK;
        }
        if !self.contains(WindowFlags::VISIBLE) {
            self &= WindowFlags::INVISIBLE_AND_MASK;
        }
        if !self.contains(WindowFlags::DECORATIONS) {
            self &= WindowFlags::NO_DECORATIONS_AND_MASK;
        }
        self
    }

    pub fn to_window_styles(self) -> (DWORD, DWORD) {
        use winapi::um::winuser::*;

        let (mut style, mut style_ex) = (0, 0);

        if self.contains(WindowFlags::RESIZABLE) {
            style |= WS_SIZEBOX | WS_MAXIMIZEBOX;
        }
        if self.contains(WindowFlags::DECORATIONS) {
            style |= WS_CAPTION | WS_MINIMIZEBOX | WS_BORDER;
            style_ex = WS_EX_WINDOWEDGE;
        }
        if self.contains(WindowFlags::VISIBLE) {
            style |= WS_VISIBLE;
        }
        if self.contains(WindowFlags::ON_TASKBAR) {
            style_ex |= WS_EX_APPWINDOW;
        }
        if self.contains(WindowFlags::ALWAYS_ON_TOP) {
            style_ex |= WS_EX_TOPMOST;
        }
        if self.contains(WindowFlags::NO_BACK_BUFFER) {
            style_ex |= WS_EX_NOREDIRECTIONBITMAP;
        }
        if self.contains(WindowFlags::CHILD) {
            style |= WS_CHILD; // This is incompatible with WS_POPUP if that gets added eventually.
        }
        if self.contains(WindowFlags::POPUP) {
            style |= WS_POPUP;
        }
        if self.contains(WindowFlags::MINIMIZED) {
            style |= WS_MINIMIZE;
        }
        if self.contains(WindowFlags::MAXIMIZED) {
            style |= WS_MAXIMIZE;
        }

        style |= WS_CLIPSIBLINGS | WS_CLIPCHILDREN | WS_SYSMENU;
        style_ex |= WS_EX_ACCEPTFILES;

        if self.intersects(
            WindowFlags::MARKER_EXCLUSIVE_FULLSCREEN | WindowFlags::MARKER_BORDERLESS_FULLSCREEN,
        ) {
            style &= !WS_OVERLAPPEDWINDOW;
        }

        (style, style_ex)
    }

    /// Adjust the window client rectangle to the return value, if present.
    fn apply_diff(mut self, window: HWND, mut new: WindowFlags) {
        self = self.mask();
        new = new.mask();

        let diff = self ^ new;
        if diff == WindowFlags::empty() {
            return;
        }

        if diff.contains(WindowFlags::VISIBLE) {
            unsafe {
                winuser::ShowWindow(
                    window,
                    match new.contains(WindowFlags::VISIBLE) {
                        true => winuser::SW_SHOW,
                        false => winuser::SW_HIDE,
                    },
                );
            }
        }
        if diff.contains(WindowFlags::ALWAYS_ON_TOP) {
            unsafe {
                winuser::SetWindowPos(
                    window,
                    match new.contains(WindowFlags::ALWAYS_ON_TOP) {
                        true => winuser::HWND_TOPMOST,
                        false => winuser::HWND_NOTOPMOST,
                    },
                    0,
                    0,
                    0,
                    0,
                    winuser::SWP_ASYNCWINDOWPOS
                        | winuser::SWP_NOMOVE
                        | winuser::SWP_NOSIZE
                        | winuser::SWP_NOACTIVATE,
                );
                winuser::InvalidateRgn(window, ptr::null_mut(), 0);
            }
        }

        if diff.contains(WindowFlags::MAXIMIZED) || new.contains(WindowFlags::MAXIMIZED) {
            unsafe {
                winuser::ShowWindow(
                    window,
                    match new.contains(WindowFlags::MAXIMIZED) {
                        true => winuser::SW_MAXIMIZE,
                        false => winuser::SW_RESTORE,
                    },
                );
            }
        }

        // Minimize operations should execute after maximize for proper window animations
        if diff.contains(WindowFlags::MINIMIZED) {
            unsafe {
                winuser::ShowWindow(
                    window,
                    match new.contains(WindowFlags::MINIMIZED) {
                        true => winuser::SW_MINIMIZE,
                        false => winuser::SW_RESTORE,
                    },
                );
            }
        }

        if diff != WindowFlags::empty() {
            let (style, style_ex) = new.to_window_styles();

            unsafe {
                winuser::SendMessageW(window, *event_loop::SET_RETAIN_STATE_ON_SIZE_MSG_ID, 1, 0);

                // This condition is necessary to avoid having an unrestorable window
                if !new.contains(WindowFlags::MINIMIZED) {
                    winuser::SetWindowLongW(window, winuser::GWL_STYLE, style as _);
                    winuser::SetWindowLongW(window, winuser::GWL_EXSTYLE, style_ex as _);
                }

                let mut flags = winuser::SWP_NOZORDER
                    | winuser::SWP_NOMOVE
                    | winuser::SWP_NOSIZE
                    | winuser::SWP_FRAMECHANGED;

                // We generally don't want style changes here to affect window
                // focus, but for fullscreen windows they must be activated
                // (i.e. focused) so that they appear on top of the taskbar
                if !new.contains(WindowFlags::MARKER_EXCLUSIVE_FULLSCREEN)
                    && !new.contains(WindowFlags::MARKER_BORDERLESS_FULLSCREEN)
                {
                    flags |= winuser::SWP_NOACTIVATE;
                }

                // Refresh the window frame
                winuser::SetWindowPos(window, ptr::null_mut(), 0, 0, 0, 0, flags);
                winuser::SendMessageW(window, *event_loop::SET_RETAIN_STATE_ON_SIZE_MSG_ID, 0, 0);
            }
        }
    }
}

impl CursorFlags {
    fn refresh_os_cursor(self, window: HWND) -> Result<(), io::Error> {
        let client_rect = util::get_client_rect(window)?;

        if util::is_focused(window) {
            let cursor_clip = match self.contains(CursorFlags::GRABBED) {
                true => Some(client_rect),
                false => None,
            };

            let rect_to_tuple = |rect: RECT| (rect.left, rect.top, rect.right, rect.bottom);
            let active_cursor_clip = rect_to_tuple(util::get_cursor_clip()?);
            let desktop_rect = rect_to_tuple(util::get_desktop_rect());

            let active_cursor_clip = match desktop_rect == active_cursor_clip {
                true => None,
                false => Some(active_cursor_clip),
            };

            // We do this check because calling `set_cursor_clip` incessantly will flood the event
            // loop with `WM_MOUSEMOVE` events, and `refresh_os_cursor` is called by `set_cursor_flags`
            // which at times gets called once every iteration of the eventloop.
            if active_cursor_clip != cursor_clip.map(rect_to_tuple) {
                util::set_cursor_clip(cursor_clip)?;
            }
        }

        let cursor_in_client = self.contains(CursorFlags::IN_WINDOW);
        if cursor_in_client {
            util::set_cursor_hidden(self.contains(CursorFlags::HIDDEN));
        } else {
            util::set_cursor_hidden(false);
        }

        Ok(())
    }
}
