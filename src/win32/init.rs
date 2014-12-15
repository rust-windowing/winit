use std::sync::atomic::AtomicBool;
use std::ptr;
use super::event;
use super::Window;
use {CreationError, Event};
use CreationError::OsError;

use std::cell::RefCell;
use std::rc::Rc;

use libc;
use super::gl;
use winapi;

/// Stores the current window and its events dispatcher.
/// 
/// We only have one window per thread. We still store the HWND in case where we
///  receive an event for another window.
thread_local!(static WINDOW: Rc<RefCell<Option<(winapi::HWND, Sender<Event>)>>> = Rc::new(RefCell::new(None)))

pub fn new_window(builder_dimensions: Option<(uint, uint)>, builder_title: String,
                  builder_monitor: Option<super::MonitorID>,
                  builder_gl_version: Option<(uint, uint)>, builder_debug: bool,
                  builder_vsync: bool, builder_hidden: bool,
                  builder_sharelists: Option<winapi::HGLRC>, builder_multisampling: Option<u16>)
                  -> Result<Window, CreationError>
{
    use std::mem;
    use std::os;

    // initializing variables to be sent to the task
    let title = builder_title.as_slice().utf16_units()
        .chain(Some(0).into_iter()).collect::<Vec<u16>>();    // title to utf16
    //let hints = hints.clone();
    let (tx, rx) = channel();

    // GetMessage must be called in the same thread as CreateWindow,
    //  so we create a new thread dedicated to this window.
    // This is the only safe method. Using `nosend` wouldn't work for non-native runtime.
    spawn(move || {
        // registering the window class
        let class_name = {
            let class_name: Vec<u16> = "Window Class".utf16_units().chain(Some(0).into_iter())
                .collect::<Vec<u16>>();
            
            let class = winapi::WNDCLASSEXW {
                cbSize: mem::size_of::<winapi::WNDCLASSEXW>() as winapi::UINT,
                style: winapi::CS_HREDRAW | winapi::CS_VREDRAW | winapi::CS_OWNDC,
                lpfnWndProc: callback,
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: unsafe { winapi::GetModuleHandleW(ptr::null()) },
                hIcon: ptr::null_mut(),
                hCursor: ptr::null_mut(),
                hbrBackground: ptr::null_mut(),
                lpszMenuName: ptr::null(),
                lpszClassName: class_name.as_ptr(),
                hIconSm: ptr::null_mut(),
            };

            // We ignore errors because registering the same window class twice would trigger
            //  an error, and because errors here are detected during CreateWindowEx anyway.
            // Also since there is no weird element in the struct, there is no reason for this
            //  call to fail.
            unsafe { winapi::RegisterClassExW(&class) };

            class_name
        };

        // building a RECT object with coordinates
        let mut rect = winapi::RECT {
            left: 0, right: builder_dimensions.unwrap_or((1024, 768)).val0() as winapi::LONG,
            top: 0, bottom: builder_dimensions.unwrap_or((1024, 768)).val1() as winapi::LONG,
        };

        // switching to fullscreen if necessary
        // this means adjusting the window's position so that it overlaps the right monitor,
        //  and change the monitor's resolution if necessary
        if builder_monitor.is_some() {
            let monitor = builder_monitor.as_ref().unwrap();

            // adjusting the rect
            {
                let pos = monitor.get_position();
                rect.left += pos.val0() as winapi::LONG;
                rect.right += pos.val0() as winapi::LONG;
                rect.top += pos.val1() as winapi::LONG;
                rect.bottom += pos.val1() as winapi::LONG;
            }

            // changing device settings
            let mut screen_settings: winapi::DEVMODEW = unsafe { mem::zeroed() };
            screen_settings.dmSize = mem::size_of::<winapi::DEVMODEW>() as winapi::WORD;
            screen_settings.dmPelsWidth = (rect.right - rect.left) as winapi::DWORD;
            screen_settings.dmPelsHeight = (rect.bottom - rect.top) as winapi::DWORD;
            screen_settings.dmBitsPerPel = 32;      // TODO: ?
            screen_settings.dmFields = winapi::DM_BITSPERPEL | winapi::DM_PELSWIDTH | winapi::DM_PELSHEIGHT;

            let result = unsafe { winapi::ChangeDisplaySettingsExW(monitor.get_system_name().as_ptr(),
                &mut screen_settings, ptr::null_mut(), winapi::CDS_FULLSCREEN, ptr::null_mut()) };
            
            if result != winapi::DISP_CHANGE_SUCCESSFUL {
                tx.send(Err(OsError(format!("ChangeDisplaySettings failed: {}", result))));
                return;
            }
        }

        // computing the style and extended style of the window
        let (ex_style, style) = if builder_monitor.is_some() {
            (winapi::WS_EX_APPWINDOW, winapi::WS_POPUP | winapi::WS_CLIPSIBLINGS | winapi::WS_CLIPCHILDREN)
        } else {
            (winapi::WS_EX_APPWINDOW | winapi::WS_EX_WINDOWEDGE,
                winapi::WS_OVERLAPPEDWINDOW | winapi::WS_CLIPSIBLINGS | winapi::WS_CLIPCHILDREN)
        };

        // adjusting the window coordinates using the style
        unsafe { winapi::AdjustWindowRectEx(&mut rect, style, 0, ex_style) };

        // getting the address of wglCreateContextAttribsARB and the pixel format
        //  that we will use
        let (extra_functions, pixel_format) = {
            // creating a dummy invisible window for GL initialization
            let dummy_window = unsafe {
                let handle = winapi::CreateWindowExW(ex_style, class_name.as_ptr(),
                    title.as_ptr() as winapi::LPCWSTR,
                    style | winapi::WS_CLIPSIBLINGS | winapi::WS_CLIPCHILDREN,
                    winapi::CW_USEDEFAULT, winapi::CW_USEDEFAULT,
                    rect.right - rect.left, rect.bottom - rect.top,
                    ptr::null_mut(), ptr::null_mut(), winapi::GetModuleHandleW(ptr::null()),
                    ptr::null_mut());

                if handle.is_null() {
                    use std::os;
                    tx.send(Err(OsError(format!("CreateWindowEx function failed: {}",
                        os::error_string(os::errno() as uint)))));
                    return;
                }

                handle
            };

            // getting the HDC of the dummy window
            let dummy_hdc = {
                let hdc = unsafe { winapi::GetDC(dummy_window) };
                if hdc.is_null() {
                    tx.send(Err(OsError(format!("GetDC function failed: {}",
                        os::error_string(os::errno() as uint)))));
                    unsafe { winapi::DestroyWindow(dummy_window); }
                    return;
                }
                hdc
            };

            // getting the pixel format that we will use
            let pixel_format = {
                // initializing a PIXELFORMATDESCRIPTOR that indicates what we want
                let mut output: winapi::PIXELFORMATDESCRIPTOR = unsafe { mem::zeroed() };
                output.nSize = mem::size_of::<winapi::PIXELFORMATDESCRIPTOR>() as winapi::WORD;
                output.nVersion = 1;
                output.dwFlags = winapi::PFD_DRAW_TO_WINDOW | winapi::PFD_DOUBLEBUFFER |
                    winapi::PFD_SUPPORT_OPENGL | winapi::PFD_GENERIC_ACCELERATED;
                output.iPixelType = winapi::PFD_TYPE_RGBA;
                output.cColorBits = 24;
                output.cAlphaBits = 8;
                output.cAccumBits = 0;
                output.cDepthBits = 24;
                output.cStencilBits = 8;
                output.cAuxBuffers = 0;
                output.iLayerType = winapi::PFD_MAIN_PLANE;

                let pf_index = unsafe { winapi::ChoosePixelFormat(dummy_hdc, &output) };

                if pf_index == 0 {
                    tx.send(Err(OsError(format!("ChoosePixelFormat function failed: {}",
                        os::error_string(os::errno() as uint)))));
                    unsafe { winapi::DestroyWindow(dummy_window); }
                    return;
                }

                if unsafe { winapi::DescribePixelFormat(dummy_hdc, pf_index,
                    mem::size_of::<winapi::PIXELFORMATDESCRIPTOR>() as winapi::UINT, &mut output) } == 0
                {
                    tx.send(Err(OsError(format!("DescribePixelFormat function failed: {}",
                        os::error_string(os::errno() as uint)))));
                    unsafe { winapi::DestroyWindow(dummy_window); }
                    return;
                }

                output
            };

            // calling SetPixelFormat
            unsafe {
                if winapi::SetPixelFormat(dummy_hdc, 1, &pixel_format) == 0 {
                    tx.send(Err(OsError(format!("SetPixelFormat function failed: {}",
                        os::error_string(os::errno() as uint)))));
                    winapi::DestroyWindow(dummy_window);
                    return;
                }
            }

            // creating the dummy OpenGL context
            let dummy_context = {
                let ctxt = unsafe { gl::wgl::CreateContext(dummy_hdc as *const libc::c_void) };
                if ctxt.is_null() {
                    tx.send(Err(OsError(format!("wglCreateContext function failed: {}",
                        os::error_string(os::errno() as uint)))));
                    unsafe { winapi::DestroyWindow(dummy_window); }
                    return;
                }
                ctxt
            };

            // making context current
            unsafe { gl::wgl::MakeCurrent(dummy_hdc as *const libc::c_void, dummy_context); }

            // loading the extra WGL functions
            let extra_functions = gl::wgl_extra::Wgl::load_with(|addr| {
                unsafe {
                    addr.with_c_str(|s| {
                        use libc;
                        gl::wgl::GetProcAddress(s) as *const libc::c_void
                    })
                }
            });

            // removing current context
            unsafe { gl::wgl::MakeCurrent(ptr::null(), ptr::null()); }

            // destroying the context and the window
            unsafe { gl::wgl::DeleteContext(dummy_context); }
            unsafe { winapi::DestroyWindow(dummy_window); }

            // returning the address
            (extra_functions, pixel_format)
        };

        // creating the real window this time
        let real_window = unsafe {
            let (width, height) = if builder_monitor.is_some() || builder_dimensions.is_some() {
                (Some(rect.right - rect.left), Some(rect.bottom - rect.top))
            } else {
                (None, None)
            };

            let style = if builder_hidden {
                style
            } else {
                style | winapi::WS_VISIBLE
            };

            let handle = winapi::CreateWindowExW(ex_style, class_name.as_ptr(),
                title.as_ptr() as winapi::LPCWSTR,
                style | winapi::WS_CLIPSIBLINGS | winapi::WS_CLIPCHILDREN,
                if builder_monitor.is_some() { 0 } else { winapi::CW_USEDEFAULT },
                if builder_monitor.is_some() { 0 } else { winapi::CW_USEDEFAULT },
                width.unwrap_or(winapi::CW_USEDEFAULT), height.unwrap_or(winapi::CW_USEDEFAULT),
                ptr::null_mut(), ptr::null_mut(), winapi::GetModuleHandleW(ptr::null()),
                ptr::null_mut());

            if handle.is_null() {
                use std::os;
                tx.send(Err(OsError(format!("CreateWindowEx function failed: {}",
                    os::error_string(os::errno() as uint)))));
                return;
            }

            handle
        };

        // getting the HDC of the window
        let hdc = {
            let hdc = unsafe { winapi::GetDC(real_window) };
            if hdc.is_null() {
                tx.send(Err(OsError(format!("GetDC function failed: {}",
                    os::error_string(os::errno() as uint)))));
                unsafe { winapi::DestroyWindow(real_window); }
                return;
            }
            hdc
        };

        // calling SetPixelFormat
        unsafe {
            if winapi::SetPixelFormat(hdc, 1, &pixel_format) == 0 {
                tx.send(Err(OsError(format!("SetPixelFormat function failed: {}",
                    os::error_string(os::errno() as uint)))));
                winapi::DestroyWindow(real_window);
                return;
            }
        }

        // creating the OpenGL context
        let context = {
            use libc;

            let mut attributes = Vec::new();

            if builder_gl_version.is_some() {
                let version = builder_gl_version.as_ref().unwrap();
                attributes.push(gl::wgl_extra::CONTEXT_MAJOR_VERSION_ARB as libc::c_int);
                attributes.push(version.val0() as libc::c_int);
                attributes.push(gl::wgl_extra::CONTEXT_MINOR_VERSION_ARB as libc::c_int);
                attributes.push(version.val1() as libc::c_int);
            }

            if builder_debug {
                attributes.push(gl::wgl_extra::CONTEXT_FLAGS_ARB as libc::c_int);
                attributes.push(gl::wgl_extra::CONTEXT_DEBUG_BIT_ARB as libc::c_int);
            }

            attributes.push(0);

            let ctxt = unsafe {
                if extra_functions.CreateContextAttribsARB.is_loaded() {
                    let share = if let Some(c) = builder_sharelists { c } else { ptr::null_mut() };
                    extra_functions.CreateContextAttribsARB(hdc as *const libc::c_void,
                                                            share as *const libc::c_void,
                                                            attributes.as_slice().as_ptr())

                } else {
                    let ctxt = gl::wgl::CreateContext(hdc as *const libc::c_void);
                    if let Some(c) = builder_sharelists {
                        gl::wgl::ShareLists(c as *const libc::c_void, ctxt);
                    };
                    ctxt
                }
            };

            if ctxt.is_null() {
                tx.send(Err(OsError(format!("OpenGL context creation failed: {}",
                    os::error_string(os::errno() as uint)))));
                unsafe { winapi::DestroyWindow(real_window); }
                return;
            }

            ctxt
        };

        // calling SetForegroundWindow if fullscreen
        if builder_monitor.is_some() {
            unsafe { winapi::SetForegroundWindow(real_window) };
        }

        // filling the WINDOW task-local storage
        let events_receiver = {
            let (tx, rx) = channel();
            let mut tx = Some(tx);
            WINDOW.with(|window| {
                (*window.borrow_mut()) = Some((real_window, tx.take().unwrap()));
            });
            rx
        };

        // loading the opengl32 module
        let gl_library = {
            let name = "opengl32.dll".utf16_units().chain(Some(0).into_iter())
                .collect::<Vec<u16>>().as_ptr();
            let lib = unsafe { winapi::LoadLibraryW(name) };
            if lib.is_null() {
                tx.send(Err(OsError(format!("LoadLibrary function failed: {}",
                    os::error_string(os::errno() as uint)))));
                unsafe { gl::wgl::DeleteContext(context); }
                unsafe { winapi::DestroyWindow(real_window); }
                return;
            }
            lib
        };

        // handling vsync
        if builder_vsync {
            if extra_functions.SwapIntervalEXT.is_loaded() {
                unsafe { gl::wgl::MakeCurrent(hdc as *const libc::c_void, context) };
                if unsafe { extra_functions.SwapIntervalEXT(1) } == 0 {
                    tx.send(Err(OsError(format!("wglSwapIntervalEXT failed"))));
                    unsafe { gl::wgl::DeleteContext(context); }
                    unsafe { winapi::DestroyWindow(real_window); }
                    return;
                }

                // it is important to remove the current context, otherwise you get very weird
                // errors
                unsafe { gl::wgl::MakeCurrent(ptr::null(), ptr::null()); }
            }
        }

        // building the struct
        let window = Window{
            window: real_window,
            hdc: hdc as winapi::HDC,
            context: context as winapi::HGLRC,
            gl_library: gl_library,
            events_receiver: events_receiver,
            is_closed: AtomicBool::new(false),
        };

        // sending
        tx.send(Ok(window));

        // now that the `Window` struct is initialized, the main `Window::new()` function will
        //  return and this events loop will run in parallel
        loop {
            let mut msg = unsafe { mem::uninitialized() };

            if unsafe { winapi::GetMessageW(&mut msg, ptr::null_mut(), 0, 0) } == 0 {
                break;
            }

            unsafe { winapi::TranslateMessage(&msg) };
            unsafe { winapi::DispatchMessageW(&msg) };     // calls `callback` (see below)
        }
    });

    rx.recv()
}

