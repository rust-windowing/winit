#![cfg(target_os = "windows")]

use std::{io, mem, ptr};
use std::cell::Cell;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::channel;

use winapi::ctypes::c_int;
use winapi::shared::minwindef::{DWORD, LPARAM, UINT, WORD, WPARAM};
use winapi::shared::windef::{HWND, POINT, RECT};
use winapi::um::{combaseapi, dwmapi, libloaderapi, winuser};
use winapi::um::objbase::COINIT_MULTITHREADED;
use winapi::um::shobjidl_core::{CLSID_TaskbarList, ITaskbarList2};
use winapi::um::wingdi::{CreateRectRgn, DeleteObject};
use winapi::um::winnt::{LONG, LPCWSTR};

use {
    CreationError,
    Icon,
    LogicalPosition,
    LogicalSize,
    MonitorId as RootMonitorId,
    MouseCursor,
    PhysicalSize,
    WindowAttributes,
};
use platform::platform::{PlatformSpecificWindowBuilderAttributes, WindowId};
use platform::platform::dpi::{dpi_to_scale_factor, get_hwnd_dpi};
use platform::platform::events_loop::{self, EventsLoop, DESTROY_MSG_ID, INITIAL_DPI_MSG_ID};
use platform::platform::icon::{self, IconType, WinIcon};
use platform::platform::monitor::get_available_monitors;
use platform::platform::raw_input::register_all_mice_and_keyboards_for_raw_input;
use platform::platform::util;
use platform::platform::window_state::{CursorFlags, SavedWindow, WindowFlags, WindowState};

/// The Win32 implementation of the main `Window` object.
pub struct Window {
    /// Main handle for the window.
    window: WindowWrapper,

    /// The current window state.
    window_state: Arc<Mutex<WindowState>>,

    // The events loop proxy.
    events_loop_proxy: events_loop::EventsLoopProxy,
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
            let rect = util::adjust_window_rect(
                self.window.0,
                RECT {
                    top: 0,
                    left: 0,
                    bottom: y as LONG,
                    right: x as LONG,
                }
            ).expect("adjust_window_rect failed");

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
        let window = self.window.clone();
        let window_state = Arc::clone(&self.window_state);

