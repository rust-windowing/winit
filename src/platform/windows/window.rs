#![cfg(target_os = "windows")]

use std::cell::{Cell, RefCell};
use std::ffi::OsStr;
use std::{io, mem, ptr};
use std::os::windows::ffi::OsStrExt;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::channel;

use winapi::ctypes::c_int;
use winapi::shared::minwindef::{BOOL, DWORD, FALSE, LPARAM, TRUE, UINT, WORD, WPARAM};
use winapi::shared::windef::{HDC, HWND, LPPOINT, POINT, RECT};
use winapi::um::{combaseapi, dwmapi, libloaderapi, winuser};
use winapi::um::objbase::{COINIT_MULTITHREADED};
use winapi::um::shobjidl_core::{CLSID_TaskbarList, ITaskbarList2};
use winapi::um::winnt::{LONG, LPCWSTR};

use {
    CreationError,
    CursorState,
    Icon,
    LogicalPosition,
    LogicalSize,
    MonitorId as RootMonitorId,
    MouseCursor,
    PhysicalSize,
    WindowAttributes,
};
use platform::platform::{Cursor, EventsLoop, PlatformSpecificWindowBuilderAttributes, WindowId};
use platform::platform::dpi::{BASE_DPI, dpi_to_scale_factor, get_window_dpi, get_window_scale_factor};
use platform::platform::events_loop::{self, DESTROY_MSG_ID, INITIAL_DPI_MSG_ID};
use platform::platform::icon::{self, IconType, WinIcon};
use platform::platform::raw_input::register_all_mice_and_keyboards_for_raw_input;
use platform::platform::util;

/// The Win32 implementation of the main `Window` object.
pub struct Window {
    /// Main handle for the window.
    window: WindowWrapper,

    decorations: Cell<bool>,
    maximized: Cell<bool>,
    resizable: Cell<bool>,
    fullscreen: RefCell<Option<::MonitorId>>,
    always_on_top: Cell<bool>,

    /// The current window state.
    window_state: Arc<Mutex<events_loop::WindowState>>,

    window_icon: Cell<Option<WinIcon>>,
    taskbar_icon: Cell<Option<WinIcon>>,

    // The events loop proxy.
    events_loop_proxy: events_loop::EventsLoopProxy,
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

// https://blogs.msdn.microsoft.com/oldnewthing/20131017-00/?p=2903
// The idea here is that we use the Adjust­Window­Rect­Ex function to calculate how much additional
// non-client area gets added due to the styles we passed. To make the math simple,
// we ask for a zero client rectangle, so that the resulting window is all non-client.
// And then we pass in the empty rectangle represented by the dot in the middle,
// and the Adjust­Window­Rect­Ex expands the rectangle in all dimensions.
// We see that it added ten pixels to the left, right, and bottom,
// and it added fifty pixels to the top.
// From this we can perform the reverse calculation: Instead of expanding the rectangle, we shrink it.
unsafe fn unjust_window_rect(prc: &mut RECT, style: DWORD, ex_style: DWORD) -> BOOL {
    let mut rc: RECT = mem::uninitialized();
    winuser::SetRectEmpty(&mut rc);
    let status = winuser::AdjustWindowRectEx(&mut rc, style, 0, ex_style);
    if status != 0 {
        prc.left -= rc.left;
        prc.top -= rc.top;
        prc.right -= rc.right;
        prc.bottom -= rc.bottom;
    }
    status
}

impl Window {
    pub fn new(
        events_loop: &EventsLoop,
        w_attr: WindowAttributes,
        pl_attr: PlatformSpecificWindowBuilderAttributes,
    ) -> Result<Window, CreationError> {
        let (tx, rx) = channel();
        let proxy = events_loop.create_proxy();
        events_loop.execute_in_thread(move |inserter| {
            // We dispatch an `init` function because of code style.
            // First person to remove the need for cloning here gets a cookie!
            let win = unsafe { init(w_attr.clone(), pl_attr.clone(), inserter, proxy.clone()) };
            let _ = tx.send(win);
        });
        rx.recv().unwrap()
    }

    pub fn set_title(&self, text: &str) {
        let text = OsStr::new(text)
            .encode_wide()
            .chain(Some(0).into_iter())
            .collect::<Vec<_>>();
        unsafe {
            winuser::SetWindowTextW(self.window.0, text.as_ptr() as LPCWSTR);
        }
    }

    #[inline]
    pub fn show(&self) {
        unsafe {
            winuser::ShowWindow(self.window.0, winuser::SW_SHOW);
        }
    }

    #[inline]
    pub fn hide(&self) {
        unsafe {
            winuser::ShowWindow(self.window.0, winuser::SW_HIDE);
        }
    }

    pub(crate) fn get_position_physical(&self) -> Option<(i32, i32)> {
        util::get_window_rect(self.window.0)
            .map(|rect| (rect.left as i32, rect.top as i32))
    }

    #[inline]
    pub fn get_position(&self) -> Option<LogicalPosition> {
        self.get_position_physical()
            .map(|physical_position| {
                let dpi_factor = self.get_hidpi_factor();
                LogicalPosition::from_physical(physical_position, dpi_factor)
            })
    }

    pub(crate) fn get_inner_position_physical(&self) -> Option<(i32, i32)> {
        let mut position: POINT = unsafe { mem::zeroed() };
        if unsafe { winuser::ClientToScreen(self.window.0, &mut position) } == 0 {
            return None;
        }
        Some((position.x, position.y))
    }

