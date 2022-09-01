#![cfg(target_os = "windows")]

use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, Win32WindowHandle, WindowsDisplayHandle,
};
use std::{
    cell::Cell,
    ffi::c_void,
    io, mem, panic, ptr,
    sync::{mpsc::channel, Arc, Mutex, MutexGuard},
};

use windows_sys::Win32::{
    Foundation::{
        HINSTANCE, HWND, LPARAM, OLE_E_WRONGCOMPOBJ, POINT, POINTS, RECT, RPC_E_CHANGED_MODE, S_OK,
        WPARAM,
    },
    Graphics::{
        Dwm::{DwmEnableBlurBehindWindow, DWM_BB_BLURREGION, DWM_BB_ENABLE, DWM_BLURBEHIND},
        Gdi::{
            ChangeDisplaySettingsExW, ClientToScreen, CreateRectRgn, DeleteObject, InvalidateRgn,
            RedrawWindow, CDS_FULLSCREEN, DISP_CHANGE_BADFLAGS, DISP_CHANGE_BADMODE,
            DISP_CHANGE_BADPARAM, DISP_CHANGE_FAILED, DISP_CHANGE_SUCCESSFUL, RDW_INTERNALPAINT,
        },
    },
    System::{
        Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
        },
        Ole::{OleInitialize, RegisterDragDrop},
    },
    UI::{
        Input::{
            KeyboardAndMouse::{
                EnableWindow, GetActiveWindow, MapVirtualKeyW, ReleaseCapture, SendInput, INPUT,
                INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP,
                VK_LMENU, VK_MENU,
            },
            Touch::{RegisterTouchWindow, TWF_WANTPALM},
        },
        WindowsAndMessaging::{
            CreateWindowExW, FlashWindowEx, GetClientRect, GetCursorPos, GetForegroundWindow,
            GetSystemMetrics, GetWindowPlacement, IsWindowVisible, LoadCursorW, PeekMessageW,
            PostMessageW, RegisterClassExW, SetCursor, SetCursorPos, SetForegroundWindow,
            SetWindowPlacement, SetWindowPos, SetWindowTextW, CS_HREDRAW, CS_VREDRAW,
            CW_USEDEFAULT, FLASHWINFO, FLASHW_ALL, FLASHW_STOP, FLASHW_TIMERNOFG, FLASHW_TRAY,
            GWLP_HINSTANCE, HTCAPTION, MAPVK_VK_TO_VSC, NID_READY, PM_NOREMOVE, SM_DIGITIZER,
            SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, WM_NCLBUTTONDOWN,
            WNDCLASSEXW,
        },
    },
};

use crate::{
    dpi::{PhysicalPosition, PhysicalSize, Position, Size},
    error::{ExternalError, NotSupportedError, OsError as RootOsError},
    icon::Icon,
    monitor::MonitorHandle as RootMonitorHandle,
    platform_impl::platform::{
        dark_mode::try_theme,
        definitions::{
            CLSID_TaskbarList, IID_ITaskbarList, IID_ITaskbarList2, ITaskbarList, ITaskbarList2,
        },
        dpi::{dpi_to_scale_factor, enable_non_client_dpi_scaling, hwnd_dpi},
        drop_handler::FileDropHandler,
        event_loop::{self, EventLoopWindowTarget, DESTROY_MSG_ID},
        icon::{self, IconType},
        ime::ImeContext,
        monitor, util,
        window_state::{CursorFlags, SavedWindow, WindowFlags, WindowState},
        Parent, PlatformSpecificWindowBuilderAttributes, WindowId,
    },
    window::{CursorGrabMode, CursorIcon, Fullscreen, Theme, UserAttentionType, WindowAttributes},
};

/// The Win32 implementation of the main `Window` object.
pub struct Window {
    /// Main handle for the window.
    window: WindowWrapper,

    /// The current window state.
    window_state: Arc<Mutex<WindowState>>,

    // The events loop proxy.
    thread_executor: event_loop::EventLoopThreadExecutor,
}