        self.events_loop_proxy.execute_in_thread(move |_| {
            WindowState::set_window_flags(
                window_state.lock().unwrap(),
                window.0,
                None,
                |f| f.set(WindowFlags::RESIZABLE, resizable),
            );
        });
    }

    /// Returns the `hwnd` of this window.
    #[inline]
    pub fn hwnd(&self) -> HWND {
        self.window.0
    }

    #[inline]
    pub fn set_cursor(&self, cursor: MouseCursor) {
        self.window_state.lock().unwrap().mouse.cursor = cursor;
        self.events_loop_proxy.execute_in_thread(move |_| unsafe {
            let cursor = winuser::LoadCursorW(
                ptr::null_mut(),
                cursor.to_windows_cursor(),
            );
            winuser::SetCursor(cursor);
        });
    }

    #[inline]
    pub fn grab_cursor(&self, grab: bool) -> Result<(), String> {
        let window = self.window.clone();
        let window_state = Arc::clone(&self.window_state);
        let (tx, rx) = channel();

        self.events_loop_proxy.execute_in_thread(move |_| {
            let result = window_state.lock().unwrap().mouse
                .set_cursor_flags(window.0, |f| f.set(CursorFlags::GRABBED, grab))
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
        rx.recv().unwrap()
    }

    #[inline]
    pub fn hide_cursor(&self, hide: bool) {
        let window = self.window.clone();
        let window_state = Arc::clone(&self.window_state);
        let (tx, rx) = channel();

        self.events_loop_proxy.execute_in_thread(move |_| {
            let result = window_state.lock().unwrap().mouse
                .set_cursor_flags(window.0, |f| f.set(CursorFlags::HIDDEN, hide))
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
        rx.recv().unwrap().ok();
    }

    #[inline]
    pub fn get_hidpi_factor(&self) -> f64 {
        self.window_state.lock().unwrap().dpi_factor
    }

    fn set_cursor_position_physical(&self, x: i32, y: i32) -> Result<(), String> {
        let mut point = POINT { x, y };
        unsafe {
            if winuser::ClientToScreen(self.window.0, &mut point) == 0 {
                return Err("`ClientToScreen` failed".to_owned());
            }
            if winuser::SetCursorPos(point.x, point.y) == 0 {
                return Err("`SetCursorPos` failed".to_owned());
            }
        }
        Ok(())
    }

    #[inline]
    pub fn set_cursor_position(&self, logical_position: LogicalPosition) -> Result<(), String> {
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
        let window = self.window.clone();
        let window_state = Arc::clone(&self.window_state);

        self.events_loop_proxy.execute_in_thread(move |_| {
            WindowState::set_window_flags(
                window_state.lock().unwrap(),
                window.0,
                None,
                |f| f.set(WindowFlags::MAXIMIZED, maximized),
            );
        });
    }

    #[inline]
    pub fn set_fullscreen(&self, monitor: Option<RootMonitorId>) {
        unsafe {
            let window = self.window.clone();
            let window_state = Arc::clone(&self.window_state);

            match &monitor {
                &Some(RootMonitorId { ref inner }) => {
                    let (x, y): (i32, i32) = inner.get_position().into();
                    let (width, height): (u32, u32) = inner.get_dimensions().into();

                    let mut monitor = monitor.clone();
                    self.events_loop_proxy.execute_in_thread(move |_| {
                        let mut window_state_lock = window_state.lock().unwrap();

                        let client_rect = util::get_client_rect(window.0).expect("get client rect failed!");
                        window_state_lock.saved_window = Some(SavedWindow {
                            client_rect,
                            dpi_factor: window_state_lock.dpi_factor
                        });

                        window_state_lock.fullscreen = monitor.take();
                        WindowState::refresh_window_state(
                            window_state_lock,
                            window.0,
                            Some(RECT {
                                left: x,
                                top: y,
                                right: x + width as c_int,
                                bottom: y + height as c_int,
                            })
                        );

                        mark_fullscreen(window.0, true);
                    });
                }
                &None => {
                    self.events_loop_proxy.execute_in_thread(move |_| {
                        let mut window_state_lock = window_state.lock().unwrap();
                        window_state_lock.fullscreen = None;

                        if let Some(SavedWindow{client_rect, dpi_factor}) = window_state_lock.saved_window {
                            window_state_lock.dpi_factor = dpi_factor;
                            window_state_lock.saved_window = None;

                            WindowState::refresh_window_state(
                                window_state_lock,
                                window.0,
                                Some(client_rect)
                            );
                        }

                        mark_fullscreen(window.0, false);
                    });
                }
            }
        }
    }

    #[inline]
    pub fn set_decorations(&self, decorations: bool) {
        let window = self.window.clone();
        let window_state = Arc::clone(&self.window_state);

        self.events_loop_proxy.execute_in_thread(move |_| {
            let client_rect = util::get_client_rect(window.0).expect("get client rect failed!");
            WindowState::set_window_flags(
                window_state.lock().unwrap(),
                window.0,
                Some(client_rect),
                |f| f.set(WindowFlags::DECORATIONS, decorations),
            );
        });
    }

    #[inline]
    pub fn set_always_on_top(&self, always_on_top: bool) {
        let window = self.window.clone();
        let window_state = Arc::clone(&self.window_state);

        self.events_loop_proxy.execute_in_thread(move |_| {
            WindowState::set_window_flags(
                window_state.lock().unwrap(),
                window.0,
                None,
                |f| f.set(WindowFlags::ALWAYS_ON_TOP, always_on_top),
            );
        });
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
        self.window_state.lock().unwrap().window_icon = window_icon;
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
        self.window_state.lock().unwrap().taskbar_icon = taskbar_icon;
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
pub struct WindowWrapper(HWND);

// Send and Sync are not implemented for HWND and HDC, we have to wrap it and implement them manually.
// For more info see:
// https://github.com/retep998/winapi-rs/issues/360
// https://github.com/retep998/winapi-rs/issues/396
unsafe impl Sync for WindowWrapper {}
unsafe impl Send for WindowWrapper {}

pub unsafe fn adjust_size(
    physical_size: PhysicalSize,
    style: DWORD,
    ex_style: DWORD,
) -> (LONG, LONG) {
    let (width, height): (u32, u32) = physical_size.into();
    let mut rect = RECT {
        left: 0,
        right: width as LONG,
        top: 0,
        bottom: height as LONG,
    };
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

    let guessed_dpi_factor = {
        let monitors = get_available_monitors();
        let dpi_factor = if !monitors.is_empty() {
            let mut dpi_factor = Some(monitors[0].get_hidpi_factor());
            for monitor in &monitors {
                if Some(monitor.get_hidpi_factor()) != dpi_factor {
                    dpi_factor = None;
                }
            }
            dpi_factor
        } else {
            return Err(CreationError::OsError(format!("No monitors were detected.")));
        };
        dpi_factor.unwrap_or_else(|| {
            util::get_cursor_pos()
                .and_then(|cursor_pos| {
                    let mut dpi_factor = None;
                    for monitor in &monitors {
                        if monitor.contains_point(&cursor_pos) {
                            dpi_factor = Some(monitor.get_hidpi_factor());
                            break;
                        }
                    }
                    dpi_factor
                })
                .unwrap_or(1.0)
        })
    };
    info!("Guessed window DPI factor: {}", guessed_dpi_factor);

    let dimensions = attributes.dimensions.unwrap_or_else(|| (1024, 768).into());

    let mut window_flags = WindowFlags::empty();
    window_flags.set(WindowFlags::DECORATIONS, attributes.decorations);
    window_flags.set(WindowFlags::ALWAYS_ON_TOP, attributes.always_on_top);
    window_flags.set(WindowFlags::NO_BACK_BUFFER, pl_attribs.no_redirection_bitmap);
    window_flags.set(WindowFlags::TRANSPARENT, attributes.transparent);
    // WindowFlags::VISIBLE and MAXIMIZED are set down below after the window has been configured.
    window_flags.set(WindowFlags::RESIZABLE, attributes.resizable);
    window_flags.set(WindowFlags::CHILD, pl_attribs.parent.is_some());
    window_flags.set(WindowFlags::ON_TASKBAR, true);

    // creating the real window this time, by using the functions in `extra_functions`
    let real_window = {
        let (style, ex_style) = window_flags.to_window_styles();
        let handle = winuser::CreateWindowExW(
            ex_style,
            class_name.as_ptr(),
            title.as_ptr() as LPCWSTR,
            style,
            winuser::CW_USEDEFAULT, winuser::CW_USEDEFAULT,
            winuser::CW_USEDEFAULT, winuser::CW_USEDEFAULT,
            pl_attribs.parent.unwrap_or(ptr::null_mut()),
            ptr::null_mut(),
            libloaderapi::GetModuleHandleW(ptr::null()),
            ptr::null_mut(),
        );

        if handle.is_null() {
            return Err(CreationError::OsError(format!("CreateWindowEx function failed: {}",
                                              format!("{}", io::Error::last_os_error()))));
        }

        WindowWrapper(handle)
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

    let dpi = get_hwnd_dpi(real_window.0);
    let dpi_factor = dpi_to_scale_factor(dpi);
    if dpi_factor != guessed_dpi_factor {
        let (width, height): (u32, u32) = dimensions.into();
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

    // making the window transparent
    if attributes.transparent && !pl_attribs.no_redirection_bitmap {
        let region = CreateRectRgn(0, 0, -1, -1); // makes the window transparent

        let bb = dwmapi::DWM_BLURBEHIND {
            dwFlags: dwmapi::DWM_BB_ENABLE | dwmapi::DWM_BB_BLURREGION,
            fEnable: 1,
            hRgnBlur: region,
            fTransitionOnMaximized: 0,
        };

        dwmapi::DwmEnableBlurBehindWindow(real_window.0, &bb);
        DeleteObject(region as _);

        if attributes.decorations {
            // HACK: When opaque (opacity 255), there is a trail whenever
            // the transparent window is moved. By reducing it to 254,
            // the window is rendered properly.
            let opacity = 254;

            // The color key can be any value except for black (0x0).
            let color_key = 0x0030c100;

            winuser::SetLayeredWindowAttributes(real_window.0, color_key, opacity, winuser::LWA_ALPHA);
        }
    }

    window_flags.set(WindowFlags::VISIBLE, attributes.visible);
    window_flags.set(WindowFlags::MAXIMIZED, attributes.maximized);

    let window_state = {
        let mut window_state = WindowState::new(
            &attributes,
            window_icon,
            taskbar_icon,
            dpi_factor,
        );
        let window_state = Arc::new(Mutex::new(window_state));
        WindowState::set_window_flags(
            window_state.lock().unwrap(),
            real_window.0,
            None,
            |f| *f = window_flags,
        );
        window_state
    };

    let win = Window {
        window: real_window,
        window_state,
        events_loop_proxy,
    };

    if let Some(_) = attributes.fullscreen {
        win.set_fullscreen(attributes.fullscreen);
        force_window_active(win.window.0);
    }

    if let Some(dimensions) = attributes.dimensions {
        win.set_inner_size(dimensions);
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