/// Checks that the window is the good one, and if so send the event to it.
fn send_event(input_window: winapi::HWND, event: Event) {
    WINDOW.with(|window| {
        let window = window.borrow();
        let stored = match *window {
            None => return,
            Some(ref v) => v
        };

        let &(ref win, ref sender) = stored;

        if win != &input_window {
            return;
        }

        sender.send_opt(event).ok();  // ignoring if closed
    });
}

/// This is the callback that is called by `DispatchMessage` in the events loop.
/// 
/// Returning 0 tells the Win32 API that the message has been processed.
extern "system" fn callback(window: winapi::HWND, msg: winapi::UINT,
    wparam: winapi::WPARAM, lparam: winapi::LPARAM) -> winapi::LRESULT
{
    match msg {
        winapi::WM_DESTROY => {
            use events::Event::Closed;

            WINDOW.with(|w| {
                let w = w.borrow();
                let &(ref win, _) = match *w {
                    None => return,
                    Some(ref v) => v
                };

                if win == &window {
                    unsafe { winapi::PostQuitMessage(0); }
                }
            });

            send_event(window, Closed);
            0
        },

        winapi::WM_ERASEBKGND => {
            1
        },

        winapi::WM_SIZE => {
            use events::Event::Resized;
            let w = winapi::LOWORD(lparam as winapi::DWORD) as uint;
            let h = winapi::HIWORD(lparam as winapi::DWORD) as uint;
            send_event(window, Resized(w, h));
            0
        },

        winapi::WM_MOVE => {
            use events::Event::Moved;
            let x = winapi::LOWORD(lparam as winapi::DWORD) as i16 as int;
            let y = winapi::HIWORD(lparam as winapi::DWORD) as i16 as int;
            send_event(window, Moved(x, y));
            0
        },

        winapi::WM_CHAR => {
            use std::mem;
            use events::Event::ReceivedCharacter;
            let chr: char = unsafe { mem::transmute(wparam as u32) };
            send_event(window, ReceivedCharacter(chr));
            0
        },

        winapi::WM_MOUSEMOVE => {
            use events::Event::MouseMoved;

            let x = winapi::GET_X_LPARAM(lparam) as int;
            let y = winapi::GET_Y_LPARAM(lparam) as int;

            send_event(window, MouseMoved((x, y)));

            0
        },

        winapi::WM_MOUSEWHEEL => {
            use events::Event::MouseWheel;

            let value = (wparam >> 16) as i16;
            let value = value as i32;

            send_event(window, MouseWheel(value));

            0
        },

        winapi::WM_KEYDOWN => {
            use events::Event::KeyboardInput;
            use events::ElementState::Pressed;
            let scancode = ((lparam >> 16) & 0xff) as u8;
            let vkey = event::vkeycode_to_element(wparam);
            send_event(window, KeyboardInput(Pressed, scancode, vkey));
            0
        },

        winapi::WM_KEYUP => {
            use events::Event::KeyboardInput;
            use events::ElementState::Released;
            let scancode = ((lparam >> 16) & 0xff) as u8;
            let vkey = event::vkeycode_to_element(wparam);
            send_event(window, KeyboardInput(Released, scancode, vkey));
            0
        },

        winapi::WM_LBUTTONDOWN => {
            use events::Event::MouseInput;
            use events::MouseButton::LeftMouseButton;
            use events::ElementState::Pressed;
            send_event(window, MouseInput(Pressed, LeftMouseButton));
            0
        },

        winapi::WM_LBUTTONUP => {
            use events::Event::MouseInput;
            use events::MouseButton::LeftMouseButton;
            use events::ElementState::Released;
            send_event(window, MouseInput(Released, LeftMouseButton));
            0
        },

        winapi::WM_RBUTTONDOWN => {
            use events::Event::MouseInput;
            use events::MouseButton::RightMouseButton;
            use events::ElementState::Pressed;
            send_event(window, MouseInput(Pressed, RightMouseButton));
            0
        },

        winapi::WM_RBUTTONUP => {
            use events::Event::MouseInput;
            use events::MouseButton::RightMouseButton;
            use events::ElementState::Released;
            send_event(window, MouseInput(Released, RightMouseButton));
            0
        },

        winapi::WM_MBUTTONDOWN => {
            use events::Event::MouseInput;
            use events::MouseButton::MiddleMouseButton;
            use events::ElementState::Pressed;
            send_event(window, MouseInput(Pressed, MiddleMouseButton));
            0
        },

        winapi::WM_MBUTTONUP => {
            use events::Event::MouseInput;
            use events::MouseButton::MiddleMouseButton;
            use events::ElementState::Released;
            send_event(window, MouseInput(Released, MiddleMouseButton));
            0
        },

        winapi::WM_SETFOCUS => {
            use events::Event::Focused;
            send_event(window, Focused(true));
            0
        },

        winapi::WM_KILLFOCUS => {
            use events::Event::Focused;
            send_event(window, Focused(false));
            0
        },

        _ => unsafe {
            winapi::DefWindowProcW(window, msg, wparam, lparam)
        }
    }
}