    #[inline]
    pub fn get_inner_position(&self) -> Option<LogicalPosition> {
        self.get_inner_position_physical()
            .map(|physical_position| {
                let dpi_factor = self.get_hidpi_factor();
                LogicalPosition::from_physical(physical_position, dpi_factor)
            })
    }

    pub(crate) fn set_position_physical(&self, x: i32, y: i32) {
        unsafe {
            winuser::SetWindowPos(
                self.window.0,
                ptr::null_mut(),
                x as c_int,
                y as c_int,
                0,
                0,
                winuser::SWP_ASYNCWINDOWPOS | winuser::SWP_NOZORDER | winuser::SWP_NOSIZE,
            );
            winuser::UpdateWindow(self.window.0);
        }
    }

    #[inline]
    pub fn set_position(&self, logical_position: LogicalPosition) {
        let dpi_factor = self.get_hidpi_factor();
        let (x, y) = logical_position.to_physical(dpi_factor).into();
        self.set_position_physical(x, y);
    }

    pub(crate) fn get_inner_size_physical(&self) -> Option<(u32, u32)> {
        let mut rect: RECT = unsafe { mem::uninitialized() };
        if unsafe { winuser::GetClientRect(self.window.0, &mut rect) } == 0 {
            return None;
        }
        Some((
            (rect.right - rect.left) as u32,
            (rect.bottom - rect.top) as u32,
        ))
    }

    #[inline]
    pub fn get_inner_size(&self) -> Option<LogicalSize> {
        self.get_inner_size_physical()
            .map(|physical_size| {
                let dpi_factor = self.get_hidpi_factor();
                LogicalSize::from_physical(physical_size, dpi_factor)
            })
    }

    pub(crate) fn get_outer_size_physical(&self) -> Option<(u32, u32)> {
        util::get_window_rect(self.window.0)
            .map(|rect| (
                (rect.right - rect.left) as u32,
                (rect.bottom - rect.top) as u32,
            ))
    }

    #[inline]
    pub fn get_outer_size(&self) -> Option<LogicalSize> {
        self.get_outer_size_physical()
            .map(|physical_size| {
                let dpi_factor = self.get_hidpi_factor();
                LogicalSize::from_physical(physical_size, dpi_factor)
            })
    }

    pub(crate) fn set_inner_size_physical(&self, x: u32, y: u32) {
        unsafe {
            let mut rect = RECT {
                top: 0,
                left: 0,
                bottom: y as LONG,
                right: x as LONG,
            };
            let dw_style = winuser::GetWindowLongA(self.window.0, winuser::GWL_STYLE) as DWORD;
            let b_menu = !winuser::GetMenu(self.window.0).is_null() as BOOL;
            let dw_style_ex = winuser::GetWindowLongA(self.window.0, winuser::GWL_EXSTYLE) as DWORD;
            winuser::AdjustWindowRectEx(&mut rect, dw_style, b_menu, dw_style_ex);
            let outer_x = (rect.right - rect.left).abs() as c_int;
            let outer_y = (rect.top - rect.bottom).abs() as c_int;
            winuser::SetWindowPos(
                self.window.0,
                ptr::null_mut(),
                0,
                0,
                outer_x,
                outer_y,
                winuser::SWP_ASYNCWINDOWPOS
                | winuser::SWP_NOZORDER
                | winuser::SWP_NOREPOSITION
                | winuser::SWP_NOMOVE,
            );
            winuser::UpdateWindow(self.window.0);
        }
    }

    #[inline]
    pub fn set_inner_size(&self, logical_size: LogicalSize) {
        let dpi_factor = self.get_hidpi_factor();
        let (width, height) = logical_size.to_physical(dpi_factor).into();
        self.set_inner_size_physical(width, height);
    }

    pub(crate) fn set_min_dimensions_physical(&self, dimensions: Option<(u32, u32)>) {
        self.window_state.lock().unwrap().min_size = dimensions.map(Into::into);
        // Make windows re-check the window size bounds.
        self.get_inner_size_physical()
            .map(|(width, height)| self.set_inner_size_physical(width, height));
    }

    #[inline]
    pub fn set_min_dimensions(&self, logical_size: Option<LogicalSize>) {
        let physical_size = logical_size.map(|logical_size| {
            let dpi_factor = self.get_hidpi_factor();
            logical_size.to_physical(dpi_factor).into()
        });
        self.set_min_dimensions_physical(physical_size);
    }

    pub fn set_max_dimensions_physical(&self, dimensions: Option<(u32, u32)>) {
        self.window_state.lock().unwrap().max_size = dimensions.map(Into::into);
        // Make windows re-check the window size bounds.
        self.get_inner_size_physical()
            .map(|(width, height)| self.set_inner_size_physical(width, height));
    }

    #[inline]
    pub fn set_max_dimensions(&self, logical_size: Option<LogicalSize>) {
        let physical_size = logical_size.map(|logical_size| {
            let dpi_factor = self.get_hidpi_factor();
            logical_size.to_physical(dpi_factor).into()
        });
        self.set_max_dimensions_physical(physical_size);
    }

