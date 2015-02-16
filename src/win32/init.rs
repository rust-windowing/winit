use std::sync::atomic::AtomicBool;
use std::ptr;
use super::callback;
use super::Window;
use BuilderAttribs;
use CreationError;
use CreationError::OsError;

use std::ffi::CString;
use std::sync::mpsc::channel;

use libc;
use super::gl;
use winapi;
use kernel32;
use user32;
use gdi32;

/// Work-around the fact that HGLRC doesn't implement Send
pub struct ContextHack(pub winapi::HGLRC);
unsafe impl Send for ContextHack {}

pub fn new_window(builder: BuilderAttribs<'static>, builder_sharelists: Option<ContextHack>)
                  -> Result<Window, CreationError>
{
    use std::mem;
    use std::os;

    // initializing variables to be sent to the task
    let title = builder.title.as_slice().utf16_units()
        .chain(Some(0).into_iter()).collect::<Vec<u16>>();    // title to utf16
    //let hints = hints.clone();
    let (tx, rx) = channel();

    // GetMessage must be called in the same thread as CreateWindow,
    //  so we create a new thread dedicated to this window.
    // This is the only safe method. Using `nosend` wouldn't work for non-native runtime.
    ::std::thread::Thread::spawn(move || {
        let builder_sharelists = builder_sharelists.map(|s| s.0);

        // registering the window class
        let class_name = {
            let class_name: Vec<u16> = "Window Class".utf16_units().chain(Some(0).into_iter())
                .collect::<Vec<u16>>();
            
            let class = winapi::WNDCLASSEXW {
                cbSize: mem::size_of::<winapi::WNDCLASSEXW>() as winapi::UINT,
                style: winapi::CS_HREDRAW | winapi::CS_VREDRAW | winapi::CS_OWNDC,
                lpfnWndProc: callback::callback,
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: unsafe { kernel32::GetModuleHandleW(ptr::null()) },
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
            unsafe { user32::RegisterClassExW(&class) };

            class_name
        };

        // building a RECT object with coordinates
        let mut rect = winapi::RECT {
            left: 0, right: builder.dimensions.unwrap_or((1024, 768)).0 as winapi::LONG,
            top: 0, bottom: builder.dimensions.unwrap_or((1024, 768)).1 as winapi::LONG,
        };

        // switching to fullscreen if necessary
        // this means adjusting the window's position so that it overlaps the right monitor,
        //  and change the monitor's resolution if necessary
        if builder.monitor.is_some() {
            let monitor = builder.monitor.as_ref().unwrap();

            // adjusting the rect
            {
                let pos = monitor.get_position();
                rect.left += pos.0 as winapi::LONG;
                rect.right += pos.0 as winapi::LONG;
                rect.top += pos.1 as winapi::LONG;
                rect.bottom += pos.1 as winapi::LONG;
            }

            // changing device settings
            let mut screen_settings: winapi::DEVMODEW = unsafe { mem::zeroed() };
            screen_settings.dmSize = mem::size_of::<winapi::DEVMODEW>() as winapi::WORD;
            screen_settings.dmPelsWidth = (rect.right - rect.left) as winapi::DWORD;
            screen_settings.dmPelsHeight = (rect.bottom - rect.top) as winapi::DWORD;
            screen_settings.dmBitsPerPel = 32;      // TODO: ?
            screen_settings.dmFields = winapi::DM_BITSPERPEL | winapi::DM_PELSWIDTH | winapi::DM_PELSHEIGHT;

            let result = unsafe { user32::ChangeDisplaySettingsExW(monitor.get_system_name().as_ptr(),
                &mut screen_settings, ptr::null_mut(), winapi::CDS_FULLSCREEN, ptr::null_mut()) };
            
            if result != winapi::DISP_CHANGE_SUCCESSFUL {
                tx.send(Err(OsError(format!("ChangeDisplaySettings failed: {}", result))));
                return;
            }
        }

        // computing the style and extended style of the window
        let (ex_style, style) = if builder.monitor.is_some() {
            (winapi::WS_EX_APPWINDOW, winapi::WS_POPUP | winapi::WS_CLIPSIBLINGS | winapi::WS_CLIPCHILDREN)
        } else {
            (winapi::WS_EX_APPWINDOW | winapi::WS_EX_WINDOWEDGE,
                winapi::WS_OVERLAPPEDWINDOW | winapi::WS_CLIPSIBLINGS | winapi::WS_CLIPCHILDREN)
        };

        // adjusting the window coordinates using the style
        unsafe { user32::AdjustWindowRectEx(&mut rect, style, 0, ex_style) };

        // getting the address of wglCreateContextAttribsARB and the pixel format
        //  that we will use
        let (extra_functions, pixel_format) = {
            // creating a dummy invisible window for GL initialization
            let dummy_window = unsafe {
                let handle = user32::CreateWindowExW(ex_style, class_name.as_ptr(),
                    title.as_ptr() as winapi::LPCWSTR,
                    style | winapi::WS_CLIPSIBLINGS | winapi::WS_CLIPCHILDREN,
                    winapi::CW_USEDEFAULT, winapi::CW_USEDEFAULT,
                    rect.right - rect.left, rect.bottom - rect.top,
                    ptr::null_mut(), ptr::null_mut(), kernel32::GetModuleHandleW(ptr::null()),
                    ptr::null_mut());

                if handle.is_null() {
                    use std::os;
                    tx.send(Err(OsError(format!("CreateWindowEx function failed: {}",
                        os::error_string(os::errno() as usize)))));
                    return;
                }

                handle
            };

            // getting the HDC of the dummy window
            let dummy_hdc = {
                let hdc = unsafe { user32::GetDC(dummy_window) };
                if hdc.is_null() {
                    tx.send(Err(OsError(format!("GetDC function failed: {}",
                        os::error_string(os::errno() as usize)))));
                    unsafe { user32::DestroyWindow(dummy_window); }
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

                let pf_index = unsafe { gdi32::ChoosePixelFormat(dummy_hdc, &output) };

                if pf_index == 0 {
                    tx.send(Err(OsError(format!("ChoosePixelFormat function failed: {}",
                        os::error_string(os::errno() as usize)))));
                    unsafe { user32::DestroyWindow(dummy_window); }
                    return;
                }

                if unsafe { gdi32::DescribePixelFormat(dummy_hdc, pf_index,
                    mem::size_of::<winapi::PIXELFORMATDESCRIPTOR>() as winapi::UINT, &mut output) } == 0
                {
                    tx.send(Err(OsError(format!("DescribePixelFormat function failed: {}",
                        os::error_string(os::errno() as usize)))));
                    unsafe { user32::DestroyWindow(dummy_window); }
                    return;
                }

                output
            };

            // calling SetPixelFormat
            unsafe {
                if gdi32::SetPixelFormat(dummy_hdc, 1, &pixel_format) == 0 {
                    tx.send(Err(OsError(format!("SetPixelFormat function failed: {}",
                        os::error_string(os::errno() as usize)))));
                    user32::DestroyWindow(dummy_window);
                    return;
                }
            }

            // creating the dummy OpenGL context
            let dummy_context = {
                let ctxt = unsafe { gl::wgl::CreateContext(dummy_hdc as *const libc::c_void) };
                if ctxt.is_null() {
                    tx.send(Err(OsError(format!("wglCreateContext function failed: {}",
                        os::error_string(os::errno() as usize)))));
                    unsafe { user32::DestroyWindow(dummy_window); }
                    return;
                }
                ctxt
            };

            // making context current
            unsafe { gl::wgl::MakeCurrent(dummy_hdc as *const libc::c_void, dummy_context); }

            // loading the extra WGL functions
            let extra_functions = gl::wgl_extra::Wgl::load_with(|addr| {
                use libc;

                let addr = CString::from_slice(addr.as_bytes());
                let addr = addr.as_slice_with_nul().as_ptr();

                unsafe {
                    gl::wgl::GetProcAddress(addr) as *const libc::c_void
                }
            });

            // removing current context
            unsafe { gl::wgl::MakeCurrent(ptr::null(), ptr::null()); }

            // destroying the context and the window
            unsafe { gl::wgl::DeleteContext(dummy_context); }
            unsafe { user32::DestroyWindow(dummy_window); }

            // returning the address
            (extra_functions, pixel_format)
        };

        // creating the real window this time
        let real_window = unsafe {
            let (width, height) = if builder.monitor.is_some() || builder.dimensions.is_some() {
                (Some(rect.right - rect.left), Some(rect.bottom - rect.top))
            } else {
                (None, None)
            };

            let style = if !builder.visible || builder.headless {
                style
            } else {
                style | winapi::WS_VISIBLE
            };

            let handle = user32::CreateWindowExW(ex_style, class_name.as_ptr(),
                title.as_ptr() as winapi::LPCWSTR,
                style | winapi::WS_CLIPSIBLINGS | winapi::WS_CLIPCHILDREN,
                if builder.monitor.is_some() { 0 } else { winapi::CW_USEDEFAULT },
                if builder.monitor.is_some() { 0 } else { winapi::CW_USEDEFAULT },
                width.unwrap_or(winapi::CW_USEDEFAULT), height.unwrap_or(winapi::CW_USEDEFAULT),
                ptr::null_mut(), ptr::null_mut(), kernel32::GetModuleHandleW(ptr::null()),
                ptr::null_mut());

            if handle.is_null() {
                use std::os;
                tx.send(Err(OsError(format!("CreateWindowEx function failed: {}",
                    os::error_string(os::errno() as usize)))));
                return;
            }

            handle
        };

        // getting the HDC of the window
        let hdc = {
            let hdc = unsafe { user32::GetDC(real_window) };
            if hdc.is_null() {
                tx.send(Err(OsError(format!("GetDC function failed: {}",
                    os::error_string(os::errno() as usize)))));
                unsafe { user32::DestroyWindow(real_window); }
                return;
            }
            hdc
        };

        // calling SetPixelFormat
        unsafe {
            if gdi32::SetPixelFormat(hdc, 1, &pixel_format) == 0 {
                tx.send(Err(OsError(format!("SetPixelFormat function failed: {}",
                    os::error_string(os::errno() as usize)))));
                user32::DestroyWindow(real_window);
                return;
            }
        }

        // creating the OpenGL context
        let context = {
            use libc;

            let mut attributes = Vec::new();

            if builder.gl_version.is_some() {
                let version = builder.gl_version.as_ref().unwrap();
                attributes.push(gl::wgl_extra::CONTEXT_MAJOR_VERSION_ARB as libc::c_int);
                attributes.push(version.0 as libc::c_int);
                attributes.push(gl::wgl_extra::CONTEXT_MINOR_VERSION_ARB as libc::c_int);
                attributes.push(version.1 as libc::c_int);
            }

            if builder.gl_debug {
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
                    os::error_string(os::errno() as usize)))));
                unsafe { user32::DestroyWindow(real_window); }
                return;
            }

            ctxt
        };

        // calling SetForegroundWindow if fullscreen
        if builder.monitor.is_some() {
            unsafe { user32::SetForegroundWindow(real_window) };
        }

        // filling the WINDOW task-local storage
        let events_receiver = {
            let (tx, rx) = channel();
            let mut tx = Some(tx);
            callback::WINDOW.with(|window| {
                (*window.borrow_mut()) = Some((real_window, tx.take().unwrap()));
            });
            rx
        };

        // loading the opengl32 module
        let gl_library = {
            let name = "opengl32.dll".utf16_units().chain(Some(0).into_iter())
                .collect::<Vec<u16>>().as_ptr();
            let lib = unsafe { kernel32::LoadLibraryW(name) };
            if lib.is_null() {
                tx.send(Err(OsError(format!("LoadLibrary function failed: {}",
                    os::error_string(os::errno() as usize)))));
                unsafe { gl::wgl::DeleteContext(context); }
                unsafe { user32::DestroyWindow(real_window); }
                return;
            }
            lib
        };

        // handling vsync
        if builder.vsync {
            if extra_functions.SwapIntervalEXT.is_loaded() {
                unsafe { gl::wgl::MakeCurrent(hdc as *const libc::c_void, context) };
                if unsafe { extra_functions.SwapIntervalEXT(1) } == 0 {
                    tx.send(Err(OsError(format!("wglSwapIntervalEXT failed"))));
                    unsafe { gl::wgl::DeleteContext(context); }
                    unsafe { user32::DestroyWindow(real_window); }
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

            if unsafe { user32::GetMessageW(&mut msg, ptr::null_mut(), 0, 0) } == 0 {
                break;
            }

            unsafe { user32::TranslateMessage(&msg) };
            unsafe { user32::DispatchMessageW(&msg) };     // calls `callback` (see below)
        }
    });

    rx.recv().unwrap()
}