/*fn hints_to_pixelformat(hints: &Hints) -> winapi::PIXELFORMATDESCRIPTOR {
    use std::mem;

    winapi::PIXELFORMATDESCRIPTOR {
        nSize: size_of::<winapi::PIXELFORMATDESCRIPTOR>(),
        nVersion: 1,
        dwFlags:
            if hints.stereo { PFD_STEREO } else { 0 },
        iPixelType: PFD_TYPE_RGBA,
        cColorBits: hints.red_bits + hints.green_bits + hints.blue_bits,
        cRedBits: 

    pub nSize: WORD,
    pub nVersion: WORD,
    pub dwFlags: DWORD,
    pub iPixelType: BYTE,
    pub cColorBits: BYTE,
    pub cRedBits: BYTE,
    pub cRedShift: BYTE,
    pub cGreenBits: BYTE,
    pub cGreenShift: BYTE,
    pub cBlueBits: BYTE,
    pub cBlueShift: BYTE,
    pub cAlphaBits: BYTE,
    pub cAlphaShift: BYTE,
    pub cAccumBits: BYTE,
    pub cAccumRedBits: BYTE,
    pub cAccumGreenBits: BYTE,
    pub cAccumBlueBits: BYTE,
    pub cAccumAlphaBits: BYTE,
    pub cDepthBits: BYTE,
    pub cStencilBits: BYTE,
    pub cAuxBuffers: BYTE,
    pub iLayerType: BYTE,
    pub bReserved: BYTE,
    pub dwLayerMask: DWORD,
    pub dwVisibleMask: DWORD,
    pub dwDamageMask: DWORD,
    }
}*/
