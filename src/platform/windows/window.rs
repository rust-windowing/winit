#![cfg(target_os = "windows")]

use std::cell::Cell;
use std::ffi::OsStr;
use std::{io, mem, ptr};
use std::os::raw;
use std::os::windows::ffi::OsStrExt;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::channel;

use winapi::shared::minwindef::{BOOL, DWORD, UINT};
use winapi::shared::windef::{HDC, HWND, POINT, RECT};
use winapi::um::{combaseapi, dwmapi, libloaderapi, processthreadsapi, winuser};
use winapi::um::objbase::{COINIT_MULTITHREADED};
use winapi::um::unknwnbase::{IUnknown, IUnknownVtbl};
use winapi::um::winnt::{HRESULT, LONG, LPCWSTR};

use CreationError;
use CursorState;
use Icon;
use MonitorId as RootMonitorId;
use MouseCursor;
use WindowAttributes;

use platform::platform::{EventsLoop, PlatformSpecificWindowBuilderAttributes, WindowId};
use platform::platform::events_loop::{self, DESTROY_MSG_ID};
use platform::platform::icon::{self, IconType, WinIcon};
use platform::platform::raw_input::register_all_mice_and_keyboards_for_raw_input;

/// The Win32 implementation of the main `Window` object.
pub struct Window {
    /// Main handle for the window.
    window: WindowWrapper,

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
    let mut rc: RECT = mem::zeroed();

    winuser::SetRectEmpty(&mut rc);

    let frc = winuser::AdjustWindowRectEx(&mut rc, style, 0, ex_style);
    if frc != 0 {
        prc.left -= rc.left;
        prc.top -= rc.top;
        prc.right -= rc.right;
        prc.bottom -= rc.bottom;
    }

    frc
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

    /// See the docs in the crate root file.
    pub fn get_position(&self) -> Option<(i32, i32)> {
        let mut rect: RECT = unsafe { mem::uninitialized() };

        if unsafe { winuser::GetWindowRect(self.window.0, &mut rect) } != 0 {
            Some((rect.left as i32, rect.top as i32))
        } else {
            None
        }
    }

    pub fn get_inner_position(&self) -> Option<(i32, i32)> {
        use std::mem;

        let mut position: POINT = unsafe { mem::zeroed() };
        if unsafe { winuser::ClientToScreen(self.window.0, &mut position) } == 0 {
            return None;
        }

        Some((position.x, position.y))
    }

    /// See the docs in the crate root file.
    pub fn set_position(&self, x: i32, y: i32) {
        unsafe {
            winuser::SetWindowPos(self.window.0, ptr::null_mut(), x as raw::c_int, y as raw::c_int,
                                 0, 0, winuser::SWP_ASYNCWINDOWPOS | winuser::SWP_NOZORDER | winuser::SWP_NOSIZE);
            winuser::UpdateWindow(self.window.0);
        }
    }

    /// See the docs in the crate root file.
    #[inline]
    pub fn get_inner_size(&self) -> Option<(u32, u32)> {
        let mut rect: RECT = unsafe { mem::uninitialized() };

        if unsafe { winuser::GetClientRect(self.window.0, &mut rect) } == 0 {
            return None
        }

        Some((
            (rect.right - rect.left) as u32,
            (rect.bottom - rect.top) as u32
        ))
    }

    /// See the docs in the crate root file.
    #[inline]
    pub fn get_outer_size(&self) -> Option<(u32, u32)> {
        let mut rect: RECT = unsafe { mem::uninitialized() };

        if unsafe { winuser::GetWindowRect(self.window.0, &mut rect) } == 0 {
            return None
        }

        Some((
            (rect.right - rect.left) as u32,
            (rect.bottom - rect.top) as u32
        ))
    }

