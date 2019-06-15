use crate::{
    dpi::LogicalSize,
    platform_impl::platform::{event_loop, icon::WinIcon, util},
    window::{CursorIcon, Fullscreen, WindowAttributes},
};
use parking_lot::MutexGuard;
use std::{ffi::OsStr, io, os::windows::ffi::OsStrExt, ptr};
use winapi::{
    shared::{
        minwindef::DWORD,
        windef::{HWND, RECT},
    },
    um::winuser,
};

/// Contains information about states and the window that the callback is going to use.
#[derive(Clone)]
pub struct WindowState {
    pub mouse: MouseProperties,

    /// Used by `WM_GETMINMAXINFO`.
    pub min_size: Option<LogicalSize>,
    pub max_size: Option<LogicalSize>,

    pub window_icon: Option<WinIcon>,
    pub taskbar_icon: Option<WinIcon>,

    pub saved_window: Option<SavedWindow>,
    pub dpi_factor: f64,

    pub fullscreen: Option<Fullscreen>,
    window_flags: WindowFlags,
}

#[derive(Clone)]
pub struct SavedWindow {
    pub client_rect: RECT,
    pub dpi_factor: f64,
}

#[derive(Clone)]
pub struct MouseProperties {
    pub cursor: CursorIcon,
    pub buttons_down: u32,
    cursor_flags: CursorFlags,
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

        /// Marker flag for fullscreen. Should always match `WindowState::fullscreen`, but is
        /// included here to make masking easier.
        const MARKER_FULLSCREEN = 1 << 9;

        /// The `WM_SIZE` event contains some parameters that can effect the state of `WindowFlags`.
        /// In most cases, it's okay to let those parameters change the state. However, when we're
        /// running the `WindowFlags::apply_diff` function, we *don't* want those parameters to
        /// effect our stored state, because the purpose of `apply_diff` is to update the actual
        /// window's state to match our stored state. This controls whether to accept those changes.
        const MARKER_RETAIN_STATE_ON_SIZE = 1 << 10;

        const FULLSCREEN_AND_MASK = !(
            WindowFlags::DECORATIONS.bits |
            WindowFlags::RESIZABLE.bits |
            WindowFlags::MAXIMIZED.bits
        );
        const NO_DECORATIONS_AND_MASK = !WindowFlags::RESIZABLE.bits;
        const INVISIBLE_AND_MASK = !WindowFlags::MAXIMIZED.bits;
    }
}

impl WindowState {
    pub fn new(
        attributes: &WindowAttributes,
        window_icon: Option<WinIcon>,
        taskbar_icon: Option<WinIcon>,
        dpi_factor: f64,
    ) -> WindowState {
        WindowState {
            mouse: MouseProperties {
                cursor: CursorIcon::default(),
                buttons_down: 0,
                cursor_flags: CursorFlags::empty(),
            },

            min_size: attributes.min_inner_size,
            max_size: attributes.max_inner_size,

            window_icon,
            taskbar_icon,

            saved_window: None,
            dpi_factor,

            fullscreen: None,
            window_flags: WindowFlags::empty(),
        }
    }

    pub fn window_flags(&self) -> WindowFlags {
        self.window_flags
    }

    pub fn set_window_flags<F>(
        mut this: MutexGuard<'_, Self>,
        window: HWND,
        set_client_rect: Option<RECT>,
        f: F,
    ) where
        F: FnOnce(&mut WindowFlags),
    {
        let old_flags = this.window_flags;
        f(&mut this.window_flags);

        let is_fullscreen = this.fullscreen.is_some();
        this.window_flags
            .set(WindowFlags::MARKER_FULLSCREEN, is_fullscreen);
        let new_flags = this.window_flags;

        // Change video mode if necessary
        if old_flags & WindowFlags::MARKER_FULLSCREEN != new_flags & WindowFlags::MARKER_FULLSCREEN
        {
            let res = if let Some(Fullscreen::Exclusive(ref video_mode)) = this.fullscreen {
                let monitor = video_mode.monitor();

                // The display name on Windows is the same we get from
                // EnumDisplayDevices, so it's good for ChangeDisplaySettingsExW
                let mut display_name = OsStr::new(monitor.name().as_ref().unwrap())
                    .encode_wide()
                    .collect::<Vec<_>>();
                // encode_wide does not add a null-terminator but
                // ChangeDisplaySettingsExW requires a null-terminated string
                display_name.push(0);

                let mut native_video_mode = video_mode.video_mode.native_video_mode.clone();

                unsafe {
                    winuser::ChangeDisplaySettingsExW(
                        display_name.as_ptr(),
                        &mut native_video_mode,
                        std::ptr::null_mut(),
                        winuser::CDS_FULLSCREEN,
                        std::ptr::null_mut(),
                    )
                }
            } else {
                unsafe {
                    winuser::ChangeDisplaySettingsExW(
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        winuser::CDS_FULLSCREEN,
                        std::ptr::null_mut(),
                    )
                }
            };

            // These are separate asserts so we can easily see which error we
            // hit here
            debug_assert!(res != winuser::DISP_CHANGE_BADFLAGS);
            debug_assert!(res != winuser::DISP_CHANGE_BADMODE);
            debug_assert!(res != winuser::DISP_CHANGE_BADPARAM);
            debug_assert!(res != winuser::DISP_CHANGE_FAILED);
            assert_eq!(res, winuser::DISP_CHANGE_SUCCESSFUL);
        }

        drop(this);
        old_flags.apply_diff(window, new_flags, set_client_rect);
    }