impl Window {
    pub(crate) fn new<T: 'static>(
        event_loop: &EventLoopWindowTarget<T>,
        w_attr: WindowAttributes,
        pl_attr: PlatformSpecificWindowBuilderAttributes,
    ) -> Result<Window, RootOsError> {
        // We dispatch an `init` function because of code style.
        // First person to remove the need for cloning here gets a cookie!
        //
        // done. you owe me -- ossi
        unsafe { init(w_attr, pl_attr, event_loop) }
    }

    fn window_state_lock(&self) -> MutexGuard<'_, WindowState> {
        self.window_state.lock().unwrap()
    }

    pub fn set_title(&self, text: &str) {
        let wide_text = util::encode_wide(text);
        unsafe {
            SetWindowTextW(self.hwnd(), wide_text.as_ptr());
        }
    }

    #[inline]
    pub fn set_visible(&self, visible: bool) {
        let window = self.window.clone();
        let window_state = Arc::clone(&self.window_state);
        self.thread_executor.execute_in_thread(move || {
            let _ = &window;
            WindowState::set_window_flags(window_state.lock().unwrap(), window.0, |f| {
                f.set(WindowFlags::VISIBLE, visible)
            });
        });
    }

    #[inline]
    pub fn is_visible(&self) -> Option<bool> {
        Some(unsafe { IsWindowVisible(self.window.0) == 1 })
    }

    #[inline]
    pub fn request_redraw(&self) {
        unsafe {
            RedrawWindow(self.hwnd(), ptr::null(), 0, RDW_INTERNALPAINT);
        }
    }

    #[inline]
    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        util::WindowArea::Outer.get_rect(self.hwnd())
            .map(|rect| Ok(PhysicalPosition::new(rect.left as i32, rect.top as i32)))
            .expect("Unexpected GetWindowRect failure; please report this error to https://github.com/rust-windowing/winit")
    }

    #[inline]
    pub fn inner_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        let mut position: POINT = unsafe { mem::zeroed() };
        if unsafe { ClientToScreen(self.hwnd(), &mut position) } == false.into() {
            panic!("Unexpected ClientToScreen failure: please report this error to https://github.com/rust-windowing/winit")
        }
        Ok(PhysicalPosition::new(position.x as i32, position.y as i32))
    }

    #[inline]
    pub fn set_outer_position(&self, position: Position) {
        let (x, y): (i32, i32) = position.to_physical::<i32>(self.scale_factor()).into();

        let window_state = Arc::clone(&self.window_state);
        let window = self.window.clone();
        self.thread_executor.execute_in_thread(move || {
            let _ = &window;
            WindowState::set_window_flags(window_state.lock().unwrap(), window.0, |f| {
                f.set(WindowFlags::MAXIMIZED, false)
            });
        });

        unsafe {
            SetWindowPos(
                self.hwnd(),
                0,
                x,
                y,
                0,
                0,
                SWP_ASYNCWINDOWPOS | SWP_NOZORDER | SWP_NOSIZE | SWP_NOACTIVATE,
            );
            InvalidateRgn(self.hwnd(), 0, false.into());
        }
    }

    #[inline]
    pub fn inner_size(&self) -> PhysicalSize<u32> {
        let mut rect: RECT = unsafe { mem::zeroed() };
        if unsafe { GetClientRect(self.hwnd(), &mut rect) } == false.into() {
            panic!("Unexpected GetClientRect failure: please report this error to https://github.com/rust-windowing/winit")
        }
        PhysicalSize::new(
            (rect.right - rect.left) as u32,
            (rect.bottom - rect.top) as u32,
        )
    }

    #[inline]
    pub fn outer_size(&self) -> PhysicalSize<u32> {
        util::WindowArea::Outer
            .get_rect(self.hwnd())
            .map(|rect| {
                PhysicalSize::new(
                    (rect.right - rect.left) as u32,
                    (rect.bottom - rect.top) as u32,
                )
            })
            .unwrap()
    }

    #[inline]
    pub fn set_inner_size(&self, size: Size) {
        let scale_factor = self.scale_factor();
        let physical_size = size.to_physical::<u32>(scale_factor);

        let window_state = Arc::clone(&self.window_state);
        let window = self.window.clone();
        self.thread_executor.execute_in_thread(move || {
            let _ = &window;
            WindowState::set_window_flags(window_state.lock().unwrap(), window.0, |f| {
                f.set(WindowFlags::MAXIMIZED, false)
            });
        });

        let window_flags = self.window_state_lock().window_flags;
        window_flags.set_size(self.hwnd(), physical_size);
    }

    #[inline]
    pub fn set_min_inner_size(&self, size: Option<Size>) {
        self.window_state_lock().min_size = size;
        // Make windows re-check the window size bounds.
        let size = self.inner_size();
        self.set_inner_size(size.into());
    }

    #[inline]
    pub fn set_max_inner_size(&self, size: Option<Size>) {
        self.window_state_lock().max_size = size;
        // Make windows re-check the window size bounds.
        let size = self.inner_size();
        self.set_inner_size(size.into());
    }

    #[inline]
    pub fn set_resizable(&self, resizable: bool) {
        let window = self.window.clone();
        let window_state = Arc::clone(&self.window_state);

        self.thread_executor.execute_in_thread(move || {
            let _ = &window;
            WindowState::set_window_flags(window_state.lock().unwrap(), window.0, |f| {
                f.set(WindowFlags::RESIZABLE, resizable)
            });
        });
    }

    #[inline]
    pub fn is_resizable(&self) -> bool {
        let window_state = self.window_state_lock();
        window_state.window_flags.contains(WindowFlags::RESIZABLE)
    }

    /// Returns the `hwnd` of this window.
    #[inline]
    pub fn hwnd(&self) -> HWND {
        self.window.0
    }

    #[inline]
    pub fn hinstance(&self) -> HINSTANCE {
        unsafe { super::get_window_long(self.hwnd(), GWLP_HINSTANCE) }
    }

    #[inline]
    pub fn raw_window_handle(&self) -> RawWindowHandle {
        let mut window_handle = Win32WindowHandle::empty();
        window_handle.hwnd = self.window.0 as *mut _;
        window_handle.hinstance = self.hinstance() as *mut _;
        RawWindowHandle::Win32(window_handle)
    }

    #[inline]
    pub fn raw_display_handle(&self) -> RawDisplayHandle {
        RawDisplayHandle::Windows(WindowsDisplayHandle::empty())
    }

    #[inline]
    pub fn set_cursor_icon(&self, cursor: CursorIcon) {
        self.window_state_lock().mouse.cursor = cursor;
        self.thread_executor.execute_in_thread(move || unsafe {
            let cursor = LoadCursorW(0, cursor.to_windows_cursor());
            SetCursor(cursor);
        });
    }

    #[inline]
    pub fn set_cursor_grab(&self, mode: CursorGrabMode) -> Result<(), ExternalError> {
        let confine = match mode {
            CursorGrabMode::None => false,
            CursorGrabMode::Confined => true,
            CursorGrabMode::Locked => {
                return Err(ExternalError::NotSupported(NotSupportedError::new()))
            }
        };

        let window = self.window.clone();
        let window_state = Arc::clone(&self.window_state);
        let (tx, rx) = channel();

        self.thread_executor.execute_in_thread(move || {
            let _ = &window;
            let result = window_state
                .lock()
                .unwrap()
                .mouse
                .set_cursor_flags(window.0, |f| f.set(CursorFlags::GRABBED, confine))
                .map_err(|e| ExternalError::Os(os_error!(e)));
            let _ = tx.send(result);
        });
        rx.recv().unwrap()
    }

    #[inline]
    pub fn set_cursor_visible(&self, visible: bool) {
        let window = self.window.clone();
        let window_state = Arc::clone(&self.window_state);
        let (tx, rx) = channel();

        self.thread_executor.execute_in_thread(move || {
            let _ = &window;
            let result = window_state
                .lock()
                .unwrap()
                .mouse
                .set_cursor_flags(window.0, |f| f.set(CursorFlags::HIDDEN, !visible))
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
        rx.recv().unwrap().ok();
    }

    #[inline]
    pub fn scale_factor(&self) -> f64 {
        self.window_state_lock().scale_factor
    }

    #[inline]
    pub fn set_cursor_position(&self, position: Position) -> Result<(), ExternalError> {
        let scale_factor = self.scale_factor();
        let (x, y) = position.to_physical::<i32>(scale_factor).into();

        let mut point = POINT { x, y };
        unsafe {
            if ClientToScreen(self.hwnd(), &mut point) == false.into() {
                return Err(ExternalError::Os(os_error!(io::Error::last_os_error())));
            }
            if SetCursorPos(point.x, point.y) == false.into() {
                return Err(ExternalError::Os(os_error!(io::Error::last_os_error())));
            }
        }
        Ok(())
    }

    #[inline]
    pub fn drag_window(&self) -> Result<(), ExternalError> {
        unsafe {
            let points = {
                let mut pos = mem::zeroed();
                GetCursorPos(&mut pos);
                pos
            };
            let points = POINTS {
                x: points.x as i16,
                y: points.y as i16,
            };
            ReleaseCapture();
            PostMessageW(
                self.hwnd(),
                WM_NCLBUTTONDOWN,
                HTCAPTION as WPARAM,
                &points as *const _ as LPARAM,
            );
        }

        Ok(())
    }

    #[inline]
    pub fn set_cursor_hittest(&self, hittest: bool) -> Result<(), ExternalError> {
        let window = self.window.clone();
        let window_state = Arc::clone(&self.window_state);
        self.thread_executor.execute_in_thread(move || {
            WindowState::set_window_flags(window_state.lock().unwrap(), window.0, |f| {
                f.set(WindowFlags::IGNORE_CURSOR_EVENT, !hittest)
            });
        });

        Ok(())
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        WindowId(self.hwnd())
    }

    #[inline]
    pub fn set_minimized(&self, minimized: bool) {
        let window = self.window.clone();
        let window_state = Arc::clone(&self.window_state);

        self.thread_executor.execute_in_thread(move || {
            let _ = &window;
            WindowState::set_window_flags(window_state.lock().unwrap(), window.0, |f| {
                f.set(WindowFlags::MINIMIZED, minimized)
            });
        });
    }

    #[inline]
    pub fn set_maximized(&self, maximized: bool) {
        let window = self.window.clone();
        let window_state = Arc::clone(&self.window_state);

        self.thread_executor.execute_in_thread(move || {
            let _ = &window;
            WindowState::set_window_flags(window_state.lock().unwrap(), window.0, |f| {
                f.set(WindowFlags::MAXIMIZED, maximized)
            });
        });
    }

    #[inline]
    pub fn is_maximized(&self) -> bool {
        let window_state = self.window_state_lock();
        window_state.window_flags.contains(WindowFlags::MAXIMIZED)
    }

    #[inline]
    pub fn fullscreen(&self) -> Option<Fullscreen> {
        let window_state = self.window_state_lock();
        window_state.fullscreen.clone()
    }

    #[inline]
    pub fn set_fullscreen(&self, fullscreen: Option<Fullscreen>) {
        let window = self.window.clone();
        let window_state = Arc::clone(&self.window_state);

        let mut window_state_lock = window_state.lock().unwrap();
        let old_fullscreen = window_state_lock.fullscreen.clone();
        if window_state_lock.fullscreen == fullscreen {
            return;
        }
        window_state_lock.fullscreen = fullscreen.clone();
        drop(window_state_lock);

        self.thread_executor.execute_in_thread(move || {
            let _ = &window;
            // Change video mode if we're transitioning to or from exclusive
            // fullscreen
            match (&old_fullscreen, &fullscreen) {
                (_, Some(Fullscreen::Exclusive(video_mode))) => {
                    let monitor = video_mode.monitor();
                    let monitor_info = monitor::get_monitor_info(monitor.inner.hmonitor()).unwrap();

                    let res = unsafe {
                        ChangeDisplaySettingsExW(
                            monitor_info.szDevice.as_ptr(),
                            &*video_mode.video_mode.native_video_mode,
                            0,
                            CDS_FULLSCREEN,
                            ptr::null(),
                        )
                    };

                    debug_assert!(res != DISP_CHANGE_BADFLAGS);
                    debug_assert!(res != DISP_CHANGE_BADMODE);
                    debug_assert!(res != DISP_CHANGE_BADPARAM);
                    debug_assert!(res != DISP_CHANGE_FAILED);
                    assert_eq!(res, DISP_CHANGE_SUCCESSFUL);
                }
                (Some(Fullscreen::Exclusive(_)), _) => {
                    let res = unsafe {
                        ChangeDisplaySettingsExW(
                            ptr::null(),
                            ptr::null(),
                            0,
                            CDS_FULLSCREEN,
                            ptr::null(),
                        )
                    };

                    debug_assert!(res != DISP_CHANGE_BADFLAGS);
                    debug_assert!(res != DISP_CHANGE_BADMODE);
                    debug_assert!(res != DISP_CHANGE_BADPARAM);
                    debug_assert!(res != DISP_CHANGE_FAILED);
                    assert_eq!(res, DISP_CHANGE_SUCCESSFUL);
                }
                _ => (),
            }

            unsafe {
                // There are some scenarios where calling `ChangeDisplaySettingsExW` takes long
                // enough to execute that the DWM thinks our program has frozen and takes over
                // our program's window. When that happens, the `SetWindowPos` call below gets
                // eaten and the window doesn't get set to the proper fullscreen position.
                //
                // Calling `PeekMessageW` here notifies Windows that our process is still running
                // fine, taking control back from the DWM and ensuring that the `SetWindowPos` call
                // below goes through.
                let mut msg = mem::zeroed();
                PeekMessageW(&mut msg, 0, 0, 0, PM_NOREMOVE);
            }

            // Update window style
            WindowState::set_window_flags(window_state.lock().unwrap(), window.0, |f| {
                f.set(
                    WindowFlags::MARKER_EXCLUSIVE_FULLSCREEN,
                    matches!(fullscreen, Some(Fullscreen::Exclusive(_))),
                );
                f.set(
                    WindowFlags::MARKER_BORDERLESS_FULLSCREEN,
                    matches!(fullscreen, Some(Fullscreen::Borderless(_))),
                );
            });

            // Mark as fullscreen window wrt to z-order
            //
            // this needs to be called before the below fullscreen SetWindowPos as this itself
            // will generate WM_SIZE messages of the old window size that can race with what we set below
            unsafe {
                taskbar_mark_fullscreen(window.0, fullscreen.is_some());
            }

            // Update window bounds
            match &fullscreen {
                Some(fullscreen) => {
                    // Save window bounds before entering fullscreen
                    let placement = unsafe {
                        let mut placement = mem::zeroed();
                        GetWindowPlacement(window.0, &mut placement);
                        placement
                    };

                    window_state.lock().unwrap().saved_window = Some(SavedWindow { placement });

                    let monitor = match &fullscreen {
                        Fullscreen::Exclusive(video_mode) => video_mode.monitor(),
                        Fullscreen::Borderless(Some(monitor)) => monitor.clone(),
                        Fullscreen::Borderless(None) => RootMonitorHandle {
                            inner: monitor::current_monitor(window.0),
                        },
                    };

                    let position: (i32, i32) = monitor.position().into();
                    let size: (u32, u32) = monitor.size().into();

                    unsafe {
                        SetWindowPos(
                            window.0,
                            0,
                            position.0,
                            position.1,
                            size.0 as i32,
                            size.1 as i32,
                            SWP_ASYNCWINDOWPOS | SWP_NOZORDER,
                        );
                        InvalidateRgn(window.0, 0, false.into());
                    }
                }
                None => {
                    let mut window_state_lock = window_state.lock().unwrap();
                    if let Some(SavedWindow { placement }) = window_state_lock.saved_window.take() {
                        drop(window_state_lock);
                        unsafe {
                            SetWindowPlacement(window.0, &placement);
                            InvalidateRgn(window.0, 0, false.into());
                        }
                    }
                }
            }
        });
    }

    #[inline]
    pub fn set_decorations(&self, decorations: bool) {
        let window = self.window.clone();
        let window_state = Arc::clone(&self.window_state);

        self.thread_executor.execute_in_thread(move || {
            let _ = &window;
            WindowState::set_window_flags(window_state.lock().unwrap(), window.0, |f| {
                f.set(WindowFlags::MARKER_DECORATIONS, decorations)
            });
        });
    }

    #[inline]
    pub fn is_decorated(&self) -> bool {
        let window_state = self.window_state_lock();
        window_state
            .window_flags
            .contains(WindowFlags::MARKER_DECORATIONS)
    }

    #[inline]
    pub fn set_always_on_top(&self, always_on_top: bool) {
        let window = self.window.clone();
        let window_state = Arc::clone(&self.window_state);

        self.thread_executor.execute_in_thread(move || {
            let _ = &window;
            WindowState::set_window_flags(window_state.lock().unwrap(), window.0, |f| {
                f.set(WindowFlags::ALWAYS_ON_TOP, always_on_top)
            });
        });
    }

    #[inline]
    pub fn current_monitor(&self) -> Option<RootMonitorHandle> {
        Some(RootMonitorHandle {
            inner: monitor::current_monitor(self.hwnd()),
        })
    }

    #[inline]
    pub fn set_window_icon(&self, window_icon: Option<Icon>) {
        if let Some(ref window_icon) = window_icon {
            window_icon
                .inner
                .set_for_window(self.hwnd(), IconType::Small);
        } else {
            icon::unset_for_window(self.hwnd(), IconType::Small);
        }
        self.window_state_lock().window_icon = window_icon;
    }

    #[inline]
    pub fn set_enable(&self, enabled: bool) {
        unsafe { EnableWindow(self.hwnd(), enabled.into()) };
    }

    #[inline]
    pub fn set_taskbar_icon(&self, taskbar_icon: Option<Icon>) {
        if let Some(ref taskbar_icon) = taskbar_icon {
            taskbar_icon
                .inner
                .set_for_window(self.hwnd(), IconType::Big);
        } else {
            icon::unset_for_window(self.hwnd(), IconType::Big);
        }
        self.window_state_lock().taskbar_icon = taskbar_icon;
    }

    #[inline]
    pub fn set_ime_position(&self, spot: Position) {
        unsafe {
            ImeContext::current(self.hwnd()).set_ime_position(spot, self.scale_factor());
        }
    }

    #[inline]
    pub fn set_ime_allowed(&self, allowed: bool) {
        self.window_state_lock().ime_allowed = allowed;
        unsafe {
            ImeContext::set_ime_allowed(self.hwnd(), allowed);
        }
    }

    #[inline]
    pub fn request_user_attention(&self, request_type: Option<UserAttentionType>) {
        let window = self.window.clone();
        let active_window_handle = unsafe { GetActiveWindow() };
        if window.0 == active_window_handle {
            return;
        }

        self.thread_executor.execute_in_thread(move || unsafe {
            let _ = &window;
            let (flags, count) = request_type
                .map(|ty| match ty {
                    UserAttentionType::Critical => (FLASHW_ALL | FLASHW_TIMERNOFG, u32::MAX),
                    UserAttentionType::Informational => (FLASHW_TRAY | FLASHW_TIMERNOFG, 0),
                })
                .unwrap_or((FLASHW_STOP, 0));

            let flash_info = FLASHWINFO {
                cbSize: mem::size_of::<FLASHWINFO>() as u32,
                hwnd: window.0,
                dwFlags: flags,
                uCount: count,
                dwTimeout: 0,
            };
            FlashWindowEx(&flash_info);
        });
    }

    #[inline]
    pub fn theme(&self) -> Theme {
        self.window_state_lock().current_theme
    }

    #[inline]
    pub fn set_skip_taskbar(&self, skip: bool) {
        self.window_state_lock().skip_taskbar = skip;
        unsafe { set_skip_taskbar(self.hwnd(), skip) };
    }

    #[inline]
    pub fn set_undecorated_shadow(&self, shadow: bool) {
        let window = self.window.clone();
        let window_state = Arc::clone(&self.window_state);

        self.thread_executor.execute_in_thread(move || {
            let _ = &window;
            WindowState::set_window_flags(window_state.lock().unwrap(), window.0, |f| {
                f.set(WindowFlags::MARKER_UNDECORATED_SHADOW, shadow)
            });
        });
    }

    #[inline]
    pub fn focus_window(&self) {
        let window = self.window.clone();
        let window_flags = self.window_state_lock().window_flags();

        let is_visible = window_flags.contains(WindowFlags::VISIBLE);
        let is_minimized = window_flags.contains(WindowFlags::MINIMIZED);
        let is_foreground = window.0 == unsafe { GetForegroundWindow() };

        if is_visible && !is_minimized && !is_foreground {
            unsafe { force_window_active(window.0) };
        }
    }
}

impl Drop for Window {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            // The window must be destroyed from the same thread that created it, so we send a
            // custom message to be handled by our callback to do the actual work.
            PostMessageW(self.hwnd(), *DESTROY_MSG_ID, 0, 0);
        }
    }
}