    /// See the docs in the crate root file.
    pub fn set_inner_size(&self, x: u32, y: u32) {
        unsafe {
            // Calculate the outer size based upon the specified inner size
            let mut rect = RECT { top: 0, left: 0, bottom: y as LONG, right: x as LONG };
            let dw_style = winuser::GetWindowLongA(self.window.0, winuser::GWL_STYLE) as DWORD;
            let b_menu = !winuser::GetMenu(self.window.0).is_null() as BOOL;
            let dw_style_ex = winuser::GetWindowLongA(self.window.0, winuser::GWL_EXSTYLE) as DWORD;
            winuser::AdjustWindowRectEx(&mut rect, dw_style, b_menu, dw_style_ex);
            let outer_x = (rect.right - rect.left).abs() as raw::c_int;
            let outer_y = (rect.top - rect.bottom).abs() as raw::c_int;

            winuser::SetWindowPos(self.window.0, ptr::null_mut(), 0, 0, outer_x, outer_y,
                winuser::SWP_ASYNCWINDOWPOS | winuser::SWP_NOZORDER | winuser::SWP_NOREPOSITION | winuser::SWP_NOMOVE);
            winuser::UpdateWindow(self.window.0);
        }
    }

    /// See the docs in the crate root file.
    #[inline]
    pub fn set_min_dimensions(&self, dimensions: Option<(u32, u32)>) {
        let mut window_state = self.window_state.lock().unwrap();
        window_state.attributes.min_dimensions = dimensions;

        // Make windows re-check the window size bounds.
        if let Some(inner_size) = self.get_inner_size() {
            unsafe {
                let mut rect = RECT { top: 0, left: 0, bottom: inner_size.1 as LONG, right: inner_size.0 as LONG };
                let dw_style = winuser::GetWindowLongA(self.window.0, winuser::GWL_STYLE) as DWORD;
                let b_menu = !winuser::GetMenu(self.window.0).is_null() as BOOL;
                let dw_style_ex = winuser::GetWindowLongA(self.window.0, winuser::GWL_EXSTYLE) as DWORD;
                winuser::AdjustWindowRectEx(&mut rect, dw_style, b_menu, dw_style_ex);
                let outer_x = (rect.right - rect.left).abs() as raw::c_int;
                let outer_y = (rect.top - rect.bottom).abs() as raw::c_int;

                winuser::SetWindowPos(self.window.0, ptr::null_mut(), 0, 0, outer_x, outer_y,
                    winuser::SWP_ASYNCWINDOWPOS | winuser::SWP_NOZORDER | winuser::SWP_NOREPOSITION | winuser::SWP_NOMOVE);
            }
        }
    }

    /// See the docs in the crate root file.
    #[inline]
    pub fn set_max_dimensions(&self, dimensions: Option<(u32, u32)>) {
        let mut window_state = self.window_state.lock().unwrap();
        window_state.attributes.max_dimensions = dimensions;

        // Make windows re-check the window size bounds.
        if let Some(inner_size) = self.get_inner_size() {
            unsafe {
                let mut rect = RECT { top: 0, left: 0, bottom: inner_size.1 as LONG, right: inner_size.0 as LONG };
                let dw_style = winuser::GetWindowLongA(self.window.0, winuser::GWL_STYLE) as DWORD;
                let b_menu = !winuser::GetMenu(self.window.0).is_null() as BOOL;
                let dw_style_ex = winuser::GetWindowLongA(self.window.0, winuser::GWL_EXSTYLE) as DWORD;
                winuser::AdjustWindowRectEx(&mut rect, dw_style, b_menu, dw_style_ex);
                let outer_x = (rect.right - rect.left).abs() as raw::c_int;
                let outer_y = (rect.top - rect.bottom).abs() as raw::c_int;

                winuser::SetWindowPos(self.window.0, ptr::null_mut(), 0, 0, outer_x, outer_y,
                    winuser::SWP_ASYNCWINDOWPOS | winuser::SWP_NOZORDER | winuser::SWP_NOREPOSITION | winuser::SWP_NOMOVE);
            }
        }
    }

    // TODO: remove
    pub fn platform_display(&self) -> *mut ::libc::c_void {
        panic!()        // Deprecated function ; we don't care anymore
    }
    // TODO: remove
    pub fn platform_window(&self) -> *mut ::libc::c_void {
        self.window.0 as *mut ::libc::c_void
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
            _ => winuser::IDC_ARROW, // use arrow for the missing cases.
        };