    #[inline]
    pub fn set_resizable(&self, resizable: bool) {
        if resizable == self.resizable.get() {
            return;
        }
        if self.fullscreen.borrow().is_some() {
            // If we're in fullscreen, update stored configuration but don't apply anything.
            self.resizable.replace(resizable);
            return;
        }

        let mut style = unsafe {
            winuser::GetWindowLongW(self.window.0, winuser::GWL_STYLE)
        };
        if resizable {
            style |= winuser::WS_SIZEBOX as LONG;
        } else {
            style &= !winuser::WS_SIZEBOX as LONG;
        }

        unsafe {
            winuser::SetWindowLongW(
                self.window.0,
                winuser::GWL_STYLE,
                style as _,
            );
        };
        self.resizable.replace(resizable);
    }

    /// Returns the `hwnd` of this window.
    #[inline]
    pub fn hwnd(&self) -> HWND {
        self.window.0
    }

    #[inline]
    pub fn set_cursor(&self, cursor: MouseCursor) {
        let cursor_id = match cursor {
            MouseCursor::Arrow | MouseCursor::Default => winuser::IDC_ARROW,
            MouseCursor::Hand => winuser::IDC_HAND,
            MouseCursor::Crosshair => winuser::IDC_CROSS,
            MouseCursor::Text | MouseCursor::VerticalText => winuser::IDC_IBEAM,
            MouseCursor::NotAllowed | MouseCursor::NoDrop => winuser::IDC_NO,
            MouseCursor::Grab | MouseCursor::Grabbing |
            MouseCursor::Move | MouseCursor::AllScroll => winuser::IDC_SIZEALL,
            MouseCursor::EResize | MouseCursor::WResize |
            MouseCursor::EwResize | MouseCursor::ColResize => winuser::IDC_SIZEWE,
            MouseCursor::NResize | MouseCursor::SResize |
            MouseCursor::NsResize | MouseCursor::RowResize => winuser::IDC_SIZENS,
            MouseCursor::NeResize | MouseCursor::SwResize |
            MouseCursor::NeswResize => winuser::IDC_SIZENESW,
            MouseCursor::NwResize | MouseCursor::SeResize |
            MouseCursor::NwseResize => winuser::IDC_SIZENWSE,
            MouseCursor::Wait => winuser::IDC_WAIT,
            MouseCursor::Progress => winuser::IDC_APPSTARTING,
            MouseCursor::Help => winuser::IDC_HELP,
            MouseCursor::NoneCursor => ptr::null(),
            _ => winuser::IDC_ARROW, // use arrow for the missing cases.
        };

        let mut cur = self.window_state.lock().unwrap();
        cur.cursor = Cursor(cursor_id);
    }

    unsafe fn cursor_is_grabbed(&self) -> Result<bool, String> {
        let mut client_rect: RECT = mem::uninitialized();
        let mut clip_rect: RECT = mem::uninitialized();
        if winuser::GetClientRect(self.window.0, &mut client_rect) == 0 {
            return Err("`GetClientRect` failed".to_owned());
        }
        // A `POINT` is two `LONG`s (x, y), and the `RECT` field after `left` is `top`.
        if winuser::ClientToScreen(self.window.0, &mut client_rect.left as *mut _ as LPPOINT) == 0 {
            return Err("`ClientToScreen` (left, top) failed".to_owned());
        }
        if winuser::ClientToScreen(self.window.0, &mut client_rect.right as *mut _ as LPPOINT) == 0 {
            return Err("`ClientToScreen` (right, bottom) failed".to_owned());
        }
        if winuser::GetClipCursor(&mut clip_rect) == 0 {
            return Err("`GetClipCursor` failed".to_owned());
        }
        Ok(util::rect_eq(&client_rect, &clip_rect))
    }

    fn change_cursor_state(
        window: &WindowWrapper,
        current_state: CursorState,
        state: CursorState,
    ) -> Result<CursorState, String> {
        match (current_state, state) {
            (CursorState::Normal, CursorState::Normal)
            | (CursorState::Hide, CursorState::Hide)
            | (CursorState::Grab, CursorState::Grab) => (), // no-op

            (CursorState::Normal, CursorState::Hide) => unsafe {
                winuser::ShowCursor(FALSE);
            },

            (CursorState::Grab, CursorState::Hide) => unsafe {
                if winuser::ClipCursor(ptr::null()) == 0 {
                    return Err("`ClipCursor` failed".to_owned());
                }
            },

            (CursorState::Hide, CursorState::Normal) => unsafe {
                winuser::ShowCursor(TRUE);
            },

            (CursorState::Normal, CursorState::Grab)
            | (CursorState::Hide, CursorState::Grab) => unsafe {
                let mut rect = mem::uninitialized();
                if winuser::GetClientRect(window.0, &mut rect) == 0 {
                    return Err("`GetClientRect` failed".to_owned());
                }
                if winuser::ClientToScreen(window.0, &mut rect.left as *mut _ as LPPOINT) == 0 {
                    return Err("`ClientToScreen` (left, top) failed".to_owned());
                }
                if winuser::ClientToScreen(window.0, &mut rect.right as *mut _ as LPPOINT) == 0 {
                    return Err("`ClientToScreen` (right, bottom) failed".to_owned());
                }
                if winuser::ClipCursor(&rect) == 0 {
                    return Err("`ClipCursor` failed".to_owned());
                }
                if current_state != CursorState::Hide {
                    winuser::ShowCursor(FALSE);
                }
            },

            (CursorState::Grab, CursorState::Normal) => unsafe {
                if winuser::ClipCursor(ptr::null()) == 0 {
                    return Err("`ClipCursor` failed".to_owned());
                }
                winuser::ShowCursor(TRUE);
            },
        };
        Ok(state)
    }