/// A simple non-owning wrapper around a window.
#[doc(hidden)]
#[derive(Clone)]
pub struct WindowWrapper(HWND);

// Send and Sync are not implemented for HWND and HDC, we have to wrap it and implement them manually.
// For more info see:
// https://github.com/retep998/winapi-rs/issues/360
// https://github.com/retep998/winapi-rs/issues/396
unsafe impl Sync for WindowWrapper {}
unsafe impl Send for WindowWrapper {}

pub(super) struct InitData<'a, T: 'static> {
    // inputs
    pub event_loop: &'a EventLoopWindowTarget<T>,
    pub attributes: WindowAttributes,
    pub pl_attribs: PlatformSpecificWindowBuilderAttributes,
    pub window_flags: WindowFlags,
    // outputs
    pub window: Option<Window>,
}

impl<'a, T: 'static> InitData<'a, T> {
    unsafe fn create_window(&self, window: HWND) -> Window {
        // Register for touch events if applicable
        {
            let digitizer = GetSystemMetrics(SM_DIGITIZER) as u32;
            if digitizer & NID_READY != 0 {
                RegisterTouchWindow(window, TWF_WANTPALM);
            }
        }

        let dpi = hwnd_dpi(window);
        let scale_factor = dpi_to_scale_factor(dpi);

        // If the system theme is dark, we need to set the window theme now
        // before we update the window flags (and possibly show the
        // window for the first time).
        let current_theme = try_theme(window, self.pl_attribs.preferred_theme);

        let window_state = {
            let window_state = WindowState::new(
                &self.attributes,
                self.pl_attribs.taskbar_icon.clone(),
                scale_factor,
                current_theme,
                self.pl_attribs.preferred_theme,
            );
            let window_state = Arc::new(Mutex::new(window_state));
            WindowState::set_window_flags(window_state.lock().unwrap(), window, |f| {
                *f = self.window_flags
            });
            window_state
        };

        enable_non_client_dpi_scaling(window);

        ImeContext::set_ime_allowed(window, false);

        Window {
            window: WindowWrapper(window),
            window_state,
            thread_executor: self.event_loop.create_thread_executor(),
        }
    }

    unsafe fn create_window_data(&self, win: &Window) -> event_loop::WindowData<T> {
        let file_drop_handler = if self.pl_attribs.drag_and_drop {
            let ole_init_result = OleInitialize(ptr::null_mut());
            // It is ok if the initialize result is `S_FALSE` because it might happen that
            // multiple windows are created on the same thread.
            if ole_init_result == OLE_E_WRONGCOMPOBJ {
                panic!("OleInitialize failed! Result was: `OLE_E_WRONGCOMPOBJ`");
            } else if ole_init_result == RPC_E_CHANGED_MODE {
                panic!(
                    "OleInitialize failed! Result was: `RPC_E_CHANGED_MODE`. \
                    Make sure other crates are not using multithreaded COM library \
                    on the same thread or disable drag and drop support."
                );
            }

            let file_drop_runner = self.event_loop.runner_shared.clone();
            let file_drop_handler = FileDropHandler::new(
                win.window.0,
                Box::new(move |event| {
                    if let Ok(e) = event.map_nonuser_event() {
                        file_drop_runner.send_event(e)
                    }
                }),
            );

            let handler_interface_ptr =
                &mut (*file_drop_handler.data).interface as *mut _ as *mut c_void;

            assert_eq!(RegisterDragDrop(win.window.0, handler_interface_ptr), S_OK);
            Some(file_drop_handler)
        } else {
            None
        };

        self.event_loop.runner_shared.register_window(win.window.0);

        event_loop::WindowData {
            window_state: win.window_state.clone(),
            event_loop_runner: self.event_loop.runner_shared.clone(),
            _file_drop_handler: file_drop_handler,
            userdata_removed: Cell::new(false),
            recurse_depth: Cell::new(0),
        }
    }

    // Returns a pointer to window user data on success.
    // The user data will be registered for the window and can be accessed within the window event callback.
    pub unsafe fn on_nccreate(&mut self, window: HWND) -> Option<isize> {
        let runner = self.event_loop.runner_shared.clone();
        let result = runner.catch_unwind(|| {
            let window = self.create_window(window);
            let window_data = self.create_window_data(&window);
            (window, window_data)
        });

        result.map(|(win, userdata)| {
            self.window = Some(win);
            let userdata = Box::into_raw(Box::new(userdata));
            userdata as _
        })
    }

    pub unsafe fn on_create(&mut self) {
        let win = self.window.as_mut().expect("failed window creation");

        // making the window transparent
        if self.attributes.transparent && !self.pl_attribs.no_redirection_bitmap {
            // Empty region for the blur effect, so the window is fully transparent
            let region = CreateRectRgn(0, 0, -1, -1);

            let bb = DWM_BLURBEHIND {
                dwFlags: DWM_BB_ENABLE | DWM_BB_BLURREGION,
                fEnable: true.into(),
                hRgnBlur: region,
                fTransitionOnMaximized: false.into(),
            };
            let hr = DwmEnableBlurBehindWindow(win.hwnd(), &bb);
            if hr < 0 {
                warn!(
                    "Setting transparent window is failed. HRESULT Code: 0x{:X}",
                    hr
                );
            }
            DeleteObject(region);
        }

        win.set_skip_taskbar(self.pl_attribs.skip_taskbar);

        let attributes = self.attributes.clone();

        // Set visible before setting the size to ensure the
        // attribute is correctly applied.
        win.set_visible(attributes.visible);

        if attributes.fullscreen.is_some() {
            win.set_fullscreen(attributes.fullscreen);
            force_window_active(win.window.0);
        } else {
            let size = attributes
                .inner_size
                .unwrap_or_else(|| PhysicalSize::new(800, 600).into());
            let max_size = attributes
                .max_inner_size
                .unwrap_or_else(|| PhysicalSize::new(f64::MAX, f64::MAX).into());
            let min_size = attributes
                .min_inner_size
                .unwrap_or_else(|| PhysicalSize::new(0, 0).into());
            let clamped_size = Size::clamp(size, min_size, max_size, win.scale_factor());
            win.set_inner_size(clamped_size);

            if attributes.maximized {
                // Need to set MAXIMIZED after setting `inner_size` as
                // `Window::set_inner_size` changes MAXIMIZED to false.
                win.set_maximized(true);
            }
        }

        // let margins = MARGINS {
        //     cxLeftWidth: 1,
        //     cxRightWidth: 1,
        //     cyTopHeight: 1,
        //     cyBottomHeight: 1,
        // };
        // dbg!(DwmExtendFrameIntoClientArea(win.hwnd(), &margins as *const _));

        if let Some(position) = attributes.position {
            win.set_outer_position(position);
        }
    }
}
unsafe fn init<T>(
    attributes: WindowAttributes,
    pl_attribs: PlatformSpecificWindowBuilderAttributes,
    event_loop: &EventLoopWindowTarget<T>,
) -> Result<Window, RootOsError>
where
    T: 'static,
{
    let title = util::encode_wide(&attributes.title);

    let class_name = register_window_class::<T>(&attributes.window_icon, &pl_attribs.taskbar_icon);

    let mut window_flags = WindowFlags::empty();
    window_flags.set(WindowFlags::MARKER_DECORATIONS, attributes.decorations);
    window_flags.set(
        WindowFlags::MARKER_UNDECORATED_SHADOW,
        pl_attribs.decoration_shadow,
    );
    window_flags.set(WindowFlags::ALWAYS_ON_TOP, attributes.always_on_top);
    window_flags.set(
        WindowFlags::NO_BACK_BUFFER,
        pl_attribs.no_redirection_bitmap,
    );
    window_flags.set(WindowFlags::TRANSPARENT, attributes.transparent);
    // WindowFlags::VISIBLE and MAXIMIZED are set down below after the window has been configured.
    window_flags.set(WindowFlags::RESIZABLE, attributes.resizable);

    let parent = match pl_attribs.parent {
        Parent::ChildOf(parent) => {
            window_flags.set(WindowFlags::CHILD, true);
            if pl_attribs.menu.is_some() {
                warn!("Setting a menu on a child window is unsupported");
            }
            Some(parent)
        }
        Parent::OwnedBy(parent) => {
            window_flags.set(WindowFlags::POPUP, true);
            Some(parent)
        }
        Parent::None => {
            window_flags.set(WindowFlags::ON_TASKBAR, true);
            None
        }
    };

    let mut initdata = InitData {
        event_loop,
        attributes,
        pl_attribs: pl_attribs.clone(),
        window_flags,
        window: None,
    };

    let (style, ex_style) = window_flags.to_window_styles();
    let handle = CreateWindowExW(
        ex_style,
        class_name.as_ptr(),
        title.as_ptr(),
        style,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        parent.unwrap_or(0),
        pl_attribs.menu.unwrap_or(0),
        util::get_instance_handle(),
        &mut initdata as *mut _ as *mut _,
    );

    // If the window creation in `InitData` panicked, then should resume panicking here
    if let Err(panic_error) = event_loop.runner_shared.take_panic_error() {
        panic::resume_unwind(panic_error)
    }

    if handle == 0 {
        return Err(os_error!(io::Error::last_os_error()));
    }

    // If the handle is non-null, then window creation must have succeeded, which means
    // that we *must* have populated the `InitData.window` field.
    Ok(initdata.window.unwrap())
}

