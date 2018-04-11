#![cfg(target_os = "windows")]
#![allow(non_upper_case_globals)]
#![allow(non_snake_case)]
#![allow(dead_code)]

use std::ffi::OsStr;
use std::io;
use std::mem;
use std::os::raw;
use std::os::windows::ffi::OsStrExt;
use std::ptr;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc::channel;
use std::cell::Cell;

use platform::platform::events_loop;
use platform::platform::EventsLoop;
use platform::platform::PlatformSpecificWindowBuilderAttributes;
use platform::platform::WindowId;

use CreationError;
use CursorState;
use MouseCursor;
use WindowAttributes;
use MonitorId as RootMonitorId;

use winapi::shared::minwindef::{UINT, DWORD, BOOL};
use winapi::shared::windef::{HWND, HDC, RECT, POINT};
use winapi::shared::hidusage;
use winapi::um::{winuser, dwmapi, libloaderapi, processthreadsapi};
use winapi::um::winnt::{LPCWSTR, LONG, HRESULT};
use winapi::um::combaseapi;
use winapi::um::objbase::{COINIT_MULTITHREADED};
use winapi::um::unknwnbase::{IUnknown, IUnknownVtbl};

/// The Win32 implementation of the main `Window` object.
pub struct Window {
    /// Main handle for the window.
    window: WindowWrapper,

    /// The current window state.
    window_state: Arc<Mutex<events_loop::WindowState>>,

    // The events loop proxy.
    events_loop_proxy: events_loop::EventsLoopProxy,
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

// https://blogs.msdn.microsoft.com/oldnewthing/20131017-00/?p=2903
unsafe fn unjust_window_rect(prc: &mut RECT, style: DWORD, ex_style: DWORD) -> BOOL {
    let mut rc: RECT = mem::zeroed();

    winuser::SetRectEmpty(&mut rc);

    let fRc = winuser::AdjustWindowRectEx(&mut rc, style, 0, ex_style);
    if fRc != 0 {
        prc.left -= rc.left;
        prc.top -= rc.top;
        prc.right -= rc.right;
        prc.bottom -= rc.bottom;
    }
    return fRc;
}

impl Window {
    pub fn new(events_loop: &EventsLoop, w_attr: &WindowAttributes,
               pl_attr: &PlatformSpecificWindowBuilderAttributes) -> Result<Window, CreationError>
    {
        let mut w_attr = Some(w_attr.clone());
        let mut pl_attr = Some(pl_attr.clone());

        let (tx, rx) = channel();

        let proxy = events_loop.create_proxy();

        events_loop.execute_in_thread(move |inserter| {
            // We dispatch an `init` function because of code style.
            let win = unsafe { init(w_attr.take().unwrap(), pl_attr.take().unwrap(), inserter, proxy.clone()) };
            let _ = tx.send(win);
        });

        rx.recv().unwrap()
    }

