use std::sync::{Arc, Mutex};
use std::io;
use std::ptr;
use std::mem;
use std::thread;

use super::callback;
use super::WindowState;
use super::Window;
use super::MonitorId;
use super::WindowWrapper;
use super::PlatformSpecificWindowBuilderAttributes;

use CreationError;
use CreationError::OsError;
use CursorState;
use WindowAttributes;

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::sync::mpsc::channel;

use winapi;
use kernel32;
use dwmapi;
use user32;

pub fn new_window(window: &WindowAttributes,
                  pl_attribs: &PlatformSpecificWindowBuilderAttributes)
                  -> Result<Window, CreationError> {
    let window = window.clone();
    let attribs = pl_attribs.clone();
    // initializing variables to be sent to the task

    let title = OsStr::new(&window.title)
        .encode_wide()
        .chain(Some(0).into_iter())
        .collect::<Vec<_>>();

    let (tx, rx) = channel();

    // `GetMessage` must be called in the same thread as CreateWindow, so we create a new thread
    // dedicated to this window.
    thread::spawn(move || {
        unsafe {
            // creating and sending the `Window`
            match init(title, &window, attribs) {
                Ok(w) => tx.send(Ok(w)).ok(),
                Err(e) => {
                    tx.send(Err(e)).ok();
                    return;
                }
            };

            // now that the `Window` struct is initialized, the main `Window::new()` function will
            //  return and this events loop will run in parallel
            loop {
                let mut msg = mem::uninitialized();

                if user32::GetMessageW(&mut msg, ptr::null_mut(), 0, 0) == 0 {
                    break;
                }

                user32::TranslateMessage(&msg);
                user32::DispatchMessageW(&msg); // calls `callback` (see the callback module)
            }
        }
    });

    rx.recv().unwrap()
}

unsafe fn init(title: Vec<u16>,
               window: &WindowAttributes,
               pl_attribs: PlatformSpecificWindowBuilderAttributes)
               -> Result<Window, CreationError> {
    // registering the window class
    let class_name = register_window_class();

    // building a RECT object with coordinates
    let mut rect = winapi::RECT {
        left: 0,
        right: window.dimensions.unwrap_or((1024, 768)).0 as winapi::LONG,
        top: 0,
        bottom: window.dimensions.unwrap_or((1024, 768)).1 as winapi::LONG,
    };

    // switching to fullscreen if necessary
    // this means adjusting the window's position so that it overlaps the right monitor,
    //  and change the monitor's resolution if necessary
    if window.monitor.is_some() {
        let monitor = window.monitor.as_ref().unwrap();
        try!(switch_to_fullscreen(&mut rect, monitor));
    }

    // computing the style and extended style of the window
    let (ex_style, style) = if window.monitor.is_some() || !window.decorations {
        (winapi::WS_EX_APPWINDOW,
         //winapi::WS_POPUP is incompatible with winapi::WS_CHILD
         if pl_attribs.parent.is_some() {
             winapi::WS_CLIPSIBLINGS | winapi::WS_CLIPCHILDREN
         } else {
             winapi::WS_POPUP | winapi::WS_CLIPSIBLINGS | winapi::WS_CLIPCHILDREN
         })
    } else {
        (winapi::WS_EX_APPWINDOW | winapi::WS_EX_WINDOWEDGE,
         winapi::WS_OVERLAPPEDWINDOW | winapi::WS_CLIPSIBLINGS | winapi::WS_CLIPCHILDREN)
    };

    // adjusting the window coordinates using the style
    user32::AdjustWindowRectEx(&mut rect, style, 0, ex_style);

    // creating the real window this time, by using the functions in `extra_functions`
    let real_window = {
        let (width, height) = if window.monitor.is_some() || window.dimensions.is_some() {
            (Some(rect.right - rect.left), Some(rect.bottom - rect.top))
        } else {
            (None, None)
        };

        let (x, y) = if window.monitor.is_some() {
            (Some(rect.left), Some(rect.top))
        } else {
            (None, None)
        };

        let mut style = if !window.visible {
            style
        } else {
            style | winapi::WS_VISIBLE
        };

        if pl_attribs.parent.is_some() {
            style |= winapi::WS_CHILD;
        }

        let handle = user32::CreateWindowExW(ex_style | winapi::WS_EX_ACCEPTFILES,
                                             class_name.as_ptr(),
                                             title.as_ptr() as winapi::LPCWSTR,
                                             style | winapi::WS_CLIPSIBLINGS |
                                             winapi::WS_CLIPCHILDREN,
                                             x.unwrap_or(winapi::CW_USEDEFAULT),
                                             y.unwrap_or(winapi::CW_USEDEFAULT),
                                             width.unwrap_or(winapi::CW_USEDEFAULT),
                                             height.unwrap_or(winapi::CW_USEDEFAULT),
                                             pl_attribs.parent.unwrap_or(ptr::null_mut()),
                                             ptr::null_mut(),
                                             kernel32::GetModuleHandleW(ptr::null()),
                                             ptr::null_mut());

        if handle.is_null() {
            return Err(OsError(format!("CreateWindowEx function failed: {}",
                                       format!("{}", io::Error::last_os_error()))));
        }

        let hdc = user32::GetDC(handle);
        if hdc.is_null() {
            return Err(OsError(format!("GetDC function failed: {}",
                                       format!("{}", io::Error::last_os_error()))));
        }

        WindowWrapper(handle, hdc)
    };

    // making the window transparent
    if window.transparent {
        let bb = winapi::DWM_BLURBEHIND {
            dwFlags: 0x1, // FIXME: DWM_BB_ENABLE;
            fEnable: 1,
            hRgnBlur: ptr::null_mut(),
            fTransitionOnMaximized: 0,
        };

        dwmapi::DwmEnableBlurBehindWindow(real_window.0, &bb);
    }

    // calling SetForegroundWindow if fullscreen
    if window.monitor.is_some() {
        user32::SetForegroundWindow(real_window.0);
    }

    // Creating a mutex to track the current window state
    let window_state = Arc::new(Mutex::new(WindowState {
        cursor: winapi::IDC_ARROW, // use arrow by default
        cursor_state: CursorState::Normal,
        attributes: window.clone(),
    }));

    // filling the CONTEXT_STASH task-local storage so that we can start receiving events
    let events_receiver = {
        let (tx, rx) = channel();
        let mut tx = Some(tx);
        callback::CONTEXT_STASH.with(|context_stash| {
            let data = callback::ThreadLocalData {
                win: real_window.0,
                sender: tx.take().unwrap(),
                window_state: window_state.clone(),
                mouse_in_window: false,
            };
            (*context_stash.borrow_mut()) = Some(data);
        });
        rx
    };

    // building the struct
    Ok(Window {
        window: real_window,
        events_receiver: events_receiver,
        window_state: window_state,
    })
}