unsafe fn register_window_class<T: 'static>(
    window_icon: &Option<Icon>,
    taskbar_icon: &Option<Icon>,
) -> Vec<u16> {
    let class_name = util::encode_wide("Window Class");

    let h_icon = taskbar_icon
        .as_ref()
        .map(|icon| icon.inner.as_raw_handle())
        .unwrap_or(0);
    let h_icon_small = window_icon
        .as_ref()
        .map(|icon| icon.inner.as_raw_handle())
        .unwrap_or(0);

    use windows_sys::Win32::UI::WindowsAndMessaging::COLOR_WINDOWFRAME;
    let class = WNDCLASSEXW {
        cbSize: mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(super::event_loop::public_window_callback::<T>),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: util::get_instance_handle(),
        hIcon: h_icon,
        hCursor: 0, // must be null in order for cursor state to work properly
        hbrBackground: COLOR_WINDOWFRAME as _,
        lpszMenuName: ptr::null(),
        lpszClassName: class_name.as_ptr(),
        hIconSm: h_icon_small,
    };

    // We ignore errors because registering the same window class twice would trigger
    //  an error, and because errors here are detected during CreateWindowEx anyway.
    // Also since there is no weird element in the struct, there is no reason for this
    //  call to fail.
    RegisterClassExW(&class);

    class_name
}