    pub fn set_cursor_state(&self, state: CursorState) -> Result<(), String> {
        let is_grabbed = unsafe { self.cursor_is_grabbed() }?;
        let (tx, rx) = channel();
        let window = self.window.clone();
        let window_state = Arc::clone(&self.window_state);
        self.events_loop_proxy.execute_in_thread(move |_| {
            let mut window_state_lock = window_state.lock().unwrap();
            // We should probably also check if the cursor is hidden,
            // but `GetCursorInfo` isn't in winapi-rs yet, and it doesn't seem to matter as much.
            let current_state = match window_state_lock.cursor_state {
                CursorState::Normal if is_grabbed => CursorState::Grab,
                CursorState::Grab if !is_grabbed => CursorState::Normal,
                current_state => current_state,
            };
            let result = Self::change_cursor_state(&window, current_state, state)
                .map(|_| {
                    window_state_lock.cursor_state = state;
                });
            let _ = tx.send(result);
        });
        rx.recv().unwrap()
    }

    #[inline]
    pub fn get_hidpi_factor(&self) -> f64 {
        get_window_scale_factor(self.window.0, self.window.1)
    }

    fn set_cursor_position_physical(&self, x: i32, y: i32) -> Result<(), ()> {
        let mut point = POINT { x, y };
        unsafe {
            if winuser::ClientToScreen(self.window.0, &mut point) == 0 {
                return Err(());
            }
            if winuser::SetCursorPos(point.x, point.y) == 0 {
                return Err(());
            }
        }
        Ok(())
    }

    #[inline]
    pub fn set_cursor_position(&self, logical_position: LogicalPosition) -> Result<(), ()> {
        let dpi_factor = self.get_hidpi_factor();
        let (x, y) = logical_position.to_physical(dpi_factor).into();
        self.set_cursor_position_physical(x, y)
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        WindowId(self.window.0)
    }

    #[inline]
    pub fn set_maximized(&self, maximized: bool) {
        self.maximized.replace(maximized);
        // We only maximize if we're not in fullscreen.
        if self.fullscreen.borrow().is_some() { return; }

        let window = self.window.clone();
        unsafe {
            // `ShowWindow` resizes the window, so it must be called from the main thread.
            self.events_loop_proxy.execute_in_thread(move |_| {
                winuser::ShowWindow(
                    window.0,
                    if maximized {
                        winuser::SW_MAXIMIZE
                    } else {
                        winuser::SW_RESTORE
                    },
                );
            });
        }
    }

    unsafe fn set_fullscreen_style(&self) -> (LONG, LONG) {
        let mut window_state = self.window_state.lock().unwrap();

        if self.fullscreen.borrow().is_none() || window_state.saved_window_info.is_none() {
            let rect = util::get_window_rect(self.window.0).expect("`GetWindowRect` failed");
            let dpi_factor = Some(self.get_hidpi_factor());
            window_state.saved_window_info = Some(events_loop::SavedWindowInfo {
                style: winuser::GetWindowLongW(self.window.0, winuser::GWL_STYLE),
                ex_style: winuser::GetWindowLongW(self.window.0, winuser::GWL_EXSTYLE),
                rect,
                is_fullscreen: true,
                dpi_factor,
            });
        }

        // We sync the system maximized state here, it will be used when restoring
        let mut placement: winuser::WINDOWPLACEMENT = mem::zeroed();
        placement.length = mem::size_of::<winuser::WINDOWPLACEMENT>() as u32;
        winuser::GetWindowPlacement(self.window.0, &mut placement);
        self.maximized.replace(placement.showCmd == (winuser::SW_SHOWMAXIMIZED as u32));
        let saved_window_info = window_state.saved_window_info.as_ref().unwrap();

        (saved_window_info.style, saved_window_info.ex_style)
    }

    unsafe fn restore_saved_window(&self) {
        let mut window_state = self.window_state.lock().unwrap();

        // 'saved_window_info' can be None if the window has never been
        // in fullscreen mode before this method gets called.
        if window_state.saved_window_info.is_none() {
            return;
        }

        // Reset original window style and size.  The multiple window size/moves
        // here are ugly, but if SetWindowPos() doesn't redraw, the taskbar won't be
        // repainted.  Better-looking methods welcome.
        {
            let saved_window_info = window_state.saved_window_info.as_mut().unwrap();
            saved_window_info.is_fullscreen = false;
        }
        let saved_window_info = window_state.saved_window_info.as_ref().unwrap();

        let rect = saved_window_info.rect.clone();
        let window = self.window.clone();
        let (mut style, ex_style) = (saved_window_info.style, saved_window_info.ex_style);

        let maximized = self.maximized.get();
        let resizable = self.resizable.get();

        // We're restoring the window to its size and position from before being fullscreened.
        // `ShowWindow` resizes the window, so it must be called from the main thread.
        self.events_loop_proxy.execute_in_thread(move |_| {
            if resizable {
                style |= winuser::WS_SIZEBOX as LONG;
            } else {
                style &= !winuser::WS_SIZEBOX as LONG;
            }
            winuser::SetWindowLongW(window.0, winuser::GWL_STYLE, style);
            winuser::SetWindowLongW(window.0, winuser::GWL_EXSTYLE, ex_style);

            winuser::SetWindowPos(
                window.0,
                ptr::null_mut(),
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                winuser::SWP_ASYNCWINDOWPOS
                | winuser::SWP_NOZORDER
                | winuser::SWP_NOACTIVATE
                | winuser::SWP_FRAMECHANGED,
            );

            // We apply any requested changes to maximization state that occurred while we were in fullscreen.
            winuser::ShowWindow(
                window.0,
                if maximized {
                    winuser::SW_MAXIMIZE
                } else {
                    winuser::SW_RESTORE
                },
            );

            mark_fullscreen(window.0, false);
        });
    }

