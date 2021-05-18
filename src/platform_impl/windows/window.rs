#![cfg(target_os = "windows")]

use parking_lot::Mutex;
use raw_window_handle::{windows::WindowsHandle, RawWindowHandle};
use std::{
    cell::Cell,
    ffi::OsStr,
    io, mem,
    os::windows::ffi::OsStrExt,
    ptr,
    sync::{mpsc::channel, Arc},
};

use winapi::{
    ctypes::c_int,
    shared::{
        minwindef::{HINSTANCE, LPARAM, UINT, WPARAM},
        windef::{HWND, POINT, POINTS, RECT},
    },
    um::{
        combaseapi, dwmapi,
        imm::{CFS_POINT, COMPOSITIONFORM},
        libloaderapi,
        objbase::COINIT_APARTMENTTHREADED,
        ole2,
        oleidl::LPDROPTARGET,
        shobjidl_core::{CLSID_TaskbarList, ITaskbarList2},
        wingdi::{CreateRectRgn, DeleteObject},
        winnt::{LPCWSTR, SHORT},
        winuser,
    },
};

use crate::{
    dpi::{PhysicalPosition, PhysicalSize, Position, Size},
    error::{ExternalError, NotSupportedError, OsError as RootOsError},
    icon::Icon,
    monitor::MonitorHandle as RootMonitorHandle,
    platform_impl::platform::{
        dark_mode::try_theme,
        dpi::{dpi_to_scale_factor, hwnd_dpi},
        drop_handler::FileDropHandler,
        event_loop::{self, EventLoopWindowTarget, DESTROY_MSG_ID},
        icon::{self, IconType},
        monitor, util,
        window_state::{CursorFlags, SavedWindow, WindowFlags, WindowState},
        Parent, PlatformSpecificWindowBuilderAttributes, WindowId,
    },
    window::{CursorIcon, Fullscreen, Theme, UserAttentionType, WindowAttributes},
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
    pub fn new<T: 'static>(
        event_loop: &EventLoopWindowTarget<T>,
        w_attr: WindowAttributes,
        pl_attr: PlatformSpecificWindowBuilderAttributes,
    ) -> Result<Window, RootOsError> {
        // We dispatch an `init` function because of code style.
        // First person to remove the need for cloning here gets a cookie!
        //
        // done. you owe me -- ossi
        unsafe {
            let drag_and_drop = pl_attr.drag_and_drop;
            init(w_attr, pl_attr, event_loop).map(|win| {
                let file_drop_handler = if drag_and_drop {
                    use winapi::shared::winerror::{OLE_E_WRONGCOMPOBJ, RPC_E_CHANGED_MODE, S_OK};

                    let ole_init_result = ole2::OleInitialize(ptr::null_mut());
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

                    let file_drop_runner = event_loop.runner_shared.clone();
                    let file_drop_handler = FileDropHandler::new(
                        win.window.0,
                        Box::new(move |event| {
                            if let Ok(e) = event.map_nonuser_event() {
                                file_drop_runner.send_event(e)
                            }
                        }),
                    );
                    let handler_interface_ptr =
                        &mut (*file_drop_handler.data).interface as LPDROPTARGET;

                    assert_eq!(
                        ole2::RegisterDragDrop(win.window.0, handler_interface_ptr),
                        S_OK
                    );
                    Some(file_drop_handler)
                } else {
                    None
                };

                let subclass_input = event_loop::SubclassInput {
                    window_state: win.window_state.clone(),
                    event_loop_runner: event_loop.runner_shared.clone(),
                    file_drop_handler,
                    subclass_removed: Cell::new(false),
                    recurse_depth: Cell::new(0),
                };

                event_loop::subclass_window(win.window.0, subclass_input);
                win
            })
        }
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
    pub fn set_visible(&self, visible: bool) {
        let window = self.window.clone();
        let window_state = Arc::clone(&self.window_state);
        self.thread_executor.execute_in_thread(move || {
            WindowState::set_window_flags(window_state.lock(), window.0, |f| {
                f.set(WindowFlags::VISIBLE, visible)
            });
        });
    }

    #[inline]
    pub fn request_redraw(&self) {
        unsafe {
            winuser::RedrawWindow(
                self.window.0,
                ptr::null(),
                ptr::null_mut(),
                winuser::RDW_INTERNALPAINT,
            );
        }
    }

    #[inline]
    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        util::get_window_rect(self.window.0)
            .map(|rect| Ok(PhysicalPosition::new(rect.left as i32, rect.top as i32)))
            .expect("Unexpected GetWindowRect failure; please report this error to https://github.com/rust-windowing/winit")
    }

    #[inline]
    pub fn inner_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        let mut position: POINT = unsafe { mem::zeroed() };
        if unsafe { winuser::ClientToScreen(self.window.0, &mut position) } == 0 {
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
            WindowState::set_window_flags(window_state.lock(), window.0, |f| {
                f.set(WindowFlags::MAXIMIZED, false)
            });
        });

        unsafe {
            winuser::SetWindowPos(
                self.window.0,
                ptr::null_mut(),
                x as c_int,
                y as c_int,
                0,
                0,
                winuser::SWP_ASYNCWINDOWPOS
                    | winuser::SWP_NOZORDER
                    | winuser::SWP_NOSIZE
                    | winuser::SWP_NOACTIVATE,
            );
            winuser::InvalidateRgn(self.window.0, ptr::null_mut(), 0);
        }
    }

    #[inline]
    pub fn inner_size(&self) -> PhysicalSize<u32> {
        let mut rect: RECT = unsafe { mem::zeroed() };
        if unsafe { winuser::GetClientRect(self.window.0, &mut rect) } == 0 {
            panic!("Unexpected GetClientRect failure: please report this error to https://github.com/rust-windowing/winit")
        }
        PhysicalSize::new(
            (rect.right - rect.left) as u32,
            (rect.bottom - rect.top) as u32,
        )
    }

    #[inline]
    pub fn outer_size(&self) -> PhysicalSize<u32> {
        util::get_window_rect(self.window.0)
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
        let (width, height) = size.to_physical::<u32>(scale_factor).into();

        let window_state = Arc::clone(&self.window_state);
        let window = self.window.clone();
        self.thread_executor.execute_in_thread(move || {
            WindowState::set_window_flags(window_state.lock(), window.0, |f| {
                f.set(WindowFlags::MAXIMIZED, false)
            });
        });

        util::set_inner_size_physical(self.window.0, width, height);
    }

    #[inline]
    pub fn set_min_inner_size(&self, size: Option<Size>) {
        self.window_state.lock().min_size = size;
        // Make windows re-check the window size bounds.
        let size = self.inner_size();
        self.set_inner_size(size.into());
    }

    #[inline]
    pub fn set_max_inner_size(&self, size: Option<Size>) {
        self.window_state.lock().max_size = size;
        // Make windows re-check the window size bounds.
        let size = self.inner_size();
        self.set_inner_size(size.into());
    }

    #[inline]
    pub fn set_resizable(&self, resizable: bool) {
        let window = self.window.clone();
        let window_state = Arc::clone(&self.window_state);

        self.thread_executor.execute_in_thread(move || {
            WindowState::set_window_flags(window_state.lock(), window.0, |f| {
                f.set(WindowFlags::RESIZABLE, resizable)
            });
        });
    }

    /// Returns the `hwnd` of this window.
    #[inline]
    pub fn hwnd(&self) -> HWND {
        self.window.0
    }

    #[inline]
    pub fn hinstance(&self) -> HINSTANCE {
        unsafe { winuser::GetWindowLongPtrW(self.hwnd(), winuser::GWLP_HINSTANCE) as *mut _ }
    }

    #[inline]
    pub fn raw_window_handle(&self) -> RawWindowHandle {
        let handle = WindowsHandle {
            hwnd: self.window.0 as *mut _,
            hinstance: self.hinstance() as *mut _,
            ..WindowsHandle::empty()
        };
        RawWindowHandle::Windows(handle)
    }

    #[inline]
    pub fn set_cursor_icon(&self, cursor: CursorIcon) {
        self.window_state.lock().mouse.cursor = cursor;
        self.thread_executor.execute_in_thread(move || unsafe {
            let cursor = winuser::LoadCursorW(ptr::null_mut(), cursor.to_windows_cursor());
            winuser::SetCursor(cursor);
        });
    }

    #[inline]
    pub fn set_cursor_grab(&self, grab: bool) -> Result<(), ExternalError> {
        let window = self.window.clone();
        let window_state = Arc::clone(&self.window_state);
        let (tx, rx) = channel();

        self.thread_executor.execute_in_thread(move || {
            let result = window_state
                .lock()
                .mouse
                .set_cursor_flags(window.0, |f| f.set(CursorFlags::GRABBED, grab))
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
            let result = window_state
                .lock()
                .mouse
                .set_cursor_flags(window.0, |f| f.set(CursorFlags::HIDDEN, !visible))
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
        rx.recv().unwrap().ok();
    }

    #[inline]
    pub fn scale_factor(&self) -> f64 {
        self.window_state.lock().scale_factor
    }

    #[inline]
    pub fn set_cursor_position(&self, position: Position) -> Result<(), ExternalError> {
        let scale_factor = self.scale_factor();
        let (x, y) = position.to_physical::<i32>(scale_factor).into();

        let mut point = POINT { x, y };
        unsafe {
            if winuser::ClientToScreen(self.window.0, &mut point) == 0 {
                return Err(ExternalError::Os(os_error!(io::Error::last_os_error())));
            }
            if winuser::SetCursorPos(point.x, point.y) == 0 {
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
                winuser::GetCursorPos(&mut pos);
                pos
            };
            let points = POINTS {
                x: points.x as SHORT,
                y: points.y as SHORT,
            };
            winuser::ReleaseCapture();
            winuser::PostMessageW(
                self.window.0,
                winuser::WM_NCLBUTTONDOWN,
                winuser::HTCAPTION as WPARAM,
                &points as *const _ as LPARAM,
            );
        }

        Ok(())
    }

    #[inline]
    pub fn id(&self) -> WindowId {
        WindowId(self.window.0)
    }

    #[inline]
    pub fn set_minimized(&self, minimized: bool) {
        let window = self.window.clone();
        let window_state = Arc::clone(&self.window_state);

        self.thread_executor.execute_in_thread(move || {
            WindowState::set_window_flags(window_state.lock(), window.0, |f| {
                f.set(WindowFlags::MINIMIZED, minimized)
            });
        });
    }

    #[inline]
    pub fn set_maximized(&self, maximized: bool) {
        let window = self.window.clone();
        let window_state = Arc::clone(&self.window_state);

        self.thread_executor.execute_in_thread(move || {
            WindowState::set_window_flags(window_state.lock(), window.0, |f| {
                f.set(WindowFlags::MAXIMIZED, maximized)
            });
        });
    }

    #[inline]
    pub fn is_maximized(&self) -> bool {
        let window_state = self.window_state.lock();
        window_state.window_flags.contains(WindowFlags::MAXIMIZED)
    }

    #[inline]
    pub fn fullscreen(&self) -> Option<Fullscreen> {
        let window_state = self.window_state.lock();
        window_state.fullscreen.clone()
    }

    #[inline]
    pub fn set_fullscreen(&self, fullscreen: Option<Fullscreen>) {
        let window = self.window.clone();
        let window_state = Arc::clone(&self.window_state);

        let mut window_state_lock = window_state.lock();
        let old_fullscreen = window_state_lock.fullscreen.clone();
        if window_state_lock.fullscreen == fullscreen {
            return;
        }
        window_state_lock.fullscreen = fullscreen.clone();
        drop(window_state_lock);

        self.thread_executor.execute_in_thread(move || {
            // Change video mode if we're transitioning to or from exclusive
            // fullscreen
            match (&old_fullscreen, &fullscreen) {
                (&None, &Some(Fullscreen::Exclusive(ref video_mode)))
                | (
                    &Some(Fullscreen::Borderless(_)),
                    &Some(Fullscreen::Exclusive(ref video_mode)),
                )
                | (&Some(Fullscreen::Exclusive(_)), &Some(Fullscreen::Exclusive(ref video_mode))) =>
                {
                    let monitor = video_mode.monitor();

                    let mut display_name = OsStr::new(&monitor.inner.native_identifier())
                        .encode_wide()
                        .collect::<Vec<_>>();
                    // `encode_wide` does not add a null-terminator but
                    // `ChangeDisplaySettingsExW` requires a null-terminated
                    // string, so add it
                    display_name.push(0);

                    let mut native_video_mode = video_mode.video_mode.native_video_mode.clone();

                    let res = unsafe {
                        winuser::ChangeDisplaySettingsExW(
                            display_name.as_ptr(),
                            &mut native_video_mode,
                            std::ptr::null_mut(),
                            winuser::CDS_FULLSCREEN,
                            std::ptr::null_mut(),
                        )
                    };

                    debug_assert!(res != winuser::DISP_CHANGE_BADFLAGS);
                    debug_assert!(res != winuser::DISP_CHANGE_BADMODE);
                    debug_assert!(res != winuser::DISP_CHANGE_BADPARAM);
                    debug_assert!(res != winuser::DISP_CHANGE_FAILED);
                    assert_eq!(res, winuser::DISP_CHANGE_SUCCESSFUL);
                }
                (&Some(Fullscreen::Exclusive(_)), &None)
                | (&Some(Fullscreen::Exclusive(_)), &Some(Fullscreen::Borderless(_))) => {
                    let res = unsafe {
                        winuser::ChangeDisplaySettingsExW(
                            std::ptr::null_mut(),
                            std::ptr::null_mut(),
                            std::ptr::null_mut(),
                            winuser::CDS_FULLSCREEN,
                            std::ptr::null_mut(),
                        )
                    };

                    debug_assert!(res != winuser::DISP_CHANGE_BADFLAGS);
                    debug_assert!(res != winuser::DISP_CHANGE_BADMODE);
                    debug_assert!(res != winuser::DISP_CHANGE_BADPARAM);
                    debug_assert!(res != winuser::DISP_CHANGE_FAILED);
                    assert_eq!(res, winuser::DISP_CHANGE_SUCCESSFUL);
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
                winuser::PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, 0);
            }

            // Update window style
            WindowState::set_window_flags(window_state.lock(), window.0, |f| {
                f.set(
                    WindowFlags::MARKER_EXCLUSIVE_FULLSCREEN,
                    matches!(fullscreen, Some(Fullscreen::Exclusive(_))),
                );
                f.set(
                    WindowFlags::MARKER_BORDERLESS_FULLSCREEN,
                    matches!(fullscreen, Some(Fullscreen::Borderless(_))),
                );
            });

            // Update window bounds
            match &fullscreen {
                Some(fullscreen) => {
                    // Save window bounds before entering fullscreen
                    let placement = unsafe {
                        let mut placement = mem::zeroed();
                        winuser::GetWindowPlacement(window.0, &mut placement);
                        placement
                    };

                    window_state.lock().saved_window = Some(SavedWindow { placement });

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
                        winuser::SetWindowPos(
                            window.0,
                            ptr::null_mut(),
                            position.0,
                            position.1,
                            size.0 as i32,
                            size.1 as i32,
                            winuser::SWP_ASYNCWINDOWPOS | winuser::SWP_NOZORDER,
                        );
                        winuser::InvalidateRgn(window.0, ptr::null_mut(), 0);
                    }
                }
                None => {
                    let mut window_state_lock = window_state.lock();
                    if let Some(SavedWindow { placement }) = window_state_lock.saved_window.take() {
                        drop(window_state_lock);
                        unsafe {
                            winuser::SetWindowPlacement(window.0, &placement);
                            winuser::InvalidateRgn(window.0, ptr::null_mut(), 0);
                        }
                    }
                }
            }

            unsafe {
                taskbar_mark_fullscreen(window.0, fullscreen.is_some());
            }
        });
    }

    #[inline]
    pub fn set_decorations(&self, decorations: bool) {
        let window = self.window.clone();
        let window_state = Arc::clone(&self.window_state);

        self.thread_executor.execute_in_thread(move || {
            WindowState::set_window_flags(window_state.lock(), window.0, |f| {
                f.set(WindowFlags::DECORATIONS, decorations)
            });
        });
    }

    #[inline]
    pub fn set_always_on_top(&self, always_on_top: bool) {
        let window = self.window.clone();
        let window_state = Arc::clone(&self.window_state);

        self.thread_executor.execute_in_thread(move || {
            WindowState::set_window_flags(window_state.lock(), window.0, |f| {
                f.set(WindowFlags::ALWAYS_ON_TOP, always_on_top)
            });
        });
    }

    #[inline]
    pub fn current_monitor(&self) -> Option<RootMonitorHandle> {
        Some(RootMonitorHandle {
            inner: monitor::current_monitor(self.window.0),
        })
    }

    #[inline]
    pub fn set_window_icon(&self, window_icon: Option<Icon>) {
        if let Some(ref window_icon) = window_icon {
            window_icon
                .inner
                .set_for_window(self.window.0, IconType::Small);
        } else {
            icon::unset_for_window(self.window.0, IconType::Small);
        }
        self.window_state.lock().window_icon = window_icon;
    }

    #[inline]
    pub fn set_taskbar_icon(&self, taskbar_icon: Option<Icon>) {
        if let Some(ref taskbar_icon) = taskbar_icon {
            taskbar_icon
                .inner
                .set_for_window(self.window.0, IconType::Big);
        } else {
            icon::unset_for_window(self.window.0, IconType::Big);
        }
        self.window_state.lock().taskbar_icon = taskbar_icon;
    }

    pub(crate) fn set_ime_position_physical(&self, x: i32, y: i32) {
        if unsafe { winuser::GetSystemMetrics(winuser::SM_IMMENABLED) } != 0 {
            let mut composition_form = COMPOSITIONFORM {
                dwStyle: CFS_POINT,
                ptCurrentPos: POINT { x, y },
                rcArea: unsafe { mem::zeroed() },
            };
            unsafe {
                let himc = winapi::um::imm::ImmGetContext(self.window.0);
                winapi::um::imm::ImmSetCompositionWindow(himc, &mut composition_form);
                winapi::um::imm::ImmReleaseContext(self.window.0, himc);
            }
        }
    }

    #[inline]
    pub fn set_ime_position(&self, spot: Position) {
        let (x, y) = spot.to_physical::<i32>(self.scale_factor()).into();
        self.set_ime_position_physical(x, y);
    }

    #[inline]
    pub fn request_user_attention(&self, request_type: Option<UserAttentionType>) {
        let window = self.window.clone();
        let active_window_handle = unsafe { winuser::GetActiveWindow() };
        if window.0 == active_window_handle {
            return;
        }

        self.thread_executor.execute_in_thread(move || unsafe {
            let (flags, count) = request_type
                .map(|ty| match ty {
                    UserAttentionType::Critical => {
                        (winuser::FLASHW_ALL | winuser::FLASHW_TIMERNOFG, u32::MAX)
                    }
                    UserAttentionType::Informational => {
                        (winuser::FLASHW_TRAY | winuser::FLASHW_TIMERNOFG, 0)
                    }
                })
                .unwrap_or((winuser::FLASHW_STOP, 0));

            let mut flash_info = winuser::FLASHWINFO {
                cbSize: mem::size_of::<winuser::FLASHWINFO>() as UINT,
                hwnd: window.0,
                dwFlags: flags,
                uCount: count,
                dwTimeout: 0,
            };
            winuser::FlashWindowEx(&mut flash_info);
        });
    }

    #[inline]
    pub fn theme(&self) -> Theme {
        self.window_state.lock().current_theme
    }

    #[inline]
    pub fn focus_window(&self) {
        let window = self.window.clone();
        let window_flags = self.window_state.lock().window_flags();

        let is_visible = window_flags.contains(WindowFlags::VISIBLE);
        let is_minimized = window_flags.contains(WindowFlags::MINIMIZED);
        let is_foreground = window.0 == unsafe { winuser::GetForegroundWindow() };

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
            winuser::PostMessageW(self.window.0, *DESTROY_MSG_ID, 0, 0);
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

unsafe fn init<T: 'static>(
    attributes: WindowAttributes,
    pl_attribs: PlatformSpecificWindowBuilderAttributes,
    event_loop: &EventLoopWindowTarget<T>,
) -> Result<Window, RootOsError> {
    let title = OsStr::new(&attributes.title)
        .encode_wide()
        .chain(Some(0).into_iter())
        .collect::<Vec<_>>();

    // registering the window class
    let class_name = register_window_class(&attributes.window_icon, &pl_attribs.taskbar_icon);

    let mut window_flags = WindowFlags::empty();
    window_flags.set(WindowFlags::DECORATIONS, attributes.decorations);
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

    // creating the real window this time, by using the functions in `extra_functions`
    let real_window = {
        let (style, ex_style) = window_flags.to_window_styles();
        let handle = winuser::CreateWindowExW(
            ex_style,
            class_name.as_ptr(),
            title.as_ptr() as LPCWSTR,
            style,
            winuser::CW_USEDEFAULT,
            winuser::CW_USEDEFAULT,
            winuser::CW_USEDEFAULT,
            winuser::CW_USEDEFAULT,
            parent.unwrap_or(ptr::null_mut()),
            pl_attribs.menu.unwrap_or(ptr::null_mut()),
            libloaderapi::GetModuleHandleW(ptr::null()),
            ptr::null_mut(),
        );

        if handle.is_null() {
            return Err(os_error!(io::Error::last_os_error()));
        }

        WindowWrapper(handle)
    };

    // Register for touch events if applicable
    {
        let digitizer = winuser::GetSystemMetrics(winuser::SM_DIGITIZER) as u32;
        if digitizer & winuser::NID_READY != 0 {
            winuser::RegisterTouchWindow(real_window.0, winuser::TWF_WANTPALM);
        }
    }

    let dpi = hwnd_dpi(real_window.0);
    let scale_factor = dpi_to_scale_factor(dpi);

    // making the window transparent
    if attributes.transparent && !pl_attribs.no_redirection_bitmap {
        // Empty region for the blur effect, so the window is fully transparent
        let region = CreateRectRgn(0, 0, -1, -1);

        let bb = dwmapi::DWM_BLURBEHIND {
            dwFlags: dwmapi::DWM_BB_ENABLE | dwmapi::DWM_BB_BLURREGION,
            fEnable: 1,
            hRgnBlur: region,
            fTransitionOnMaximized: 0,
        };

        dwmapi::DwmEnableBlurBehindWindow(real_window.0, &bb);
        DeleteObject(region as _);
    }

    // If the system theme is dark, we need to set the window theme now
    // before we update the window flags (and possibly show the
    // window for the first time).
    let current_theme = try_theme(real_window.0, pl_attribs.preferred_theme);

    let window_state = {
        let window_state = WindowState::new(
            &attributes,
            pl_attribs.taskbar_icon,
            scale_factor,
            current_theme,
            pl_attribs.preferred_theme,
        );
        let window_state = Arc::new(Mutex::new(window_state));
        WindowState::set_window_flags(window_state.lock(), real_window.0, |f| *f = window_flags);
        window_state
    };

    let win = Window {
        window: real_window,
        window_state,
        thread_executor: event_loop.create_thread_executor(),
    };

    let dimensions = attributes
        .inner_size
        .unwrap_or_else(|| PhysicalSize::new(800, 600).into());
    win.set_inner_size(dimensions);
    if attributes.maximized {
        // Need to set MAXIMIZED after setting `inner_size` as
        // `Window::set_inner_size` changes MAXIMIZED to false.
        win.set_maximized(true);
    }
    win.set_visible(attributes.visible);

    if let Some(_) = attributes.fullscreen {
        win.set_fullscreen(attributes.fullscreen);
        force_window_active(win.window.0);
    }

    if let Some(position) = attributes.position {
        win.set_outer_position(position);
    }

    Ok(win)
}

unsafe fn register_window_class(
    window_icon: &Option<Icon>,
    taskbar_icon: &Option<Icon>,
) -> Vec<u16> {
    let class_name: Vec<_> = OsStr::new("Window Class")
        .encode_wide()
        .chain(Some(0).into_iter())
        .collect();

    let h_icon = taskbar_icon
        .as_ref()
        .map(|icon| icon.inner.as_raw_handle())
        .unwrap_or(ptr::null_mut());
    let h_icon_small = window_icon
        .as_ref()
        .map(|icon| icon.inner.as_raw_handle())
        .unwrap_or(ptr::null_mut());

    let class = winuser::WNDCLASSEXW {
        cbSize: mem::size_of::<winuser::WNDCLASSEXW>() as UINT,
        style: winuser::CS_HREDRAW | winuser::CS_VREDRAW | winuser::CS_OWNDC,
        lpfnWndProc: Some(winuser::DefWindowProcW),
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

thread_local! {
    static COM_INITIALIZED: ComInitialized = {
        unsafe {
            combaseapi::CoInitializeEx(ptr::null_mut(), COINIT_APARTMENTTHREADED);
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
unsafe fn taskbar_mark_fullscreen(handle: HWND, fullscreen: bool) {
    com_initialized();

    TASKBAR_LIST.with(|task_bar_list_ptr| {
        let mut task_bar_list = task_bar_list_ptr.get();

        if task_bar_list == ptr::null_mut() {
            use winapi::{shared::winerror::S_OK, Interface};

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