    pub fn set_title(&self, text: &str) {
        unsafe {
            let text = OsStr::new(text).encode_wide().chain(Some(0).into_iter())
                                       .collect::<Vec<_>>();

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
        use std::mem;

        let mut placement: winuser::WINDOWPLACEMENT = unsafe { mem::zeroed() };
        placement.length = mem::size_of::<winuser::WINDOWPLACEMENT>() as UINT;

        if unsafe { winuser::GetWindowPlacement(self.window.0, &mut placement) } == 0 {
            return None
        }

        let ref rect = placement.rcNormalPosition;
        Some((rect.left as i32, rect.top as i32))
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
            MouseCursor::EResize => winuser::IDC_SIZEWE,
            MouseCursor::NResize => winuser::IDC_SIZENS,
            MouseCursor::WResize => winuser::IDC_SIZEWE,
            MouseCursor::SResize => winuser::IDC_SIZENS,
            MouseCursor::EwResize | MouseCursor::ColResize => winuser::IDC_SIZEWE,
            MouseCursor::NsResize | MouseCursor::RowResize => winuser::IDC_SIZENS,
            MouseCursor::Wait | MouseCursor::Progress => winuser::IDC_WAIT,
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
        let window = self.window.clone();
        
        unsafe {            
            // And because ShowWindow will resize the window
            // We call it in the main thread
            self.events_loop_proxy.execute_in_thread(move |_| {
                winuser::ShowWindow(window.0, if maximized { winuser::SW_MAXIMIZE } else {winuser::SW_RESTORE} );
            });            
        }
    }

    unsafe fn set_fullscreen_style(&self) {
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

        let saved_window_info = window_state.saved_window_info.as_ref().unwrap();

        winuser::SetWindowLongW(
            self.window.0,
            winuser::GWL_STYLE,
            ((saved_window_info.style as DWORD) & !(winuser::WS_CAPTION | winuser::WS_THICKFRAME)) as LONG,
        );

        winuser::SetWindowLongW(
            self.window.0,
            winuser::GWL_EXSTYLE,
            ((saved_window_info.ex_style as DWORD)
                & !((winuser::WS_EX_DLGMODALFRAME | winuser::WS_EX_WINDOWEDGE
                    | winuser::WS_EX_CLIENTEDGE | winuser::WS_EX_STATICEDGE)
                    )) as LONG,
        );
    }

    unsafe fn restore_saved_window(&self) {
        let window_state = self.window_state.lock().unwrap();
        // Reset original window style and size.  The multiple window size/moves
        // here are ugly, but if SetWindowPos() doesn't redraw, the taskbar won't be
        // repainted.  Better-looking methods welcome.
        let saved_window_info = window_state.saved_window_info.as_ref().unwrap();

        winuser::SetWindowLongW(self.window.0, winuser::GWL_STYLE, saved_window_info.style);
        winuser::SetWindowLongW(
            self.window.0,
            winuser::GWL_EXSTYLE,
            saved_window_info.ex_style,
        );

        let rect = saved_window_info.rect.clone();
        let window = self.window.clone();

        // On restore, resize to the previous saved rect size.
        // And because SetWindowPos will resize the window
        // We call it in the main thread
        self.events_loop_proxy.execute_in_thread(move |_| {
            winuser::SetWindowPos(
                window.0,
                ptr::null_mut(),
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                winuser::SWP_ASYNCWINDOWPOS | winuser::SWP_NOZORDER | winuser::SWP_NOACTIVATE | winuser::SWP_FRAMECHANGED,
            );
        });
    }
    

    #[inline]
    pub fn set_fullscreen(&self, monitor: Option<RootMonitorId>) {
        unsafe {
            match &monitor {
                &Some(RootMonitorId { ref inner }) => {
                    self.set_fullscreen_style();
                   
                    let pos = inner.get_position();
                    let dim = inner.get_dimensions();

                    winuser::SetWindowPos(
                        self.window.0,
                        ptr::null_mut(),
                        pos.0,
                        pos.1,
                        dim.0 as i32,
                        dim.1 as i32,
                        winuser::SWP_ASYNCWINDOWPOS | winuser::SWP_NOZORDER | winuser::SWP_NOACTIVATE | winuser::SWP_FRAMECHANGED,
                    );
                }
                &None => {
                    self.restore_saved_window();
                }
            }

            mark_fullscreen(self.window.0, monitor.is_some())
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
                    winuser::AdjustWindowRectEx(&mut saved.rect, saved.style as _, 0, saved.ex_style as _);
                }

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

                winuser::SetWindowLongW(self.window.0, winuser::GWL_STYLE, style);
                winuser::SetWindowLongW(self.window.0, winuser::GWL_EXSTYLE, ex_style);
                winuser::AdjustWindowRectEx(&mut rect, style as _, 0, ex_style as _);                
                
                let window = self.window.clone();

                self.events_loop_proxy.execute_in_thread(move |_| {
                    winuser::SetWindowPos(
                        window.0,
                        ptr::null_mut(),
                        rect.left,
                        rect.top,
                        rect.right - rect.left,
                        rect.bottom - rect.top,
                        winuser::SWP_ASYNCWINDOWPOS | winuser::SWP_NOZORDER | winuser::SWP_NOACTIVATE | winuser::SWP_FRAMECHANGED,
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
}

impl Drop for Window {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            // We are sending WM_CLOSE, and our callback will process this by calling DefWindowProcW,
            // which in turn will send a WM_DESTROY.
            winuser::PostMessageW(self.window.0, winuser::WM_CLOSE, 0, 0);
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

unsafe fn init(window: WindowAttributes, pl_attribs: PlatformSpecificWindowBuilderAttributes,
               inserter: events_loop::Inserter, events_loop_proxy: events_loop::EventsLoopProxy) -> Result<Window, CreationError> {
    let title = OsStr::new(&window.title).encode_wide().chain(Some(0).into_iter())
        .collect::<Vec<_>>();

    // registering the window class
    let class_name = register_window_class();

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
                .map(|d| (d.0 as raw::c_int, d.1 as raw::c_int))
                .unwrap_or((0, 0));
            let max_dimensions = window.max_dimensions
                .map(|d| (d.0 as raw::c_int, d.1 as raw::c_int))
                .unwrap_or((raw::c_int::max_value(), raw::c_int::max_value()));

            (
                Some((rect.right - rect.left).min(max_dimensions.0).max(min_dimensions.0)),
                Some((rect.bottom - rect.top).min(max_dimensions.1).max(min_dimensions.1))
            )
        } else {
            (None, None)
        };

        let (x, y) = (None, None);

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
            x.unwrap_or(winuser::CW_USEDEFAULT), y.unwrap_or(winuser::CW_USEDEFAULT),
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

    // Set up raw mouse input
    {
        let mut rid: winuser::RAWINPUTDEVICE = mem::uninitialized();
        rid.usUsagePage = hidusage::HID_USAGE_PAGE_GENERIC;
        rid.usUsage = hidusage::HID_USAGE_GENERIC_MOUSE;
        rid.dwFlags = 0;
        rid.hwndTarget = real_window.0;

        winuser::RegisterRawInputDevices(&rid, 1, mem::size_of::<winuser::RAWINPUTDEVICE>() as u32);
    }

    // Register for touch events if applicable
    {
        let digitizer = winuser::GetSystemMetrics( winuser::SM_DIGITIZER ) as u32;
        if digitizer & winuser::NID_READY != 0 {
            winuser::RegisterTouchWindow( real_window.0, winuser::TWF_WANTPALM );
        }
    }
    
    // Creating a mutex to track the current window state
    let window_state = Arc::new(Mutex::new(events_loop::WindowState {
        cursor: winuser::IDC_ARROW, // use arrow by default
        cursor_state: CursorState::Normal,
        attributes: window.clone(),
        mouse_in_window: false,
        saved_window_info: None,
    }));

    // making the window transparent
    if window.transparent {
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
        events_loop_proxy
    };

    if let Some(_) = window.fullscreen {
        win.set_fullscreen(window.fullscreen);
        force_window_active(win.window.0);
    }

    inserter.insert(win.window.0, win.window_state.clone());
    
    Ok(win)
}

unsafe fn register_window_class() -> Vec<u16> {
    let class_name = OsStr::new("Window Class").encode_wide().chain(Some(0).into_iter())
                                               .collect::<Vec<_>>();

    let class = winuser::WNDCLASSEXW {
        cbSize: mem::size_of::<winuser::WNDCLASSEXW>() as UINT,
        style: winuser::CS_HREDRAW | winuser::CS_VREDRAW | winuser::CS_OWNDC,
        lpfnWndProc: Some(events_loop::callback),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: libloaderapi::GetModuleHandleW(ptr::null()),
        hIcon: ptr::null_mut(),
        hCursor: ptr::null_mut(),       // must be null in order for cursor state to work properly
        hbrBackground: ptr::null_mut(),
        lpszMenuName: ptr::null(),
        lpszClassName: class_name.as_ptr(),
        hIconSm: ptr::null_mut(),
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

// TODO: remove these when they get added to winapi
// https://github.com/retep998/winapi-rs/pull/592
DEFINE_GUID!{CLSID_TaskbarList,
    0x56fdf344, 0xfd6d, 0x11d0, 0x95, 0x8a, 0x00, 0x60, 0x97, 0xc9, 0xa0, 0x90}

RIDL!(
#[uuid(0x56fdf342, 0xfd6d, 0x11d0, 0x95, 0x8a, 0x00, 0x60, 0x97, 0xc9, 0xa0, 0x90)]
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

RIDL!(
#[uuid(0x602d4995, 0xb13a, 0x429b, 0xa6, 0x6e, 0x19, 0x35, 0xe4, 0x4f, 0x43, 0x17)]
interface ITaskbarList2(ITaskbarList2Vtbl): ITaskbarList(ITaskbarListVtbl) {
    fn MarkFullscreenWindow(
        hwnd: HWND,
        fFullscreen: BOOL,
    ) -> HRESULT,    
});

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

    TASKBAR_LIST.with(|task_bar_list_ptr|{
        let mut task_bar_list = task_bar_list_ptr.get();

        if task_bar_list == ptr::null_mut() {
            use winapi::Interface;
            use winapi::shared::winerror::{S_OK};    

            let hr = combaseapi::CoCreateInstance(
                &CLSID_TaskbarList,
                ptr::null_mut(), combaseapi::CLSCTX_ALL,
                &ITaskbarList2::uuidof(),
                &mut task_bar_list as *mut _ as *mut _);

            if hr != S_OK || (*task_bar_list).HrInit() != S_OK {
                // In some old windows, the taskbar object could not be created, we just ignore it
                return;
            }
            task_bar_list_ptr.set(task_bar_list)
        }

        task_bar_list = task_bar_list_ptr.get();
        (*task_bar_list).MarkFullscreenWindow(handle, if fullscreen {1} else {0} );
    })
}

unsafe fn force_window_active(handle: HWND) {
    // In some situation, calling SetForegroundWindow could not bring up the window,
    // This is a little hack which can "steal" the foreground window permission
    // We only call this function in the window creation, so it should be fine.
    // See : https://stackoverflow.com/questions/10740346/setforegroundwindow-only-working-while-visual-studio-is-open
    const ALT : i32 = 0xA4;
    const EXTENDEDKEY : u32 = 0x1;
    const KEYUP : u32 = 0x2;

    // Simulate a key press
    winuser::keybd_event(ALT as _, 0x45, EXTENDEDKEY | 0, 0);

    // Simulate a key release
    winuser::keybd_event(0xA4, 0x45, EXTENDEDKEY | KEYUP, 0);

    winuser::SetForegroundWindow(handle);    
}