    #[inline]
    pub fn set_fullscreen(&self, monitor: Option<RootMonitorId>) {
        unsafe {
            match &monitor {
                &Some(RootMonitorId { ref inner }) => {
                    let (x, y): (i32, i32) = inner.get_position().into();
                    let (width, height): (u32, u32) = inner.get_dimensions().into();
                    let window = self.window.clone();

                    let (style, ex_style) = self.set_fullscreen_style();

                    self.events_loop_proxy.execute_in_thread(move |_| {
                        winuser::SetWindowLongW(
                            window.0,
                            winuser::GWL_STYLE,
                            ((style as DWORD) & !(winuser::WS_CAPTION | winuser::WS_THICKFRAME))
                                as LONG,
                        );

                        winuser::SetWindowLongW(
                            window.0,
                            winuser::GWL_EXSTYLE,
                            ((ex_style as DWORD)
                                & !(winuser::WS_EX_DLGMODALFRAME | winuser::WS_EX_WINDOWEDGE
                                    | winuser::WS_EX_CLIENTEDGE
                                    | winuser::WS_EX_STATICEDGE))
                                as LONG,
                        );

                        winuser::SetWindowPos(
                            window.0,
                            ptr::null_mut(),
                            x as c_int,
                            y as c_int,
                            width as c_int,
                            height as c_int,
                            winuser::SWP_ASYNCWINDOWPOS | winuser::SWP_NOZORDER
                                | winuser::SWP_NOACTIVATE
                                | winuser::SWP_FRAMECHANGED,
                        );

                        mark_fullscreen(window.0, true);
                    });
                }
                &None => {
                    self.restore_saved_window();
                }
            }
        }

        self.fullscreen.replace(monitor);
    }

    #[inline]
    pub fn set_decorations(&self, decorations: bool) {
        if self.decorations.get() == decorations {
            return;
        }

        let style_flags = (winuser::WS_CAPTION | winuser::WS_THICKFRAME) as LONG;
        let ex_style_flags = (winuser::WS_EX_WINDOWEDGE) as LONG;

        // if we are in fullscreen mode, we only change the saved window info
        if self.fullscreen.borrow().is_some() {
            {
                let mut window_state = self.window_state.lock().unwrap();
                let saved = window_state.saved_window_info.as_mut().unwrap();

                unsafe {
                    unjust_window_rect(&mut saved.rect, saved.style as _, saved.ex_style as _);
                }

                if decorations {
                    saved.style = saved.style | style_flags;
                    saved.ex_style = saved.ex_style | ex_style_flags;
                } else {
                    saved.style = saved.style & !style_flags;
                    saved.ex_style = saved.ex_style & !ex_style_flags;
                }

                unsafe {
                    winuser::AdjustWindowRectEx(
                        &mut saved.rect,
                        saved.style as _,
                        0,
                        saved.ex_style as _,
                    );
                }
            }

            self.decorations.replace(decorations);
            return;
        }

        unsafe {
            let mut rect: RECT = mem::zeroed();
            winuser::GetWindowRect(self.window.0, &mut rect);

            let mut style = winuser::GetWindowLongW(self.window.0, winuser::GWL_STYLE);
            let mut ex_style = winuser::GetWindowLongW(self.window.0, winuser::GWL_EXSTYLE);
            unjust_window_rect(&mut rect, style as _, ex_style as _);

            if decorations {
                style = style | style_flags;
                ex_style = ex_style | ex_style_flags;
            } else {
                style = style & !style_flags;
                ex_style = ex_style & !ex_style_flags;
            }

            let window = self.window.clone();

            self.events_loop_proxy.execute_in_thread(move |_| {
                winuser::SetWindowLongW(window.0, winuser::GWL_STYLE, style);
                winuser::SetWindowLongW(window.0, winuser::GWL_EXSTYLE, ex_style);
                winuser::AdjustWindowRectEx(&mut rect, style as _, 0, ex_style as _);

                winuser::SetWindowPos(
                    window.0,
                    ptr::null_mut(),
                    rect.left,
                    rect.top,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    winuser::SWP_ASYNCWINDOWPOS
                    | winuser::SWP_NOZORDER
                    | winuser::SWP_NOACTIVATE
                    | winuser::SWP_FRAMECHANGED,
                );
            });
        }

        self.decorations.replace(decorations);
    }