struct ComInitialized(*mut ());
impl Drop for ComInitialized {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

thread_local! {
    static COM_INITIALIZED: ComInitialized = {
        unsafe {
            CoInitializeEx(ptr::null(), COINIT_APARTMENTTHREADED);
            ComInitialized(ptr::null_mut())
        }
    };

    static TASKBAR_LIST: Cell<*mut ITaskbarList> = Cell::new(ptr::null_mut());
    static TASKBAR_LIST2: Cell<*mut ITaskbarList2> = Cell::new(ptr::null_mut());
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
unsafe fn taskbar_mark_fullscreen(handle: HWND, fullscreen: bool) {
    com_initialized();

    TASKBAR_LIST2.with(|task_bar_list2_ptr| {
        let mut task_bar_list2 = task_bar_list2_ptr.get();

        if task_bar_list2.is_null() {
            let hr = CoCreateInstance(
                &CLSID_TaskbarList,
                ptr::null_mut(),
                CLSCTX_ALL,
                &IID_ITaskbarList2,
                &mut task_bar_list2 as *mut _ as *mut _,
            );

            let hr_init = (*(*task_bar_list2).lpVtbl).parent.HrInit;

            if hr != S_OK || hr_init(task_bar_list2.cast()) != S_OK {
                // In some old windows, the taskbar object could not be created, we just ignore it
                return;
            }
            task_bar_list2_ptr.set(task_bar_list2)
        }

        task_bar_list2 = task_bar_list2_ptr.get();
        let mark_fullscreen_window = (*(*task_bar_list2).lpVtbl).MarkFullscreenWindow;
        mark_fullscreen_window(task_bar_list2, handle, if fullscreen { 1 } else { 0 });
    })
}

pub(crate) unsafe fn set_skip_taskbar(hwnd: HWND, skip: bool) {
    com_initialized();
    TASKBAR_LIST.with(|task_bar_list_ptr| {
        let mut task_bar_list = task_bar_list_ptr.get();

        if task_bar_list.is_null() {
            let hr = CoCreateInstance(
                &CLSID_TaskbarList,
                ptr::null_mut(),
                CLSCTX_ALL,
                &IID_ITaskbarList,
                &mut task_bar_list as *mut _ as *mut _,
            );

            let hr_init = (*(*task_bar_list).lpVtbl).HrInit;

            if hr != S_OK || hr_init(task_bar_list.cast()) != S_OK {
                // In some old windows, the taskbar object could not be created, we just ignore it
                return;
            }
            task_bar_list_ptr.set(task_bar_list)
        }

        task_bar_list = task_bar_list_ptr.get();
        if skip {
            let delete_tab = (*(*task_bar_list).lpVtbl).DeleteTab;
            delete_tab(task_bar_list, hwnd);
        } else {
            let add_tab = (*(*task_bar_list).lpVtbl).AddTab;
            add_tab(task_bar_list, hwnd);
        }
    });
}

unsafe fn force_window_active(handle: HWND) {
    // In some situation, calling SetForegroundWindow could not bring up the window,
    // This is a little hack which can "steal" the foreground window permission
    // We only call this function in the window creation, so it should be fine.
    // See : https://stackoverflow.com/questions/10740346/setforegroundwindow-only-working-while-visual-studio-is-open
    let alt_sc = MapVirtualKeyW(VK_MENU as u32, MAPVK_VK_TO_VSC);

    let inputs = [
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_LMENU,
                    wScan: alt_sc as u16,
                    dwFlags: KEYEVENTF_EXTENDEDKEY,
                    dwExtraInfo: 0,
                    time: 0,
                },
            },
        },
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_LMENU,
                    wScan: alt_sc as u16,
                    dwFlags: KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP,
                    dwExtraInfo: 0,
                    time: 0,
                },
            },
        },
    ];

    // Simulate a key press and release
    SendInput(
        inputs.len() as u32,
        inputs.as_ptr(),
        mem::size_of::<INPUT>() as i32,
    );

    SetForegroundWindow(handle);
}