        let mut cur = self.window_state.lock().unwrap();
        cur.cursor = cursor_id;
    }

    // TODO: it should be possible to rework this function by using the `execute_in_thread` method
    // of the events loop.
    pub fn set_cursor_state(&self, state: CursorState) -> Result<(), String> {
        let mut current_state = self.window_state.lock().unwrap();

        let foreground_thread_id = unsafe { winuser::GetWindowThreadProcessId(self.window.0, ptr::null_mut()) };
        let current_thread_id = unsafe { processthreadsapi::GetCurrentThreadId() };

        unsafe { winuser::AttachThreadInput(foreground_thread_id, current_thread_id, 1) };

        let res = match (state, current_state.cursor_state) {
            (CursorState::Normal, CursorState::Normal) => Ok(()),
            (CursorState::Hide, CursorState::Hide) => Ok(()),
            (CursorState::Grab, CursorState::Grab) => Ok(()),

            (CursorState::Hide, CursorState::Normal) => {
                current_state.cursor_state = CursorState::Hide;
                Ok(())
            },

            (CursorState::Normal, CursorState::Hide) => {
                current_state.cursor_state = CursorState::Normal;
                Ok(())
            },

            (CursorState::Grab, CursorState::Normal) | (CursorState::Grab, CursorState::Hide) => {
                unsafe {
                    let mut rect = mem::uninitialized();
                    if winuser::GetClientRect(self.window.0, &mut rect) == 0 {
                        return Err(format!("GetWindowRect failed"));
                    }
                    winuser::ClientToScreen(self.window.0, mem::transmute(&mut rect.left));
                    winuser::ClientToScreen(self.window.0, mem::transmute(&mut rect.right));
                    if winuser::ClipCursor(&rect) == 0 {
                        return Err(format!("ClipCursor failed"));
                    }
                    current_state.cursor_state = CursorState::Grab;
                    Ok(())
                }
            },

            (CursorState::Normal, CursorState::Grab) => {
                unsafe {
                    if winuser::ClipCursor(ptr::null()) == 0 {
                        return Err(format!("ClipCursor failed"));
                    }
                    current_state.cursor_state = CursorState::Normal;
                    Ok(())
                }
            },

            _ => unimplemented!(),
        };

        unsafe { winuser::AttachThreadInput(foreground_thread_id, current_thread_id, 0) };

        res
    }

    #[inline]
    pub fn hidpi_factor(&self) -> f32 {
        1.0
    }

    pub fn set_cursor_position(&self, x: i32, y: i32) -> Result<(), ()> {
        let mut point = POINT {
            x: x,
            y: y,
        };

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
    pub fn id(&self) -> WindowId {
        WindowId(self.window.0)
    }

    #[inline]
    pub fn set_maximized(&self, maximized: bool) {
        let mut window_state = self.window_state.lock().unwrap();

        window_state.attributes.maximized = maximized;
        // we only maximized if we are not in fullscreen
        if window_state.attributes.fullscreen.is_some() {
            return;
        }

        let window = self.window.clone();
        unsafe {
            // And because ShowWindow will resize the window
            // We call it in the main thread
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

        if window_state.attributes.fullscreen.is_none() || window_state.saved_window_info.is_none() {
            let mut rect: RECT = mem::zeroed();

            winuser::GetWindowRect(self.window.0, &mut rect);

            window_state.saved_window_info = Some(events_loop::SavedWindowInfo {
                style: winuser::GetWindowLongW(self.window.0, winuser::GWL_STYLE),
                ex_style: winuser::GetWindowLongW(self.window.0, winuser::GWL_EXSTYLE),
                rect,
            });
        }

        // We sync the system maximized state here, it will be used when restoring
        let mut placement: winuser::WINDOWPLACEMENT = mem::zeroed();
        placement.length = mem::size_of::<winuser::WINDOWPLACEMENT>() as u32;
        winuser::GetWindowPlacement(self.window.0, &mut placement);
        window_state.attributes.maximized =
            placement.showCmd == (winuser::SW_SHOWMAXIMIZED as u32);
        let saved_window_info = window_state.saved_window_info.as_ref().unwrap();

        (saved_window_info.style, saved_window_info.ex_style)
    }

    unsafe fn restore_saved_window(&self) {
        let window_state = self.window_state.lock().unwrap();

        // 'saved_window_info' can be None if the window has never been
        // in fullscreen mode before this method gets called.
        if window_state.saved_window_info.is_none() {
            return;
        }

        // Reset original window style and size.  The multiple window size/moves
        // here are ugly, but if SetWindowPos() doesn't redraw, the taskbar won't be
        // repainted.  Better-looking methods welcome.
        let saved_window_info = window_state.saved_window_info.as_ref().unwrap();

        let rect = saved_window_info.rect.clone();
        let window = self.window.clone();
        let (style, ex_style) = (saved_window_info.style, saved_window_info.ex_style);

        let maximized = window_state.attributes.maximized;

        // On restore, resize to the previous saved rect size.
        // And because SetWindowPos will resize the window
        // We call it in the main thread
        self.events_loop_proxy.execute_in_thread(move |_| {
            winuser::SetWindowLongW(window.0, winuser::GWL_STYLE, style);
            winuser::SetWindowLongW(window.0, winuser::GWL_EXSTYLE, ex_style);

            winuser::SetWindowPos(
                window.0,
                ptr::null_mut(),
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                winuser::SWP_ASYNCWINDOWPOS | winuser::SWP_NOZORDER | winuser::SWP_NOACTIVATE
                    | winuser::SWP_FRAMECHANGED,
            );

            // if it was set to maximized when it were fullscreened, we restore it as well
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
                    let pos = inner.get_position();
                    let dim = inner.get_dimensions();
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
                            pos.0,
                            pos.1,
                            dim.0 as i32,
                            dim.1 as i32,
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

        let mut window_state = self.window_state.lock().unwrap();
        window_state.attributes.fullscreen = monitor;
    }

    #[inline]
    pub fn set_decorations(&self, decorations: bool) {
        if let Ok(mut window_state) = self.window_state.lock() {
            if window_state.attributes.decorations == decorations {
                return;
            }

            let style_flags = (winuser::WS_CAPTION | winuser::WS_THICKFRAME) as LONG;
            let ex_style_flags = (winuser::WS_EX_WINDOWEDGE) as LONG;

            // if we are in fullscreen mode, we only change the saved window info
            if window_state.attributes.fullscreen.is_some() {
                {
                    let mut saved = window_state.saved_window_info.as_mut().unwrap();

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

                window_state.attributes.decorations = decorations;
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
                        winuser::SWP_ASYNCWINDOWPOS | winuser::SWP_NOZORDER
                            | winuser::SWP_NOACTIVATE
                            | winuser::SWP_FRAMECHANGED,
                    );
                });
            }

            window_state.attributes.decorations = decorations;
        }
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
    pub fn set_ime_spot(&self, _x: i32, _y: i32) {
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

pub unsafe fn adjust_size(
    (x, y): (u32, u32), style: DWORD, ex_style: DWORD,
) -> (LONG, LONG) {
    let mut rect = RECT { left: 0, right: x as LONG, top: 0, bottom: y as LONG };
    winuser::AdjustWindowRectEx(&mut rect, style, 0, ex_style);
    (rect.right - rect.left, rect.bottom - rect.top)
}

unsafe fn init(
    mut window: WindowAttributes,
    mut pl_attribs: PlatformSpecificWindowBuilderAttributes,
    inserter: events_loop::Inserter,
    events_loop_proxy: events_loop::EventsLoopProxy,
) -> Result<Window, CreationError> {
    let title = OsStr::new(&window.title)
        .encode_wide()
        .chain(Some(0).into_iter())
        .collect::<Vec<_>>();

    let window_icon = {
        let icon = window.window_icon
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

    // building a RECT object with coordinates
    let mut rect = RECT {
        left: 0, right: window.dimensions.unwrap_or((1024, 768)).0 as LONG,
        top: 0, bottom: window.dimensions.unwrap_or((1024, 768)).1 as LONG,
    };

    // computing the style and extended style of the window
    let (ex_style, style) = if !window.decorations {
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

    // adjusting the window coordinates using the style
    winuser::AdjustWindowRectEx(&mut rect, style, 0, ex_style);

    // creating the real window this time, by using the functions in `extra_functions`
    let real_window = {
        let (width, height) = if window.dimensions.is_some() {
            let min_dimensions = window.min_dimensions
                .map(|d| adjust_size(d, style, ex_style))
                .unwrap_or((0, 0));
            let max_dimensions = window.max_dimensions
                .map(|d| adjust_size(d, style, ex_style))
                .unwrap_or((raw::c_int::max_value(), raw::c_int::max_value()));

            (
                Some((rect.right - rect.left).min(max_dimensions.0).max(min_dimensions.0)),
                Some((rect.bottom - rect.top).min(max_dimensions.1).max(min_dimensions.1))
            )
        } else {
            (None, None)
        };

        let mut style = if !window.visible {
            style
        } else {
            style | winuser::WS_VISIBLE
        };

        if pl_attribs.parent.is_some() {
            style |= winuser::WS_CHILD;
        }

        let handle = winuser::CreateWindowExW(ex_style | winuser::WS_EX_ACCEPTFILES,
            class_name.as_ptr(),
            title.as_ptr() as LPCWSTR,
            style | winuser::WS_CLIPSIBLINGS | winuser::WS_CLIPCHILDREN,
            winuser::CW_USEDEFAULT, winuser::CW_USEDEFAULT,
            width.unwrap_or(winuser::CW_USEDEFAULT), height.unwrap_or(winuser::CW_USEDEFAULT),
            pl_attribs.parent.unwrap_or(ptr::null_mut()),
            ptr::null_mut(), libloaderapi::GetModuleHandleW(ptr::null()),
            ptr::null_mut());

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

    let (transparent, maximized, fullscreen) = (
        window.transparent.clone(), window.maximized.clone(), window.fullscreen.clone()
    );

    // Creating a mutex to track the current window state
    let window_state = Arc::new(Mutex::new(events_loop::WindowState {
        cursor: winuser::IDC_ARROW, // use arrow by default
        cursor_state: CursorState::Normal,
        attributes: window,
        mouse_in_window: false,
        saved_window_info: None,
    }));

    // making the window transparent
    if transparent {
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
        window_icon: Cell::new(window_icon),
        taskbar_icon: Cell::new(taskbar_icon),
        events_loop_proxy,
    };

    win.set_maximized(maximized);
    if let Some(_) = fullscreen {
        win.set_fullscreen(fullscreen);
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

    static TASKBAR_LIST: Cell<*mut taskbar::ITaskbarList2> = Cell::new(ptr::null_mut());
}

pub fn com_initialized() {
    COM_INITIALIZED.with(|_| {});
}

// TODO: remove these when they get added to winapi
// https://github.com/retep998/winapi-rs/pull/592
#[allow(non_upper_case_globals)]
#[allow(non_snake_case)]
#[allow(dead_code)]
mod taskbar {
    use super::{IUnknown,IUnknownVtbl,HRESULT, HWND,BOOL};
    DEFINE_GUID!{CLSID_TaskbarList,
        0x56fdf344, 0xfd6d, 0x11d0, 0x95, 0x8a, 0x00, 0x60, 0x97, 0xc9, 0xa0, 0x90}

    RIDL!(#[uuid(0x56fdf342, 0xfd6d, 0x11d0, 0x95, 0x8a, 0x00, 0x60, 0x97, 0xc9, 0xa0, 0x90)]
    interface ITaskbarList(ITaskbarListVtbl): IUnknown(IUnknownVtbl) {
        fn HrInit() -> HRESULT,
        fn AddTab(
            hwnd: HWND,
        ) -> HRESULT,
        fn DeleteTab(
            hwnd: HWND,
        ) -> HRESULT,
        fn ActivateTab(
            hwnd: HWND,
        ) -> HRESULT,
        fn SetActiveAlt(
            hwnd: HWND,
        ) -> HRESULT,
    });

    RIDL!(#[uuid(0x602d4995, 0xb13a, 0x429b, 0xa6, 0x6e, 0x19, 0x35, 0xe4, 0x4f, 0x43, 0x17)]
    interface ITaskbarList2(ITaskbarList2Vtbl): ITaskbarList(ITaskbarListVtbl) {
        fn MarkFullscreenWindow(
            hwnd: HWND,
            fFullscreen: BOOL,
        ) -> HRESULT,
    });
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
                &taskbar::CLSID_TaskbarList,
                ptr::null_mut(),
                combaseapi::CLSCTX_ALL,
                &taskbar::ITaskbarList2::uuidof(),
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