    #[inline]
    pub fn set_always_on_top(&self, always_on_top: bool) {
        if self.always_on_top.get() == always_on_top {
            return;
        }

        let window = self.window.clone();
        self.events_loop_proxy.execute_in_thread(move |_| {
            let insert_after = if always_on_top {
                winuser::HWND_TOPMOST
            } else {
                winuser::HWND_NOTOPMOST
            };
            unsafe {
                winuser::SetWindowPos(
                    window.0,
                    insert_after,
                    0,
                    0,
                    0,
                    0,
                    winuser::SWP_ASYNCWINDOWPOS | winuser::SWP_NOMOVE | winuser::SWP_NOSIZE,
                );
                winuser::UpdateWindow(window.0);
            }
        });

        self.always_on_top.replace(always_on_top);
    }

    #[inline]
    pub fn get_current_monitor(&self) -> RootMonitorId {
        RootMonitorId {
            inner: EventsLoop::get_current_monitor(self.window.0),
        }
    }

    #[inline]
    pub fn set_window_icon(&self, mut window_icon: Option<Icon>) {
        let window_icon = window_icon
            .take()
            .map(|icon| WinIcon::from_icon(icon).expect("Failed to create `ICON_SMALL`"));
        if let Some(ref window_icon) = window_icon {
            window_icon.set_for_window(self.window.0, IconType::Small);
        } else {
            icon::unset_for_window(self.window.0, IconType::Small);
        }
        self.window_icon.replace(window_icon);
    }

    #[inline]
    pub fn set_taskbar_icon(&self, mut taskbar_icon: Option<Icon>) {
        let taskbar_icon = taskbar_icon
            .take()
            .map(|icon| WinIcon::from_icon(icon).expect("Failed to create `ICON_BIG`"));
        if let Some(ref taskbar_icon) = taskbar_icon {
            taskbar_icon.set_for_window(self.window.0, IconType::Big);
        } else {
            icon::unset_for_window(self.window.0, IconType::Big);
        }
        self.taskbar_icon.replace(taskbar_icon);
    }

    #[inline]
    pub fn set_ime_spot(&self, _logical_spot: LogicalPosition) {
        unimplemented!();
    }
}

impl Drop for Window {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            // The window must be destroyed from the same thread that created it, so we send a
            // custom message to be handled by our callback to do the actual work.
            winuser::PostMessageW(self.window.0, *DESTROY_MSG_ID, 0, 0);
        }
    }
}

/// A simple non-owning wrapper around a window.
#[doc(hidden)]
#[derive(Clone)]
pub struct WindowWrapper(HWND, HDC);

// Send is not implemented for HWND and HDC, we have to wrap it and implement it manually.
// For more info see:
// https://github.com/retep998/winapi-rs/issues/360
// https://github.com/retep998/winapi-rs/issues/396
unsafe impl Send for WindowWrapper {}

pub unsafe fn adjust_size(physical_size: PhysicalSize, style: DWORD, ex_style: DWORD) -> (LONG, LONG) {
    let (width, height): (u32, u32) = physical_size.into();
    let mut rect = RECT { left: 0, right: width as LONG, top: 0, bottom: height as LONG };
    winuser::AdjustWindowRectEx(&mut rect, style, 0, ex_style);
    (rect.right - rect.left, rect.bottom - rect.top)
}