    pub fn refresh_window_state(
        this: MutexGuard<'_, Self>,
        window: HWND,
        set_client_rect: Option<RECT>,
    ) {
        Self::set_window_flags(this, window, set_client_rect, |_| ());
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
        if self.contains(WindowFlags::MARKER_FULLSCREEN) {
            self &= WindowFlags::FULLSCREEN_AND_MASK;
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
        // if self.contains(WindowFlags::TRANSPARENT) {
        // }
        if self.contains(WindowFlags::CHILD) {
            style |= WS_CHILD; // This is incompatible with WS_POPUP if that gets added eventually.
        }
        if self.contains(WindowFlags::MAXIMIZED) {
            style |= WS_MAXIMIZE;
        }

        style |= WS_CLIPSIBLINGS | WS_CLIPCHILDREN | WS_SYSMENU;
        style_ex |= WS_EX_ACCEPTFILES;

        (style, style_ex)
    }

    /// Adjust the window client rectangle to the return value, if present.
    fn apply_diff(mut self, window: HWND, mut new: WindowFlags, set_client_rect: Option<RECT>) {
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
                    winuser::SWP_ASYNCWINDOWPOS | winuser::SWP_NOMOVE | winuser::SWP_NOSIZE,
                );
                winuser::UpdateWindow(window);
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

        if diff != WindowFlags::empty() {
            let (style, style_ex) = new.to_window_styles();

            unsafe {
                winuser::SendMessageW(window, *event_loop::SET_RETAIN_STATE_ON_SIZE_MSG_ID, 1, 0);

                winuser::SetWindowLongW(window, winuser::GWL_STYLE, style as _);
                winuser::SetWindowLongW(window, winuser::GWL_EXSTYLE, style_ex as _);

                match set_client_rect
                    .and_then(|r| util::adjust_window_rect_with_styles(window, style, style_ex, r))
                {
                    Some(client_rect) => {
                        let (x, y, w, h) = (
                            client_rect.left,
                            client_rect.top,
                            client_rect.right - client_rect.left,
                            client_rect.bottom - client_rect.top,
                        );
                        winuser::SetWindowPos(
                            window,
                            ptr::null_mut(),
                            x,
                            y,
                            w,
                            h,
                            winuser::SWP_NOZORDER
                                | winuser::SWP_FRAMECHANGED
                                | winuser::SWP_NOACTIVATE,
                        );
                    }
                    None => {
                        // Refresh the window frame.
                        winuser::SetWindowPos(
                            window,
                            ptr::null_mut(),
                            0,
                            0,
                            0,
                            0,
                            winuser::SWP_NOZORDER
                                | winuser::SWP_NOMOVE
                                | winuser::SWP_NOSIZE
                                | winuser::SWP_FRAMECHANGED
                                | winuser::SWP_NOACTIVATE,
                        );
                    }
                }
                winuser::SendMessageW(window, *event_loop::SET_RETAIN_STATE_ON_SIZE_MSG_ID, 0, 0);
            }
        }
    }
}

impl CursorFlags {
    fn refresh_os_cursor(self, window: HWND) -> Result<(), io::Error> {
        let client_rect = util::get_client_rect(window)?;

        if util::is_focused(window) {
            if self.contains(CursorFlags::GRABBED) {
                util::set_cursor_clip(Some(client_rect))?;
            } else {
                util::set_cursor_clip(None)?;
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