unsafe fn register_window_class() -> Vec<u16> {
    let class_name = OsStr::new("Window Class")
        .encode_wide()
        .chain(Some(0).into_iter())
        .collect::<Vec<_>>();

    let class = winapi::WNDCLASSEXW {
        cbSize: mem::size_of::<winapi::WNDCLASSEXW>() as winapi::UINT,
        style: winapi::CS_HREDRAW | winapi::CS_VREDRAW | winapi::CS_OWNDC,
        lpfnWndProc: Some(callback::callback),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: kernel32::GetModuleHandleW(ptr::null()),
        hIcon: ptr::null_mut(),
        hCursor: ptr::null_mut(), // must be null in order for cursor state to work properly
        hbrBackground: ptr::null_mut(),
        lpszMenuName: ptr::null(),
        lpszClassName: class_name.as_ptr(),
        hIconSm: ptr::null_mut(),
    };

    // We ignore errors because registering the same window class twice would trigger
    //  an error, and because errors here are detected during CreateWindowEx anyway.
    // Also since there is no weird element in the struct, there is no reason for this
    //  call to fail.
    user32::RegisterClassExW(&class);

    class_name
}

unsafe fn switch_to_fullscreen(rect: &mut winapi::RECT,
                               monitor: &MonitorId)
                               -> Result<(), CreationError> {
    // adjusting the rect
    {
        let pos = monitor.get_position();
        rect.left += pos.0 as winapi::LONG;
        rect.right += pos.0 as winapi::LONG;
        rect.top += pos.1 as winapi::LONG;
        rect.bottom += pos.1 as winapi::LONG;
    }

    // changing device settings
    let mut screen_settings: winapi::DEVMODEW = mem::zeroed();
    screen_settings.dmSize = mem::size_of::<winapi::DEVMODEW>() as winapi::WORD;
    screen_settings.dmPelsWidth = (rect.right - rect.left) as winapi::DWORD;
    screen_settings.dmPelsHeight = (rect.bottom - rect.top) as winapi::DWORD;
    screen_settings.dmBitsPerPel = 32; // TODO: ?
    screen_settings.dmFields = winapi::DM_BITSPERPEL | winapi::DM_PELSWIDTH | winapi::DM_PELSHEIGHT;

    let result = user32::ChangeDisplaySettingsExW(monitor.get_adapter_name().as_ptr(),
                                                  &mut screen_settings,
                                                  ptr::null_mut(),
                                                  winapi::CDS_FULLSCREEN,
                                                  ptr::null_mut());

    if result != winapi::DISP_CHANGE_SUCCESSFUL {
        return Err(OsError(format!("ChangeDisplaySettings failed: {}", result)));
    }

    Ok(())
}