unsafe fn init(
    mut attributes: WindowAttributes,
    mut pl_attribs: PlatformSpecificWindowBuilderAttributes,
    inserter: events_loop::Inserter,
    events_loop_proxy: events_loop::EventsLoopProxy,
) -> Result<Window, CreationError> {
    let title = OsStr::new(&attributes.title)
        .encode_wide()
        .chain(Some(0).into_iter())
        .collect::<Vec<_>>();

    let window_icon = {
        let icon = attributes.window_icon
            .take()
            .map(WinIcon::from_icon);
        if icon.is_some() {
            Some(icon.unwrap().map_err(|err| {
                CreationError::OsError(format!("Failed to create `ICON_SMALL`: {:?}", err))
            })?)
        } else {
            None
        }
    };
    let taskbar_icon = {
        let icon = pl_attribs.taskbar_icon
            .take()
            .map(WinIcon::from_icon);
        if icon.is_some() {
            Some(icon.unwrap().map_err(|err| {
                CreationError::OsError(format!("Failed to create `ICON_BIG`: {:?}", err))
            })?)
        } else {
            None
        }
    };

    // registering the window class
    let class_name = register_window_class(&window_icon, &taskbar_icon);

    let (width, height) = attributes.dimensions
        .map(Into::into)
        .unwrap_or((1024, 768));
    // building a RECT object with coordinates
    let mut rect = RECT {
        left: 0,
        right: width as LONG,
        top: 0,
        bottom: height as LONG,
    };

    // computing the style and extended style of the window
    let (mut ex_style, style) = if !attributes.decorations {
        (winuser::WS_EX_APPWINDOW,
            //winapi::WS_POPUP is incompatible with winapi::WS_CHILD
            if pl_attribs.parent.is_some() {
                winuser::WS_CLIPSIBLINGS | winuser::WS_CLIPCHILDREN
            }
            else {
                winuser::WS_POPUP | winuser::WS_CLIPSIBLINGS | winuser::WS_CLIPCHILDREN
            }
        )
    } else {
        (winuser::WS_EX_APPWINDOW | winuser::WS_EX_WINDOWEDGE,
            winuser::WS_OVERLAPPEDWINDOW | winuser::WS_CLIPSIBLINGS | winuser::WS_CLIPCHILDREN)
    };

    if attributes.always_on_top {
        ex_style |= winuser::WS_EX_TOPMOST;
    }

    // adjusting the window coordinates using the style
    winuser::AdjustWindowRectEx(&mut rect, style, 0, ex_style);

    // creating the real window this time, by using the functions in `extra_functions`
    let real_window = {
        let (adjusted_width, adjusted_height) = if attributes.dimensions.is_some() {
            let min_dimensions = attributes.min_dimensions
                .map(|logical_size| PhysicalSize::from_logical(logical_size, 1.0))
                .map(|physical_size| adjust_size(physical_size, style, ex_style))
                .unwrap_or((0, 0));
            let max_dimensions = attributes.max_dimensions
                .map(|logical_size| PhysicalSize::from_logical(logical_size, 1.0))
                .map(|physical_size| adjust_size(physical_size, style, ex_style))
                .unwrap_or((c_int::max_value(), c_int::max_value()));
            (
                Some((rect.right - rect.left).min(max_dimensions.0).max(min_dimensions.0)),
                Some((rect.bottom - rect.top).min(max_dimensions.1).max(min_dimensions.1))
            )
        } else {
            (None, None)
        };

        let mut style = if !attributes.visible {
            style
        } else {
            style | winuser::WS_VISIBLE
        };

        if !attributes.resizable {
            style &= !winuser::WS_SIZEBOX;
        }

        if pl_attribs.parent.is_some() {
            style |= winuser::WS_CHILD;
        }

        let handle = winuser::CreateWindowExW(ex_style | winuser::WS_EX_ACCEPTFILES,
            class_name.as_ptr(),
            title.as_ptr() as LPCWSTR,
            style | winuser::WS_CLIPSIBLINGS | winuser::WS_CLIPCHILDREN,
            winuser::CW_USEDEFAULT, winuser::CW_USEDEFAULT,
            adjusted_width.unwrap_or(winuser::CW_USEDEFAULT),
            adjusted_height.unwrap_or(winuser::CW_USEDEFAULT),
            pl_attribs.parent.unwrap_or(ptr::null_mut()),
            ptr::null_mut(),
            libloaderapi::GetModuleHandleW(ptr::null()),
            ptr::null_mut(),
        );

        if handle.is_null() {
            return Err(CreationError::OsError(format!("CreateWindowEx function failed: {}",
                                              format!("{}", io::Error::last_os_error()))));
        }

        let hdc = winuser::GetDC(handle);
        if hdc.is_null() {
            return Err(CreationError::OsError(format!("GetDC function failed: {}",
                                              format!("{}", io::Error::last_os_error()))));
        }

        WindowWrapper(handle, hdc)
    };

    // Set up raw input
    register_all_mice_and_keyboards_for_raw_input(real_window.0);

    // Register for touch events if applicable
    {
        let digitizer = winuser::GetSystemMetrics( winuser::SM_DIGITIZER ) as u32;
        if digitizer & winuser::NID_READY != 0 {
            winuser::RegisterTouchWindow( real_window.0, winuser::TWF_WANTPALM );
        }
    }

    let dpi = get_window_dpi(real_window.0, real_window.1);
    let dpi_factor = dpi_to_scale_factor(dpi);
    if dpi != BASE_DPI {
        let mut packed_dimensions = 0;
        // MAKELPARAM isn't provided by winapi yet.
        let ptr = &mut packed_dimensions as *mut LPARAM as *mut WORD;
        *ptr.offset(0) = width as WORD;
        *ptr.offset(1) = height as WORD;
        winuser::PostMessageW(
            real_window.0,
            *INITIAL_DPI_MSG_ID,
            dpi as WPARAM,
            packed_dimensions,
        );
    }

    let window_state = {
        let max_size = attributes.max_dimensions
            .map(|logical_size| PhysicalSize::from_logical(logical_size, dpi_factor));
        let min_size = attributes.min_dimensions
            .map(|logical_size| PhysicalSize::from_logical(logical_size, dpi_factor));
        let mut window_state = events_loop::WindowState {
            cursor: Cursor(winuser::IDC_ARROW), // use arrow by default
            cursor_state: CursorState::Normal,
            max_size,
            min_size,
            mouse_in_window: false,
            saved_window_info: None,
            dpi_factor,
        };
        // Creating a mutex to track the current window state
        Arc::new(Mutex::new(window_state))
    };

    // making the window transparent
    if attributes.transparent {
        let bb = dwmapi::DWM_BLURBEHIND {
            dwFlags: 0x1, // FIXME: DWM_BB_ENABLE;
            fEnable: 1,
            hRgnBlur: ptr::null_mut(),
            fTransitionOnMaximized: 0,
        };

        dwmapi::DwmEnableBlurBehindWindow(real_window.0, &bb);
    }

    let win = Window {
        window: real_window,
        window_state: window_state,
        decorations: Cell::new(attributes.decorations),
        maximized: Cell::new(attributes.maximized.clone()),
        resizable: Cell::new(attributes.resizable.clone()),
        fullscreen: RefCell::new(attributes.fullscreen.clone()),
        always_on_top: Cell::new(attributes.always_on_top),
        window_icon: Cell::new(window_icon),
        taskbar_icon: Cell::new(taskbar_icon),
        events_loop_proxy,
    };

    win.set_maximized(attributes.maximized);
    if let Some(_) = attributes.fullscreen {
        win.set_fullscreen(attributes.fullscreen);
        force_window_active(win.window.0);
    }

    inserter.insert(win.window.0, win.window_state.clone());

    Ok(win)
}

unsafe fn register_window_class(
    window_icon: &Option<WinIcon>,
    taskbar_icon: &Option<WinIcon>,
) -> Vec<u16> {
    let class_name: Vec<_> = OsStr::new("Window Class")
        .encode_wide()
        .chain(Some(0).into_iter())
        .collect();

    let h_icon = taskbar_icon
        .as_ref()
        .map(|icon| icon.handle)
        .unwrap_or(ptr::null_mut());
    let h_icon_small = window_icon
        .as_ref()
        .map(|icon| icon.handle)
        .unwrap_or(ptr::null_mut());

    let class = winuser::WNDCLASSEXW {
        cbSize: mem::size_of::<winuser::WNDCLASSEXW>() as UINT,
        style: winuser::CS_HREDRAW | winuser::CS_VREDRAW | winuser::CS_OWNDC,
        lpfnWndProc: Some(events_loop::callback),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: libloaderapi::GetModuleHandleW(ptr::null()),
        hIcon: h_icon,
        hCursor: ptr::null_mut(), // must be null in order for cursor state to work properly
        hbrBackground: ptr::null_mut(),
        lpszMenuName: ptr::null(),
        lpszClassName: class_name.as_ptr(),
        hIconSm: h_icon_small,
    };

    // We ignore errors because registering the same window class twice would trigger
    //  an error, and because errors here are detected during CreateWindowEx anyway.
    // Also since there is no weird element in the struct, there is no reason for this
    //  call to fail.
    winuser::RegisterClassExW(&class);

    class_name
}


struct ComInitialized(*mut ());
impl Drop for ComInitialized {
    fn drop(&mut self) {
        unsafe { combaseapi::CoUninitialize() };
    }
}

thread_local!{
    static COM_INITIALIZED: ComInitialized = {
        unsafe {
            combaseapi::CoInitializeEx(ptr::null_mut(), COINIT_MULTITHREADED);
            ComInitialized(ptr::null_mut())
        }
    };

    static TASKBAR_LIST: Cell<*mut ITaskbarList2> = Cell::new(ptr::null_mut());
}

pub fn com_initialized() {
    COM_INITIALIZED.with(|_| {});
}

// Reference Implementation:
// https://github.com/chromium/chromium/blob/f18e79d901f56154f80eea1e2218544285e62623/ui/views/win/fullscreen_handler.cc
//
// As per MSDN marking the window as fullscreen should ensure that the
// taskbar is moved to the bottom of the Z-order when the fullscreen window
// is activated. If the window is not fullscreen, the Shell falls back to
// heuristics to determine how the window should be treated, which means
// that it could still consider the window as fullscreen. :(
unsafe fn mark_fullscreen(handle: HWND, fullscreen: bool) {
    com_initialized();

    TASKBAR_LIST.with(|task_bar_list_ptr| {
        let mut task_bar_list = task_bar_list_ptr.get();

        if task_bar_list == ptr::null_mut() {
            use winapi::shared::winerror::S_OK;
            use winapi::Interface;

            let hr = combaseapi::CoCreateInstance(
                &CLSID_TaskbarList,
                ptr::null_mut(),
                combaseapi::CLSCTX_ALL,
                &ITaskbarList2::uuidof(),
                &mut task_bar_list as *mut _ as *mut _,
            );

            if hr != S_OK || (*task_bar_list).HrInit() != S_OK {
                // In some old windows, the taskbar object could not be created, we just ignore it
                return;
            }
            task_bar_list_ptr.set(task_bar_list)
        }

        task_bar_list = task_bar_list_ptr.get();
        (*task_bar_list).MarkFullscreenWindow(handle, if fullscreen { 1 } else { 0 });
    })
}

unsafe fn force_window_active(handle: HWND) {
    // In some situation, calling SetForegroundWindow could not bring up the window,
    // This is a little hack which can "steal" the foreground window permission
    // We only call this function in the window creation, so it should be fine.
    // See : https://stackoverflow.com/questions/10740346/setforegroundwindow-only-working-while-visual-studio-is-open
    let alt_sc = winuser::MapVirtualKeyW(winuser::VK_MENU as _, winuser::MAPVK_VK_TO_VSC);

    let mut inputs: [winuser::INPUT; 2] = mem::zeroed();
    inputs[0].type_ = winuser::INPUT_KEYBOARD;
    inputs[0].u.ki_mut().wVk = winuser::VK_LMENU as _;
    inputs[0].u.ki_mut().wScan = alt_sc as _;
    inputs[0].u.ki_mut().dwFlags = winuser::KEYEVENTF_EXTENDEDKEY;

    inputs[1].type_ = winuser::INPUT_KEYBOARD;
    inputs[1].u.ki_mut().wVk = winuser::VK_LMENU as _;
    inputs[1].u.ki_mut().wScan = alt_sc as _;
    inputs[1].u.ki_mut().dwFlags = winuser::KEYEVENTF_EXTENDEDKEY | winuser::KEYEVENTF_KEYUP;

    // Simulate a key press and release
    winuser::SendInput(
        inputs.len() as _,
        inputs.as_mut_ptr(),
        mem::size_of::<winuser::INPUT>() as _,
    );

    winuser::SetForegroundWindow(handle);
}
